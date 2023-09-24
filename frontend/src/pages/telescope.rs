use crate::components::target_selector::TargetSelector;
use common::TelescopeTarget;
use log::debug;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct Props {
    pub id: String,
}

pub struct TelescopePage {
    target: Option<TelescopeTarget>,
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    ChangeTarget(Option<TelescopeTarget>),
}

impl Component for TelescopePage {
    type Message = Message;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self { target: None }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::ChangeTarget(target) => {
                debug!("Updating target to {:?}", &target);
                self.target = target
            }
        };
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let change_target = ctx.link().callback(Message::ChangeTarget);
        html! {
            <div class="telescope">
                <div class="telescope-name">
                    <h1>{ ctx.props().id.clone() }</h1>
                </div>
                <TargetSelector target={self.target} on_target_change={change_target} />
                <div class="telescope-receiver">
                    { "Telescope receiver" }
                </div>
            </div>
        }
    }
}
