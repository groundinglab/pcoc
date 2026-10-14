//! Time left, labelled an estimate (DESIGN-v2 section 2.5, open item 10).
//!
//! The input is the count of headers still to come. It does not use the leg
//! rate. Header arrivals are modelled as a Poisson process at one per
//! [`HEADER_INTERVAL_S`], so the wait for `k` headers is Erlang(k). The
//! point figure is the mean less the age of the current tip. "Most
//! finishes" is the 10th to 90th percentile of that wait.

use super::HEADER_INTERVAL_S;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Estimate {
    pub headers_to_come: u64,
    pub point_s: u64,
    pub low_s: u64,
    pub high_s: u64,
}

impl Estimate {
    /// The "≈ HH:MM UTC" figure: now plus the point estimate as displayed,
    /// so the two figures on screen agree.
    pub fn finish_unix_s(&self, now_unix_s: u64) -> u64 {
        now_unix_s + self.point_s / 60 * 60
    }
}

pub fn estimate(headers_to_come: u64, tip_age_s: u64) -> Option<Estimate> {
    if headers_to_come == 0 {
        return None;
    }
    let mean = headers_to_come * HEADER_INTERVAL_S;
    let low = erlang_quantile(headers_to_come, 0.10) * HEADER_INTERVAL_S as f64;
    let high = erlang_quantile(headers_to_come, 0.90) * HEADER_INTERVAL_S as f64;
    // The range is shown in whole minutes and rounded outwards, so the
    // display never narrows it.
    Some(Estimate {
        headers_to_come,
        point_s: mean.saturating_sub(tip_age_s),
        low_s: (low / 60.0).floor() as u64 * 60,
        high_s: (high / 60.0).ceil() as u64 * 60,
    })
}

/// P(X ≤ x) for Erlang(k, rate 1).
fn erlang_cdf(k: u64, x: f64) -> f64 {
    let mut term = (-x).exp();
    let mut sum = term;
    for n in 1..k {
        term *= x / n as f64;
        sum += term;
    }
    1.0 - sum
}

fn erlang_quantile(k: u64, p: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, (k as f64) * 10.0 + 10.0);
    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        if erlang_cdf(k, mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::duration_hm;

    /// Moment R: 12 headers to come, tip 2 m 03 s old.
    #[test]
    fn design_moment_r() {
        let e = estimate(12, 123).unwrap();
        assert_eq!(duration_hm(e.point_s), "1 h 57 m");
        assert_eq!(duration_hm(e.low_s), "1 h 18 m");
        assert_eq!(duration_hm(e.high_s), "2 h 46 m");
        assert_eq!(
            crate::format::clock_hm(e.finish_unix_s(1_791_533_491)),
            "10:08"
        );
    }

    #[test]
    fn nothing_to_come_is_no_estimate() {
        assert!(estimate(0, 5).is_none());
        let one = estimate(1, 0).unwrap();
        assert!(one.low_s < one.point_s && one.point_s < one.high_s);
    }
}
