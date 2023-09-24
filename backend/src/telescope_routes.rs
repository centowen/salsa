use crate::telescope::TelescopeCollection;
use warp::Filter;

pub fn routes(
    telescopes: TelescopeCollection,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    filters::get_telescope_direction(telescopes.clone())
        .or(filters::get_telescope_target(telescopes.clone()))
        .or(filters::set_telescope_target(telescopes.clone()))
        .or(filters::get_telescope_info(telescopes.clone()))
        .or(filters::set_receiver_configuration(telescopes.clone()))
}

mod filters {
    use super::handlers;
    use crate::telescope::TelescopeCollection;
    use warp::{Filter, Rejection, Reply};

    pub fn get_telescope_direction(
        telescope_collection: TelescopeCollection,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        warp::path!("api" / "telescope" / String / "direction")
            .and(warp::get())
            .and(with_telescopes(telescope_collection))
            .and_then(handlers::get_telescope_direction)
    }

    pub fn get_telescope_target(
        telescopes: TelescopeCollection,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        warp::path!("api" / "telescope" / String / "target")
            .and(warp::get())
            .and(with_telescopes(telescopes))
            .and_then(handlers::get_telescope_target)
    }

    pub fn set_telescope_target(
        telescopes: TelescopeCollection,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        warp::path!("api" / "telescope" / String / "target")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_telescopes(telescopes))
            .and_then(handlers::set_telescope_target)
    }

    pub fn set_receiver_configuration(
        telescopes: TelescopeCollection,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        warp::path!("api" / "telescope" / String / "receiver")
            .and(warp::post())
            .and(warp::body::json())
            .and(with_telescopes(telescopes))
            .and_then(handlers::set_receiver_configuration)
    }

    pub fn get_telescope_info(
        telescopes: TelescopeCollection,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
        warp::path!("api" / "telescope" / String)
            .and(warp::get())
            .and(with_telescopes(telescopes))
            .and_then(handlers::get_telescope_info)
    }

    fn with_telescopes(
        telescope_collection: TelescopeCollection,
    ) -> impl Filter<Extract = (TelescopeCollection,), Error = std::convert::Infallible> + Clone
    {
        warp::any().map(move || telescope_collection.clone())
    }
}

mod handlers {
    use crate::telescope::{Telescope, TelescopeCollection};
    use common::{ReceiverConfiguration, TelescopeTarget};
    use warp::{Rejection, Reply};

    async fn get_telescope(
        telescopes: TelescopeCollection,
        id: &str,
    ) -> Result<tokio::sync::OwnedMutexGuard<dyn Telescope>, Rejection> {
        let telescope = {
            let telescopes = telescopes.read().await;
            telescopes.get(id).cloned()
        };
        if let Some(telescope) = telescope {
            Ok(telescope.lock_owned().await)
        } else {
            Err(warp::reject::not_found())
        }
    }

    pub async fn get_telescope_direction(
        id: String,
        telescopes: TelescopeCollection,
    ) -> Result<impl Reply, Rejection> {
        let telescope = get_telescope(telescopes, &id).await?;
        let direction = telescope.get_direction().await;
        Ok(warp::reply::json(&direction))
    }

    pub async fn get_telescope_target(
        id: String,
        telescopes: TelescopeCollection,
    ) -> Result<impl Reply, Rejection> {
        let telescope = get_telescope(telescopes, &id).await?;
        let target = telescope.get_target().await;
        Ok(warp::reply::json(&target))
    }

    pub async fn set_telescope_target(
        id: String,
        target: TelescopeTarget,
        telescopes: TelescopeCollection,
    ) -> Result<impl Reply, Rejection> {
        let mut telescope = get_telescope(telescopes, &id).await?;
        let result = telescope.set_target(target).await;
        Ok(warp::reply::json(&result))
    }

    pub async fn set_receiver_configuration(
        id: String,
        receiver_configuration: ReceiverConfiguration,
        telescopes: TelescopeCollection,
    ) -> Result<impl Reply, Rejection> {
        let mut telescope = get_telescope(telescopes, &id).await?;
        let result = telescope
            .set_receiver_configuration(receiver_configuration)
            .await;
        Ok(warp::reply::json(&result))
    }

    pub async fn get_telescope_info(
        id: String,
        telescopes: TelescopeCollection,
    ) -> Result<impl Reply, Rejection> {
        let telescope = get_telescope(telescopes, &id).await?;
        let info = telescope.get_info().await;
        Ok(warp::reply::json(&info))
    }
}
