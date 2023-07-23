use crate::telescope::Telescope;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::coords::{horizontal_from_equatorial, horizontal_from_galactic};
use common::{
    Direction, Location, Measurement, ObservedSpectra, ReceiverConfiguration, ReceiverError,
    TelescopeError, TelescopeInfo, TelescopeStatus, TelescopeTarget,
};
use hex_literal::hex;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::str::FromStr;
use std::time::Duration;

use rustfft::{num_complex::Complex, FftPlanner};
use uhd::{self, StreamCommand, StreamCommandType, StreamTime, TuneRequest, Usrp};

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
            "Unexpected response to {} command: {:02X?}",
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
            "Unexpected response to {} command: {:02X?}",
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

pub struct ActiveIntegration {
    cancellation_token: CancellationToken,
    measurement_task: tokio::task::JoinHandle<()>,
}

pub struct SalsaTelescope {
    name: String,
    controller_address: String,
    receiver_address: String,
    timeout: Duration,
    target: TelescopeTarget,
    commanded_horizontal: Option<Direction>,
    current_direction: Option<Direction>,
    location: Location,
    most_recent_error: Option<TelescopeError>,
    receiver_configuration: ReceiverConfiguration,
    connection_established: bool,
    lowest_allowed_altitude: f64,
    telescope_restart_request_at: Option<DateTime<Utc>>,
    wait_time_after_restart: chrono::Duration,
    measurements: Arc<Mutex<Vec<Measurement>>>,
    active_integration: Option<ActiveIntegration>,
}

pub fn create(
    name: String,
    controller_address: String,
    receiver_address: String,
) -> SalsaTelescope {
    SalsaTelescope {
        name,
        controller_address,
        receiver_address,
        timeout: Duration::from_secs(1),
        target: TelescopeTarget::Stopped,
        commanded_horizontal: None,
        current_direction: None,
        location: Location {
            longitude: 0.20802143022, //(11.0+55.0/60.0+7.5/3600.0) * PI / 180.0. Sign positive, handled in gmst calc
            latitude: 1.00170457462,  //(57.0+23.0/60.0+36.4/3600.0) * PI / 180.0
        },
        most_recent_error: None,
        receiver_configuration: ReceiverConfiguration { integrate: false },
        connection_established: false,
        lowest_allowed_altitude: LOWEST_ALLOWED_ALTITUDE,
        telescope_restart_request_at: None,
        wait_time_after_restart: chrono::Duration::seconds(10),
        measurements: Arc::new(Mutex::new(Vec::new())),
        active_integration: None,
    }
}

