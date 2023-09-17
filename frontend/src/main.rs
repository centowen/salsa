#![recursion_limit = "1024"]

use log::debug;

mod components;
mod coords;
mod pages;
mod services;

use common::TelescopeTarget;
use components::navbar::NavBar;
use pages::bookings::BookingsPage;
use pages::make_booking::MakeBookingPage;
use pages::observe::ObservePage;
use pages::telescope::TelescopePage;
use pages::weather::WeatherPage;
use pages::welcome::WelcomePage;
use yew::prelude::*;
use yew::{html, Context};
use yew_router::prelude::*;

pub enum Msg {
    MoveTelescope(TelescopeTarget),
}

pub struct Salsa {}

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum Route {
    #[not_found]
    #[at("/salsa/")]
    Home,
    #[at("/salsa/weather")]
    Weather,
    #[at("/salsa/bookings")]
    Bookings,
    #[at("/salsa/make_bookings")]
    MakeBooking,
    #[at("/salsa/observe")]
    Observe,
    #[at("/salsa/telescope/:id")]
    Telescope { id: String },
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! {
            <WelcomePage />
        },
        Route::Weather => html! {
            <WeatherPage />
        },
        Route::Bookings => html! {
            <BookingsPage />
        },
        Route::MakeBooking => html! {
            <MakeBookingPage />
        },
        Route::Observe => html! {
            <ObservePage />
        },
        Route::Telescope { id } => html! {
            <TelescopePage id={ id }/>
        },
    }
}

impl Component for Salsa {
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
            <div id="page">
                <BrowserRouter>
                    <header>
                        <div id="logo">{"SALSA"}</div>
                        <NavBar />
                    </header>
                    <div id="main-content" class="section">
                        <Switch<Route> render={switch} />
                    </div>
                    <footer>
                        { "Made by weirdos 🦆" }
                    </footer>
                </BrowserRouter>
            </div>
        }
    }
}

fn main() {
    console_error_panic_hook::set_once();

    // web_logger::init();
    wasm_logger::init(wasm_logger::Config::default());
    debug!("Starting front end!");
    yew::Renderer::<Salsa>::new().render();
}
