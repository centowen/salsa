use crate::telescope::Telescope;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
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

pub const LOWEST_ALLOWED_ALTITUDE: f64 = 5.0f64 / 180.0f64 * std::f64::consts::PI;

#[derive(Copy, Clone, Debug, PartialEq)]
enum TelescopeCommand {
    Stop,
    Restart,
    GetDirection,
    SetDirection(Direction),
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum TelescopeResponse {
    Ack,
    CurrentDirection(Direction),
}

fn parse_ack_response(
    bytes: &[u8],
    command_name: &str,
) -> Result<TelescopeResponse, TelescopeError> {
    if bytes.len() == 12 && bytes[0] == 0x57 && bytes[11] == 0x20 {
        Ok(TelescopeResponse::Ack)
    } else {
        Err(TelescopeError::TelescopeIOError(format!(
            "Unexpected response to {} command: {:?}",
            command_name, bytes,
        )))
    }
}

fn parse_direction_response(
    bytes: &[u8],
    command_name: &str,
) -> Result<TelescopeResponse, TelescopeError> {
    if bytes.len() == 12 && bytes[0] == 0x58 && bytes[11] == 0x20 {
        let azimuth = rot2prog_bytes_to_angle(&bytes[1..=5]);
        let altitude = rot2prog_bytes_to_angle(&bytes[6..=10]);
        Ok(TelescopeResponse::CurrentDirection(Direction {
            azimuth,
            altitude,
        }))
    } else {
        Err(TelescopeError::TelescopeIOError(format!(
            "Unexpected response to {} command: {:?}",
            command_name, bytes,
        )))
    }
}

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

    fn parse_response(&self, bytes: &[u8]) -> Result<TelescopeResponse, TelescopeError> {
        match self {
            TelescopeCommand::Stop => parse_ack_response(bytes, "stop"),
            TelescopeCommand::Restart => parse_ack_response(bytes, "restart"),
            TelescopeCommand::GetDirection => parse_direction_response(bytes, "get direction"),
            TelescopeCommand::SetDirection(_) => parse_direction_response(bytes, "set direction"),
        }
    }
}

pub struct Measurement {
    data: Vec<f64>,
    glon: f64,
    glat: f64,
    nchan: usize,
    ch0freq: f64,
    chres: f64,
    start: DateTime<Utc>,
    duration: Option<chrono::Duration>,
    vlsr_correction: f64,
    telname: String,
    tellat: f64,
    tellon: f64,
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
    telescope_restart_request_at: Option<DateTime<Utc>>,
    wait_time_after_restart: chrono::Duration,
    measurements: Vec<Measurement>,
}

