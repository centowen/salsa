use crate::database::{DataBase, Storage};
use warp::Filter;

pub const BOOKINGS_KEY: &str = "bookings";

pub fn routes<StorageType>(
    db: DataBase<StorageType>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
where
    StorageType: Storage,
{
    filters::get_bookings(db.clone()).or(filters::add_booking(db.clone()))
}

mod filters {
    use super::handlers;
    use crate::database::{DataBase, Storage};
    use warp::{Filter, Rejection, Reply};

    pub fn get_bookings<StorageType>(
        db: DataBase<StorageType>,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
    where
        StorageType: Storage,
    {
        warp::path!("api" / "bookings")
            .and(warp::get())
            .and(with_database(db))
            .and_then(handlers::get_bookings)
    }

    pub fn add_booking<StorageType>(
        db: DataBase<StorageType>,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
    where
        StorageType: Storage,
    {
        warp::path!("api" / "booking")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_database(db))
            .then(handlers::add_booking)
    }

    fn with_database<StorageType>(
        db: DataBase<StorageType>,
    ) -> impl Filter<Extract = (DataBase<StorageType>,), Error = std::convert::Infallible> + Clone
    where
        StorageType: Storage,
    {
        warp::any().map(move || db.clone())
    }
}

mod handlers {
    use crate::database::{DataBase, Storage};
    use common::Booking;
    use warp::{Rejection, Reply};

    use super::BOOKINGS_KEY;

    pub async fn get_bookings<StorageType>(
        db: DataBase<StorageType>,
    ) -> Result<impl Reply, Rejection>
    where
        StorageType: Storage,
    {
        let bookings = db
            .get_data::<Vec<Booking>>(BOOKINGS_KEY)
            .await
            .expect("As long as no one is manually editing the database, this should never fail.");
        Ok(warp::reply::json(&bookings))
    }

    pub async fn add_booking<StorageType>(booking: Booking, db: DataBase<StorageType>) -> impl Reply
    where
        StorageType: Storage,
    {
        match db.update_data::<Vec<Booking>, _>(BOOKINGS_KEY, |mut v| { v.push(booking); v }).await {
            Ok(_) => warp::reply::with_status(
                db.get_data::<Vec<Booking>>(BOOKINGS_KEY).await.expect("As long as no one is manually editing the database, this should never fail.").len().to_string(),
                warp::http::StatusCode::CREATED,
            ),
            Err(_) => warp::reply::with_status(
                "Database unavailable".to_string(),
                warp::http::StatusCode::SERVICE_UNAVAILABLE,
            ),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::database::create_in_memory_database;

    use super::*;
    use common::Booking;

    #[tokio::test]
    async fn test_get_bookings() {
        let db = create_in_memory_database();
        let booking = Booking {
            telescope_name: "test-telescope".to_string(),
            user_name: "test-user".to_string(),
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
        };
        db.update_data::<Vec<Booking>, _>(BOOKINGS_KEY, |mut bookings| {
            bookings.push(booking.clone());
            bookings
        })
        .await
        .expect("should be possible to set db data");
        let response = warp::test::request()
            .method("GET")
            .path("/api/bookings")
            .reply(&routes(db))
            .await;
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.body(),
            serde_json::to_string(&[booking]).unwrap().as_bytes()
        );
    }

    #[tokio::test]
    async fn test_add_booking() {
        let db = create_in_memory_database();
        let booking = Booking {
            telescope_name: "test-telescope".to_string(),
            user_name: "test-user".to_string(),
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
        };
        let response = warp::test::request()
            .method("POST")
            .path("/api/booking")
            .json(&booking)
            .reply(&routes(db.clone()))
            .await;
        assert_eq!(response.status(), warp::http::StatusCode::CREATED);
        assert_eq!(response.body(), "1"); // 1 because the database is empty before the request

        assert_eq!(
            vec![booking],
            db.get_data::<Vec<Booking>>(BOOKINGS_KEY).await.expect(
                "As long as no one is manually editing the database, this should never fail."
            )
        );
    }
}
