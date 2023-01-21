use yew::prelude::*;
use yew_router::prelude::*;
use yew::{html, Context};

use crate::Route;

pub enum Msg {}

pub struct WelcomePage {}

impl Component for WelcomePage {
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
            <div class="welcome">
                <div class="list-entry">
                    {"Welcome"}
                </div>
                <div class="list-entry">
                    <Link<Route> to={Route::Observe}>{ "Observe" }</Link<Route>>
                </div>
                <div class="list-entry">
                    <Link<Route> to={Route::Weather}>{ "Weather information" }</Link<Route>>
                </div>
            </div>
        }
    }
}
