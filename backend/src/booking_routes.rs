use warp::Filter;

pub fn routes() -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    filters::get_bookings()
}

mod filters {
    use super::handlers;
    use warp::{Filter, Rejection, Reply};

    pub fn get_bookings() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        warp::path!("api" / "bookings")
            .and(warp::get())
            .and_then(handlers::get_bookings)
    }
}

mod handlers {
    use chrono::{offset::Utc, TimeZone};
    use common::Booking;
    use warp::{Rejection, Reply};

    pub async fn get_bookings() -> Result<impl Reply, Rejection> {
        let bookings = vec![
            Booking {
                start_time: Utc.with_ymd_and_hms(2023, 5, 20, 14, 00, 00).unwrap(),
                end_time: Utc.with_ymd_and_hms(2023, 5, 20, 16, 00, 00).unwrap(),
                telescope_name: "Vale".to_string(),
                user_name: "Anonymous".to_string(),
            },
            Booking {
                start_time: Utc.with_ymd_and_hms(2023, 5, 23, 00, 00, 00).unwrap(),
                end_time: Utc.with_ymd_and_hms(2023, 5, 24, 00, 00, 00).unwrap(),
                telescope_name: "Brage".to_string(),
                user_name: "Salsa Admin".to_string(),
            },
        ];
        Ok(warp::reply::json(&bookings))
    }
}
