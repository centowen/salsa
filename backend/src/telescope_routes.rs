use crate::telescope::TelescopeControl;
use warp::Filter;

pub fn routes<Telescope>(
    telescope: Telescope,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone
where
    Telescope: TelescopeControl + Send + Clone,
{
    filters::get_telescope_direction(telescope.clone())
        .or(filters::get_telescope_target(telescope.clone()))
        .or(filters::set_telescope_target(telescope.clone()))
        .or(filters::get_telescope_info(telescope.clone()))
}

mod filters {
    use super::handlers;
    use crate::telescope::{TelescopeControl, TelescopeError};
    use warp::reject::Reject;
    use warp::{Filter, Rejection, Reply};

    impl Reject for TelescopeError {}

    pub fn get_telescope_direction<Telescope>(
        telescope_control: Telescope,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
    where
        Telescope: TelescopeControl + Clone,
    {
        warp::path!("telescope" / "direction" / String)
            .and(warp::get())
            .and(with_telescope_control::<Telescope>(telescope_control))
            .and_then(handlers::get_telescope_direction)
    }

    pub fn get_telescope_target<Telescope>(
        telescope: Telescope,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
    where
        Telescope: TelescopeControl + Clone,
    {
        warp::path!("telescope" / "target" / String)
            .and(warp::get())
            .and(with_telescope_control(telescope))
            .and_then(handlers::get_telescope_target)
    }

    pub fn set_telescope_target<Telescope>(
        telescope: Telescope,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
    where
        Telescope: TelescopeControl + Clone,
    {
        warp::path!("telescope" / "target" / String)
            .and(warp::post())
            .and(warp::body::json())
            .and(with_telescope_control(telescope))
            .and_then(handlers::set_telescope_target)
    }

    pub fn get_telescope_info<Telescope>(
        telescope: Telescope,
    ) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
    where
        Telescope: TelescopeControl + Clone,
    {
        warp::path!("telescope" / String)
            .and(warp::get())
            .and(with_telescope_control(telescope))
            .and_then(handlers::get_telescope_info)
    }

    fn with_telescope_control<Telescope>(
        telescope: Telescope,
    ) -> impl Filter<Extract = (Telescope,), Error = std::convert::Infallible> + Clone
    where
        Telescope: TelescopeControl + Clone,
    {
        warp::any().map(move || telescope.clone())
    }
}

mod handlers {
    use crate::telescope::TelescopeControl;
    use common::TelescopeTarget;
    use warp::http::StatusCode;
    use warp::{Rejection, Reply};

    pub async fn get_telescope_direction<Telescope>(
        id: String,
        telescope: Telescope,
    ) -> Result<impl Reply, Rejection>
    where
        Telescope: TelescopeControl,
    {
        let direction = telescope.get_direction(&id).await?;
        Ok(warp::reply::json(&direction))
    }

    pub async fn get_telescope_target<Telescope>(
        id: String,
        telescope: Telescope,
    ) -> Result<impl Reply, Rejection>
    where
        Telescope: TelescopeControl,
    {
        let target = telescope.get_target(&id).await?;
        Ok(warp::reply::json(&target))
    }

    pub async fn set_telescope_target<Telescope>(
        id: String,
        target: TelescopeTarget,
        telescope: Telescope,
    ) -> Result<impl Reply, Rejection>
    where
        Telescope: TelescopeControl,
    {
        telescope.set_target(&id, target).await?;
        Ok(warp::reply::with_status("", StatusCode::OK))
    }

    pub async fn get_telescope_info<Telescope>(
        id: String,
        telescope: Telescope,
    ) -> Result<impl Reply, Rejection>
    where
        Telescope: TelescopeControl,
    {
        let info = telescope.get_info(&id).await?;
        Ok(warp::reply::json(&info))
    }
}
