use crate::app::AppState;
use crate::bookings::Booking;
use crate::index::render_main;
use crate::template::HtmlTemplate;
use crate::user::User;
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
) -> impl IntoResponse {
    let data_model = state
        .database
        .get_data()
        .await
        .expect("As long as no one is manually editing the database, this should never fail.");
    let bookings = data_model.bookings;
    let now = Utc::now();
    let my_bookings = match user {
        Some(ref user) => bookings
            .iter()
            .filter(|b| b.user_name == user.name)
            .cloned()
            .map(|b| MyBooking {
                inner: b.clone(),
                active: now > b.start_time && now < b.end_time,
            })
            .collect(),
        None => Vec::new(),
    };
    let telescope_names: Vec<String> = data_model
        .telescopes
        .iter()
        .map(|t| t.name.clone())
        .collect();
    let content = BookingsTemplate {
        my_bookings,
        bookings,
        telescope_names,
    }
    .render()
    .expect("Template rendering should always succeed");
    let content = if headers.get("hx-request").is_some() {
        content
    } else {
        render_main(user, content)
    };
    Html(content).into_response()
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
    State(state): State<AppState>,
    Form(booking_form): Form<BookingForm>,
) -> Response {
    if user.is_none() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let user = user.unwrap();

    let naive_datetime = NaiveDateTime::new(booking_form.start_date, booking_form.start_time);
    let start_time: DateTime<Utc> = Utc.from_utc_datetime(&naive_datetime);
    let end_time = start_time + Duration::hours(booking_form.duration);

    let booking = Booking {
        start_time,
        end_time,
        user_name: user.name.clone(),
        telescope_name: booking_form.telescope,
    };
    let mut skip = false;
    if state
        .database
        .get_data()
        // Error handling!
        .await
        .expect("Failed to get data")
        .bookings
        .iter()
        .filter(|b| b.telescope_name == booking.telescope_name && b.overlaps(&booking))
        .any(|_| true)
    {
        // There is already a booking of the selected telescope overlapping
        // with the new booking. The new booking must be rejected.
        skip = true;
    }

    if !skip {
        state
            .database
            .update_data(|mut data_model| {
                data_model.bookings.push(booking);
                data_model
            })
            .await
            .expect("failed to insert item into db")
    }

    let data_model = state
        .database
        .get_data()
        .await
        .expect("As long as no one is manually editing the database, this should never fail.");
    let bookings = data_model.bookings;
    let now = Utc::now();
    let my_bookings = bookings
        .iter()
        .filter(|b| b.user_name == user.name)
        .cloned()
        .map(|b| MyBooking {
            inner: b.clone(),
            active: now > b.start_time && now < b.end_time,
        })
        .collect();
    let telescope_names: Vec<String> = data_model
        .telescopes
        .iter()
        .map(|t| t.name.clone())
        .collect();

    HtmlTemplate(BookingsTemplate {
        my_bookings,
        bookings,
        telescope_names,
    })
    .into_response()
}
