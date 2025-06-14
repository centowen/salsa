use crate::bookings::Booking;
use crate::database::{DataBase, Storage};
use crate::index::render_main;
use crate::template::HtmlTemplate;
use crate::user::User;
use askama::Template;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse};
use axum::{Extension, Form};
use axum::{Router, extract::State, routing::get};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde::Deserialize;

pub fn routes(database: DataBase<impl Storage + 'static>) -> Router {
    Router::new()
        .route("/", get(get_bookings).post(create_booking))
        .with_state(database)
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

async fn get_bookings<StorageType>(
    Extension(user): Extension<User>,
    headers: HeaderMap,
    State(db): State<DataBase<StorageType>>,
) -> impl IntoResponse
where
    StorageType: Storage,
{
    let data_model = db
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
        render_main(user.name, content)
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

async fn create_booking<StorageType>(
    Extension(user): Extension<User>,
    State(db): State<DataBase<StorageType>>,
    Form(booking_form): Form<BookingForm>,
) -> impl IntoResponse
where
    StorageType: Storage,
{
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
    if db
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
        db.update_data(|mut data_model| {
            data_model.bookings.push(booking);
            data_model
        })
        .await
        .expect("failed to insert item into db")
    }

    let data_model = db
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
}
