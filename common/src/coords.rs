use crate::{Direction, Location};
use chrono::prelude::*;
use std::f64::consts::PI;

// Obliquity of the ecliptic, accurate to 1 arcmin per century from J2000
const EC: f64 = 0.40909260052;
// The generally accepted motion of the Sun in the LSR is that it is moving at a speed of about
// 20 km/s towards an “apex” that is rather close to the direction of Vega, with coordinates
// R.A. = 18 hr = 270 deg, and Dec. = 30 deg. Define this apex as ARA, ADE:
const ARA: f64 = 4.71238898038;
const ADE: f64 = 0.52359877559;

fn julian_day(when: DateTime<Utc>) -> f64 {
    // Calculate decimal julian day for specified date. We can simplify
    // since we do not need to cover dates in the past, only the future!
    // From https://aa.usno.navy.mil/data/JulianDate we get that for
    // A.D. 2000 January 1 	12:00:00.0 correspond to julian day 2451545.0.
    // Calculate difference to this date
    let jdref = Utc.with_ymd_and_hms(2000, 1, 1, 12, 0, 0).unwrap();
    let diff = when.signed_duration_since(jdref);
    // Need f64 for precision
    2451545.0 + (diff.num_milliseconds() as f64 / (24.0 * 60.0 * 60.0 * 1000.0))
}

fn gmst(when: DateTime<Utc>) -> f64 {
    // Algoritm from https://aa.usno.navy.mil/faq/GAST
    let jd = julian_day(when);
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

fn ecliptic_from_equatorial(ra: f64, dec: f64) -> (f64, f64) {
    // From javascript code behind calculations at https://frostydrew.org/utilities.dc/convert/tool-eq_coordinates/
    let l = (ra.tan() * EC.cos() + dec.tan() * EC.sin() / ra.cos()).atan();
    let b = (dec.sin() * EC.cos() - dec.cos() * EC.sin() * ra.sin()).asin();
    (l, b)
}

//fn ecliptic_from_galactic(gl: f64, gb: f64) -> (f64, f64) {
//    let (ra, dec) = equatorial_from_galactic(gl, gb);
//    let (l, b) = ecliptic_from_equatorial(ra, dec);
//    (l, b)
//}

fn ecliptic_from_sun(when: DateTime<Utc>) -> (f64, f64) {
    // Algorithm from https://aa.usno.navy.mil/faq/sun_approx
    // for computing the Sun's angular coordinates to an accuracy of about 1 arcminute within two centuries of 2000
    let d = julian_day(when) - 2451545.0;
    //Mean anomaly of the Sun:
    let g = (357.529 + 0.98560028 * d) % 360.0;
    //Mean longitude of the Sun:
    let q = (280.459 + 0.98564736 * d) % 360.0;
    // Geocentric apparent ecliptic longitude of the Sun (adjusted for aberration):
    let l = (q + 1.915 * g.to_radians().sin() + 0.020 * (2.0 * g).to_radians().sin()) % 360.0;
    // where all the constants (therefore g, q, and L) are in degrees.
    // It may be necessary or desirable to reduce g, q, and L to the range 0° to 360°.
    //The Sun's ecliptic latitude, b, can be approximated by b=0.
    let b = 0.0;
    // return in radians
    (l.to_radians(), b)
}

fn equatorial_from_sun(when: DateTime<Utc>) -> (f64, f64) {
    // Algorithm from https://aa.usno.navy.mil/faq/sun_approx
    // for computing the Sun's angular coordinates to an accuracy of about 1 arcminute within two centuries of 2000
    let (l, _b) = ecliptic_from_sun(when);
    let d = julian_day(when) - 2451545.0;
    //First compute the mean obliquity of the ecliptic:
    let e = (23.439 - 0.00000036 * d).to_radians();
    let ra = (e.cos() * l.sin()).atan2(l.cos());
    let dec = (e.sin() * l.sin()).asin();
    (ra, dec)
}

pub fn horizontal_from_sun(location: Location, when: DateTime<Utc>) -> Direction {
    let (ra, dec) = equatorial_from_sun(when);
    horizontal_from_equatorial(location, when, ra, dec)
}

pub fn vlsrcorr_from_galactic(l: f64, b: f64, when: DateTime<Utc>) -> f64 {
    // From http://web.mit.edu/8.13/www/srt_software/vlsr.pdf

    // Target direction in equatorial coordinates
    let (ra, dec) = equatorial_from_galactic(l, b);

    // Movement of Sun with respect to LSR: dot product of target & apex vectors, in km/s
    let vsun = 20.0
        * (ARA.cos() * ADE.cos() * ra.cos() * dec.cos()
            + ARA.sin() * ADE.cos() * ra.sin() * dec.cos()
            + ADE.sin() * dec.sin());

    // Target direction in ecliptic coordinates
    let (tl, tb) = ecliptic_from_equatorial(ra, dec);

    // Sun direction in ecliptic coordinates
    let (sl, _sb) = ecliptic_from_sun(when);

    // Calculate correction due to earth movement relative to the Sun, in km/s
    let vorb = 30.0 * tb.cos() * (sl - tl).sin();

    // return total correction in m/s
    1e3 * (vsun + vorb)
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

    #[test]
    fn test_horizontal_from_sun() {
        // Test that we get the correct horizontal position for the Sun
        // given specific location and time
        let jdref = Utc.with_ymd_and_hms(2023, 4, 4, 12, 0, 0).unwrap();
        // Use SALSA Onsala location
        let locref = Location {
            longitude: 0.20802143022,
            latitude: 1.00170457462,
        };
        let dir = horizontal_from_sun(locref, jdref);
        // Expected horizontal coordinates in radians
        let expected_az = 3.386904823113701;
        let expected_alt = 0.6557470215389855;
        assert_similar!(dir.azimuth, expected_az, 1e-6);
        assert_similar!(dir.altitude, expected_alt, 1e-6);
    }

    #[test]
    fn test_vlsrcorr_from_galactic() {
        // Test that we get the correct horizontal position for the Sun
        // given specific location and time
        let jdref = Utc.with_ymd_and_hms(2023, 4, 4, 15, 0, 0).unwrap();
        let glon = 140.0_f64.to_radians();
        let vlsrcorr = vlsrcorr_from_galactic(glon, 0.0, jdref);
        // Expected horizontal coordinates in radians
        let expected_vlsrcorr = -15443.385967834394;
        assert_similar!(vlsrcorr, expected_vlsrcorr, 1e-6);
    }
}
