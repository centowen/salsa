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
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Booking, InternalError> {
        let conn = connection.lock().await;
        conn.execute(
            "insert into booking (user_id, telescope_id, start_timestamp, end_timestamp)
                 values ((?1), (?2), (?3), (?4))",
            (&user.id, &telescope_id, start.timestamp(), end.timestamp()),
        )
        .map_err(|err| InternalError::new(format!("Failed to insert user in db: {err}")))?;
        Ok(Booking {
            start_time: start,
            end_time: end,
            telescope_name: telescope_id,
            user_name: user.name,
        })
    }

    pub async fn fetch_all(
        connection: Arc<Mutex<Connection>>,
    ) -> Result<Vec<Booking>, InternalError> {
        let conn = connection.lock().await;
        let mut stmt = conn
            .prepare(
                "select start_timestamp, end_timestamp, telescope_id, username
                        from booking, user
                        where booking.user_id = user.id",
            )
            .map_err(|err| InternalError::new(format!("Failed to prepare statement: {err}")))?;
        let bookings = stmt
            .query_map([], |row| {
                Ok(Booking {
                    start_time: DateTime::<Utc>::from_timestamp(row.get(0)?, 0).unwrap(),
                    end_time: DateTime::<Utc>::from_timestamp(row.get(1)?, 0).unwrap(),
                    telescope_name: row.get(2)?,
                    user_name: row.get(3)?,
                })
            })
            .map_err(|err| InternalError::new(format!("Failed to query_map: {err}")))?;

        let mut res = Vec::new();
        for booking in bookings {
            match booking {
                Ok(booking) => res.push(booking),
                Err(err) => {
                    return Err(InternalError::new(format!("Failed to map row: {err}")));
                }
            }
        }
        Ok(res)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum AddBookingError {
    ServiceUnavailable,
    Conflict,
}