fn create_connection(telescope: &SalsaTelescope) -> Result<TcpStream, std::io::Error> {
    let address = SocketAddr::from_str(telescope.controller_address.as_str()).unwrap();
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

fn measure_switched(
    usrp: &mut Usrp,
    sfreq: f64,
    rfreq: f64,
    fft_pts: usize,
    tint: f64,
    avg_pts: usize,
    srate: f64,
    spec: &mut Vec<f64>,
) {
    let mut spec_sig: Vec<f64> = vec![];
    measure_single(
        usrp,
        sfreq,
        fft_pts,
        0.5 * tint,
        avg_pts,
        srate,
        &mut spec_sig,
    );
    let mut spec_ref: Vec<f64> = vec![];
    measure_single(
        usrp,
        rfreq,
        fft_pts,
        0.5 * tint,
        avg_pts,
        srate,
        &mut spec_ref,
    );
    // Form sig-ref difference and scale with Tsys
    // Hard coded Tsys for now
    let tsys = 285.0;
    for i in 0..avg_pts {
        spec[i] = tsys * (spec_sig[i] - spec_ref[i]) / spec_ref[i];
    }
}

fn measure_single(
    usrp: &mut Usrp,
    cfreq: f64,
    fft_pts: usize,
    tint: f64,
    avg_pts: usize,
    srate: f64,
    fft_avg: &mut Vec<f64>,
) {
    let nsamp: f64 = tint * srate; // total number of samples to request
    let nstack: usize = (nsamp as usize) / fft_pts;
    let navg: usize = fft_pts / avg_pts;

    usrp.set_rx_frequency(&TuneRequest::with_frequency(cfreq), 0)
        .unwrap(); // The N210 only has one input channel 0.

    let mut receiver = usrp
        .get_rx_stream(&uhd::StreamArgs::<Complex<i16>>::new("sc16"))
        .unwrap();

    let mut buffer = vec![Complex::<i16>::default(); nsamp as usize];

    receiver
        .send_command(&StreamCommand {
            command_type: StreamCommandType::CountAndDone(buffer.len() as u64),
            time: StreamTime::Now,
        })
        .unwrap();
    receiver.receive_simple(buffer.as_mut()).unwrap();

    // array to store power spectrum (abs of FFT result)
    let mut fft_abs: Vec<f64> = Vec::with_capacity(fft_pts);
    fft_abs.resize(fft_pts, 0.0);
    // setup fft
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_pts);
    // Loop through the samples, taking fft_pts each time
    for n in 0..nstack {
        let mut fft_buffer: Vec<Complex<f64>> = buffer[n * fft_pts..(n + 1) * fft_pts]
            .iter()
            .copied()
            .map(|x| Complex::<f64>::new(x.re as f64, x.im as f64))
            .collect();
        // Do the FFT
        fft.process(&mut fft_buffer);
        // Add absolute values to stacked spectrum
        // Seems the pos/neg halves of spectrum are flipped, so reflip them
        // we want lowest frequency in element 0 and then increasing
        for i in 0..fft_pts / 2 {
            fft_abs[i + fft_pts / 2] = fft_abs[i + fft_pts / 2] + fft_buffer[i].norm();
            fft_abs[i] = fft_abs[i] + fft_buffer[i + fft_pts / 2].norm();
        }
    }
    // Normalise spectrum by number of stackings,
    // do **2 to get power spectrum
    for i in 0..fft_pts {
        fft_abs[i] = fft_abs[i] * fft_abs[i] / (nstack as f64);
    }

    // median window filter data
    let mwkernel = 32; //median window filter size, power of 2
    let threshold = 0.1; // thershold where to cut data and replace with median
    let nchunks = fft_pts / mwkernel;
    for i in 0..nchunks {
        let chunk = &mut fft_abs[i * mwkernel..(i + 1) * mwkernel];
        let m = median(chunk.to_vec());
        for n in 0..mwkernel {
            let diff = (chunk[n] - m).abs();
            if diff > threshold * m {
                chunk[n] = m;
            }
        }
    }

    // Average spectrum to save data
    for i in 0..avg_pts {
        let mut avg = 0.0;
        for j in navg * i..navg * (i + 1) {
            avg = avg + fft_abs[j];
        }
        fft_avg.push(avg / (navg as f64));
    }
}

fn median(mut xs: Vec<f64>) -> f64 {
    // sort in ascending order, panic on f64::NaN
    xs.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let n = xs.len();
    if n % 2 == 0 {
        (xs[n / 2] + xs[n / 2 - 1]) / 2.0
    } else {
        xs[n / 2]
    }
}

