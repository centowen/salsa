use crate::components::notification_area::{NotificationArea, NotificationAreaProps};
use chrono::{Datelike, Duration, Months, NaiveDate, NaiveTime, TimeZone, Utc, Weekday};
use common::{AddBookingError, AddBookingResult, Booking, TelescopeInfo};
use gloo_net::http::{Method, Request};
use web_sys::HtmlInputElement;
use yew::html;
use yew::platform::spawn_local;
use yew::prelude::*;

const FIRST_DAY_OF_WEEK: Weekday = Weekday::Mon;
const LAST_DAY_OF_WEEK: Weekday = Weekday::Sun;

fn monday_before(date: NaiveDate) -> NaiveDate {
    date.iter_days()
        .rev()
        .skip(1)
        .find(|d| d.weekday() == FIRST_DAY_OF_WEEK)
        .unwrap()
}

fn sunday_after(date: NaiveDate) -> NaiveDate {
    date.succ_opt()
        .unwrap()
        .iter_days()
        .find(|d| d.weekday() == LAST_DAY_OF_WEEK)
        .unwrap()
}

fn start_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap()
}

fn end_of_month(date: NaiveDate) -> NaiveDate {
    let month = date.month();
    date.iter_days()
        .take_while(|d| d.month() == month)
        .last()
        .unwrap()
}

fn dates_between(start: NaiveDate, end: NaiveDate) -> Vec<NaiveDate> {
    start.iter_days().take_while(|d| d <= &end).collect()
}

#[derive(Clone, Debug, Properties, PartialEq)]
struct CalendarProps {
    date: NaiveDate,
    callback: Callback<NaiveDate>,
}

#[function_component(Calendar)]
fn calendar(props: &CalendarProps) -> Html {
    // TODO: Mark today?
    // TODO: Disable dates in the past
    // TODO: Move calender component to a dedicated module in the components folder
    let CalendarProps { date, callback } = props.clone();
    let dates = dates_between(
        monday_before(start_of_month(date)),
        sunday_after(end_of_month(date)),
    );
    let weeks = dates.chunks_exact(7);
    let month = date.format("%B").to_string();
    let prev_month = {
        let date = date.clone();
        let callback = callback.clone();
        move |event: MouseEvent| {
            event.prevent_default();
            callback.emit(date - Months::new(1))
        }
    };
    let next_month = {
        let date = date.clone();
        let callback = callback.clone();
        move |event: MouseEvent| {
            event.prevent_default();
            callback.emit(date + Months::new(1))
        }
    };
    html! {
        <div class="calendar">
            <div class="month">
                <button class="change-month" onclick={ prev_month }>{ "<" }</button>
                { month }
                <button class="change-month" onclick={ next_month }>{ ">" }</button>
            </div>
            <table>
            <tr>
            {
                dates.iter().take(7).map(|d| {
                    html! {
                        <td>
                            { d.weekday() }
                        </td>
                    }
                }).collect::<Html>()
            }
            </tr>
            {
                weeks.map(|week| {
                    html!(
                        <tr>
                        {
                            week.into_iter().map(|d| {
                                let mut classes = classes!["date"];
                                if *d == date {
                                    classes.push("cur-date")
                                }
                                if d.month() != date.month() {
                                    classes.push("other-month")
                                }
                                html!{
                                    <td class={ classes }>
                                        <a href="#" onclick={
                                            let d = d.clone();
                                            let callback = callback.clone();
                                            move |event: MouseEvent| {event.prevent_default(); callback.emit(d)}
                                        }>
                                            { d.format("%d") }
                                        </a>
                                    </td>
                                }
                            }).collect::<Html>()
                        }
                        </tr>
                    )
                }).collect::<Html>()
            }
            </table>
        </div>
    }
}

#[derive(Debug, Clone, Properties, PartialEq)]
struct TimePickerProps {
    time: NaiveTime,
    callback: Callback<Option<NaiveTime>>,
}

#[function_component(TimePicker)]
fn time_picker(props: &TimePickerProps) -> Html {
    let TimePickerProps { time, callback } = props;
    let input_ref = use_node_ref();
    let onchange = {
        let input_ref = input_ref.clone();
        let callback = callback.clone();
        Callback::from(move |_| {
            log::info!("Time changed...");
            let time_str = value_from_ref(&input_ref).unwrap();
            let time = NaiveTime::parse_from_str(&time_str, "%H:%M").ok();
            callback.emit(time);
        })
    };
    html!(
        <div class="time-picker">
            <label for="time-picker">{ "Time" }</label>
            <input
                type="text"
                value={ time.format("%H:%M").to_string() }
                onchange={ onchange }
                name="time-picker"
                ref={ input_ref }/>
        </div>
    )
}

#[derive(Debug, Clone, Properties, PartialEq)]
struct DurationInputProps {
    duration: Duration,
    callback: Callback<Option<Duration>>,
}

#[function_component(DurationInput)]
fn duration_input(props: &DurationInputProps) -> Html {
    let DurationInputProps { duration, callback } = props;
    let input_ref = use_node_ref();
    let onchange = {
        let input_ref = input_ref.clone();
        let callback = callback.clone();
        Callback::from(move |_| {
            log::info!("Duration changed...");
            let duration_str = value_from_ref(&input_ref).unwrap();
            let duration = duration_str.parse::<i64>().ok().map(|i| Duration::hours(i));
            callback.emit(duration);
        })
    };
    html!(
        <div class="duration">
            <label for="duration">{ "Duration" }</label>
            <input
                type="text"
                value={ duration.num_hours().to_string() }
                onchange={ onchange }
                name="duration"
                ref={ input_ref }/>
        </div>
    )
}

