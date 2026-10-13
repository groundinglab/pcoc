//! Gate policy. The verifier reports measurements. This module judges them.
//!
//! The locked floors scale with the leg: strict 7/20, default 1/4, lenient
//! 1/8. The config names one of them. Rule B2 compares `q·N ≥ p·S` on the
//! bookend pair in the section 14 record. A pair that is not numeric is not
//! compared. When `N` is `not defined` from the object's own bytes, whatever
//! `S` is, the refusal is immediate (`bookends_not_defined`). A pure `not read`
//! pair, with no other verifier note, is `held`. `held` is not a refusal and
//! is not a decision-log event. A `not read` pair that arrives with another
//! verifier note is still a refusal: the other note wins. An object the
//! verifier reports as an earlier version is refused as `earlier_version`.
//! There is no profile file and no friendly list.

use crate::verifier::{Measured, Report, EARLIER_VERSION_NOTE};

pub const REFUSE_EARLIER: &str = "earlier_version";
pub const REFUSE_LEG_RATE: &str = "leg_rate_floor";
/// `N` is the section 14 token `not defined`. The object's own bytes settled
/// that side, whatever `S` is. Rule B0 and Rule B2 refuse it at once. This is
/// not the floor.
pub const REFUSE_BOOKENDS: &str = "bookends_not_defined";
pub const BOOKENDS_LINE: &str = "N and S are not defined.";

/// A bookend pair that is not two numbers, and `N` is not `not defined`,
/// when another verifier note is already a refusal.
///
/// A pure `not read` pair is [`Judgment::Held`], not this reason.
pub const REFUSE_BOOKENDS_NOT_READ: &str = "bookends_not_read";
pub const BOOKENDS_NOT_READ_LINE: &str =
    "The bookend headers were not read from the index. Submit again.";

/// Sentence the current website shows for [`REFUSE_EARLIER`].
///
/// The website sign-in page blanks a snake_case reason it does not know
/// and shows a `reason` string that contains a space. This sentence is that
/// line. It is not "could not be read".
pub const EARLIER_LINE: &str =
    "This object is an earlier version. This gate checks Satoshi Hike v2.0 only.";

pub const UNREADABLE_REASON: &str = "unreadable";
pub const UNREADABLE_LINE: &str = "This file is not a Satoshi Hike v2.0 object.";

pub const NO_KEY_REASON: &str = "no_public_key";
pub const NO_KEY_LINE: &str = "This object has no public key the gate can check.";

/// Sentence for the configured floor `p/q`. The website shows a `reason` that contains a space.
pub fn leg_rate_line(p: u64, q: u64) -> String {
    format!("The leg rate is below this gate's floor of {p}/{q} leg per second.")
}

#[derive(Clone, Debug, PartialEq)]
pub enum Judgment {
    Admit {
        /// Section 14 record bytes, including the trailing newline.
        record: String,
    },
    Refuse {
        reasons: Vec<String>,
        line: String,
        /// Section 14 record bytes, including the trailing newline.
        /// Empty of a record, this is `{}` for a fast refusal that has none.
        record: String,
    },
    /// A pure bookend `not read`. Not a refusal. Not a decision-log event.
    Held {
        /// Section 14 record bytes, including the trailing newline.
        record: String,
    },
}

/// Rule B2. The pair is below the floor when `q·N ≥ p·S` does not hold.
///
/// `S` of 0 meets the floor when `N` is not negative: `q·N ≥ 0`.
pub fn bookend_below_floor(n: i128, s: i128, p: u64, q: u64) -> bool {
    match (i128::from(q).checked_mul(n), i128::from(p).checked_mul(s)) {
        (Some(left), Some(right)) => left < right,
        _ => true,
    }
}

enum Bookend {
    /// `N` is `not defined`. The object's own bytes settled that side.
    NotDefined,
    Numeric {
        n: i128,
        s: i128,
    },
    /// `not read`, one side only, or any other non-numeric pair. No comparison.
    /// `N` is not the token `not defined`.
    NotRead,
}

