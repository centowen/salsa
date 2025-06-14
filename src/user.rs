use std::sync::Arc;

use rusqlite::{Connection, Error};
use tokio::sync::Mutex;

use crate::error::InternalError;

#[derive(Clone)]
pub struct User {
    pub name: String,
}

impl User {
    pub async fn create_from_discord(
        connection: Arc<Mutex<Connection>>,
        name: String,
        discord_id: String,
    ) -> Result<User, InternalError> {
        let conn = connection.lock().await;
        conn.execute(
            "insert into user (username, discord_id) values ((?1), (?2))",
            (&name, &discord_id),
        )
        .map_err(|err| InternalError::new(format!("Failed to insert user in db: {err}")))?;
        Ok(User { name })
    }

    pub async fn fetch_with_discord_id(
        connection: Arc<Mutex<Connection>>,
        discord_id: String,
    ) -> Result<Option<User>, InternalError> {
        let conn = connection.lock().await;
        match conn.query_row(
            "select * from user where discord_id = (?1)",
            ((discord_id),),
            |row| {
                Ok(row
                    .get::<usize, String>(1)
                    .expect("Table 'user' has known layout"))
            },
        ) {
            Ok(name) => Ok(Some(User { name })),
            Err(Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(InternalError::new(format!(
                "Failed to fetch user from db: {err}"
            ))),
        }
    }
}
