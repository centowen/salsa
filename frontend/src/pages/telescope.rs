use crate::components::graph::Graph;
use crate::components::target_selector::TargetSelector;
use crate::services::emit_info;
use common::{
    ReceiverConfiguration, ReceiverError, TelescopeError, TelescopeInfo, TelescopeStatus,
    TelescopeTarget,
};
use gloo_net::http::Request;
use std::time::Duration;
use yew::platform::spawn_local;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct Props {
    pub id: String,
}

#[derive(Debug, Clone)]
pub enum TelescopePageError {
    RequestError,
    TargetBelowHorizon,
    TelescopeIOError(String),
    TelescopeNotConnected,
}

impl From<TelescopeError> for TelescopePageError {
    fn from(value: TelescopeError) -> Self {
        match value {
            TelescopeError::TargetBelowHorizon => TelescopePageError::TargetBelowHorizon,
            TelescopeError::TelescopeIOError(error_message) => {
                TelescopePageError::TelescopeIOError(error_message)
            }
            TelescopeError::TelescopeNotConnected => TelescopePageError::TelescopeNotConnected,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum TelescopePageReceiverError {
    RequestError,
}

impl From<ReceiverError> for TelescopePageReceiverError {
    fn from(value: ReceiverError) -> Self {
        match value {
            ReceiverError::IntegrationAlreadyRunning => TelescopePageReceiverError::RequestError,
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

#[derive(Debug, Clone)]
pub enum Message {
    ChangeTarget((TelescopeTarget, bool)),
    ReceiveChangeTargetResult(Result<TelescopeTarget, TelescopePageError>),
    UpdateInfo(TelescopeInfo),
    SetReceiverConfiguration(bool),
    ReceiveSetReceiverConfigurationResult(
        Result<ReceiverConfiguration, TelescopePageReceiverError>,
    ),
}

impl Component for TelescopePage {
    type Message = Message;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let info_cb = ctx.link().callback(Message::UpdateInfo);
        let endpoint = format!("/api/telescope/{}", &ctx.props().id);
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

                let endpoint = format!("/api/telescope/{}/target", ctx.props().id);

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

                let telescope_info = Some(telescope_info);
                if &self.info != &telescope_info {
                    self.info = telescope_info;
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
            Message::SetReceiverConfiguration(integrate) => {
                let endpoint = format!("/api/telescope/{}/receiver", ctx.props().id);
                let id = ctx.props().id.clone();
                let receiver_configuration = ReceiverConfiguration { integrate };
                let result_callback = ctx
                    .link()
                    .callback(Message::ReceiveSetReceiverConfigurationResult);
                spawn_local(async move {
                    let result = match Request::post(&endpoint)
                        .json(&receiver_configuration)
                        .expect("Could not serialize received configuration")
                        .send()
                        .await
                    {
                        Ok(response) => match response
                            .json::<Result<ReceiverConfiguration, ReceiverError>>()
                            .await
                            .expect("Could not deserialize set_receiver_configuration result")
                        {
                            Ok(configuration) => Ok(configuration),
                            Err(error) => Err(error.into()),
                        },
                        Err(error_response) => {
                            log::error!("Failed to set target for {}: {}", &id, error_response);
                            Err(TelescopePageReceiverError::RequestError)
                        }
                    };

                    result_callback.emit(result);
                });
                true
            }
            Message::ReceiveSetReceiverConfigurationResult(result) => {
                match result {
                    Ok(result) => {
                        if result.integrate {
                            log::info!("Started integration for receiver")
                        } else {
                            log::info!("Stopped integration for receiver")
                        }
                    }
                    Err(error) => {
                        log::error!("Failed to change receiver configuration: {:?}", error)
                    }
                }
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

        let commanded_horizontal = self.info.as_ref().map_or("Loading".to_string(), |info| {
            if let Some(commanded_horizontal) = info.commanded_horizontal {
                format!(
                    "{:.1}°, {:.1}°",
                    commanded_horizontal.azimuth.to_degrees(),
                    commanded_horizontal.altitude.to_degrees()
                )
            } else {
                "".to_string()
            }
        });

        let current_horizontal = self.info.as_ref().map_or("Loading".to_string(), |info| {
            format!(
                "{:.1}°, {:.1}°",
                info.current_horizontal.azimuth.to_degrees(),
                info.current_horizontal.altitude.to_degrees()
            )
        });

        let (track, target) = (self.tracking_configured, self.configured_target);

        let error_text = if let Some(Some(error)) = self
            .info
            .as_ref()
            .map(|info| info.most_recent_error.clone())
        {
            format!(
                " ({})",
                match error {
                    TelescopeError::TargetBelowHorizon =>
                        "Stopped tracking selected target, it set below the horizon.".to_string(),
                    TelescopeError::TelescopeIOError(error_message) =>
                        format!("Communication with telescope failed: {}", error_message),
                    TelescopeError::TelescopeNotConnected =>
                        "No telescope connected, no observations will be possible".to_string(),
                }
            )
        } else if let Some(error) = &self.most_recent_error {
            format!(
                " ({})",
                match error {
                    TelescopePageError::RequestError => "Failed to send request".to_string(),
                    TelescopePageError::TargetBelowHorizon =>
                        "Could not track selected target, it is currently below the horizon."
                            .to_string(),
                    TelescopePageError::TelescopeIOError(error_message) =>
                        format!("Communication with telescope failed: {}", error_message),
                    TelescopePageError::TelescopeNotConnected =>
                        "No telescope connected, no observations will be possible".to_string(),
                }
            )
        } else {
            "".to_string()
        };

        let start_integrate = {
            let link = ctx.link().clone();
            Callback::from(move |_| {
                link.send_message(Self::Message::SetReceiverConfiguration(true));
            })
        };

        let stop_integrate = {
            let link = ctx.link().clone();
            Callback::from(move |_| {
                link.send_message(Self::Message::SetReceiverConfiguration(false));
            })
        };

        html! {
            <div class="telescope">
                <div class="telescope-name">
                    <h1>{ ctx.props().id.clone() }</h1>
                </div>
                <div class="telescope-target-control">
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
                </div>
                <div class="telescope-receiver">
                    if let Some(info) = self.info.as_ref() {
                        <button
                            disabled={info.measurement_in_progress
                                   || info.status != TelescopeStatus::Tracking}
                            onclick={start_integrate}
                        >
                            {"Integrate"}
                        </button>
                        <button
                            disabled={!info.measurement_in_progress}
                            onclick={stop_integrate}
                        >
                            {"Stop"}
                        </button>
                        if let Some(measurement) = info.latest_observation.as_ref() {
                            <div>{format!("Integration time: {}s",
                                          measurement.observation_time.as_secs())}</div>
                            <div>
                                <Graph id="spectra"
                                       x={measurement.frequencies.clone()}
                                       y={measurement.spectra.clone()} />
                            </div>
                        }
                    } else {
                        <div>{"Loading"}</div>
                    }
                </div>
            </div>
        }
    }
}
