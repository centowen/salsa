use yew::prelude::*;
use yew::{html, Context};
use yew_router::prelude::*;

use crate::Route;

pub enum Msg {}

pub struct ObservePage {}

impl Component for ObservePage {
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
            <div class="select-telescope">
                <div class="list-entry">
                    <Link<Route> to={Route::Telescope{id: "fake".into()}}>{ "Fake" }</Link<Route>>
                </div>
                <div class="list-entry">
                    <Link<Route> to={Route::Telescope{id: "brage".into()}}>{ "Brage" }</Link<Route>>
                    //<Link<Route> to={Route::Telescope{id: "vale".into()}}>{ "Vale" }</Link<Route>>
                </div>
            </div>
        }
    }
}
