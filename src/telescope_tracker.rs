use crate::coords::{Direction, Location};
use crate::coords::{horizontal_from_equatorial, horizontal_from_galactic};
use crate::telescope_controller::{TelescopeCommand, TelescopeController, TelescopeResponse};
use crate::telescopes::{TelescopeError, TelescopeStatus, TelescopeTarget};
use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::{Instant, sleep_until};

pub const LOWEST_ALLOWED_ALTITUDE: f64 = 5.0f64 / 180.0f64 * std::f64::consts::PI;

pub struct TelescopeTrackerInfo {
    pub target: TelescopeTarget,
    pub commanded_horizontal: Option<Direction>,
    pub current_horizontal: Direction,
    pub status: TelescopeStatus,
    pub most_recent_error: Option<TelescopeError>,
}

pub struct TelescopeTracker {
    // FIXME: Do we need to lock the whole state at a time?
    state: Arc<Mutex<TelescopeTrackerState>>,
}

impl TelescopeTracker {
    pub fn new(controller_address: String) -> TelescopeTracker {
        let state = Arc::new(Mutex::new(TelescopeTrackerState {
            target: TelescopeTarget::Stopped,
            commanded_horizontal: None,
            current_direction: None,
            most_recent_error: None,
            should_restart: false,
        }));
        // FIXME: Keep track of this task and do a proper shutdown.
        tokio::spawn(tracker_task_function(state.clone(), controller_address));
        TelescopeTracker { state }
    }

    pub fn set_target(
        &mut self,
        target: TelescopeTarget,
    ) -> Result<TelescopeTarget, TelescopeError> {
        self.state.lock().unwrap().target = target;
        Ok(target)
    }

    pub fn restart(&self) {
        self.state.lock().unwrap().should_restart = true;
    }

    pub fn info(&self) -> Result<TelescopeTrackerInfo, TelescopeError> {
        let current_horizontal = match self.state.lock().unwrap().current_direction {
            Some(current_horizontal) => current_horizontal,
            None => return Err(TelescopeError::TelescopeNotConnected),
        };
        let commanded_horizontal = self.commanded_horizontal();
        let status = match commanded_horizontal {
            Some(commanded_horizontal) => {
                // Check if more than 2 tolerances off, if so we are not tracking anymore
                if directions_are_close(commanded_horizontal, current_horizontal, 2.0) {
                    TelescopeStatus::Tracking
                } else {
                    TelescopeStatus::Slewing
                }
            }
            None => TelescopeStatus::Idle,
        };
        let (target, most_recent_error) = {
            let lock = self.state.lock().unwrap();
            (lock.target, lock.most_recent_error.clone())
        };
        Ok(TelescopeTrackerInfo {
            target,
            current_horizontal,
            commanded_horizontal,
            status,
            most_recent_error,
        })
    }

    pub fn direction(&self) -> Result<Direction, TelescopeError> {
        match self.state.lock().unwrap().current_direction {
            Some(current_direction) => Ok(current_direction),
            None => Err(TelescopeError::TelescopeNotConnected),
        }
    }

    pub fn target(&self) -> Result<TelescopeTarget, TelescopeError> {
        Ok(self.state.lock().unwrap().target)
    }

    fn commanded_horizontal(&self) -> Option<Direction> {
        self.state.lock().unwrap().commanded_horizontal
    }
}

struct TelescopeTrackerState {
    target: TelescopeTarget,
    commanded_horizontal: Option<Direction>,
    current_direction: Option<Direction>,
    most_recent_error: Option<TelescopeError>,
    should_restart: bool,
}

async fn tracker_task_function(
    state: Arc<Mutex<TelescopeTrackerState>>,
    controller_address: String,
) {
    let mut connection_established = false;

    loop {
        // 10 Hz update freq
        sleep_until(Instant::now() + Duration::from_millis(100)).await;

        let mut controller = match TelescopeController::connect(&controller_address) {
            Ok(controller) => controller,
            Err(err) => {
                state.lock().unwrap().most_recent_error = Some(err);
                continue;
            }
        };

        if !connection_established {
            let mut state_guard = state.lock().unwrap();
            state_guard.most_recent_error = controller.execute(TelescopeCommand::Stop).err();
            state_guard.commanded_horizontal = None;
            connection_established = true;
        }

        if state.lock().unwrap().should_restart {
            state.lock().unwrap().most_recent_error =
                controller.execute(TelescopeCommand::Restart).err();
            connection_established = false;
            sleep_until(Instant::now() + Duration::from_secs(10)).await;
            state.lock().unwrap().should_restart = false;
            continue;
        }

        let res = update_direction(&mut state.lock().unwrap(), Utc::now(), &mut controller);
        state.lock().unwrap().most_recent_error = res.err();
    }
}

fn update_direction(
    state: &mut TelescopeTrackerState,
    when: DateTime<Utc>,
    controller: &mut TelescopeController,
) -> Result<(), TelescopeError> {
    // FIXME: How do we handle static configuration like this?
    let location = Location {
        longitude: 0.20802143022, //(11.0+55.0/60.0+7.5/3600.0) * PI / 180.0. Sign positive, handled in gmst calc
        latitude: 1.00170457462,  //(57.0+23.0/60.0+36.4/3600.0) * PI / 180.0
    };
    let target_horizontal = calculate_target_horizontal(state.target, location, when);
    let current_horizontal = match controller.execute(TelescopeCommand::GetDirection)? {
        TelescopeResponse::CurrentDirection(direction) => Ok(direction),
        _ => Err(TelescopeError::TelescopeIOError(
            "Telescope did not respond with current direction".to_string(),
        )),
    }?;
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
                controller.execute(TelescopeCommand::SetDirection(target_horizontal))?;
            }

            Ok(())
        }
        None => {
            if state.commanded_horizontal.is_some() {
                controller.execute(TelescopeCommand::Stop)?;
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

fn directions_are_close(a: Direction, b: Direction, tol: f64) -> bool {
    // The salsa telescope works with a precision of 0.1 degrees
    // We want to send new commands whenever we exceed this tolerance
    // but to report tracking status we allow more, so that we do not flip
    // status between tracking/slewing (e.g. due to control unit rounding errors)
    // Therefore we have the "tol" multiplier here, which scales the allowed error.
    let epsilon = tol * 0.1_f64.to_radians();
    (a.azimuth - b.azimuth).abs() < epsilon && (a.altitude - b.altitude).abs() < epsilon
}
