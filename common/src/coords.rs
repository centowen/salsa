use crate::{Direction, Location};
use chrono::{Datelike, Timelike};
use std::f64::consts::PI;

fn get_current_julian_day() -> f64 {
    let now = chrono::offset::Utc::now();
    let time = now.time();
    let decimal_day = now.day() as f64
        + (time.hour() as f64 + time.minute() as f64 / 60.0 + time.second() as f64 / 3600.0) / 24.0;
    let date = astro::time::Date {
        year: now.year() as i16,
        month: now.month() as u8,
        decimal_day,
        cal_type: astro::time::CalType::Gregorian,
    };
    astro::time::julian_day(&date)
}

pub fn get_horizontal_gal(location: Location, l: f64, b: f64) -> Direction {
    get_horizontal_eq(
        location,
        astro::coords::asc_frm_gal(l, b),
        astro::coords::dec_frm_gal(l, b),
    )
}

pub fn get_horizontal_eq(location: Location, ra: f64, dec: f64) -> Direction {
    let julian_day = get_current_julian_day();
    let mean_sidereal_time = astro::time::mn_sidr(julian_day);
    let hour_angle =
        astro::coords::hr_angl_frm_observer_long(mean_sidereal_time, location.longitude, ra);
    let azimuth = astro::coords::az_frm_eq(hour_angle, dec, location.latitude) + PI;
    let altitude = astro::coords::alt_frm_eq(hour_angle, dec, location.latitude);
    Direction { azimuth, altitude }
}
