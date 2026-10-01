//! Allowed values for profile and schedule fields.
//!
//! These mirror the constraints enforced by the reference implementations
//! (`9b/fellow-aiden`, `simmerkaer/fellow-aiden-ts`, `-dotnet`).

use std::ops::RangeInclusive;

/// A numeric range sampled at a fixed step, e.g. temperatures 50–98.5 °C in 0.5 steps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stepped {
    /// Smallest allowed value.
    pub min: f64,
    /// Largest allowed value.
    pub max: f64,
    /// Distance between consecutive allowed values.
    pub step: f64,
}

impl Stepped {
    /// Whether `value` lies in range and on the step grid.
    pub fn contains(&self, value: f64) -> bool {
        if !value.is_finite() || value < self.min - 1e-9 || value > self.max + 1e-9 {
            return false;
        }
        let k = (value - self.min) / self.step;
        (k - k.round()).abs() < 1e-9
    }

    /// Every allowed value, ascending.
    pub fn values(&self) -> Vec<f64> {
        let n = ((self.max - self.min) / self.step).round() as usize;
        (0..=n).map(|i| self.min + self.step * i as f64).collect()
    }

    /// Text such as `14–20 in steps of 0.5`, for error messages.
    pub fn describe(&self) -> String {
        format!("{}–{} in steps of {}", self.min, self.max, self.step)
    }
}

/// Brew-to-water ratio (1:x).
pub const RATIO: Stepped = Stepped {
    min: 14.0,
    max: 20.0,
    step: 0.5,
};
/// Bloom ratio.
pub const BLOOM_RATIO: Stepped = Stepped {
    min: 1.0,
    max: 3.0,
    step: 0.5,
};
/// Water temperature in °C.
pub const TEMPERATURE: Stepped = Stepped {
    min: 50.0,
    max: 98.5,
    step: 0.5,
};
/// Bloom duration in seconds.
pub const BLOOM_DURATION: RangeInclusive<u32> = 1..=120;
/// Number of pulses.
pub const PULSES_NUMBER: RangeInclusive<u32> = 1..=10;
/// Interval between pulses in seconds.
pub const PULSES_INTERVAL: RangeInclusive<u32> = 5..=60;
/// Maximum title length in characters.
pub const TITLE_MAX_LEN: usize = 50;
/// Punctuation allowed in titles, besides ASCII letters, digits and space.
pub const TITLE_PUNCTUATION: &str = "!@#$%&*-+?/.,:)(";
/// Water amount per schedule in millilitres.
pub const WATER_ML: RangeInclusive<u32> = 150..=1500;
/// Last valid value of `secondFromStartOfTheDay`.
pub const MAX_SECOND_OF_DAY: u32 = 86_399;

/// Whether `c` may appear in a profile title.
pub fn is_valid_title_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == ' ' || TITLE_PUNCTUATION.contains(c)
}

/// Whether `id` looks like a profile id: `p<digits>` (cloud) or `plocal<digits>` (local).
pub fn is_valid_profile_id(id: &str) -> bool {
    let rest = id.strip_prefix("plocal").or_else(|| id.strip_prefix('p'));
    matches!(rest, Some(d) if !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepped_accepts_bounds_and_grid() {
        assert!(RATIO.contains(14.0));
        assert!(RATIO.contains(20.0));
        assert!(RATIO.contains(16.5));
        assert!(TEMPERATURE.contains(50.0));
        assert!(TEMPERATURE.contains(98.5));
        assert!(BLOOM_RATIO.contains(2.5));
    }

    #[test]
    fn stepped_rejects_out_of_range_off_grid_and_nan() {
        assert!(!RATIO.contains(13.5));
        assert!(!RATIO.contains(20.5));
        assert!(!RATIO.contains(16.25));
        assert!(!TEMPERATURE.contains(49.5));
        assert!(!TEMPERATURE.contains(99.0));
        assert!(!BLOOM_RATIO.contains(0.5));
        assert!(!RATIO.contains(f64::NAN));
        assert!(!RATIO.contains(f64::INFINITY));
    }

    #[test]
    fn stepped_values_enumerates_the_grid() {
        assert_eq!(BLOOM_RATIO.values(), vec![1.0, 1.5, 2.0, 2.5, 3.0]);
        assert_eq!(RATIO.values().len(), 13);
        assert_eq!(TEMPERATURE.values().len(), 98);
        assert!(
            TEMPERATURE
                .values()
                .iter()
                .all(|v| TEMPERATURE.contains(*v))
        );
    }

    #[test]
    fn title_chars() {
        for c in "aZ09 !@#$%&*-+?/.,:)(".chars() {
            assert!(is_valid_title_char(c), "{c:?}");
        }
        for c in ['é', '_', '"', '\n', '\'', '=', '[', '日'] {
            assert!(!is_valid_title_char(c), "{c:?}");
        }
    }

    #[test]
    fn profile_ids() {
        for ok in ["p0", "p12", "plocal7", "p000"] {
            assert!(is_valid_profile_id(ok), "{ok}");
        }
        for bad in [
            "", "p", "plocal", "q1", "p1a", "P1", "pl1", "p-1", " p1", "plocal1x",
        ] {
            assert!(!is_valid_profile_id(bad), "{bad}");
        }
    }
}
