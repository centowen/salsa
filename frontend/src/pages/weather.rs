use crate::services::emit_weather_info;
use yew::prelude::*;
use yew::virtual_dom::AttrValue;
use yew::{html, Context};

pub enum Msg {
    Temperature(AttrValue),
}

pub struct WeatherPage {
    temperature: Option<AttrValue>,
}

impl Component for WeatherPage {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let weather_info_cb = ctx.link().callback(Msg::Temperature);
        emit_weather_info(weather_info_cb);

        Self { temperature: None }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Temperature(temperature) => self.temperature = Some(temperature.clone()),
        }
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let temperature = self.temperature.as_deref().unwrap_or("Loading...");
        html! {
            <div class="section light temperature">
                {format!("{}°C", temperature)}
            </div>
        }
    }
}
