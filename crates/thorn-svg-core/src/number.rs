//! The profile's number format: what the editor writes into a geometry
//! attribute, so that an unedited re-export is byte-stable and a diff of a
//! drawing means the drawing changed.
//!
//! At most three decimals, trailing zeros dropped, no trailing point, and
//! `-0` written as `0`. Three decimals is the app's measurement — 0.006 pt of
//! error on a sixty-point pen stroke, below anything a display can show — and
//! trimming is what keeps `10` from becoming `10.000` on a file a hand wrote.

/// Format a user-unit coordinate or length the way the profile writes it.
pub fn fmt(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    let s = format!("{rounded:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Whether `text` is already in the profile's format — that is, whether
/// re-writing its value through [`fmt`] would change a byte.
pub fn is_canonical(text: &str) -> bool {
    text.parse::<f64>().is_ok_and(|v| fmt(v) == text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_decimals_then_trimmed() {
        assert_eq!(fmt(10.0), "10");
        assert_eq!(fmt(10.5), "10.5");
        assert_eq!(fmt(10.25), "10.25");
        assert_eq!(fmt(10.125), "10.125");
        assert_eq!(fmt(10.1254), "10.125");
        assert_eq!(fmt(10.1255), "10.126");
        assert_eq!(fmt(-0.0001), "0");
        assert_eq!(fmt(-1.5), "-1.5");
    }

    #[test]
    fn canonical_is_a_fixed_point_of_fmt() {
        assert!(is_canonical("10"));
        assert!(is_canonical("10.125"));
        assert!(!is_canonical("10.0"));
        assert!(!is_canonical("10.1254"));
        assert!(!is_canonical("+10"));
        assert!(!is_canonical("ten"));
    }

    /// The claim the profile makes: a value written once and read back is the
    /// value that gets written again.
    #[test]
    fn round_trip_is_stable() {
        for v in [0.0, 1.0 / 3.0, 123.4567, -98.7654, 1e-4, 4096.001] {
            let once = fmt(v);
            let twice = fmt(once.parse().unwrap());
            assert_eq!(once, twice, "{v}");
        }
    }
}
