use crate::telescope::Telescope;
use async_trait::async_trait;
use common::coords::{horizontal_from_equatorial, horizontal_from_galactic};
use common::{
    Direction, Location, ReceiverConfiguration, ReceiverError, TelescopeError, TelescopeInfo,
    TelescopeStatus, TelescopeTarget,
};
use hex_literal::hex;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Copy, Clone, Debug)]
enum TelescopeCommand {
    Stop,
    Restart,
    GetDirection,
    SetDirection(Direction),
}

pub const LOWEST_ALLOWED_ALTITUDE: f64 = 5.0;

impl TelescopeCommand {
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            TelescopeCommand::Stop => hex!("57000000000000000000000F20").into(),
            TelescopeCommand::Restart => hex!("57EFBEADDE000000000000EE20").into(),
            TelescopeCommand::GetDirection => hex!("57000000000000000000006F20").into(),
            TelescopeCommand::SetDirection(direction) => {
                let mut bytes = Vec::with_capacity(13);
                bytes.extend(hex!("57"));
                bytes.extend(rot2prog_angle_to_bytes(direction.azimuth).as_slice());
                bytes.extend(rot2prog_angle_to_bytes(direction.altitude).as_slice());
                bytes.extend(hex!("5F20"));
                bytes
            }
        }
    }
}

pub struct SalsaTelescope {
    address: String,
    timeout: Duration,
    target: TelescopeTarget,
    commanded_horizontal: Option<Direction>,
    current_direction: Option<Direction>,
    location: Location,
    most_recent_error: Option<TelescopeError>,
    connection_established: bool,
    lowest_allowed_altitude: f64,
    telescope_restart_request_at: Option<tokio::time::Instant>,
    wait_time_after_restart: Duration,
}

pub fn create(telescope_address: String) -> SalsaTelescope {
    SalsaTelescope {
        address: telescope_address,
        timeout: Duration::from_secs(1),
        target: TelescopeTarget::Stopped,
        commanded_horizontal: None,
        current_direction: None,
        location: Location {
            longitude: astro::angle::deg_frm_dms(-11, 55, 4.0).to_radians(),
            latitude: astro::angle::deg_frm_dms(57, 23, 35.0).to_radians(),
        },
        most_recent_error: None,
        connection_established: false,
        lowest_allowed_altitude: LOWEST_ALLOWED_ALTITUDE,
        telescope_restart_request_at: None,
        wait_time_after_restart: Duration::from_secs(10),
    }
}

fn create_connection(telescope: &SalsaTelescope) -> Result<TcpStream, std::io::Error> {
    let address = SocketAddr::from_str(telescope.address.as_str()).unwrap();
    let stream = TcpStream::connect_timeout(&address, telescope.timeout)?;
    stream.set_read_timeout(Some(telescope.timeout))?;
    stream.set_write_timeout(Some(telescope.timeout))?;
    Ok(stream)
}

fn send_command<Stream>(
    stream: &mut Stream,
    command: TelescopeCommand,
) -> Result<Vec<u8>, TelescopeError>
where
    Stream: Read + Write,
{
    let command_as_hex = command
        .to_bytes()
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<String>();
    log::info!("Sending command: {:?} ({})", command, command_as_hex);
    stream.write(&command.to_bytes()).unwrap();
    let mut result = vec![0; 128];
    let response_length = stream.read(&mut result).unwrap();
    result.truncate(response_length);
    let response_as_hex = result
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<String>();
    log::info!("Response: {}", response_as_hex);
    Ok(result)
}

fn rot2prog_bytes_to_int(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .rev()
        .enumerate()
        .map(|(pos, &digit)| digit as u32 * 10_u32.pow(pos as u32))
        .sum()
}

fn rot2prog_bytes_to_angle(bytes: &[u8]) -> f64 {
    rot2prog_bytes_to_int(bytes) as f64 / 100.0 - 360.0
}

// Reading the documentation of the telescope, this should be the correct way to interpret the bytes
// This would match how rot2prog_angle_to_bytes works.
fn rot2prog_bytes_to_int_documented(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .rev()
        .enumerate()
        .map(|(pos, &digit)| (digit as u32 - 0x30) * 10_u32.pow(pos as u32))
        .sum()
}

#[allow(dead_code)]
fn rot2prog_bytes_to_angle_documented(bytes: &[u8]) -> f64 {
    rot2prog_bytes_to_int_documented(bytes) as f64 / 100.0 - 360.0
}

fn rot2prog_angle_to_bytes(angle: f64) -> [u8; 5] {
    let mut bytes = [0; 5];
    let angle = ((angle + 360.0) * 100.0).round();
    bytes[0] = (angle / 10000.0) as u8 + 0x30;
    bytes[1] = ((angle % 10000.0) / 1000.0) as u8 + 0x30;
    bytes[2] = ((angle % 1000.0) / 100.0) as u8 + 0x30;
    bytes[3] = ((angle % 100.0) / 10.0) as u8 + 0x30;
    bytes[4] = (angle % 10.0) as u8 + 0x30;
    bytes
}

