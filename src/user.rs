use std::sync::Arc;

use log::trace;
use rusqlite::{Connection, Error};
use tokio::sync::Mutex;

use crate::error::InternalError;

#[derive(Clone)]
pub struct User {
    pub id: i64,
    pub name: String,
}

impl User {
    pub async fn fetch(
        connection: Arc<Mutex<Connection>>,
        id: i64,
    ) -> Result<Option<User>, InternalError> {
        let conn = connection.lock().await;
        match conn.query_row("select * from user where id = (?1)", ((id),), |row| {
            Ok((
                row.get::<usize, i64>(0)
                    .expect("Table 'user' has known layout"),
                row.get::<usize, String>(1)
                    .expect("Table 'user' has known layout"),
            ))
        }) {
            Ok((id, name)) => Ok(Some(User { id, name })),
            Err(Error::QueryReturnedNoRows) => {
                trace!("Fetch user query returned no rows");
                Ok(None)
            }
            Err(err) => {
                trace!("Error running fetch user query");
                Err(InternalError::new(format!(
                    "Failed to fetch user from db: {err}"
                )))
            }
        }
    }

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
        Ok(User {
            id: conn.last_insert_rowid(),
            name,
        })
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
                Ok((
                    row.get::<usize, i64>(0)
                        .expect("Table 'user' has known layout"),
                    row.get::<usize, String>(1)
                        .expect("Table 'user' has known layout"),
                ))
            },
        ) {
            Ok((id, name)) => Ok(Some(User { id, name })),
            Err(Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(InternalError::new(format!(
                "Failed to fetch user from db: {err}"
            ))),
        }
    }
}
