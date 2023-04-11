use crate::database::DataBase;
use common::Booking;
use warp::Filter;

pub fn routes(
    db: DataBase<Vec<Booking>>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    filters::get_bookings(db.clone()).or(filters::add_booking(db.clone()))
}

mod filters {
    use super::handlers;
    use crate::database::DataBase;
    use common::Booking;
    use warp::{Filter, Rejection, Reply};

    pub fn get_bookings(
        db: DataBase<Vec<Booking>>,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        warp::path!("api" / "bookings")
            .and(warp::get())
            .and(with_database(db))
            .and_then(handlers::get_bookings)
    }

    pub fn add_booking(
        db: DataBase<Vec<Booking>>,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        warp::path!("api" / "booking")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_database(db))
            .then(handlers::add_booking)
    }

    fn with_database(
        db: DataBase<Vec<Booking>>,
    ) -> impl Filter<Extract = (DataBase<Vec<Booking>>,), Error = std::convert::Infallible> + Clone
    {
        warp::any().map(move || db.clone())
    }
}

mod handlers {
    use crate::database::DataBase;
    use common::Booking;
    use warp::{Rejection, Reply};

    pub async fn get_bookings(db: DataBase<Vec<Booking>>) -> Result<impl Reply, Rejection> {
        let bookings = db.get_data().await;
        Ok(warp::reply::json(&*bookings))
    }

    pub async fn add_booking(booking: Booking, mut db: DataBase<Vec<Booking>>) -> impl Reply {
        match db.update_data(|mut v| v.push(booking)).await {
            Ok(_) => warp::reply::with_status(
                db.get_data().await.len().to_string(),
                warp::http::StatusCode::CREATED,
            ),
            Err(_) => warp::reply::with_status(
                "Database unavailable".to_string(),
                warp::http::StatusCode::SERVICE_UNAVAILABLE,
            ),
        }
    }
}
