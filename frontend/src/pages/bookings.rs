use common::Booking;
use gloo_net::http::Request;
use web_sys::HtmlInputElement;
use yew::platform::spawn_local;
use yew::prelude::*;
use yew::{html, Context};
fn value_from_ref(node_ref: &NodeRef) -> Option<String> {
    node_ref.cast::<HtmlInputElement>().map(|e| e.value())
}

#[function_component(NewBooking)]
fn new_booking() -> Html {
    let telescope_ref = use_node_ref();
    let start_time_ref = use_node_ref();
    let end_time_ref = use_node_ref();

    let onclick = {
        let telescope_ref = telescope_ref.clone();
        let start_time_ref = start_time_ref.clone();
        let end_time_ref = end_time_ref.clone();

        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            log::info!("Submit!");

            let telescope_name = value_from_ref(&telescope_ref).unwrap();
            let start_time = value_from_ref(&start_time_ref).unwrap();
            let end_time = value_from_ref(&end_time_ref).unwrap();
            log::info!("Telescope: {:?}", telescope_name);
            log::info!("Start time: {:?}", start_time);
            log::info!("End time: {:?}", end_time);

            let booking = Booking {
                telescope_name,
                start_time: start_time.parse().unwrap(),
                end_time: end_time.parse().unwrap(),
                user_name: "Anonymous".to_string(),
            };

            spawn_local(async move {
                match Request::post("/api/booking")
                    .json::<Booking>(&booking)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        log::info!("Got response: {:?}", response);
                        let value = response
                            .json::<usize>()
                            .await
                            .expect("Failed to deserialize response");
                        log::info!("Got response value: {:?}", value);
                    }
                    Err(error) => {
                        log::error!("Failed to fetch: {}", error);
                    }
                }
            });
        })
    };

    html!(
        <div class="new-booking">
            <form id="new-booking-form" method="get" onsubmit={ onclick }>
                <label for="telescope">{ "Telescope:" }</label>
                <select name="telescope" ref={ telescope_ref } id="telescope">
                    <option value="">{ "Any telescope" }</option>
                    <option value="brage">{ "Brage" }</option>
                    <option value="vale">{ "Vale" }</option>
                    <option value="torre">{ "Torre" }</option>
                </select>
                <label for="start-time">{ "Start time:" }</label>
                <input type="text" ref={ start_time_ref } id="start-time" name="start-time" />
                <label for="end-time">{ "End time:" }</label>
                <input type="text" ref={ end_time_ref } id="end-time" name="end-time" />
                <input type="submit" value="Submit" />
            </form>
        </div>
    )
}

pub enum Message {
    ReceiveBookings(Vec<Booking>),
}

#[derive(Debug, Default)]
pub struct BookingsPage {
    bookings: Vec<Booking>,
}

impl Component for BookingsPage {
    type Message = Message;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let result_callback = ctx.link().callback(Message::ReceiveBookings);
        spawn_local(async move {
            let result = match Request::get("/api/bookings").send().await {
                Ok(bookings) => bookings
                    .json::<Vec<Booking>>()
                    .await
                    .expect("Could not deserialize bookings"),
                Err(error_response) => {
                    log::error!("Failed to get bookings: {}", error_response);
                    Vec::new()
                }
            };

            result_callback.emit(result);
        });
        Self::default()
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::ReceiveBookings(bookings) => {
                self.bookings = bookings;
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let bookings = self
            .bookings
            .iter()
            .map(|booking| {
                html! {
                    <div>{
                        format!("{}: {} booked by {}",
                                booking.start_time.naive_local(),
                                booking.telescope_name,
                                booking.user_name)
                    }</div>
                }
            })
            .collect::<Html>();
        html! {
            <div>
                <div class="bookings">
                    { bookings }
                </div>
                <NewBooking />
            </div>
        }
    }
}
