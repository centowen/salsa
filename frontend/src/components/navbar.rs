use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;

#[function_component(NavBar)]
pub fn navbar() -> Html {
    let hidden_menu = use_state(|| true);

    let toggle_menu = Callback::from({
        let hidden_menu = hidden_menu.clone();
        move |_| {
            hidden_menu.set(!*hidden_menu);
        }
    });

    html! {
        <header>
            <div class="logo-burger">
                <div class="logo"><Link<Route> to={Route::Home}>{"SALSA"}</Link<Route>></div>
                <a href="#" onclick={ toggle_menu } class="burger"><i class="fa fa-bars" /></a>
            </div>
            <nav class={ classes!(hidden_menu.then_some("hidden-menu")) }>
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
            <nav class={ classes!(hidden_menu.then_some("hidden-menu")) }>
                <menu>
                    <li><a href="#">{ "Login" }</a></li>
                </menu>
            </nav>
        </header>
    }
}