async fn measure(
    address: String,
    measurements: Arc<Mutex<Vec<Measurement>>>,
    cancellation_token: CancellationToken,
) -> () {
    // Switched HI example
    let tint: f64 = 1.0; // integration time per cycle, seconds
    let srate: f64 = 2.5e6; // sample rate, Hz
    let sfreq: f64 = 1.4204e9;
    let rfreq: f64 = 1.4179e9;
    let avg_pts: usize = 512; // ^2 Number of points after average, setting spectral resolution
    let fft_pts: usize = 8192; // ^2 Number of points in FFT, setting spectral resolution
    let gain: f64 = 38.0;

    // Setup usrp for taking data
    let args = format!("addr={}", address);
    let mut usrp = Usrp::open(&args).unwrap(); // Brage

    // The N210 only has one input channel 0.
    usrp.set_rx_gain(gain, 0, "").unwrap(); // empty string to set all gains
    usrp.set_rx_antenna("TX/RX", 0).unwrap();
    usrp.set_rx_dc_offset_enabled(true, 0).unwrap();

    usrp.set_rx_sample_rate(srate as f64, 0).unwrap();

    {
        let mut measurements = measurements.clone().lock_owned().await;
        let mut measurement = Measurement {
            amps: vec![0.0; avg_pts],
            freqs: vec![0.0; avg_pts],
            start: Utc::now(),
            duration: Duration::from_secs(0),
        };
        for i in 0..avg_pts {
            measurement.freqs[i] = sfreq - 0.5 * srate + srate * (i as f64 / avg_pts as f64);
        }
        measurements.push(measurement);
    }

    // start taking data until integrate is false
    let mut n = 0.0;
    while !cancellation_token.is_cancelled() {
        let mut spec = vec![0.0; avg_pts];
        measure_switched(
            &mut usrp, sfreq, rfreq, fft_pts, tint, avg_pts, srate, &mut spec,
        );
        n = n + 1.0;

        let mut measurements = measurements.lock().await;
        let measurement = measurements.last_mut().unwrap();
        for i in 0..avg_pts {
            measurement.amps[i] = (measurement.amps[i] * (n - 1.0) + spec[i]) / n;
        }
        measurement.duration = Utc::now()
            .signed_duration_since(measurement.start)
            .to_std()
            .unwrap();
    }
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
        receiver_configuration: ReceiverConfiguration,
    ) -> Result<ReceiverConfiguration, ReceiverError> {
        if receiver_configuration.integrate && !self.receiver_configuration.integrate {
            if self.active_integration.is_some() {
                return Err(ReceiverError::IntegrationAlreadyRunning);
            }

            log::info!("Starting integration");
            self.receiver_configuration.integrate = true;
            let cancellation_token = CancellationToken::new();
            let measurement_task = {
                let address = self.receiver_address.clone();
                let measurements = self.measurements.clone();
                let cancellation_token = cancellation_token.clone();
                tokio::spawn(async move {
                    measure(address, measurements, cancellation_token).await;
                })
            };
            self.active_integration = Some(ActiveIntegration {
                cancellation_token: cancellation_token,
                measurement_task: measurement_task,
            });
        } else if !receiver_configuration.integrate && self.receiver_configuration.integrate {
            log::info!("Stopping integration");
            if let Some(active_integration) = &mut self.active_integration {
                active_integration.cancellation_token.cancel();
            }
            self.receiver_configuration.integrate = false;
        }
        Ok(self.receiver_configuration)
    }

    async fn get_info(&self) -> Result<TelescopeInfo, TelescopeError> {
        let current_horizontal = match self.current_direction {
            Some(current_horizontal) => current_horizontal,
            None => return Err(TelescopeError::TelescopeNotConnected),
        };

        let status = match self.commanded_horizontal {
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

        let latest_observation = {
            let measurements = self.measurements.lock().await;
            match measurements.last() {
                None => None,
                Some(measurement) => {
                    let measurement = measurement.clone();
                    let latest_observation = ObservedSpectra {
                        frequencies: measurement.freqs,
                        spectra: measurement.amps,
                        observation_time: measurement.duration,
                    };
                    Some(latest_observation)
                }
            }
        };

        Ok(TelescopeInfo {
            id: self.name.clone(),
            status,
            current_horizontal,
            commanded_horizontal: self.commanded_horizontal,
            current_target: self.target,
            most_recent_error: self.most_recent_error.clone(),
            measurement_in_progress: self.active_integration.is_some(),
            latest_observation: latest_observation,
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

        let stream = create_connection(&self);
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(telescope_error) => {
                let error: TelescopeError = telescope_error.into();
                self.most_recent_error = Some(error.clone());
                return Err(error);
            }
        };

        if let Some(active_integration) = self.active_integration.take() {
            if active_integration.measurement_task.is_finished() {
                if let Err(error) = active_integration.measurement_task.await {
                    log::error!("Error while waiting for measurement task: {}", error);
                }
            } else {
                self.active_integration = Some(active_integration);
            }
        }

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

fn directions_are_close(a: Direction, b: Direction, tol: f64) -> bool {
    // The salsa telescope works with a precision of 0.1 degrees
    // We want to send new commands whenever we exceed this tolerance
    // but to report tracking status we allow more, so that we do not flip
    // status between tracking/slewing (e.g. due to control unit rounding errors)
    // Therefore we have the "tol" multiplier here, which scales the allowed error.
    let epsilon = tol * 0.1_f64.to_radians();
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

                // Check if more than 1 tolerance off, if so we need to send track command
                if !directions_are_close(target_horizontal, current_horizontal, 1.0) {
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
        let mut telescope = create(
            "salsa".to_string(),
            "127.0.0.1:3000".to_string(),
            "127.0.0.2".to_string(),
        );

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
