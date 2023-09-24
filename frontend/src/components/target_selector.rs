use common::TelescopeTarget;
use log::debug;
use std::f64::consts::PI;
use web_sys::HtmlInputElement;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

pub fn parse_longitude(l: &str) -> Option<f64> {
    if let Ok(l) = l.parse::<f64>() {
        let l_radian = l * PI / 180.0;
        if l_radian >= -PI && l_radian <= PI {
            return Some(l_radian);
        }
    }

    None
}

pub fn format_longitude(l: f64) -> AttrValue {
    AttrValue::from((l * 180.0 / PI).to_string())
}

pub fn parse_latitude(b: &str) -> Option<f64> {
    if let Ok(b) = b.parse::<f64>() {
        let b_radian = b * PI / 180.0;
        if b_radian >= -PI / 2.0 && b_radian <= PI / 2.0 {
            return Some(b_radian);
        }
    }

    None
}

pub fn format_latitude(l: f64) -> AttrValue {
    AttrValue::from((l * 180.0 / PI).to_string())
}

pub fn parse_right_ascension(ra: &str) -> Option<f64> {
    let e = regex::Regex::new(r"(\d{1,2})[h ]+(\d{2})[m'′ ]+(\d{2}\.?\d{0,6})[″s]?").unwrap();
    if let Some(captures) = e.captures(ra) {
        if let (Ok(deg), Ok(min), Ok(sec)) = (
            captures[1].parse::<f64>(),
            captures[2].parse::<f64>(),
            captures[3].parse::<f64>(),
        ) {
            let sign = deg.signum();
            let deg = sign * deg;
            return Some(sign * (deg + min / 60. + sec / 3600.) / 12.0 * PI);
        }
        Some(0.0)
    } else {
        None
    }
}

pub fn format_right_ascension(ra: f64) -> AttrValue {
    let hours = ra * 12.0 / PI;
    let minutes = (hours - hours.floor()) * 60.0;
    let seconds = (minutes - minutes.floor()) * 60.0;
    AttrValue::from(format!(
        "{:.0}h{:.0}m{:.0}",
        hours.floor(),
        minutes.floor(),
        seconds.floor()
    ))
}

pub fn parse_declination(dec: &str) -> Option<f64> {
    let e = regex::Regex::new(r"([\+-]?\d{1,4})[d° ]+(\d{2})[m'′ ]+(\d{2}″?\.?\d{0,5})″?").unwrap();
    if let Some(captures) = e.captures(dec) {
        if let (Ok(deg), Ok(min), Ok(sec)) = (
            captures[1].parse::<f64>(),
            captures[2].parse::<f64>(),
            captures[3].replace("″", "").parse::<f64>(),
        ) {
            let sign = deg.signum();
            let deg = sign * deg;
            return Some(sign * (deg + min / 60. + sec / 3600.) / 180.0 * PI);
        }
    }

    None
}

pub fn format_declination(dec: f64) -> AttrValue {
    let degrees = dec * 180.0 / PI;
    let minutes = (degrees - degrees.floor()) * 60.0;
    let seconds = (minutes - minutes.floor()) * 60.0;
    AttrValue::from(format!(
        "{}{:.0}d{:.0}m{:.0}",
        if degrees.is_sign_positive() { "+" } else { "" },
        degrees.floor(),
        minutes.floor(),
        seconds.floor()
    ))
}

fn format_target(target: TelescopeTarget) -> (Option<AttrValue>, Option<AttrValue>) {
    match target {
        TelescopeTarget::Galactic { l, b } => (Some(format_latitude(l)), Some(format_longitude(b))),
        TelescopeTarget::Equatorial { ra, dec } => (
            Some(format_right_ascension(ra)),
            Some(format_declination(dec)),
        ),
        TelescopeTarget::Parked => (None, None),
        TelescopeTarget::Stopped => (None, None),
    }
}