/// Read `N` and `S` from the record text. The text is not rewritten.
fn bookend_pair(record: &str) -> Bookend {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(record) else {
        return Bookend::NotRead;
    };
    if matches!(
        value.get("N"),
        Some(serde_json::Value::String(n)) if n == "not defined"
    ) {
        return Bookend::NotDefined;
    }
    match (value.get("N"), value.get("S")) {
        (Some(n), Some(s)) => match (json_i128(n), json_i128(s)) {
            (Some(n), Some(s)) => Bookend::Numeric { n, s },
            _ => Bookend::NotRead,
        },
        _ => Bookend::NotRead,
    }
}

fn json_i128(value: &serde_json::Value) -> Option<i128> {
    let number = value.as_i64()?;
    Some(i128::from(number))
}

pub fn judge(report: &Report, floor_p: u64, floor_q: u64) -> Judgment {
    match report {
        Report::Earlier { .. } => Judgment::Refuse {
            reasons: vec![REFUSE_EARLIER.to_owned()],
            line: EARLIER_LINE.to_owned(),
            record: "{}".to_owned(),
        },
        Report::Unreadable => Judgment::Refuse {
            reasons: vec![UNREADABLE_REASON.to_owned()],
            line: UNREADABLE_LINE.to_owned(),
            record: "{}".to_owned(),
        },
        Report::Read(measured) => judge_measured(measured, floor_p, floor_q),
    }
}

