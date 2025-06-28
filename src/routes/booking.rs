use crate::app::AppState;
use crate::error::InternalError;
use crate::models::booking::Booking;
use crate::models::user::User;
use crate::routes::index::render_main;
use askama::Template;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use axum::{Extension, Form};
use axum::{Router, extract::State, http::StatusCode, routing::get};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde::Deserialize;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/", get(get_bookings).post(create_booking))
        .with_state(state)
}

struct MyBooking {
    inner: Booking,
    active: bool,
}

#[derive(Template)]
#[template(path = "bookings.html")]
struct BookingsTemplate {
    my_bookings: Vec<MyBooking>,
    bookings: Vec<Booking>,
    telescope_names: Vec<String>,
}

async fn get_bookings(
    Extension(user): Extension<Option<User>>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, InternalError> {
    let bookings = Booking::fetch_all(state.database_connection).await?;
    let now = Utc::now();
    let my_bookings = match user {
        Some(ref user) => bookings
            .iter()
            .filter(|b| b.user_name == user.name && b.user_provider == user.provider)
            .cloned()
            .map(|b| MyBooking {
                inner: b.clone(),
                active: now > b.start_time && now < b.end_time,
            })
            .collect(),
        None => Vec::new(),
    };

    let content = BookingsTemplate {
        my_bookings,
        bookings,
        telescope_names: state.telescopes.get_names(),
    }
    .render()
    .expect("Template rendering should always succeed");
    let content = if headers.get("hx-request").is_some() {
        content
    } else {
        render_main(user, content)
    };
    Ok(Html(content).into_response())
}

#[derive(Deserialize, Debug)]
struct BookingForm {
    start_date: NaiveDate,
    start_time: NaiveTime,
    telescope: String,
    duration: i64,
}

async fn create_booking(
    Extension(user): Extension<Option<User>>,
    headers: HeaderMap,
    State(state): State<AppState>,
    Form(booking_form): Form<BookingForm>,
) -> Result<Response, InternalError> {
    if user.is_none() {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }
    let user = user.unwrap();

    let naive_datetime = NaiveDateTime::new(booking_form.start_date, booking_form.start_time);
    let start_time: DateTime<Utc> = Utc.from_utc_datetime(&naive_datetime);
    let end_time = start_time + Duration::hours(booking_form.duration);

    if !state.telescopes.contains_key(&booking_form.telescope).await {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    }
    let booking = Booking {
        start_time,
        end_time,
        user_name: user.name.clone(),
        user_provider: user.provider.clone(),
        telescope_name: booking_form.telescope,
    };
    // TODO: Do the overlap check in the database instead.
    let bookings = Booking::fetch_all(state.database_connection.clone()).await?;
    if !bookings
        .iter()
        .filter(|b| b.telescope_name == booking.telescope_name && b.overlaps(&booking))
        .any(|_| true)
    {
        Booking::create(
            state.database_connection.clone(),
            user.clone(),
            booking.telescope_name,
            booking.start_time,
            booking.end_time,
        )
        .await?;
    }

    get_bookings(Extension(Some(user)), headers, State(state)).await
}
