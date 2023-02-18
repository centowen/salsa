use crate::coords::{
    format_target, parse_declination, parse_latitude, parse_longitude, parse_right_ascension,
};
use common::TelescopeTarget;
use web_sys::HtmlInputElement;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
struct CoordinatePairProps {
    x: Option<AttrValue>,
    y: Option<AttrValue>,
    x_label: AttrValue,
    y_label: AttrValue,
    on_x_change: Callback<Option<String>>,
    on_y_change: Callback<Option<String>>,
    enabled: bool,
}

#[function_component]
fn CoordinatePair(props: &CoordinatePairProps) -> Html {
    let x_input_ref = use_node_ref();
    let y_input_ref = use_node_ref();

    let on_x_change = {
        let x_input = x_input_ref.clone();
        let cb = props.on_x_change.clone();
        Callback::from(move |_| {
            let x_input = x_input
                .cast::<HtmlInputElement>()
                .expect("Reference for x coordinate not attached to input node");
            let x: String = x_input.value();
            if x.is_empty() {
                cb.emit(None);
            } else {
                cb.emit(Some(x));
            }
        })
    };

    let on_y_change = {
        let y_input = y_input_ref.clone();
        let cb = props.on_y_change.clone();
        Callback::from(move |_| {
            let y_input = y_input
                .cast::<HtmlInputElement>()
                .expect("Reference for y coordinate not attached to input node");
            let y: String = y_input.value();
            if y.is_empty() {
                cb.emit(None);
            } else {
                cb.emit(Some(y));
            }
        })
    };

    let x = props.x.clone().unwrap_or(AttrValue::from("".to_string()));
    let y = props.y.clone().unwrap_or(AttrValue::from("".to_string()));

    html! {
        <>
            <label for="x">{props.x_label.clone()}</label>
            <input type="text" id="x" name="x" value={x}
                ref={x_input_ref} onchange={on_x_change.clone()} disabled={!props.enabled}
            />
            <label for="y">{props.y_label.clone()}</label>
            <input type="text" id="y" name="y" value={y}
                ref={y_input_ref} onchange={on_y_change.clone()} disabled={!props.enabled}
            />
        </>
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum CoordinateSystem {
    Galactic,
    Equatorial,
}

#[derive(PartialEq, Properties)]
struct CoordinateSystemSelectorProps {
    coordinate_system: CoordinateSystem,
    on_change_coordinate_system: Callback<CoordinateSystem>,
    enabled: bool,
}

#[function_component]
fn CoordinateSystemSelector(props: &CoordinateSystemSelectorProps) -> Html {
    let coordinate_system_select_ref = use_node_ref();
    let on_select_change = {
        let on_change_coordinate_system = props.on_change_coordinate_system.clone();

        let coordinate_system_select = coordinate_system_select_ref.clone();

        Callback::from(move |_| {
            let coordinate_system_select = coordinate_system_select
                .cast::<HtmlSelectElement>()
                .expect("Reference for coordinate system not attached to select node");

            match coordinate_system_select.selected_index() {
                0 => on_change_coordinate_system.emit(CoordinateSystem::Galactic),
                1 => on_change_coordinate_system.emit(CoordinateSystem::Equatorial),
                _ => {}
            };
        })
    };
    html! {
        <select name="coordinate-system"
                ref={coordinate_system_select_ref}
                onchange={on_select_change}
                disabled={!props.enabled}
        >
            <option value="galactic"
                selected={props.coordinate_system==CoordinateSystem::Galactic}>
                {"Galactic"}
            </option>
            <option value="equatorial"
                selected={props.coordinate_system==CoordinateSystem::Equatorial}>
                {"Equatorial"}
            </option>
        </select>
    }
}

#[derive(PartialEq, Properties)]
struct TrackButtonProps {
    enabled: bool,
    track: bool,
    on_track_status_change: Callback<bool>,
}

#[function_component]
fn TrackButton(props: &TrackButtonProps) -> Html {
    let track_toggle_input_ref = use_node_ref();
    let onchange = {
        let on_track_status_change = props.on_track_status_change.clone();

        let track_toggle_input = track_toggle_input_ref.clone();

        Callback::from(move |_| {
            let track_toggle_input = track_toggle_input
                .cast::<HtmlInputElement>()
                .expect("Reference for x coordinate not attached to input node");
            let track: bool = track_toggle_input.checked();
            on_track_status_change.emit(track);
        })
    };

    html! {
        <>
            <input type="checkbox" ref={track_toggle_input_ref} {onchange}
                   checked={props.track} disabled={!props.enabled}/>{"Track"}
        </>
    }
}

#[derive(PartialEq, Properties)]
pub struct TargetSelectorProps {
    pub target: TelescopeTarget,
    pub track: bool,
    pub on_target_change: Callback<(TelescopeTarget, bool)>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ChangeCoordinateSystem {
        coordinate_system: CoordinateSystem,
    },
    ChangeConfiguration {
        track: bool,
        x: Option<AttrValue>,
        y: Option<AttrValue>,
        coordinate_system: CoordinateSystem,
        target: TelescopeTarget,
    },
    Park,
}

pub struct TargetSelector {
    coordinate_system: CoordinateSystem,
    x: Option<AttrValue>,
    y: Option<AttrValue>,
    track: bool,
    target: TelescopeTarget,
}

fn get_configured_target(selector: &TargetSelector) -> Option<TelescopeTarget> {
    if let (Some(x), Some(y)) = (&selector.x, &selector.y) {
        match selector.coordinate_system {
            CoordinateSystem::Galactic => {
                if let (Some(l), Some(b)) = (parse_longitude(x), parse_latitude(y)) {
                    Some(TelescopeTarget::Galactic { l, b })
                } else {
                    None
                }
            }
            CoordinateSystem::Equatorial => {
                if let (Some(ra), Some(dec)) = (parse_right_ascension(x), parse_declination(y)) {
                    Some(TelescopeTarget::Equatorial { ra, dec })
                } else {
                    None
                }
            }
        }
    } else {
        None
    }
}

impl Component for TargetSelector {
    type Message = Message;
    type Properties = TargetSelectorProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            coordinate_system: CoordinateSystem::Galactic,
            x: None,
            y: None,
            target: TelescopeTarget::Parked,
            track: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::ChangeCoordinateSystem { coordinate_system } => {
                self.coordinate_system = coordinate_system;
                self.x = None;
                self.y = None;

                self.target = TelescopeTarget::Stopped;
            }
            Message::ChangeConfiguration {
                track,
                x,
                y,
                coordinate_system,
                target,
            } => {
                self.track = track;
                self.x = x;
                self.y = y;
                self.coordinate_system = coordinate_system;
                self.target = target;

                if self.track {
                    if let Some(target) = get_configured_target(&self) {
                        self.target = target;
                    }
                }
            }
            Message::Park => {
                self.track = false;
                self.x = None;
                self.y = None;
                self.target = TelescopeTarget::Parked;
            }
        }

        ctx.props().on_target_change.emit((self.target, self.track));

        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if ctx.props().target != self.target || ctx.props().track != self.track {
            let (x, y, coordinate_system) = format_target(ctx.props().target);

            ctx.link().send_message(Message::ChangeConfiguration {
                track: ctx.props().track,
                x,
                y,
                coordinate_system: coordinate_system.unwrap_or(self.coordinate_system),
                target: ctx.props().target,
            });
        }

        let (x_label, y_label) = match self.coordinate_system {
            CoordinateSystem::Galactic => ("Longitude [deg]", "Latitude [deg]"),
            CoordinateSystem::Equatorial => ("Right ascension", "Declination"),
        };

        let on_x_change = {
            let track = self.track;
            let y = self.y.clone();
            let coordinate_system = self.coordinate_system;
            let target = self.target;
            ctx.link()
                .callback(move |x: Option<String>| Message::ChangeConfiguration {
                    track,
                    x: x.map(|x| AttrValue::from(x)),
                    y: y.clone(),
                    coordinate_system,
                    target,
                })
        };
        let on_y_change = {
            let track = self.track;
            let x = self.x.clone();
            let coordinate_system = self.coordinate_system;
            let target = self.target;
            ctx.link()
                .callback(move |y: Option<String>| Message::ChangeConfiguration {
                    track,
                    x: x.clone(),
                    y: y.map(|y| AttrValue::from(y)),
                    coordinate_system,
                    target,
                })
        };

        let coordinate_change = {
            let change_coordinate_system =
                ctx.link()
                    .callback(|coordinate_system| Message::ChangeCoordinateSystem {
                        coordinate_system,
                    });
            Callback::from(move |coordinate_system| {
                change_coordinate_system.emit(coordinate_system);
            })
        };

        let change_tracking_status = {
            let x = self.x.clone();
            let y = self.y.clone();
            let coordinate_system = self.coordinate_system;
            let target = self.target;
            ctx.link().callback(move |track| {
                let target = match track {
                    true => target,
                    false => TelescopeTarget::Stopped,
                };

                Message::ChangeConfiguration {
                    track,
                    x: x.clone(),
                    y: y.clone(),
                    coordinate_system,
                    target,
                }
            })
        };

        let park_telescope = {
            let link = ctx.link().clone();
            Callback::from(move |_| {
                link.send_message(Message::Park {});
            })
        };

        let configured_target_valid = get_configured_target(&self).is_some();

        html! {
            <>
                <CoordinateSystemSelector
                    coordinate_system={self.coordinate_system}
                    on_change_coordinate_system={coordinate_change}
                    enabled={!self.track}
                />
                <CoordinatePair x={self.x.clone()} y={self.y.clone()}
                    {x_label} {y_label}
                    {on_x_change}
                    {on_y_change}
                    enabled={!self.track}
                />
                <TrackButton track={self.track} on_track_status_change={change_tracking_status}
                             enabled={configured_target_valid}/>
                <button onclick={park_telescope}>{"Park"}</button>
            </>
        }
    }
}
