use yew::prelude::*;
use yew::{html, Context};

pub enum Msg {}

#[derive(PartialEq, Properties)]
pub struct Props {
    pub id: String
}

pub struct TelescopePage {}

impl Component for TelescopePage {
    type Message = Msg;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, _ctx: &Context<Self>, _msg: Self::Message) -> bool {
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        html! {
            <div class="telescope">
                <div class="telescope-name">
                    { ctx.props().id.clone() }
                </div>
                <div class="telescope-movement">
                    { "Telescope movement control" }
                </div>
                <div class="telescope-receiver">
                    { "Telescope receiver" }
                </div>
            </div>
        }
    }
}
