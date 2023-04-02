use common::Booking;
use gloo_net::http::Request;
use yew::platform::spawn_local;
use yew::prelude::*;
use yew::{html, Context};

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
            <div class="bookings">
                { bookings }
            </div>
        }
    }
}
