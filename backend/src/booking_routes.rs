use crate::database::{DataBase, Storage};
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use common::Booking;

pub fn routes(database: DataBase<impl Storage + 'static>) -> Router {
    Router::new()
        .route("/", get(get_bookings).post(add_booking))
        .with_state(database)
}

pub async fn get_bookings<StorageType>(State(db): State<DataBase<StorageType>>) -> impl IntoResponse
where
    StorageType: Storage,
{
    let data_model = db
        .get_data()
        .await
        .expect("As long as no one is manually editing the database, this should never fail.");
    Json(data_model.bookings)
}

pub async fn add_booking(
    State(db): State<DataBase<impl Storage>>,
    Json(booking): Json<Booking>,
) -> (StatusCode, String) {
    let res = db
        .update_data(|mut data_model| {
            data_model.bookings.push(booking);
            data_model
        })
        .await;
    match res {
        Ok(_) => (
            StatusCode::CREATED,
            db.get_data()
                .await
                .expect(
                    "As long as no one is manually editing the database, this should never fail.",
                )
                .bookings
                .len()
                .to_string(),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Database unavailable".to_string(),
        ),
    }
}

#[cfg(test)]
mod test {
    use crate::database::create_in_memory_database;

    use super::*;
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use common::Booking;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_bookings() {
        let booking = Booking {
            telescope_name: "test-telescope".to_string(),
            user_name: "test-user".to_string(),
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
        };

        let db = create_in_memory_database();
        db.update_data(|mut datamodel| {
            datamodel.bookings.push(booking.clone());
            datamodel
        })
        .await
        .unwrap();
        let app = routes(db);

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::GET)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let bookings: Vec<Booking> = serde_json::from_slice(&body).unwrap();
        assert_eq!(bookings, vec![booking]);
    }

    #[tokio::test]
    async fn test_add_booking() {
        let db = create_in_memory_database();
        let app = routes(db.clone());

        let booking = Booking {
            telescope_name: "test-telescope".to_string(),
            user_name: "test-user".to_string(),
            start_time: chrono::Utc::now(),
            end_time: chrono::Utc::now(),
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(serde_json::to_vec(&booking.clone()).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(body, "1"); // 1 because the database is empty before the request

        assert_eq!(
            vec![booking],
            db.get_data()
                .await
                .expect(
                    "As long as no one is manually editing the database, this should never fail."
                )
                .bookings
        );
    }
}
