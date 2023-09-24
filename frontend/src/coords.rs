use crate::components::target_selector::CoordinateSystem;
use common::TelescopeTarget;
use std::f64::consts::PI;
use yew::AttrValue;

pub fn parse_longitude(l: &str) -> Option<f64> {
    if let Ok(l) = l.parse::<f64>() {
        let l_radian = l * PI / 180.0;
        if l_radian >= -PI && l_radian <= PI {
            return Some(l_radian);
        }
    }

    None
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

pub fn format_longitude(l: f64) -> AttrValue {
    AttrValue::from((l * 180.0 / PI).to_string())
}

fn format_latitude(l: f64) -> AttrValue {
    AttrValue::from((l * 180.0 / PI).to_string())
}

fn format_right_ascension(ra: f64) -> AttrValue {
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

fn format_declination(dec: f64) -> AttrValue {
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

pub fn format_target(
    target: TelescopeTarget,
) -> (
    Option<AttrValue>,
    Option<AttrValue>,
    Option<CoordinateSystem>,
) {
    match target {
        TelescopeTarget::Galactic { l, b } => (
            Some(format_latitude(l)),
            Some(format_longitude(b)),
            Some(CoordinateSystem::Galactic),
        ),
        TelescopeTarget::Equatorial { ra, dec } => (
            Some(format_right_ascension(ra)),
            Some(format_declination(dec)),
            Some(CoordinateSystem::Equatorial),
        ),
        TelescopeTarget::Parked => (None, None, None),
        TelescopeTarget::Stopped => (None, None, None),
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
