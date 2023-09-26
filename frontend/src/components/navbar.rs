use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;

#[function_component(NavBar)]
pub fn navbar() -> Html {
    let hide_menu = use_state(|| true);

    let toggle_menu = Callback::from({
        let hide_menu = hide_menu.clone();
        move |_| {
            hide_menu.set(!*hide_menu);
        }
    });

    html! {
        <header>
            <div class="logo-burger">
                <div class="logo"><Link<Route> to={Route::Home}>{"SALSA"}</Link<Route>></div>
                <a href="#" onclick={ toggle_menu } class="burger"><i class="fa fa-bars" /></a>
            </div>
            <nav class={ classes!(hide_menu.then_some("hide-menu")) }>
                <menu>
                    <li>
                        <Link<Route> to={Route::Observe}>{ "Observe" }</Link<Route>>
                    </li>
                    <li>
                        <Link<Route> to={Route::Bookings}>{ "Bookings" }</Link<Route>>
                    </li>
                    <li>
                        <Link<Route> to={Route::MakeBooking}>{ "Make booking" }</Link<Route>>
                    </li>
                    <li>
                        <Link<Route> to={Route::Weather}>{ "Weather" }</Link<Route>>
                    </li>
                </menu>
            </nav>
            <nav class={ classes!(hide_menu.then_some("hide-menu")) }>
                <menu>
                    <li><a href="#">{ "Login" }</a></li>
                </menu>
            </nav>
        </header>
    }
}
