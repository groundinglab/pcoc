//! One refusal reason per decision line.
//!
//! The code is stable snake_case. The class is `below_policy`, `invalid`, or
//! `operational`. The catch-all `rejected` is never a code. Hike's free-text
//! notes map here. A sentence the website already shows stays a sentence.
//! The code is what the log stores.

use crate::policy::{
    self, AdmissionPolicy, PolicyCheck, REFUSE_BOOKENDS, REFUSE_BOOKENDS_NOT_READ, REFUSE_EARLIER,
    REFUSE_LEG_RATE, UNREADABLE_REASON,
};
use crate::verifier::{Measures, Report, INDEX_UNREAD_NOTE};
use crate::wire::{self, OUTCOME_EARLIER, OUTCOME_TOO_FEW, OUTCOME_TOO_SHORT};

/// `below_policy`: measured, under the groundinglab minimum.
pub const CLASS_BELOW_POLICY: &str = "below_policy";
/// `invalid`: the proof is broken or is not a v2.0 proof.
pub const CLASS_INVALID: &str = "invalid";
/// `operational`: not judged on merit.
pub const CLASS_OPERATIONAL: &str = "operational";

pub const CODE_BELOW_LEG_RATE: &str = "below_leg_rate";
pub const CODE_BELOW_WAYPOINTS: &str = "below_waypoints";
pub const CODE_EARLIER_VERSION: &str = "earlier_version";
pub const CODE_UNREADABLE: &str = "unreadable";
pub const CODE_NO_PUBLIC_KEY: &str = "no_public_key";
pub const CODE_TOO_SHORT: &str = "too_short";
pub const CODE_BOOKENDS_NOT_DEFINED: &str = "bookends_not_defined";
pub const CODE_BOOKENDS_NOT_READ: &str = "bookends_not_read";
pub const CODE_SHAPE_MISMATCH: &str = "shape_mismatch";
pub const CODE_CHECKPOINT_MISMATCH: &str = "checkpoint_mismatch";
pub const CODE_SEED_VALUE_MISMATCH: &str = "seed_value_mismatch";
pub const CODE_PATH_PLANT_MISMATCH: &str = "path_plant_mismatch";
pub const CODE_SUCCESSOR_MISMATCH: &str = "successor_mismatch";
pub const CODE_LEG_RECOMPUTE_MISMATCH: &str = "leg_recompute_mismatch";
pub const CODE_FEWER_THAN_6_LEGS: &str = "fewer_than_6_legs";
pub const CODE_PATHS_INCLUDED: &str = "paths_included_below_minimum";
pub const CODE_INDEX_UNREAD: &str = "index_unread";
pub const CODE_VERIFIER_ERROR: &str = "verifier_error";
pub const CODE_CHALLENGE_EXPIRED: &str = "challenge_expired";

/// Every code this gate writes, in table order. A test covers this list.
pub const INVENTORY: &[(&str, &str)] = &[
    (CODE_BELOW_LEG_RATE, CLASS_BELOW_POLICY),
    (CODE_BELOW_WAYPOINTS, CLASS_BELOW_POLICY),
    (CODE_EARLIER_VERSION, CLASS_INVALID),
    (CODE_UNREADABLE, CLASS_INVALID),
    (CODE_NO_PUBLIC_KEY, CLASS_INVALID),
    (CODE_TOO_SHORT, CLASS_INVALID),
    (CODE_BOOKENDS_NOT_DEFINED, CLASS_INVALID),
    (CODE_BOOKENDS_NOT_READ, CLASS_OPERATIONAL),
    (CODE_SHAPE_MISMATCH, CLASS_INVALID),
    (CODE_CHECKPOINT_MISMATCH, CLASS_INVALID),
    (CODE_SEED_VALUE_MISMATCH, CLASS_INVALID),
    (CODE_PATH_PLANT_MISMATCH, CLASS_INVALID),
    (CODE_SUCCESSOR_MISMATCH, CLASS_INVALID),
    (CODE_LEG_RECOMPUTE_MISMATCH, CLASS_INVALID),
    (CODE_FEWER_THAN_6_LEGS, CLASS_INVALID),
    (CODE_PATHS_INCLUDED, CLASS_INVALID),
    (CODE_INDEX_UNREAD, CLASS_OPERATIONAL),
    (CODE_VERIFIER_ERROR, CLASS_OPERATIONAL),
    (CODE_CHALLENGE_EXPIRED, CLASS_OPERATIONAL),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reason {
    pub code: &'static str,
    pub class: &'static str,
    /// Plain sentence. A space means the website shows it.
    pub line: String,
}

/// Map one hike free-text note, or a gate token, to one code.
pub fn code_of_note(note: &str) -> &'static str {
    match note {
        "shape is false" => CODE_SHAPE_MISMATCH,
        "checkpoint mismatch" => CODE_CHECKPOINT_MISMATCH,
        "seed value mismatches" => CODE_SEED_VALUE_MISMATCH,
        "a present path does not match the plant" => CODE_PATH_PLANT_MISMATCH,
        "a successor waypoint differs" => CODE_SUCCESSOR_MISMATCH,
        "a re-walked leg result differs" | "a recomputed leg result differs" => {
            CODE_LEG_RECOMPUTE_MISMATCH
        }
        "fewer than 6 legs, so the six indices could not be drawn" => CODE_FEWER_THAN_6_LEGS,
        "paths included is below the minimum" => CODE_PATHS_INCLUDED,
        note if note == INDEX_UNREAD_NOTE => CODE_INDEX_UNREAD,
        note if note == REFUSE_EARLIER || note == OUTCOME_EARLIER => CODE_EARLIER_VERSION,
        note if note == UNREADABLE_REASON => CODE_UNREADABLE,
        note if note == policy::NO_KEY_REASON => CODE_NO_PUBLIC_KEY,
        note if note == OUTCOME_TOO_SHORT => CODE_TOO_SHORT,
        note if note == REFUSE_BOOKENDS => CODE_BOOKENDS_NOT_DEFINED,
        note if note == REFUSE_BOOKENDS_NOT_READ => CODE_BOOKENDS_NOT_READ,
        note if note == REFUSE_LEG_RATE => CODE_BELOW_LEG_RATE,
        note if note == OUTCOME_TOO_FEW => CODE_BELOW_WAYPOINTS,
        "verifier_error" => CODE_VERIFIER_ERROR,
        CODE_CHALLENGE_EXPIRED => CODE_CHALLENGE_EXPIRED,
        _ => CODE_SHAPE_MISMATCH,
    }
}

