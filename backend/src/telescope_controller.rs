use chrono::{DateTime, Utc};
use common::{Direction, Location, TelescopeError, TelescopeStatus, TelescopeTarget};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::{sleep_until, Instant};

use common::coords::{horizontal_from_equatorial, horizontal_from_galactic};
use hex_literal::hex;

pub const LOWEST_ALLOWED_ALTITUDE: f64 = 5.0f64 / 180.0f64 * std::f64::consts::PI;

#[derive(Copy, Clone, Debug, PartialEq)]
enum TelescopeCommand {
    Stop,
    Restart,
    GetDirection,
    SetDirection(Direction),
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

fn create_connection(state: &TelescopeControllerState) -> Result<TcpStream, std::io::Error> {
    // FIXME: How to handle static configuration like timeouts etc?
    let timeout = Duration::from_secs(1);
    let address = SocketAddr::from_str(state.controller_address.as_str()).unwrap();
    let stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
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

fn directions_are_close(a: Direction, b: Direction, tol: f64) -> bool {
    // The salsa telescope works with a precision of 0.1 degrees
    // We want to send new commands whenever we exceed this tolerance
    // but to report tracking status we allow more, so that we do not flip
    // status between tracking/slewing (e.g. due to control unit rounding errors)
    // Therefore we have the "tol" multiplier here, which scales the allowed error.
    let epsilon = tol * 0.1_f64.to_radians();
    (a.azimuth - b.azimuth).abs() < epsilon && (a.altitude - b.altitude).abs() < epsilon
}

pub struct TelescopeControllerInfo {
    pub target: TelescopeTarget,
    pub commanded_horizontal: Option<Direction>,
    pub current_horizontal: Direction,
    pub status: TelescopeStatus,
    pub most_recent_error: Option<TelescopeError>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum TelescopeResponse {
    Ack,
    CurrentDirection(Direction),
}

struct TelescopeControllerState {
    controller_address: String,
    target: TelescopeTarget,
    commanded_horizontal: Option<Direction>,
    current_direction: Option<Direction>,
    most_recent_error: Option<TelescopeError>,
    should_restart: bool,
}

pub struct TelescopeController {
    // FIXME: Do we need to lock the whole state at a time?
    state: Arc<Mutex<TelescopeControllerState>>,
}

impl TelescopeController {
    pub fn new(controller_address: String) -> TelescopeController {
        let state = Arc::new(Mutex::new(TelescopeControllerState {
            controller_address,
            target: TelescopeTarget::Stopped,
            commanded_horizontal: None,
            current_direction: None,
            most_recent_error: None,
            should_restart: false,
        }));
        // FIXME: Keep track of this task and do a proper shutdown.
        let _ = spawn_controller_task(state.clone());
        TelescopeController { state }
    }

    pub async fn direction(&self) -> Result<Direction, TelescopeError> {
        match self.state.lock().unwrap().current_direction {
            Some(current_direction) => Ok(current_direction),
            None => return Err(TelescopeError::TelescopeNotConnected),
        }
    }

    pub fn target(&self) -> Result<TelescopeTarget, TelescopeError> {
        Ok(self.state.lock().unwrap().target)
    }

    pub async fn set_target(
        &mut self,
        target: TelescopeTarget,
    ) -> Result<TelescopeTarget, TelescopeError> {
        self.state.lock().unwrap().target = target;
        Ok(target)
    }

    pub fn info(&self) -> Result<TelescopeControllerInfo, TelescopeError> {
        let current_horizontal = match self.state.lock().unwrap().current_direction {
            Some(current_horizontal) => current_horizontal,
            None => return Err(TelescopeError::TelescopeNotConnected),
        };

        let status = match self.commanded_horizontal() {
            Some(commanded_direction) => {
                // Check if more than 2 tolerances off, if so we are not tracking anymore
                if directions_are_close(commanded_direction, current_horizontal, 2.0) {
                    TelescopeStatus::Tracking
                } else {
                    TelescopeStatus::Slewing
                }
            }
            None => TelescopeStatus::Idle,
        };
        Ok(TelescopeControllerInfo {
            target: self.state.lock().unwrap().target,
            current_horizontal,
            commanded_horizontal: self.commanded_horizontal(),
            status,
            most_recent_error: self.state.lock().unwrap().most_recent_error.clone(),
        })
    }

    pub fn restart(&self) {
        self.state.lock().unwrap().should_restart = true;
    }

    fn commanded_horizontal(&self) -> Option<Direction> {
        self.state.lock().unwrap().commanded_horizontal
    }
}

fn update_direction<Stream>(
    state: &mut TelescopeControllerState,
    when: DateTime<Utc>,
    stream: &mut Stream,
) -> Result<(), TelescopeError>
where
    Stream: Read + Write,
{
    // FIXME: How do we handle static configuration like this?
    let location = Location {
        longitude: 0.20802143022, //(11.0+55.0/60.0+7.5/3600.0) * PI / 180.0. Sign positive, handled in gmst calc
        latitude: 1.00170457462,  //(57.0+23.0/60.0+36.4/3600.0) * PI / 180.0
    };
    let target_horizontal = calculate_target_horizontal(state.target, location, when);
    let current_horizontal = get_current_horizontal(stream)?;
    state.current_direction = Some(current_horizontal);

    match target_horizontal {
        Some(target_horizontal) => {
            // FIXME: How to handle static configuration like this?
            if target_horizontal.altitude < LOWEST_ALLOWED_ALTITUDE {
                state.most_recent_error = Some(TelescopeError::TargetBelowHorizon);
                state.commanded_horizontal = None;
                return Err(TelescopeError::TargetBelowHorizon);
            }

            state.commanded_horizontal = Some(target_horizontal);

            // Check if more than 1 tolerance off, if so we need to send track command
            if !directions_are_close(target_horizontal, current_horizontal, 1.0) {
                send_command(stream, TelescopeCommand::SetDirection(target_horizontal))?;
            }

            Ok(())
        }
        None => {
            if state.commanded_horizontal.is_some() {
                send_command(stream, TelescopeCommand::Stop)?;
                state.commanded_horizontal = None;
            }
            Ok(())
        }
    }
}

fn calculate_target_horizontal(
    target: TelescopeTarget,
    location: Location,
    when: DateTime<Utc>,
) -> Option<Direction> {
    match target {
        TelescopeTarget::Equatorial { ra, dec } => {
            Some(horizontal_from_equatorial(location, when, ra, dec))
        }
        TelescopeTarget::Galactic { l, b } => Some(horizontal_from_galactic(location, when, l, b)),
        TelescopeTarget::Stopped => None,
        TelescopeTarget::Parked => None,
    }
}

fn get_current_horizontal<Stream>(stream: &mut Stream) -> Result<Direction, TelescopeError>
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

fn spawn_controller_task(state: Arc<Mutex<TelescopeControllerState>>) -> JoinHandle<()> {
    tokio::spawn(controller_task_function(state))
}

async fn controller_task_function(state: Arc<Mutex<TelescopeControllerState>>) {
    // commanded_horizontal: Option<Direction>,
    let mut connection_established = false;

    loop {
        // 10 Hz update freq
        sleep_until(Instant::now() + Duration::from_millis(100)).await;

        let stream = create_connection(&state.lock().unwrap());
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(telescope_error) => {
                let error: TelescopeError = telescope_error.into();
                state.lock().unwrap().most_recent_error = Some(error.clone());
                continue;
            }
        };
        if !connection_established {
            let mut state_guard = state.lock().unwrap();
            state_guard.most_recent_error = send_command(&mut stream, TelescopeCommand::Stop).err();
            state_guard.commanded_horizontal = None;
            connection_established = true;
        }

        if state.lock().unwrap().should_restart {
            state.lock().unwrap().most_recent_error =
                send_command(&mut stream, TelescopeCommand::Restart).err();
            connection_established = false;
            sleep_until(Instant::now() + Duration::from_secs(10)).await;
            state.lock().unwrap().should_restart = false;
            continue;
        }

        let res = update_direction(&mut state.lock().unwrap(), Utc::now(), &mut stream);
        state.lock().unwrap().most_recent_error = res.err();
    }
}
