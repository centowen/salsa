#![recursion_limit = "1024"]

use log::debug;

mod pages;
mod services;

use pages::weather::WeatherPage;
use yew::prelude::*;
use yew::{html, Context};
use yew_router::prelude::*;

pub enum Msg {}

pub struct Salsa {}

#[derive(Clone, Debug, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! {
            <WeatherPage />
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
            <BrowserRouter>
                <Switch<Route> render={switch} />
            </BrowserRouter>
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