pub fn create(telescope_address: String) -> SalsaTelescope {
    SalsaTelescope {
        address: telescope_address,
        timeout: Duration::from_secs(1),
        target: TelescopeTarget::Stopped,
        commanded_horizontal: None,
        current_direction: None,
        location: Location {
            longitude: 0.20802143022, //(11.0+55.0/60.0+7.5/3600.0) * PI / 180.0. Sign positive, handled in gmst calc
            latitude: 1.00170457462,  //(57.0+23.0/60.0+36.4/3600.0) * PI / 180.0
        },
        most_recent_error: None,
        connection_established: false,
        lowest_allowed_altitude: LOWEST_ALLOWED_ALTITUDE,
        telescope_restart_request_at: None,
        wait_time_after_restart: chrono::Duration::seconds(10),
        measurements: Vec::new(),

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
) -> Result<TelescopeResponse, TelescopeError>
where
    Stream: Read + Write,
{
    stream.write(&command.to_bytes()).unwrap();
    let mut result = vec![0; 128];
    let response_length = stream.read(&mut result).unwrap();
    result.truncate(response_length);
    command.parse_response(&result)
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
    (rot2prog_bytes_to_int(bytes) as f64 / 100.0 - 360.0).to_radians()
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
    (rot2prog_bytes_to_int_documented(bytes) as f64 / 100.0 - 360.0).to_radians()
}

fn rot2prog_angle_to_bytes(angle: f64) -> [u8; 5] {
    let mut bytes = [0; 5];
    let angle = ((angle.to_degrees() + 360.0) * 100.0).round();
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
        self.update_direction(Utc::now(), &mut stream).await?;

        Ok(target)
    }

    async fn set_receiver_configuration(
        &mut self,
        _receiver_configuration: ReceiverConfiguration,
    ) -> Result<ReceiverConfiguration, ReceiverError> {
        todo!()
    }

    async fn get_info(&self) -> Result<TelescopeInfo, TelescopeError> {
        let current_horizontal = match self.current_direction {
            Some(current_horizontal) => current_horizontal,
            None => return Err(TelescopeError::TelescopeNotConnected),
        };

        let status = match self.commanded_horizontal {
            Some(commanded_direction) => {
                if directions_are_close(commanded_direction, current_horizontal) {
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
        let now = Utc::now();
        if let Some(telescope_restart_request_at) = self.telescope_restart_request_at {
            if now - telescope_restart_request_at < self.wait_time_after_restart {
                return Ok(());
            } else {
                self.telescope_restart_request_at = None;
            }
        }

        //log::info!("Connecting to telescope");
        let stream = create_connection(&self);
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(telescope_error) => {
                let error: TelescopeError = telescope_error.into();
                self.most_recent_error = Some(error.clone());
                return Err(error);
            }
        };

        //log::info!("Updating telescope");
        self.update_direction(now, &mut stream).await?;
        Ok(())
    }

    async fn restart(&mut self) -> Result<(), TelescopeError> {
        let mut stream = create_connection(&self)?;
        send_command(&mut stream, TelescopeCommand::Restart)?;
        self.most_recent_error = None;
        self.connection_established = false;
        self.telescope_restart_request_at = Some(Utc::now());
        Ok(())
    }
}

fn directions_are_close(a: Direction, b: Direction) -> bool {
    // The salsa telescope works with a precision of 0.1 degrees
    let epsilon = 0.1_f64.to_radians();
    (a.azimuth - b.azimuth).abs() < epsilon && (a.altitude - b.altitude).abs() < epsilon
}

impl SalsaTelescope {
    fn calculate_target_horizontal(&self, when: DateTime<Utc>) -> Option<Direction> {
        match self.target {
            TelescopeTarget::Equatorial { ra, dec } => {
                Some(horizontal_from_equatorial(self.location, when, ra, dec))
            }
            TelescopeTarget::Galactic { l, b } => {
                Some(horizontal_from_galactic(self.location, when, l, b))
            }
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
        match send_command(stream, TelescopeCommand::GetDirection)? {
            TelescopeResponse::CurrentDirection(direction) => Ok(direction),
            _ => Err(TelescopeError::TelescopeIOError(
                "Telescope did not respond with current direction".to_string(),
            )),
        }
    }

    async fn update_direction<Stream>(
        &mut self,
        when: DateTime<Utc>,
        stream: &mut Stream,
    ) -> Result<(), TelescopeError>
    where
        Stream: Read + Write,
    {
        let target_horizontal = self.calculate_target_horizontal(when);
        let current_horizontal = self.get_current_horizontal(stream).await?;
        self.current_direction = Some(current_horizontal);

        if !self.connection_established {
            send_command(stream, TelescopeCommand::Stop)?;
            self.commanded_horizontal = None;
            self.connection_established = true;
            return Ok(());
        }

        match target_horizontal {
            Some(target_horizontal) => {
                if target_horizontal.altitude < self.lowest_allowed_altitude {
                    self.most_recent_error = Some(TelescopeError::TargetBelowHorizon);
                    self.commanded_horizontal = None;
                    return Err(TelescopeError::TargetBelowHorizon);
                }

                self.commanded_horizontal = Some(target_horizontal);

                if !directions_are_close(target_horizontal, current_horizontal) {
                    send_command(stream, TelescopeCommand::SetDirection(target_horizontal))?;
                }

                Ok(())
            }
            None => {
                if self.commanded_horizontal.is_some() {
                    send_command(stream, TelescopeCommand::Stop)?;
                    self.commanded_horizontal = None;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use chrono::TimeZone;

    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_rot2prog_angle_to_bytes() {
        assert_eq!(
            rot2prog_angle_to_bytes(0.0),
            hex!("3336303030"),
            "0.0 should be 0x3336303030 (telescope expects angle + 360)"
        );
        assert_eq!(
            rot2prog_angle_to_bytes(5.54_f64.to_radians()),
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
        assert!(
            (rot2prog_bytes_to_angle_documented(&hex!("3338323333")) - 22.33_f64.to_radians())
                .abs()
                < 0.01,
        );
    }

    #[test]
    fn test_rot2prog_bytes_to_angle() {
        assert!((rot2prog_bytes_to_angle(&hex!("0306000000")) - 0.0).abs() < 0.01,);
    }

    impl TelescopeCommand {
        fn from_bytes(bytes: &[u8]) -> TelescopeCommand {
            assert!(bytes.len() == 13);
            if bytes[0] != 0x57 {
                panic!("All commands should start with 0x57");
            } else if bytes[bytes.len() - 1] != 0x20 {
                panic!("All commands should end with 0x20");
            }

            match bytes[bytes.len() - 2] {
                0x0F => TelescopeCommand::Stop,
                0xEE => TelescopeCommand::Restart,
                0x6F => TelescopeCommand::GetDirection,
                0x5F => TelescopeCommand::SetDirection(Direction {
                    azimuth: rot2prog_bytes_to_angle_documented(&bytes[1..=5]),
                    altitude: rot2prog_bytes_to_angle_documented(&bytes[6..=10]),
                }),
                command_identifier => {
                    panic!("Unknown command identifier: {:x}", command_identifier)
                }
            }
        }
    }

    // Responses are documented as ascii encoded numbers, but the telescope seems to return the bytes directly.
    fn rot2prog_response_angle_to_bytes(angle: f64) -> [u8; 5] {
        let mut bytes = [0; 5];
        let angle = ((angle.to_degrees() + 360.0) * 100.0).round();
        bytes[0] = (angle / 10000.0) as u8;
        bytes[1] = ((angle % 10000.0) / 1000.0) as u8;
        bytes[2] = ((angle % 1000.0) / 100.0) as u8;
        bytes[3] = ((angle % 100.0) / 10.0) as u8;
        bytes[4] = (angle % 10.0) as u8;
        bytes
    }

    impl TelescopeResponse {
        fn to_bytes(&self) -> Vec<u8> {
            match self {
                TelescopeResponse::Ack => hex!("570000000000000000000020").to_vec(),
                TelescopeResponse::CurrentDirection(direction) => {
                    let mut bytes = Vec::with_capacity(13);
                    bytes.extend(hex!("58"));
                    bytes.extend(rot2prog_response_angle_to_bytes(direction.azimuth).as_slice());
                    bytes.extend(rot2prog_response_angle_to_bytes(direction.altitude).as_slice());
                    bytes.extend(hex!("20"));
                    bytes
                }
            }
        }
    }

    // This is a fake connection that can be used to test the telescope without a real connection
    // It is set up with the response that the telescope should send and will store all writes
    struct FakeTelescopeConnection {
        horizontal: Direction,
        commands: Vec<TelescopeCommand>,
    }

    impl Write for FakeTelescopeConnection {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let command = TelescopeCommand::from_bytes(buf);
            self.commands.push(command);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Read for FakeTelescopeConnection {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let response = match self.commands.last().expect("No commands sent to telescope") {
                TelescopeCommand::Stop => TelescopeResponse::Ack,
                TelescopeCommand::Restart => TelescopeResponse::Ack,
                TelescopeCommand::GetDirection => {
                    TelescopeResponse::CurrentDirection(self.horizontal)
                }
                TelescopeCommand::SetDirection(_) => {
                    TelescopeResponse::CurrentDirection(self.horizontal)
                }
            }
            .to_bytes();

            buf[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }
    }

    #[test]
    fn test_set_command() {
        let mut stream = FakeTelescopeConnection {
            horizontal: Direction {
                azimuth: 0.0,
                altitude: PI / 2.0,
            },
            commands: Vec::new(),
        };

        let response = send_command(
            &mut stream,
            TelescopeCommand::SetDirection(Direction {
                azimuth: PI,
                altitude: PI / 4.0,
            }),
        );
        assert_eq!(
            stream.commands,
            [TelescopeCommand::SetDirection(Direction {
                azimuth: PI,
                altitude: PI / 4.0,
            })]
        );

        let response = match response {
            Ok(response) => response,
            Err(e) => panic!("Error sending command: {}", e),
        };
        assert_eq!(
            response,
            TelescopeResponse::CurrentDirection(Direction {
                azimuth: 0.0,
                altitude: PI / 2.0,
            })
        );
    }

    #[tokio::test]
    async fn test_update_direction() {
        let mut telescope = create("127.0.0.1:3000".to_string());

        let mut stream = FakeTelescopeConnection {
            horizontal: Direction {
                azimuth: 0.0,
                altitude: PI / 2.0,
            },
            commands: Vec::new(),
        };

        // Send an initial stop command to set the current direction
        telescope
            .update_direction(Utc::now(), &mut stream)
            .await
            .unwrap();
        assert_eq!(
            stream.commands,
            [TelescopeCommand::GetDirection, TelescopeCommand::Stop]
        );
        assert_eq!(telescope.commanded_horizontal, None);
        assert_eq!(
            TelescopeStatus::Idle,
            telescope.get_info().await.unwrap().status
        );

        // Inject the time to ensure that the target is not below horizon
        let when = Utc.with_ymd_and_hms(2023, 4, 7, 12, 0, 0).unwrap();
        stream.commands.clear();
        telescope.target = TelescopeTarget::Galactic {
            l: PI / 2.0,
            b: 0.0,
        };
        telescope.update_direction(when, &mut stream).await.unwrap();
        assert_eq!(2, stream.commands.len());
        assert_eq!(stream.commands[0], TelescopeCommand::GetDirection);
        assert!(matches!(
            stream.commands[1],
            TelescopeCommand::SetDirection { .. }
        ));
        assert!(telescope.commanded_horizontal.is_some());
        assert_eq!(
            TelescopeStatus::Slewing,
            telescope.get_info().await.unwrap().status
        );

        // Calling update_direction when telescope is on target does not send set direction command
        stream.horizontal = telescope.commanded_horizontal.unwrap();
        stream.commands.clear();
        telescope.update_direction(when, &mut stream).await.unwrap();
        assert_eq!(stream.commands, [TelescopeCommand::GetDirection]);
        assert_eq!(
            TelescopeStatus::Tracking,
            telescope.get_info().await.unwrap().status
        );

        // Stopping telescope send stop command
        stream.commands.clear();
        telescope.target = TelescopeTarget::Stopped;
        telescope
            .update_direction(Utc::now(), &mut stream)
            .await
            .unwrap();
        assert_eq!(
            stream.commands,
            [TelescopeCommand::GetDirection, TelescopeCommand::Stop]
        );
        assert_eq!(telescope.commanded_horizontal, None);
        assert_eq!(
            TelescopeStatus::Idle,
            telescope.get_info().await.unwrap().status
        );

        // Calling update_direction again does not send set direction command
        stream.commands.clear();
        telescope
            .update_direction(Utc::now(), &mut stream)
            .await
            .unwrap();
        assert_eq!(stream.commands, [TelescopeCommand::GetDirection]);
        assert_eq!(telescope.commanded_horizontal, None);
        assert_eq!(
            TelescopeStatus::Idle,
            telescope.get_info().await.unwrap().status
        );

        // Start tracking again
        stream.commands.clear();
        telescope.target = TelescopeTarget::Galactic {
            l: PI / 2.0,
            b: 0.0,
        };
        telescope.update_direction(when, &mut stream).await.unwrap();
        assert_eq!(
            TelescopeStatus::Tracking,
            telescope.get_info().await.unwrap().status
        );

        // Wait 5 minutes when source moves across the sky
        let when = when + chrono::Duration::minutes(5);
        stream.commands.clear();
        telescope.update_direction(when, &mut stream).await.unwrap();
        assert_eq!(
            TelescopeStatus::Slewing,
            telescope.get_info().await.unwrap().status
        );
    }
}
