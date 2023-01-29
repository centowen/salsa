use log::{debug};
use web_sys::HtmlInputElement;
use web_sys::HtmlSelectElement;
use yew::prelude::*;
use std::f32::consts::PI;
use common::TelescopeTarget;

pub enum Msg {}

fn parse_longitude(l: String) -> Option<f32>
{
    if let Ok(l) = l.parse::<f32>()
    {
        let l_radian = l * PI / 180.0;
        if l_radian >= -PI && l_radian <= PI
        {
            return Some(l_radian);
        }
    }

    None
}

fn parse_latitude(b: String) -> Option<f32>
{
    if let Ok(b) = b.parse::<f32>()
    {
        let b_radian = b * PI / 180.0;
        if b_radian >= -PI / 2.0 && b_radian <= PI / 2.0
        {
            return Some(b_radian);
        }
    }

    None
}

#[function_component]
fn TelescopeMovementControl() -> Html
{
    let coordinate_system_select_ref = use_node_ref();
    let x_input_ref = use_node_ref();
    let y_input_ref = use_node_ref();
    let track_toggle_input_ref = use_node_ref();

    let onchange =
        {
            let coordinate_system_select = coordinate_system_select_ref.clone();
            let x_input = x_input_ref.clone();
            let y_input = y_input_ref.clone();
            let track_toggle_input = track_toggle_input_ref.clone();

            Callback::from(move |_| {
                let track_toggle_input = track_toggle_input
                    .cast::<HtmlInputElement>()
                    .expect("Reference for track toggle not attached to input node");

                let coordinate_system_select = coordinate_system_select
                    .cast::<HtmlSelectElement>()
                    .expect("Reference for coordinate system not attached to select node");

                let x_input = x_input
                    .cast::<HtmlInputElement>()
                    .expect("Reference for x coordinate not attached to input node");
                let y_input = y_input
                    .cast::<HtmlInputElement>()
                    .expect("Reference for y coordinate not attached to input node");

                let telescope_target = match coordinate_system_select.selected_index() {
                    0 => {
                        if let (Some(l), Some(b)) = (parse_longitude(x_input.value()), parse_latitude(y_input.value()))
                        {
                            TelescopeTarget::Galactic {
                                l,
                                b,
                            }
                        } else {
                            return;
                        }
                    }
                    1 => {
                        if let (Some(ra), Some(dec)) = (parse_longitude(x_input.value()), parse_latitude(y_input.value()))
                        {
                            TelescopeTarget::Equatorial {
                                ra,
                                dec,
                            }
                        } else {
                            return;
                        }
                    }
                    _ => panic!("Unknown coordinate system"),
                };
                if track_toggle_input.checked()
                {
                    debug!("Tracking {:?}", telescope_target);
                }
            })
        };

    html! {
        <div class="telescope-movement">
            // <Select<CoordinateSystem> options=coordinate_systems>
            <select name="coordinate-system"
                    ref={coordinate_system_select_ref}
                    onchange={onchange.clone()}
            >
                <option value="equatorial">{"Equatorial"}</option>
                <option value="galactic">{"Galactic"}</option>
            </select>
            <label for="x">{
                // let coordinate_system_select = coordinate_system_select_ref.clone()
                //     .cast::<HtmlSelectElement>()
                //     .expect("Reference for coordinate system not attached to select node");
                // // debug!("coordinate system selected: {:?}",
                // //        coordinate_system_select.selected_index());
                // match coordinate_system_select.selected_index()
                // {
                //     0 => "Longitude [deg]",
                //     1 => "Right ascension",
                //     _ => panic!("Unknown coordinate system"),
                // }
                "Longitude [deg]"
            }</label>
            <input type="text" ref={x_input_ref} id="x" name="x"  onchange={onchange.clone()} />
            <label for="y">{
                // let coordinate_system_select = coordinate_system_select_ref.clone()
                //     .cast::<HtmlSelectElement>()
                //     .expect("Reference for coordinate system not attached to select node");
                // match coordinate_system_select.selected_index()
                // {
                //     0 => "Latitude [deg]",
                //     1 => "Declination",
                //     _ => panic!("Unknown coordinate system"),
                // }
                "Latitude [deg]"
            }</label>
            <input type="text" ref={y_input_ref} id="y" name="y" onchange={onchange.clone()} />
            <input type="checkbox" ref={track_toggle_input_ref} {onchange}/>{"Track"}
        </div>
    }
}

#[derive(PartialEq, Properties)]
pub struct Props {
    pub id: String,
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
                    <h1>{ ctx.props().id.clone() }</h1>
                </div>
                <TelescopeMovementControl />
                <div class="telescope-receiver">
                    { "Telescope receiver" }
                </div>
            </div>
        }
    }
}