pub fn class_of(code: &str) -> &'static str {
    INVENTORY
        .iter()
        .find(|(known, _)| *known == code)
        .map(|(_, class)| *class)
        .unwrap_or(CLASS_INVALID)
}

/// One reason for a measured refusal.
///
/// A broken proof is `invalid` or `operational`. A sound proof under a
/// configured minimum is `below_policy`.
pub fn from_report(report: &Report, checks: &[PolicyCheck], policy: &AdmissionPolicy) -> Reason {
    if let Report::Read(measured) = report {
        if !measured.verifier_notes.is_empty() || measured.earlier_version {
            return from_notes(&measured.verifier_notes, &measured.measures);
        }
    }
    let measures = match report {
        Report::Read(measured) => Some(&measured.measures),
        _ => None,
    };
    if let Some(reason) = policy_reason(checks, policy, measures) {
        return reason;
    }
    match report {
        Report::Earlier { .. } => Reason {
            code: CODE_EARLIER_VERSION,
            class: CLASS_INVALID,
            line: policy::EARLIER_LINE.to_owned(),
        },
        Report::Unreadable => Reason {
            code: CODE_UNREADABLE,
            class: CLASS_INVALID,
            line: policy::UNREADABLE_LINE.to_owned(),
        },
        Report::Read(measured) => from_notes(&measured.verifier_notes, &measured.measures),
    }
}

fn policy_reason(
    checks: &[PolicyCheck],
    policy: &AdmissionPolicy,
    measures: Option<&Measures>,
) -> Option<Reason> {
    let (p, q) = (policy.leg_rate_floor.p, policy.leg_rate_floor.q);
    let rate = checks.iter().find(|check| check.field == "leg_rate")?;
    let waypoints = checks
        .iter()
        .find(|check| check.field == "anchored_total" || check.field == "waypoints_fetched")?;
    if !rate.met {
        return Some(Reason {
            code: CODE_BELOW_LEG_RATE,
            class: CLASS_BELOW_POLICY,
            line: match measures.and_then(|m| m.legs.zip(m.span_s)) {
                Some((n, s)) => policy::leg_rate_line(i128::from(n), i128::from(s), p, q),
                None => policy::leg_rate_floor_line(p, q),
            },
        });
    }
    if !waypoints.met {
        return Some(Reason {
            code: CODE_BELOW_WAYPOINTS,
            class: CLASS_BELOW_POLICY,
            line: match measures.and_then(|m| m.waypoints_fetched) {
                Some(anchored) => policy::too_few_line(anchored, policy.min_anchored_paths),
                None => format!(
                    "Waypoints anchored: fewer than {min} of {}, needs {min}.",
                    policy::ANCHORED_SPAN,
                    min = policy.min_anchored_paths
                ),
            },
        });
    }
    None
}

fn from_notes(notes: &[String], measures: &Measures) -> Reason {
    let note = notes
        .first()
        .map(String::as_str)
        .unwrap_or(CODE_SHAPE_MISMATCH);
    let code = code_of_note(note);
    let class = class_of(code);
    let line = sentence(code, measures);
    Reason { code, class, line }
}

