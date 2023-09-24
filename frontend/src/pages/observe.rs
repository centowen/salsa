use common::TelescopeInfo;
use gloo_net::http::Request;
use yew::platform::spawn_local;
use yew::prelude::*;
use yew::{html, Context};
use yew_router::prelude::*;

use crate::Route;

pub enum Message {
    ReceiveTelescopes(Vec<TelescopeInfo>),
}

#[derive(Default, Debug)]
pub struct ObservePage {
    telescopes: Vec<TelescopeInfo>,
}

impl Component for ObservePage {
    type Message = Message;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let result_callback = ctx.link().callback(Message::ReceiveTelescopes);
        spawn_local(async move {
            let result = match Request::get("/api/telescopes").send().await {
                Ok(bookings) => bookings
                    .json::<Vec<TelescopeInfo>>()
                    .await
                    .expect("Could not deserialize bookings"),
                Err(error_response) => {
                    log::error!("Failed to get bookings: {}", error_response);
                    Vec::new()
                }
            };

            result_callback.emit(result);
        });
        Self::default()
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::ReceiveTelescopes(telescopes) => {
                self.telescopes = telescopes;
                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let telescopes = self
            .telescopes
            .iter()
            .map(|telescope| {
                html! {
                    <div class="list-entry">
                        <Link<Route> to={Route::Telescope{id: telescope.id.clone()}}>{ telescope.id.clone() }</Link<Route>>
                    </div>
                }
            })
            .collect::<Html>();
        html! {
            <div class="select-telescope">
                { telescopes}
            </div>
        }
    }
}
