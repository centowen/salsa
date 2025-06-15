use std::sync::Arc;

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{error::InternalError, user::User};

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

    pub async fn create(
        connection: Arc<Mutex<Connection>>,
        user: User,
        telescope_id: String,
        begin: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Booking, Error> {
        // KNARK: Check overlap first!
        let conn = connection.lock().await;
        conn.execute(
            "insert into booking (user_id, telescope_id, begin_timestamp, end_timestamp) values ((?1), (?2), (?3), (?4))",
            (&user.id, &telescope_id, begin.timestamp(), end.timestamp())
        )
        .map_err(|err| Error::Internal(InternalError::new(format!("Failed to insert user in db: {err}"))))?;
        Ok(Booking {
            start_time: begin,
            end_time: end,
            telescope_name: String::new(), // KNARK: Fill this in
            user_name: user.name,          // KNARK: This isn't right
        })
    }

    pub fn fetch_all() -> Vec<Booking> {
        todo!()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum AddBookingError {
    ServiceUnavailable,
    Conflict,
}

pub enum Error {
    Internal(InternalError),
    CouldNotCreateBooking,
}