/// The verifier did not finish. Operational. Not a judgment of the proof.
pub fn verifier_error() -> Reason {
    Reason {
        code: CODE_VERIFIER_ERROR,
        class: CLASS_OPERATIONAL,
        line: "The verifier did not finish.".to_owned(),
    }
}

fn sentence(code: &str, measures: &Measures) -> String {
    match code {
        CODE_LEG_RECOMPUTE_MISMATCH => {
            let which = measures
                .legs_recomputed_ok
                .map(|ok| format!("leg {ok}"))
                .unwrap_or_else(|| "a recomputed leg".to_owned());
            format!("Invalid: {which} does not recompute.")
        }
        CODE_CHECKPOINT_MISMATCH => {
            "Invalid: the Bitcoin blocks it names do not match the public chain.".to_owned()
        }
        CODE_EARLIER_VERSION => policy::EARLIER_LINE.to_owned(),
        CODE_UNREADABLE => policy::UNREADABLE_LINE.to_owned(),
        CODE_NO_PUBLIC_KEY => policy::NO_KEY_LINE.to_owned(),
        CODE_TOO_SHORT => "The object is shorter than 32 bytes.".to_owned(),
        CODE_BOOKENDS_NOT_DEFINED => policy::BOOKENDS_LINE.to_owned(),
        CODE_BOOKENDS_NOT_READ => policy::BOOKENDS_NOT_READ_LINE.to_owned(),
        CODE_FEWER_THAN_6_LEGS => {
            "Invalid: fewer than 6 legs, so the proof cannot be drawn.".to_owned()
        }
        CODE_INDEX_UNREAD => "No decision on the PCOC: the index did not answer.".to_owned(),
        CODE_VERIFIER_ERROR => "The verifier did not finish.".to_owned(),
        CODE_CHALLENGE_EXPIRED => {
            "No decision on the PCOC: the sign-in challenge expired before it was answered."
                .to_owned()
        }
        CODE_SHAPE_MISMATCH => "Invalid: the object does not match the v2.0 shape.".to_owned(),
        CODE_SEED_VALUE_MISMATCH => "Invalid: the opening waypoint does not match.".to_owned(),
        CODE_PATH_PLANT_MISMATCH => "Invalid: a present path does not match the plant.".to_owned(),
        CODE_SUCCESSOR_MISMATCH => "Invalid: a successor waypoint differs.".to_owned(),
        CODE_PATHS_INCLUDED => "Invalid: paths included is below the spec minimum.".to_owned(),
        CODE_BELOW_LEG_RATE => "Valid, measured below the configured leg rate.".to_owned(),
        CODE_BELOW_WAYPOINTS => "Valid, fewer waypoints than the configured minimum.".to_owned(),
        _ => "Invalid: the proof does not hold.".to_owned(),
    }
}

/// Website outcome for a locked sign-in after policy.
///
/// `accepted` when the proof is sound and every policy check is met.
/// `pending` when the bookends were not read and nothing else failed.
/// Otherwise `rejected` stays the website token. The log uses [`Reason`].
pub fn website_outcome(report: &Report, checks_met: bool, held: bool) -> &'static str {
    if held {
        return wire::OUTCOME_PENDING;
    }
    if checks_met && proof_sound(report) {
        return wire::OUTCOME_ACCEPTED;
    }
    wire::OUTCOME_REJECTED
}

fn proof_sound(report: &Report) -> bool {
    match report {
        Report::Read(measured) => measured.verifier_notes.is_empty() && !measured.earlier_version,
        Report::Earlier { .. } | Report::Unreadable => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_inventory_code_is_snake_case_and_not_rejected() {
        assert!(INVENTORY.len() >= 10);
        for (code, class) in INVENTORY {
            assert!(
                code.chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch == '_' || ch.is_ascii_digit()),
                "{code}"
            );
            assert_ne!(*code, "rejected");
            assert!(matches!(
                *class,
                CLASS_BELOW_POLICY | CLASS_INVALID | CLASS_OPERATIONAL
            ));
            assert_eq!(class_of(code), *class);
        }
    }

    #[test]
    fn hike_sentences_map_to_stable_codes() {
        assert_eq!(
            code_of_note("a re-walked leg result differs"),
            CODE_LEG_RECOMPUTE_MISMATCH
        );
        assert_eq!(
            code_of_note("a recomputed leg result differs"),
            CODE_LEG_RECOMPUTE_MISMATCH
        );
        assert_eq!(code_of_note("shape is false"), CODE_SHAPE_MISMATCH);
        assert_eq!(
            code_of_note("checkpoint mismatch"),
            CODE_CHECKPOINT_MISMATCH
        );
        assert_ne!(code_of_note("shape is false"), "rejected");
    }
}
