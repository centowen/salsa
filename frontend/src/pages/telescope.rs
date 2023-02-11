use crate::components::target_selector::TargetSelector;
use common::TelescopeTarget;
use gloo_net::http::Request;
use log::debug;
use yew::platform::spawn_local;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct Props {
    pub id: String,
}

pub struct TelescopePage {
    target: TelescopeTarget,
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    ChangeTarget(TelescopeTarget),
}

impl Component for TelescopePage {
    type Message = Message;
    type Properties = Props;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            target: TelescopeTarget::Parked,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::ChangeTarget(target) => {
                debug!("Updating target for {} to {:?}", &ctx.props().id, &target);
                let endpoint = format!("http://localhost:3000/telescope/target/{}", ctx.props().id);

                {
                    let target = target;
                    let id = ctx.props().id.clone();
                    spawn_local(async move {
                        let response = Request::post(&endpoint)
                            .json(&target)
                            .expect("Could not serialize target")
                            .send()
                            .await;
                        if let Err(error_response) = response {
                            log::error!("Failed to set target for {}: {}", &id, error_response)
                        }
                    });
                }

                self.target = target;
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
