use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod api_routes;
pub mod routes;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Booking {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub telescope_name: String,
    pub user_name: String,
}

impl Booking {
    pub fn overlaps(&self, other: &Booking) -> bool {
        self.end_time >= other.start_time && self.start_time <= other.end_time
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum AddBookingError {
    ServiceUnavailable,
    Conflict,
}

pub type AddBookingResult = Result<u64, AddBookingError>;
