use warp::fs::File;
use warp::http::Uri;
use warp::{reject, Filter, Rejection, Reply};

pub fn routes(
    frontend_path: Option<String>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    match frontend_path {
        Some(frontend_path) => routes_with_frontend(frontend_path).boxed(),
        None => warp::any()
            .and_then(move || async move { Err(reject()) })
            .boxed(),
    }
}

fn routes_with_frontend(
    frontend_path: String,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let all = warp::path!("salsa" / ..)
        .and(warp::fs::dir(frontend_path.clone()))
        .map(|file: File| file);
    let index = warp::path!("salsa" / ..)
        .and(warp::fs::file(format!(
            "{}/{}",
            &frontend_path, "index.html"
        )))
        .map(|file: File| file);
    let redirect_root = warp::path!().map(|| warp::redirect::temporary(Uri::from_static("/salsa")));

    all.or(index).or(redirect_root)
}
