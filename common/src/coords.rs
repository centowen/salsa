use crate::{Direction, Location};
use chrono::prelude::*;
use std::f64::consts::PI;

fn jd_now() -> f64 {
    // Calculate decimal julian day for current date. We can simplify
    // since we do not need to cover dates in the past, only the future!
    // From https://aa.usno.navy.mil/data/JulianDate we get that for
    // A.D. 2000 January 1 	12:00:00.0 correspond to julian day 2451545.0.
    // Calculate difference to this date
    let jdref = Utc.with_ymd_and_hms(2000, 1, 1, 12, 0, 0).unwrap();
    let now: DateTime<Utc> = Utc::now();
    let diff = now.signed_duration_since(jdref);
    // Need f64 for precision
    let jd = 2451545.0 + (diff.num_milliseconds() as f64 / (24.0 * 60.0 * 60.0 * 1000.0));
    jd
}

fn gmst_now() -> f64 {
    // Algoritm from https://aa.usno.navy.mil/faq/GAST
    let jd = jd_now();
    let jd0 = jd.floor() + 0.5;
    let h = (jd - jd0) * 24.0;
    let dtt = jd - 2451545.0;
    let dut = jd0 - 2451545.0;
    let t = dtt / 36525.0;
    let gmst = (6.697375 + 0.065709824279 * dut + 1.0027379 * h + 0.0000258 * t * t) % 24.0;
    gmst
}

pub fn horizontal_from_equatorial(location: Location, ra: f64, dec: f64) -> Direction {
    // Assume input in radians
    
    // Get antenna position
    let lon = location.longitude;
    let lat = location.latitude;

    // Equatorial to Horizontal conversion from https://aa.usno.navy.mil/faq/alt_az
    let gast = gmst_now();
    let ra = ra * 12.0 / PI; // hours from radians
    let lha = (gast - ra) * 15.0 * PI / 180.0 + lon;
    let alt = (lha.cos() * dec.cos() * lat.cos() + dec.sin() * lat.sin()).asin();
    let az = (-lha.sin()).atan2(dec.tan() * lat.cos() - lat.sin() * lha.cos());
    // Convert to degrees
    let alt = alt * 180.0/PI;
    // Ensure positive az
    let az = ((az*180.0/PI % 360.0) + 360.0 ) % 360.0;
    // TODO: return radians?
    Direction{azimuth: az, altitude: alt}
}

fn equatorial_from_galactic(l: f64, b: f64) -> (f64, f64) {
    // Assume input in radians
    
    // Calculation from https://physics.stackexchange.com/questions/88663/converting-between-galactic-and-ecliptic-coordinates
    let ra_ngp: f64 = 192.85948 * PI / 180.0; // R.A. North Galactic Pole
    let dec_ngp: f64 = 27.12825 * PI / 180.0; // Declination North Galactic Pole
    let l_ncp: f64 = 122.93192 * PI / 180.0; // Galactic Longitude North Celestial Pole
    // Declination is straight forward from link above
    let dec = (dec_ngp.sin() * b.sin() + dec_ngp.cos() * b.cos() * (l_ncp - l).cos()).asin();
    // To get one equation for R.A., divide the two equations on link above we get
    // tan(ra-ra_ngp) on left side and long thing on right sida. Use atan2 to recover
    // ra-ra_ngp, and then get ra.
    let ra = (b.cos() * (l_ncp - l).sin())
        .atan2(dec_ngp.cos() * b.sin() - dec_ngp.sin() * b.cos() * (l_ncp - l).cos())
        + ra_ngp;
    (ra, dec)
}

pub fn horizontal_from_galactic(location: Location, l: f64, b: f64) -> Direction {
    // First convert galactic to equatorial, then to horizontal
    let (ra, dec) = equatorial_from_galactic(l, b);
    horizontal_from_equatorial(location, ra, dec)
}
