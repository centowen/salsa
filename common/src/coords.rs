use crate::{Direction, Location};
use chrono::prelude::*;
use std::f64::consts::PI;

fn julian_day(now: DateTime<Utc>) -> f64 {
    // Calculate decimal julian day for current date. We can simplify
    // since we do not need to cover dates in the past, only the future!
    // From https://aa.usno.navy.mil/data/JulianDate we get that for
    // A.D. 2000 January 1 	12:00:00.0 correspond to julian day 2451545.0.
    // Calculate difference to this date
    let jdref = Utc.with_ymd_and_hms(2000, 1, 1, 12, 0, 0).unwrap();
    let diff = now.signed_duration_since(jdref);
    // Need f64 for precision
    2451545.0 + (diff.num_milliseconds() as f64 / (24.0 * 60.0 * 60.0 * 1000.0))
}

fn gmst(now: DateTime<Utc>) -> f64 {
    // Algoritm from https://aa.usno.navy.mil/faq/GAST
    let jd = julian_day(now);
    let jd0 = jd.floor() + 0.5;
    let h = (jd - jd0) * 24.0;
    let dtt = jd - 2451545.0;
    let dut = jd0 - 2451545.0;
    let t = dtt / 36525.0;
    (6.697375 + 0.065709824279 * dut + 1.0027379 * h + 0.0000258 * t * t) % 24.0
}

/// Convert equatorial coordinates to horizontal coordinates
/// # Arguments
/// * `location` - Location struct with latitude and longitude
/// * `ra` - Right ascension in radians
/// * `dec` - Declination in radians
/// # Returns
/// * `Direction` struct with azimuth and altitude
pub fn horizontal_from_equatorial(
    location: Location,
    when: DateTime<Utc>,
    ra: f64,
    dec: f64,
) -> Direction {
    // Assume input in radians

    // Get antenna position
    let lon = location.longitude;
    let lat = location.latitude;

    // Equatorial to Horizontal conversion from https://aa.usno.navy.mil/faq/alt_az
    let gast = gmst(when);
    let ra = ra * 12.0 / PI; // hours from radians
    let lha = (gast - ra) * 15.0_f64.to_radians() + lon;
    let alt = (lha.cos() * dec.cos() * lat.cos() + dec.sin() * lat.sin()).asin();
    let az = (-lha.sin()).atan2(dec.tan() * lat.cos() - lat.sin() * lha.cos());

    // Ensure positive az
    let full_circle = 2.0 * PI;
    let az = ((az % full_circle) + full_circle) % full_circle;

    Direction {
        azimuth: az,
        altitude: alt,
    }
}

fn equatorial_from_galactic(l: f64, b: f64) -> (f64, f64) {
    // Assume input in radians

    // Calculation from https://physics.stackexchange.com/questions/88663/converting-between-galactic-and-ecliptic-coordinates
    let ra_ngp: f64 = 192.85948_f64.to_radians(); // R.A. North Galactic Pole
    let dec_ngp: f64 = 27.12825_f64.to_radians(); // Declination North Galactic Pole
    let l_ncp: f64 = 122.93192_f64.to_radians(); // Galactic Longitude North Celestial Pole
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

pub fn horizontal_from_galactic(
    location: Location,
    when: DateTime<Utc>,
    l: f64,
    b: f64,
) -> Direction {
    // First convert galactic to equatorial, then to horizontal
    let (ra, dec) = equatorial_from_galactic(l, b);
    horizontal_from_equatorial(location, when, ra, dec)
}

#[cfg(test)]
mod test {
    use chrono::Duration;

    use super::*;

    macro_rules! assert_similar {
        ($left:expr, $right:expr, $precision: expr) => {
            assert!(
                ($left - $right).abs() < $precision,
                "expected {} = {}",
                $left,
                $right,
            );
        };
    }

    #[test]
    fn test_julian_day() {
        // Test that we get the correct julian day for a given date
        let jdref = Utc.with_ymd_and_hms(2000, 1, 1, 12, 0, 0).unwrap();
        let expected_jd = 2451545.0;
        assert_similar!(julian_day(jdref), expected_jd, 1e-6);
        assert_similar!(
            julian_day(jdref + Duration::days(1)),
            expected_jd + 1.0,
            1e-6
        );
        assert_similar!(
            julian_day(jdref + Duration::days(365)),
            expected_jd + 365.0,
            1e-6
        );
    }
}
