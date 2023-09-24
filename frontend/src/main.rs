#![recursion_limit = "1024"]

use log::debug;

mod pages;
mod services;

use pages::welcome::WelcomePage;
use pages::weather::WeatherPage;
use pages::telescope::TelescopePage;
use pages::observe::ObservePage;
use yew::prelude::*;
use yew::{html, Context};
use yew_router::prelude::*;
use common::TelescopeTarget;

pub enum Msg {
    MoveTelescope(TelescopeTarget)
}

pub struct Salsa {}

#[derive(Debug, Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/weather")]
    Weather,
    #[at("/observe")]
    Observe,
    #[at("/telescope/:id")]
    Telescope {id: String},
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! {
            <WelcomePage />
        },
        Route::Weather => html! {
            <WeatherPage />
        },
        Route::Observe => html! {
            <ObservePage />
        },
        Route::Telescope {id} => html! {
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
