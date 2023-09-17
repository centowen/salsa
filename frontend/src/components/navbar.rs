use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;

#[function_component(NavBar)]
pub fn navbar() -> Html {
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
