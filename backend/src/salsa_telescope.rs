use crate::telescope::Telescope;
use async_trait::async_trait;
use common::{
    Direction, ReceiverConfiguration, ReceiverError, TelescopeError, TelescopeInfo, TelescopeTarget,
};
use hex_literal::hex;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::Duration;

#[derive(Copy, Clone, Debug)]
enum TelescopeCommand {
    Stop,
    GetDirection,
}

impl TelescopeCommand {
    fn get_bytes(&self) -> Vec<u8> {
        match self {
            TelescopeCommand::Stop => hex!("57000000000000000000000F20").into(),
            TelescopeCommand::GetDirection => hex!("57000000000000000000006F20").into(),
        }
    }
}

pub struct SalsaTelescope {
    name: String,
    address: String,
    timeout: Duration,
    target: TelescopeTarget,
}

pub fn create(name: String, telescope_address: String) -> SalsaTelescope {
    SalsaTelescope {
        name,
        address: telescope_address,
        timeout: Duration::from_secs(1),
        target: TelescopeTarget::Stopped,
    }
}

fn create_connection(telescope: &SalsaTelescope) -> Result<TcpStream, std::io::Error> {
    let address = SocketAddr::from_str(telescope.address.as_str()).unwrap();
    Ok(TcpStream::connect_timeout(&address, telescope.timeout)?)
}

fn send_command(
    stream: &mut TcpStream,
    command: TelescopeCommand,
) -> Result<Vec<u8>, TelescopeError> {
    stream.write(&command.get_bytes()).unwrap();
    let mut result = vec![0; 128];
    let response_length = stream.read(&mut result).unwrap();
    result.truncate(response_length);
    Ok(result)
}

fn rot2prog_get_int(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .rev()
        .enumerate()
        .map(|(pos, &digit)| digit as u32 * 10_u32.pow(pos as u32))
        .sum()
}

#[async_trait]
impl Telescope for SalsaTelescope {
    async fn get_direction(&self) -> Result<Direction, TelescopeError> {
        let mut stream = create_connection(&self)?;
        log::info!("Sending command get direction command to {}", self.name);
        let result = send_command(&mut stream, TelescopeCommand::GetDirection)?;

        let azimuth = rot2prog_get_int(&result[1..=5]);
        let elevation = rot2prog_get_int(&result[6..=10]);
        let azimuth = (azimuth as f64) / 100.0 - 360.0;
        let elevation = (elevation as f64) / 100.0 - 360.0;

        Ok(Direction {
            azimuth,
            altitude: elevation,
        })
    }

    async fn get_target(&self) -> Result<TelescopeTarget, TelescopeError> {
        Ok(self.target)
    }

    async fn set_target(
        &mut self,
        target: TelescopeTarget,
    ) -> Result<TelescopeTarget, TelescopeError> {
        let mut stream = create_connection(&self)?;

        self.target = target;

        match target {
            TelescopeTarget::Equatorial { .. } => todo!(),
            TelescopeTarget::Galactic { .. } => todo!(),
            TelescopeTarget::Parked => todo!(),
            TelescopeTarget::Stopped => send_command(&mut stream, TelescopeCommand::Stop)?,
        };

        Ok(target)
    }

    async fn set_receiver_configuration(
        &mut self,
        _receiver_configuration: ReceiverConfiguration,
    ) -> Result<ReceiverConfiguration, ReceiverError> {
        todo!()
    }

    async fn get_info(&self) -> Result<TelescopeInfo, TelescopeError> {
        todo!()
    }

    async fn update(&mut self, _delta_time: Duration) -> Result<(), TelescopeError> {
        Ok(())
    }
}
