use crate::components::target_selector::TargetSelector;
use crate::services::emit_info;
use common::{TelescopeError, TelescopeInfo, TelescopeStatus, TelescopeTarget};
use gloo_net::http::Request;
use std::time::Duration;
use yew::platform::spawn_local;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct Props {
    pub id: String,
}

#[derive(Debug, Copy, Clone)]
pub enum TelescopePageError {
    RequestError,
    TargetBelowHorizon,
}

impl From<TelescopeError> for TelescopePageError {
    fn from(value: TelescopeError) -> Self {
        match value {
            TelescopeError::TargetBelowHorizon { .. } => TelescopePageError::TargetBelowHorizon,
        }
    }
}

#[derive(Debug)]
pub struct TelescopePage {
    configured_target: TelescopeTarget,
    tracking_configured: bool,
    info: Option<TelescopeInfo>,
    most_recent_error: Option<TelescopePageError>,
    waiting_for_command: bool,
}

#[derive(Debug, Copy, Clone)]
pub enum Message {
    ChangeTarget((TelescopeTarget, bool)),
    ReceiveChangeTargetResult(Result<TelescopeTarget, TelescopePageError>),
    UpdateInfo(TelescopeInfo),
}

impl Component for TelescopePage {
    type Message = Message;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let info_cb = ctx.link().callback(Message::UpdateInfo);
        let endpoint = format!("http://localhost:3000/telescope/{}", &ctx.props().id);
        emit_info(info_cb, endpoint, Duration::from_millis(200));
        Self {
            configured_target: TelescopeTarget::Parked,
            tracking_configured: false,
            info: None,
            most_recent_error: None,
            waiting_for_command: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::ChangeTarget((target, track)) => {
                if self.configured_target == target && self.tracking_configured == track {
                    return false;
                }

                self.configured_target = target;
                self.tracking_configured = track;
                self.waiting_for_command = true;

                let endpoint = format!("http://localhost:3000/telescope/target/{}", ctx.props().id);

                {
                    let target = target;
                    let id = ctx.props().id.clone();
                    let result_callback = ctx.link().callback(Message::ReceiveChangeTargetResult);

                    spawn_local(async move {
                        let result = match Request::post(&endpoint)
                            .json(&target)
                            .expect("Could not serialize target")
                            .send()
                            .await
                        {
                            Ok(response) => match response
                                .json::<Result<TelescopeTarget, TelescopeError>>()
                                .await
                                .expect("Could not deserialize set_target result")
                            {
                                Ok(_) => Ok(target),
                                Err(error) => Err(error.into()),
                            },
                            Err(error_response) => {
                                log::error!("Failed to set target for {}: {}", &id, error_response);
                                Err(TelescopePageError::RequestError)
                            }
                        };

                        result_callback.emit(result);
                    });
                }

                true
            }
            Message::UpdateInfo(telescope_info) => {
                let mut updated = false;

                if !self.waiting_for_command {
                    let is_tracking = telescope_info.status != TelescopeStatus::Idle;
                    let target = telescope_info.current_target;
                    if self.tracking_configured != is_tracking {
                        self.tracking_configured = is_tracking;
                        updated = true;
                    }
                    if is_tracking && self.configured_target != target {
                        self.configured_target = target;
                        updated = true;
                    }
                }

                if self.info != Some(telescope_info) {
                    self.info = Some(telescope_info);
                    updated = true;
                }

                updated
            }
            Message::ReceiveChangeTargetResult(result) => {
                match result {
                    Ok(_) => {
                        self.info = None;
                        self.most_recent_error = None;
                    }
                    Err(error) => {
                        self.most_recent_error = Some(error);
                    }
                }
                self.waiting_for_command = false;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let change_target = ctx.link().callback(Message::ChangeTarget);
        let telescope_status = match &self.info {
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

        let (track, target) = (self.tracking_configured, self.configured_target);

        let error_text = if let Some(Some(error)) = self.info.map(|info| info.most_recent_error) {
            format!(
                " ({})",
                match error {
                    TelescopeError::TargetBelowHorizon =>
                        "Stopped tracking selected target, it set below the horizon.",
                }
            )
        } else if let Some(error) = self.most_recent_error {
            format!(
                " ({})",
                match error {
                    TelescopePageError::RequestError => "Failed to send request",
                    TelescopePageError::TargetBelowHorizon =>
                        "Could not track selected target, it is currently below the horizon.",
                }
            )
        } else {
            "".to_string()
        };

        html! {
            <div class="telescope">
                <div class="telescope-name">
                    <h1>{ ctx.props().id.clone() }</h1>
                </div>
                <div class="telescope-status">
                    {format!("Status: {}{}", telescope_status, error_text)}
                </div>
                <TargetSelector target={target} track={track} on_target_change={change_target} />
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