#[async_trait]
impl Telescope for SalsaTelescope {
    async fn get_direction(&self) -> Result<Direction, TelescopeError> {
        let mut stream = create_connection(&self)?;

        self.get_current_horizontal(&mut stream).await
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
        self.update_direction(&mut stream).await?;

        Ok(target)
    }

    async fn set_receiver_configuration(
        &mut self,
        _receiver_configuration: ReceiverConfiguration,
    ) -> Result<ReceiverConfiguration, ReceiverError> {
        todo!()
    }

    async fn get_info(&self) -> Result<TelescopeInfo, TelescopeError> {
        let current_horizontal = if let Some(current_horizontal) = self.current_direction {
            current_horizontal
        } else {
            return Err(TelescopeError::TelescopeNotConnected);
        };

        let status = match self.commanded_horizontal {
            Some(commanded_direction) => {
                let horizontal_offset_squared =
                    (current_horizontal.azimuth - commanded_direction.azimuth).powi(2)
                        + (current_horizontal.altitude - commanded_direction.altitude).powi(2);
                if horizontal_offset_squared < 2.0 * 0.2f64.powi(2) {
                    TelescopeStatus::Tracking
                } else {
                    TelescopeStatus::Slewing
                }
            }
            None => TelescopeStatus::Idle,
        };

        Ok(TelescopeInfo {
            status,
            current_horizontal,
            commanded_horizontal: self.commanded_horizontal,
            current_target: self.target,
            most_recent_error: self.most_recent_error.clone(),
            measurement_in_progress: false,
            latest_observation: None,
        })
    }

    async fn update(&mut self, _delta_time: Duration) -> Result<(), TelescopeError> {
        if let Some(telescope_restart_request_at) = self.telescope_restart_request_at {
            if Instant::now() - telescope_restart_request_at < self.wait_time_after_restart {
                return Ok(());
            } else {
                self.telescope_restart_request_at = None;
            }
        }

        log::info!("Connecting to telescope");
        let stream = create_connection(&self);
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(telescope_error) => {
                let error: TelescopeError = telescope_error.into();
                self.most_recent_error = Some(error.clone());
                return Err(error);
            }
        };

        log::info!("Updating telescope");
        self.update_direction(&mut stream).await?;
        Ok(())
    }

    async fn restart(&mut self) -> Result<(), TelescopeError> {
        let mut stream = create_connection(&self)?;
        let response = send_command(&mut stream, TelescopeCommand::Restart)?;
        if response.len() == 0 || response[0] != 0x57 {
            return Err(TelescopeError::TelescopeIOError(format!(
                "Unexpected response to restart command: {:?}",
                response
            )));
        }
        self.most_recent_error = None;
        self.connection_established = false;
        self.telescope_restart_request_at = Some(Instant::now());
        Ok(())
    }
}

fn directions_are_close(a: Direction, b: Direction) -> bool {
    // The salsa telescope works with a precision of 0.1 degrees
    let epsilon = 0.2;
    (a.azimuth - b.azimuth).abs() < epsilon && (a.altitude - b.altitude).abs() < epsilon
}

impl SalsaTelescope {
    fn calculate_target_horizontal(&self) -> Option<Direction> {
        match self.target {
            TelescopeTarget::Equatorial { ra, dec } => {
                Some(horizontal_from_equatorial(self.location, ra, dec))
            }
            TelescopeTarget::Galactic { l, b } => Some(horizontal_from_galactic(self.location, l, b)),
            TelescopeTarget::Stopped => None,
            TelescopeTarget::Parked => None,
        }
    }

    async fn get_current_horizontal<Stream>(
        &self,
        stream: &mut Stream,
    ) -> Result<Direction, TelescopeError>
    where
        Stream: Read + Write,
    {
        let result = send_command(stream, TelescopeCommand::GetDirection)?;
        Ok(Direction {
            azimuth: rot2prog_bytes_to_angle(result[1..=5].as_ref()),
            altitude: rot2prog_bytes_to_angle(result[6..=10].as_ref()),
        })
    }

