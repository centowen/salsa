use crate::database::DataBase;
use common::Booking;
use warp::Filter;

pub fn routes(
    db: DataBase<Vec<Booking>>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    filters::get_bookings(db)
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
        // TODO: Do I need to clone here instead?
        Ok(warp::reply::json(&*bookings))
    }
}
