// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Numeric policy for requested deadlines.

/// The longest deadline the guard accepts, in seconds (24 hours).
pub const MAX_SECONDS: f64 = 86400.0;

#[inline(always)]
pub(crate) fn invalid_seconds(seconds: f64) -> bool {
    !seconds.is_finite() || seconds <= 0.0 || seconds > MAX_SECONDS
}

#[cfg(test)]
mod tests {
    use super::{invalid_seconds, MAX_SECONDS};

    #[test]
    fn numeric_deadlines_accept_positive_finite_values_up_to_the_cap() {
        assert_eq!(MAX_SECONDS, 86400.0);
        let below_cap = f64::from_bits(MAX_SECONDS.to_bits() - 1);
        for seconds in [f64::from_bits(1), f64::MIN_POSITIVE, 0.5, below_cap, MAX_SECONDS] {
            assert!(!invalid_seconds(seconds), "{seconds}");
        }
    }

    #[test]
    fn numeric_deadlines_reject_nonpositive_nonfinite_and_over_cap_values() {
        let above_cap = f64::from_bits(MAX_SECONDS.to_bits() + 1);
        for seconds in [0.0, -0.0, -0.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, above_cap] {
            assert!(invalid_seconds(seconds), "{seconds}");
        }
    }
}