    async fn update_direction<Stream>(&mut self, stream: &mut Stream) -> Result<(), TelescopeError>
    where
        Stream: Read + Write,
    {
        let target_horizontal = self.calculate_target_horizontal();
        let current_horizontal = self.get_current_horizontal(stream).await?;
        self.current_direction = Some(current_horizontal);

        if !self.connection_established {
            let response = send_command(stream, TelescopeCommand::Stop)?;
            if response.len() == 0 || response[0] != 0x57 {
                return Err(TelescopeError::TelescopeIOError(
                    "Telescope did not respond to stop command".to_string(),
                ));
            }
            self.commanded_horizontal = None;
            self.connection_established = true;
            return Ok(());
        }

        match target_horizontal {
            Some(target_horizontal) => {
                log::info!("Target horizontal: {:?}", target_horizontal);
                log::info!("Current horizontal: {:?}", current_horizontal);
                if directions_are_close(target_horizontal, current_horizontal) {
                    self.commanded_horizontal = None;
                    return Ok(());
                }

                if target_horizontal.altitude < self.lowest_allowed_altitude {
                    self.most_recent_error = Some(TelescopeError::TargetBelowHorizon);
                    self.commanded_horizontal = None;
                    return Err(TelescopeError::TargetBelowHorizon);
                }

                let response =
                    send_command(stream, TelescopeCommand::SetDirection(target_horizontal))?;

                if response.len() == 12 && response[0] == 0x58 && response[11] == 0x20 {
                    self.commanded_horizontal = Some(target_horizontal);
                    return Ok(());
                } else {
                    return Err(TelescopeError::TelescopeIOError(
                        "Telescope did not respond to set direction command".to_string(),
                    ));
                }
            }
            None => {
                if self.commanded_horizontal.is_some() {
                    let response = send_command(stream, TelescopeCommand::Stop)?;
                    if response.len() == 0 || response[0] != 0x57 {
                        return Err(TelescopeError::TelescopeIOError(
                            "Telescope did not respond to stop command".to_string(),
                        ));
                    }
                    self.commanded_horizontal = None;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_rot2prog_angle_to_bytes() {
        assert_eq!(
            rot2prog_angle_to_bytes(0.0),
            hex!("3336303030"),
            "0.0 should be 0x3336303030 (telescope expects angle + 360)"
        );
        assert_eq!(
            rot2prog_angle_to_bytes(5.54),
            hex!("3336353534"),
            "5.54 should be 0x3336353534 (example from documentation)"
        );
    }

    #[test]
    fn test_rot2prog_bytes_to_angle_documented() {
        // This behavior is what I expect reading the documentation, but the telescope seems to work with returned bytes
        // directly instead of ascii encoded numbers. E.g. 0x03 instead of 0x33 which is '3' in ascii.
        assert!((rot2prog_bytes_to_angle_documented(&hex!("3336303030")) - 0.0).abs() < 0.01,);
        // Example from documentation
        assert!((rot2prog_bytes_to_angle_documented(&hex!("3338323333")) - 22.33).abs() < 0.01,);
    }

    #[test]
    fn test_rot2prog_bytes_to_angle() {
        assert!((rot2prog_bytes_to_angle(&hex!("0306000000")) - 0.0).abs() < 0.01,);
    }

    // This is a fake connection that can be used to test the telescope without a real connection
    // It is set up with the response that the telescope should send and will store all writes
    struct FakeConnection {
        request: Vec<Vec<u8>>,  // Each write is to the fake connection is stored here
        response: Vec<Vec<u8>>, // Response to a read on the fake connection
    }

    impl Read for FakeConnection {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let response = self.response.remove(0);
            buf[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }
    }

    impl Write for FakeConnection {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.request.push(buf.to_vec());
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_update_direction() {
        let mut telescope = create("127.0.0.1:3000".to_string());

        let mut stream = FakeConnection {
            request: Vec::new(),
            response: Vec::new(),
        };

        // Send an initial stop command to set the current direction
        stream
            .response
            .push(hex!("58333635353433373030355F20").to_vec());
        stream.response.push(hex!("570000").to_vec());
        telescope.update_direction(&mut stream).await.unwrap();
        assert_eq!(stream.request.len(), 2);
        assert_eq!(stream.request[1], TelescopeCommand::Stop.to_bytes());
        assert_eq!(telescope.commanded_horizontal, None);

        // TODO: Inject time to ensure that the target is not below horizon
        // Update direction with target sends command

        stream
            .response
            .push(hex!("58333635353433373030355F20").to_vec());
        stream
            .response
            .push(hex!("58333635353433373030355F20").to_vec());
        stream.request.clear();
        telescope.target = TelescopeTarget::Galactic { l: 180.0, b: 0.0 };
        telescope.update_direction(&mut stream).await.unwrap();
        assert_eq!(stream.request.len(), 2);
        assert_eq!(stream.request[1].len(), 13);
        assert_eq!(stream.request[1][0], 0x57);
        assert_eq!(stream.request[1][11..=12], hex!("5F20"));
        assert!(telescope.commanded_horizontal.is_some());

        // Avoid testing update_direction again with the same target because we do not inject the time.
        // Depending on the time the telescope may or may not update the commanded direction.

        // Stopping telescope send stop command
        stream
            .response
            .push(hex!("58333635353433373030355F20").to_vec());
        stream.response.push(hex!("570000").to_vec());
        stream.request.clear();
        telescope.target = TelescopeTarget::Stopped;
        telescope.update_direction(&mut stream).await.unwrap();
        assert_eq!(stream.request.len(), 2);
        assert_eq!(stream.request[1], TelescopeCommand::Stop.to_bytes());
        assert_eq!(telescope.commanded_horizontal, None);

        // Calling update_direction again does not send any command
        stream
            .response
            .push(hex!("58333635353433373030355F20").to_vec());
        stream.request.clear();
        telescope.update_direction(&mut stream).await.unwrap();
        assert_eq!(stream.request.len(), 1);
    }
}
