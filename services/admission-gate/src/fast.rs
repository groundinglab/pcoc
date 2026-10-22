//! Fast phase. SPEC §14.7 decode, kind, and shape. No terrain fill. No leg.
//!
//! Any kind other than `v2.2-pcoc-object` is Rule A5. The reason is
//! `unknown_kind` (SPEC §15.4). There is no earlier-version ignore.
//! The profile is section 6 only (AC-D3). This module does not call a
//! byte-source draw.

use crate::cbor_limit::{self, MAX_CBOR_NESTING};
use crate::policy::AdmissionPolicy;
use crate::reads::{self, ReadStop};
use crate::verifier::IndexClient;
use pcoc::profile::Profile;
use pcoc::verify::Precheck;
#[cfg(test)]
use std::cell::Cell;
use std::time::Instant;

// Test-only elapsed of the fast pass, in milliseconds. Thread-local, so one
// test cannot inject a budget into another. Not a draw-path hook.
#[cfg(test)]
thread_local! {
    static FAST_ELAPSED_MS: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Run `body` with the fast-phase clock replaced by `ms` milliseconds.
#[cfg(test)]
pub fn with_fast_elapsed<T>(ms: u64, body: impl FnOnce() -> T) -> T {
    FAST_ELAPSED_MS.set(Some(ms));
    let out = body();
    FAST_ELAPSED_MS.set(None);
    out
}

#[derive(Clone, Debug, PartialEq)]
pub enum FastDecision {
    /// Kind and shape match, and every fast measurement was read.
    /// No action is granted. `provisional` is on.
    Provisional { record: String, tip: Option<u64> },
    /// Same measurements, with `provisional` off. No grant. Thorough runs next.
    Direct { record: String, tip: Option<u64> },
    /// A miss settled by the object's own bytes. No thorough run. No grant.
    Refuse {
        reasons: Vec<String>,
        line: String,
        record: String,
        /// Website token. `unknown_kind` is not a §15.1 website token, so the
        /// wire stays `rejected`. The log reason is `unknown_kind`.
        website: &'static str,
    },
    /// A measurement was not read, or a fast budget was missed. Not a refusal.
    /// `budget` is a `fast_accept_bound` or `async_catch_budget` miss: it must
    /// not later become provisional.
    Held {
        record: String,
        budget: bool,
        tip: Option<u64>,
    },
}

/// `true` when the fast phase's kind check is Rule A5.
pub fn is_unknown_kind(object: &[u8]) -> bool {
    matches!(kind_precheck(object), Some(Precheck::UnknownKind(_)))
}

/// Run the fast phase. No terrain. Header and plant reads go through `index`.
pub fn decide(object: &[u8], policy: &AdmissionPolicy, index: &IndexClient) -> FastDecision {
    let started = Instant::now();
    let Some(checked) = kind_precheck(object) else {
        return refuse(
            "malformed",
            "The object is not the canonical encoding.",
            "{}",
        );
    };
    match checked {
        Precheck::UnknownKind(record) => FastDecision::Refuse {
            reasons: vec!["unknown_kind".to_owned()],
            line: "This object is not the v2.2 kind. The gate refuses it.".to_owned(),
            record,
            website: crate::wire::OUTCOME_REJECTED,
        },
        Precheck::Malformed | Precheck::DecodeFailure => refuse(
            "malformed",
            "The object is not the canonical encoding.",
            "{}",
        ),
        Precheck::Shape { shape, shape_item } => {
            if shape != "match" {
                let record = serde_json::json!({
                    "shape": shape,
                    "shape_item": shape_item,
                })
                .to_string();
                return refuse(
                    "shape_mismatch",
                    "Invalid: the object does not match the section 6 shape.",
                    &record,
                );
            }
            let decision = match reads::assess(object, policy, index) {
                Ok(view) => {
                    if policy.provisional_on() {
                        FastDecision::Provisional {
                            record: view.record,
                            tip: view.tip,
                        }
                    } else {
                        FastDecision::Direct {
                            record: view.record,
                            tip: view.tip,
                        }
                    }
                }
                Err(ReadStop::Held { record, tip }) => FastDecision::Held {
                    record,
                    budget: false,
                    tip,
                },
                Err(ReadStop::Refuse {
                    reasons,
                    line,
                    record,
                }) => FastDecision::Refuse {
                    reasons,
                    line,
                    record,
                    website: crate::wire::OUTCOME_REJECTED,
                },
            };
            apply_budget(decision, policy, elapsed_ms(started))
        }
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    #[cfg(test)]
    if let Some(injected) = FAST_ELAPSED_MS.get() {
        return injected;
    }
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn apply_budget(decision: FastDecision, policy: &AdmissionPolicy, elapsed_ms: u64) -> FastDecision {
    let over = elapsed_ms > policy.fast_accept_bound_ms || elapsed_ms > policy.async_catch_budget_ms;
    if !over {
        return decision;
    }
    match decision {
        FastDecision::Provisional { record, tip } | FastDecision::Direct { record, tip } => {
            FastDecision::Held {
                record,
                budget: true,
                tip,
            }
        }
        other => other,
    }
}

fn kind_precheck(object: &[u8]) -> Option<Precheck> {
    if cbor_limit::nesting_exceeds(object, MAX_CBOR_NESTING) {
        return None;
    }
    Some(pcoc::precheck(object, &Profile::production()))
}

fn refuse(reason: &str, line: &str, record: &str) -> FastDecision {
    FastDecision::Refuse {
        reasons: vec![reason.to_owned()],
        line: line.to_owned(),
        record: record.to_owned(),
        website: crate::wire::OUTCOME_REJECTED,
    }
}

/// Measured numbers for a fast-phase log line, read off its record.
///
/// `legs` and `span_s` are `analysis.hike_legs` and `analysis.hike_span`.
/// They are the Rule B2 pair. Bookend `N` and `S` are copied as `bookend_n`
/// and `bookend_s` and are not the rate. `None` when the record holds none
/// of those fields and no `anchored_total`.
pub fn fast_measures(record: &str) -> Option<serde_json::Value> {
    let value = serde_json::from_str::<serde_json::Value>(record).ok()?;
    let analysis = value.get("analysis");
    let legs = analysis
        .and_then(|item| item.get("hike_legs"))
        .and_then(|item| item.as_i64());
    let span_s = analysis
        .and_then(|item| item.get("hike_span"))
        .and_then(|item| item.as_i64());
    let bookend_n = value.get("N").and_then(|item| item.as_i64());
    let bookend_s = value.get("S").and_then(|item| item.as_i64());
    let anchored = value.get("anchored_total").and_then(|item| item.as_u64());
    if legs.is_none()
        && span_s.is_none()
        && bookend_n.is_none()
        && bookend_s.is_none()
        && anchored.is_none()
    {
        return None;
    }
    // Display only. Admission uses `pcoc::leg_rate_floor`, not this quotient.
    let leg_rate = match (legs, span_s) {
        (Some(n), Some(s)) if n >= 0 && s > 0 => Some(n as f64 / s as f64),
        _ => None,
    };
    Some(serde_json::json!({
        "legs": legs,
        "span_s": span_s,
        "leg_rate": leg_rate,
        "bookend_n": bookend_n,
        "bookend_s": bookend_s,
        "anchored": anchored,
        "anchored_span": crate::policy::ANCHORED_SPAN,
    }))
}
