use yew::prelude::*;
use yew::{html, Context};
use yew_router::prelude::*;

use crate::Route;

pub enum Msg {}

pub struct NavBar {}

impl Component for NavBar {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, _ctx: &Context<Self>, _msg: Self::Message) -> bool {
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <nav>
                <menu>
                    <li class="list-entry">
                        <Link<Route> to={Route::Observe}>{ "Observe" }</Link<Route>>
                    </li>
                    <li class="list-entry">
                        <Link<Route> to={Route::Bookings}>{ "Bookings" }</Link<Route>>
                    </li>
                    <li class="list-entry">
                        <Link<Route> to={Route::MakeBooking}>{ "Make booking" }</Link<Route>>
                    </li>
                    <li class="list-entry">
                        <Link<Route> to={Route::Weather}>{ "Weather" }</Link<Route>>
                    </li>
                </menu>
            </nav>
        }
    }
}
