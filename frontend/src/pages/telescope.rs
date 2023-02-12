use crate::components::target_selector::TargetSelector;
use crate::services::emit_info;
use common::{TelescopeInfo, TelescopeStatus, TelescopeTarget};
use gloo_net::http::Request;
use log::debug;
use std::time::Duration;
use yew::platform::spawn_local;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct Props {
    pub id: String,
}

pub struct TelescopePage {
    target: TelescopeTarget,
    info: Option<TelescopeInfo>,
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    ChangeTarget(TelescopeTarget),
    UpdateInfo(TelescopeInfo),
}

impl Component for TelescopePage {
    type Message = Message;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let info_cb = ctx.link().callback(Message::UpdateInfo);
        let endpoint = format!("http://localhost:3000/telescope/{}", &ctx.props().id);
        emit_info(info_cb, endpoint, Duration::from_millis(1000));
        Self {
            target: TelescopeTarget::Parked,
            info: None,
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
                true
            }
            Message::UpdateInfo(telescope_info) => {
                if self.info != Some(telescope_info) {
                    log::info!("Received new telescope info: {:?}", telescope_info);
                    self.info = Some(telescope_info);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let change_target = ctx.link().callback(Message::ChangeTarget);
        let telescope_status = match self.info {
            Some(info) => match info.status {
                TelescopeStatus::Idle => "Idle",
                TelescopeStatus::Slewing => "Slewing",
                TelescopeStatus::Tracking => "Tracking",
            },
            None => "Loading",
        };

        let commanded_horizontal = self.info.map_or("Loading".to_string(), |info| {
            format!(
                "{:.1}°, {:.1}°",
                info.commanded_horizontal.azimuth.to_degrees(),
                info.commanded_horizontal.altitude.to_degrees()
            )
        });
        let current_horizontal = self.info.map_or("Loading".to_string(), |info| {
            format!(
                "{:.1}°, {:.1}°",
                info.current_horizontal.azimuth.to_degrees(),
                info.current_horizontal.altitude.to_degrees()
            )
        });
        html! {
            <div class="telescope">
                <div class="telescope-name">
                    <h1>{ ctx.props().id.clone() }</h1>
                </div>
                <div class="telescope-status">
                    {format!("Status: {}", telescope_status)}
                </div>
                <TargetSelector target={self.target} on_target_change={change_target} />
                <div class="current-horizontal">
                    {format!("Commanded horizontal: {}", commanded_horizontal) }
                </div>
                <div class="commanded-horizontal">
                    {format!("Current horizontal: {}", current_horizontal) }
                </div>
                <div class="telescope-receiver">
                    { "Telescope receiver" }
                </div>
            </div>
        }
    }
}