// #[derive(Error)]
// enum FormError {
//     FieldUnavailable,
//     UnknownTelescope,
//     InvalidDateOrTime,
//     DateTimeHasPast,
// }

// fn value_from_ref(node_ref: &NodeRef) -> Result<String, FormError> {
//     node_ref
//         .cast::<HtmlInputElement>()
//         .map(|e| e.value())
//         .ok_or(FormError::FieldUnavailable)
// }

fn value_from_ref(node_ref: &NodeRef) -> Option<String> {
    node_ref.cast::<HtmlInputElement>().map(|e| e.value())
}

async fn fetch<D, R>(data: D, endpoint: &str, method: Method) -> Result<R, gloo_net::Error>
where
    D: serde::Serialize,
    R: serde::de::DeserializeOwned,
{
    let response = Request::new(endpoint)
        .method(method)
        .json::<D>(&data)?
        .send()
        .await?;
    Ok(response.json::<R>().await?)
}

async fn fetch_add_booking_endpoint(booking: Booking) -> AddBookingResult {
    match fetch(booking, "/api/bookings", Method::POST).await {
        Ok(value) => value,
        Err(error) => {
            log::error!("fetch error: {:?}", error);
            Err(AddBookingError::ServiceUnavailable)
        }
    }
}

#[function_component(MakeBookingPage)]
pub fn make_booking_page() -> Html {
    let default_date = NaiveDate::from_ymd_opt(2023, 04, 19).unwrap();
    let default_time = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
    let default_duration = Duration::hours(1);
    let default_notifications = NotificationAreaProps::empty();

    let current_date = use_state(|| default_date);
    let current_time = use_state(|| default_time);
    let current_duration = use_state(|| default_duration);
    let notifications = use_state(|| default_notifications);

    let telescope_ref = use_node_ref();

    let change_date = {
        let current_date = current_date.clone();
        Callback::from(move |new_date: NaiveDate| {
            log::info!("Date changed: {:?}", new_date);
            current_date.set(new_date);
        })
    };

    let change_time = {
        let current_time = current_time.clone();
        Callback::from(move |new_time: Option<NaiveTime>| {
            log::info!("Time changed: {:?}", new_time);
            current_time.set(new_time.unwrap_or(*current_time));
        })
    };

    let change_duration = {
        let current_duration = current_duration.clone();
        Callback::from(move |new_duration: Option<Duration>| {
            log::info!("Duration changed: {:?}", new_duration);
            current_duration.set(new_duration.unwrap_or(*current_duration));
        })
    };

    let onclick = {
        let notifications = notifications.clone();
        let telescope_ref = telescope_ref.clone();
        let current_date = current_date.clone();
        let current_time = current_time.clone();
        let current_duration = current_duration.clone();

        Callback::from(move |event: SubmitEvent| {
            // TODO Move all of this to a function
            event.prevent_default();
            log::info!("Submit!");

            let telescope_name = value_from_ref(&telescope_ref).unwrap();
            let start_time = Utc.from_utc_datetime(&current_date.and_time(*current_time));
            let end_time = start_time + *current_duration;
            log::info!("Telescope: {:?}", telescope_name);
            log::info!("Start time: {:?}", start_time);
            log::info!("End time: {:?}", end_time);

            let booking = Booking {
                telescope_name,
                start_time,
                end_time,
                user_name: "Anonymous".to_string(),
            };

            spawn_local({
                let notifications = notifications.clone();
                async move {
                    let result = fetch_add_booking_endpoint(booking).await;
                    match result {
                        Err(AddBookingError::ServiceUnavailable) => notifications.set(
                            NotificationAreaProps::error("Unable to contact booking server!"),
                        ),
                        Err(AddBookingError::Conflict) => {
                            notifications.set(NotificationAreaProps::error("Booking conflict!"))
                        }

                        Ok(_value) => {
                            notifications.set(NotificationAreaProps::success("Booking made!"))
                        }
                    }
                }
            });
        })
    };

    let telescope_names = use_state(|| Vec::<String>::new());
    use_effect_with_deps(
        {
            let telescope_names2 = telescope_names.clone();
            |_| {
                spawn_local(async move {
                    match Request::get("/api/telescopes").send().await {
                        Ok(response) => {
                            log::info!("Got response: {:?}", response);
                            let value = response
                                .json::<Vec<TelescopeInfo>>()
                                .await
                                .expect("Failed to deserialize response");
                            log::info!("Got response value: {:?}", value);
                            telescope_names2.set(value.into_iter().map(|t| t.id).collect());
                        }
                        Err(error) => {
                            log::error!("Failed to fetch: {}", error);
                        }
                    }
                })
            }
        },
        (),
    );

    let NotificationAreaProps { message, level } = (*notifications).clone();
    html!(
        <div class="section light new-booking">
            <NotificationArea message={message} level={level} />
            <form id="new-booking-form" method="get" onsubmit={ onclick }>
                <div id="new-booking-cols">
                    <div class="new-booking-col">
                        <Calendar date={ *current_date } callback={ change_date }/>
                    </div>
                    <div class="new-booking-col">
                        <TimePicker time={ *current_time } callback={ change_time }/>
                        <DurationInput duration={ *current_duration } callback={ change_duration }/>
                        <label for="telescope">{ "Telescope" }</label>
                        <select name="telescope" ref={ telescope_ref } id="telescope">
                            <option value="">{ "Any telescope" }</option>
                            { telescope_names.iter().map(|t| html!{
                                <option value={ t.to_string() }>{ t }</option>
                            }).collect::<Html>() }
                        </select>
                    </div>
                </div>
                <div class="submit">
                    <input type="submit" value="Book telescope" />
                </div>
            </form>
        </div>
    )
}