#[derive(PartialEq, Properties)]
struct CoordinatePairProps {
    x: Option<AttrValue>,
    y: Option<AttrValue>,
    x_label: AttrValue,
    y_label: AttrValue,
    on_x_change: Callback<Option<String>>,
    on_y_change: Callback<Option<String>>,
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
                ref={x_input_ref} onchange={on_x_change.clone()} />
            <label for="y">{props.y_label.clone()}</label>
            <input type="text" id="y" name="y" value={y}
                ref={y_input_ref} onchange={on_y_change.clone()} />
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
            debug!("Emit change tracking status to {}", track);
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
    pub on_target_change: Callback<TelescopeTarget>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ChangeCoordinateSystem(CoordinateSystem),
    ChangeXCoordinate(Option<AttrValue>),
    ChangeYCoordinate(Option<AttrValue>),
    ChangeTracking(bool),
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
            track: false,
            target: TelescopeTarget::Parked,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        if match msg {
            Message::ChangeCoordinateSystem(coordinate_system) => {
                self.coordinate_system = coordinate_system;
                self.x = None;
                self.y = None;

                self.target = TelescopeTarget::Stopped;
                true
            }
            Message::ChangeXCoordinate(x) => {
                self.x = x;

                if self.track {
                    if let Some(target) = get_configured_target(&self) {
                        self.target = target;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            Message::ChangeYCoordinate(y) => {
                self.y = y;

                if self.track {
                    if let Some(target) = get_configured_target(&self) {
                        self.target = target;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            Message::ChangeTracking(track) => {
                if track {
                    if let Some(target) = get_configured_target(&self) {
                        self.target = target;
                        self.track = track;
                        true
                    } else {
                        false
                    }
                } else {
                    self.track = track;
                    self.target = TelescopeTarget::Stopped;
                    true
                }
            }
            Message::Park => {
                self.track = false;
                self.x = None;
                self.y = None;
                self.target = TelescopeTarget::Parked;
                true
            }
        } {
            ctx.props().on_target_change.emit(self.target);
        }

        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let (x, y) = if self.target == ctx.props().target {
            (self.x.clone(), self.y.clone())
        } else {
            format_target(ctx.props().target)
        };

        let (x_label, y_label) = match self.coordinate_system {
            CoordinateSystem::Galactic => ("Longitude [deg]", "Latitude [deg]"),
            CoordinateSystem::Equatorial => ("Right ascension", "Declination"),
        };

        let create_change_callback = |cb: Callback<Option<AttrValue>>| {
            Callback::from(move |x: Option<String>| {
                cb.emit(x.map(|x| AttrValue::from(x)));
            })
        };

        let coordinate_change = {
            let change_coordinate_system = ctx.link().callback(Message::ChangeCoordinateSystem);
            Callback::from(move |coordinate_system| {
                change_coordinate_system.emit(coordinate_system);
            })
        };

        let change_tracking_status = {
            let change_tracking = ctx.link().callback(Message::ChangeTracking);
            Callback::from(move |track| {
                change_tracking.emit(track);
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
                <CoordinatePair x={x} y={y}
                    {x_label} {y_label}
                    on_x_change={create_change_callback(ctx.link().callback(Message::ChangeXCoordinate))}
                    on_y_change={create_change_callback(ctx.link().callback(Message::ChangeYCoordinate))}
                />
                <TrackButton track={self.track} on_track_status_change={change_tracking_status}
                             enabled={configured_target_valid}/>
                <button onclick={park_telescope}>{"Park"}</button>
            </>
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;

    const DEG: f64 = PI / 180.0f64;
    const ARCMINUTE: f64 = DEG / 60.0;
    const ARCSECOND: f64 = ARCMINUTE / 60.0;
    const HOUR: f64 = PI / 12f64;
    const MINUTE: f64 = HOUR / 60.0;
    const SECOND: f64 = MINUTE / 60.0;

    #[test]
    fn test_parse_declination() {
        assert_eq!(None, parse_declination("Not a coordinate"));
        assert_eq!(Some(0.0), parse_declination("+0d00m00.000"));
        assert_eq!(Some(0.0), parse_declination("-0d00m00.000"));

        assert_relative_eq!(-DEG, parse_declination("-1d00m00.000").unwrap());
        assert_relative_eq!(-63.0 * DEG, parse_declination("-63d00m00.000").unwrap());
        assert_relative_eq!(
            -(63.0 * DEG + 30.0 * ARCMINUTE),
            parse_declination("-63d30m00.000").unwrap()
        );
        assert_relative_eq!(
            -(64.0 * DEG + 30.0 * ARCMINUTE + 23.0 * ARCSECOND),
            parse_declination("-64d30m23.000").unwrap()
        );
        assert_relative_eq!(
            64.0 * DEG + 30.0 * ARCMINUTE + 23.0 * ARCSECOND,
            parse_declination("64d30m23.000").unwrap()
        );
        assert_relative_eq!(
            64.0 * DEG + 30.0 * ARCMINUTE + 23.0 * ARCSECOND,
            parse_declination("+64d30m23.000").unwrap()
        );
        assert_relative_eq!(
            23.0 * DEG + 30.0 * ARCMINUTE + 11.0 * ARCSECOND,
            parse_declination("+23° 30′ 11″").unwrap()
        );

        assert_relative_eq!(
            -(23.0 * DEG + 30.0 * ARCMINUTE + 11.2 * ARCSECOND),
            parse_declination("-23 30 11.2").unwrap()
        );
    }

    #[test]
    fn test_parse_right_ascension() {
        assert_eq!(None, parse_right_ascension("Not a coordinate"));
        assert_eq!(Some(0.0), parse_right_ascension("0h00m00.000"));

        assert_relative_eq!(HOUR, parse_right_ascension("1h00m00.000").unwrap());
        assert_relative_eq!(15.0 * HOUR, parse_right_ascension("15 00 00.000").unwrap());
        assert_relative_eq!(15.5 * HOUR, parse_right_ascension("15h30m00.000s").unwrap());
        assert_relative_eq!(
            15.0 * HOUR + 30.0 * MINUTE + 23.0 * SECOND,
            parse_right_ascension("15h30m23.000s").unwrap()
        );
        assert_relative_eq!(
            15.0 * HOUR + 34.0 * MINUTE + 57.1 * SECOND,
            parse_right_ascension("15h 34m 57.1s").unwrap()
        );
    }

    #[test]
    fn test_format_right_ascension() {
        assert_eq!(
            "15h30m23",
            format_right_ascension(15.0 * HOUR + 30.0 * MINUTE + 23.0 * SECOND).as_str()
        );
    }

    #[test]
    fn test_format_declination() {
        assert_eq!(
            "+15d30m23",
            format_declination(15.0 * DEG + 30.0 * ARCMINUTE + 23.0 * ARCSECOND).as_str()
        );
    }
}