fn judge_measured(measured: &Measured, floor_p: u64, floor_q: u64) -> Judgment {
    let record = measured.record.clone();
    if measured.earlier_version
        || measured.verifier_notes.iter().any(|note| {
            note == EARLIER_VERSION_NOTE || note == crate::verifier::EARLIER_VERSION_FACT
        })
    {
        return Judgment::Refuse {
            reasons: vec![REFUSE_EARLIER.to_owned()],
            line: EARLIER_LINE.to_owned(),
            record,
        };
    }
    let mut reasons: Vec<String> = measured
        .verifier_notes
        .iter()
        .map(|note| (*note).to_owned())
        .collect();
    match bookend_pair(&record) {
        Bookend::NotDefined => {
            reasons.insert(0, REFUSE_BOOKENDS.to_owned());
            return Judgment::Refuse {
                reasons,
                line: BOOKENDS_LINE.to_owned(),
                record,
            };
        }
        Bookend::Numeric { n, s } if bookend_below_floor(n, s, floor_p, floor_q) => {
            reasons.insert(0, REFUSE_LEG_RATE.to_owned());
            return Judgment::Refuse {
                reasons,
                line: leg_rate_line(floor_p, floor_q),
                record,
            };
        }
        Bookend::Numeric { .. } => {}
        // A pure `not read` is held. Another verifier note is still a refusal.
        Bookend::NotRead => {
            if reasons.is_empty() {
                return Judgment::Held { record };
            }
            reasons.insert(0, REFUSE_BOOKENDS_NOT_READ.to_owned());
            return Judgment::Refuse {
                reasons,
                line: BOOKENDS_NOT_READ_LINE.to_owned(),
                record,
            };
        }
    }
    if reasons.is_empty() {
        return Judgment::Admit { record };
    }
    let line = reasons.join(" ");
    Judgment::Refuse {
        reasons,
        line,
        record,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verifier::Measured;

    fn measured(record: &str, notes: &[&str]) -> Measured {
        Measured {
            earlier_version: false,
            kind: "satoshi-hike-v2.0-object".to_owned(),
            record: record.to_owned(),
            verifier_notes: notes.iter().map(|note| (*note).to_owned()).collect(),
        }
    }

    /// Key order is the caller's. The judgment returns those bytes.
    fn pair_record(n: &str, s: &str) -> String {
        format!("{{\"profile\":\"production\",\"shape\":\"match\",\"N\":{n},\"S\":{s}}}\n")
    }

    #[test]
    fn the_locked_floor_is_one_quarter_leg_per_second() {
        assert!(!bookend_below_floor(1, 4, 1, 4));
        assert!(bookend_below_floor(1, 5, 1, 4));
        assert!(!bookend_below_floor(2, 4, 1, 4));
        assert!(!bookend_below_floor(1, 0, 1, 4));
        assert!(bookend_below_floor(0, 5, 1, 4));
    }

    #[test]
    fn a_numeric_pair_under_the_floor_is_refused() {
        let record = pair_record("1", "5");
        let judgment = judge_measured(&measured(&record, &[]), 1, 4);
        assert_eq!(
            judgment,
            Judgment::Refuse {
                reasons: vec!["leg_rate_floor".to_owned()],
                line: leg_rate_line(1, 4),
                record: record.clone(),
            }
        );
        assert!(record.ends_with('\n'));
        assert!(record.starts_with("{\"profile\":\"production\","));
    }

    #[test]
    fn the_three_locked_floors_compare_the_bookend_pair() {
        let meets = pair_record("1", "4");
        assert!(matches!(
            judge_measured(&measured(&meets, &[]), 1, 4),
            Judgment::Admit { record } if record == meets
        ));
        match judge_measured(&measured(&meets, &[]), 7, 20) {
            Judgment::Refuse {
                reasons,
                line,
                record,
            } => {
                assert_eq!(reasons, vec!["leg_rate_floor".to_owned()]);
                assert_eq!(line, leg_rate_line(7, 20));
                assert_eq!(record, meets);
            }
            other => panic!("strict floor should refuse N=1 S=4, got {other:?}"),
        }
        let lenient_ok = pair_record("1", "8");
        assert!(matches!(
            judge_measured(&measured(&lenient_ok, &[]), 1, 8),
            Judgment::Admit { .. }
        ));
        match judge_measured(&measured(&pair_record("1", "9"), &[]), 1, 8) {
            Judgment::Refuse { reasons, line, .. } => {
                assert_eq!(reasons, vec!["leg_rate_floor".to_owned()]);
                assert_eq!(line, leg_rate_line(1, 8));
            }
            other => panic!("lenient floor should refuse N=1 S=9, got {other:?}"),
        }
    }

    #[test]
    fn an_object_settled_not_defined_pair_is_refused_without_a_floor_sentence() {
        let record = pair_record("\"not defined\"", "\"not defined\"");
        match judge_measured(&measured(&record, &["shape is false"]), 1, 4) {
            Judgment::Refuse {
                reasons,
                line,
                record: stored,
            } => {
                assert_eq!(
                    reasons,
                    vec![
                        "bookends_not_defined".to_owned(),
                        "shape is false".to_owned()
                    ]
                );
                assert_eq!(line, BOOKENDS_LINE);
                assert!(!line.contains("1/4"));
                assert!(!line.contains("leg per second"));
                assert_eq!(stored, record);
            }
            other => panic!("expected an immediate refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_not_read_pair_is_held() {
        for (n, s) in [("\"not read\"", "\"not read\""), ("4", "\"not read\"")] {
            let record = pair_record(n, s);
            match judge_measured(&measured(&record, &[]), 1, 4) {
                Judgment::Held { record: stored } => {
                    assert_eq!(stored, record, "{n},{s}");
                    assert!(!stored.contains("bookends_not_read"), "{n},{s}");
                }
                other => panic!("expected held for {n},{s}, got {other:?}"),
            }
        }
    }

    #[test]
    fn n_not_defined_with_s_not_read_is_refused_at_once() {
        let mixed = pair_record("\"not defined\"", "\"not read\"");
        match judge_measured(&measured(&mixed, &[]), 7, 20) {
            Judgment::Refuse {
                reasons,
                line,
                record,
            } => {
                assert_eq!(reasons, vec!["bookends_not_defined".to_owned()]);
                assert_eq!(line, BOOKENDS_LINE);
                assert!(!line.contains("7/20"));
                assert!(!line.contains("leg per second"));
                assert_eq!(record, mixed);
            }
            other => panic!("expected an immediate refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_floor_refusal_keeps_the_other_notes() {
        let record = pair_record("0", "5");
        match judge_measured(&measured(&record, &["shape is false"]), 1, 4) {
            Judgment::Refuse {
                reasons,
                line,
                record: stored,
            } => {
                assert_eq!(
                    reasons,
                    vec!["leg_rate_floor".to_owned(), "shape is false".to_owned()]
                );
                assert!(line.contains("1/4"));
                assert_eq!(stored, record);
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_earlier_line_is_not_could_not_be_read() {
        assert!(!EARLIER_LINE
            .to_ascii_lowercase()
            .contains("could not be read"));
        assert!(!UNREADABLE_LINE
            .to_ascii_lowercase()
            .contains("could not be read"));
    }
}
