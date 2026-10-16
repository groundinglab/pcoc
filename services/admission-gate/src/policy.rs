//! Gate policy. The verifier reports measurements. This module judges them.
//!
//! `policy_id` names one exact table for its whole life. The table is the
//! keys in `policy/gl-admit-v4.toml`. A change of any value is a new id.
//! The rate compares the bookend pair `(N, S)` from the section 14 record
//! as integers: `q·N ≥ p·S`. The waypoint count compares `anchored_total`
//! among `W_1` … `W_16`. Neither number is a measurement.
//!
//! v1 history: through gate 0.4.1 the code default was `gl-admit-v1`, a float
//! `min_leg_rate` of 0.25 leg/s and `min_waypoints` 6 of 18. That float and
//! the named floors (`strict` 7/20, `default` 1/4, `lenient` 1/8) are gone.
//! Those three fractions stay informative examples in spec §15.5. They are
//! not a policy key. The code default is now `gl-admit-v4`.
//!
//! When `N` is `not defined` from the object's own bytes, whatever `S` is,
//! the refusal is immediate (`bookends_not_defined`). A pure `not read` pair,
//! with no other verifier note, is `held`. `held` is not a refusal and is not
//! a decision-log event. A `not read` pair that arrives with another verifier
//! note is still a refusal: the other note wins. An object the verifier
//! reports as an earlier version is refused as `earlier_version`.

use crate::canon;
use crate::verifier::{Measured, Report, EARLIER_VERSION_NOTE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
/// A pure `not read` pair is [`Judgment::Held`], not this reason. The website
/// outcome for that hold stays the locked `pending` token. See STOP-AND-REPORT
/// in the build notes: adding a `held` website token would change §15.1.
pub const REFUSE_BOOKENDS_NOT_READ: &str = "bookends_not_read";
pub const BOOKENDS_NOT_READ_LINE: &str =
    "The bookend headers were not read from the index. Submit again.";

/// Settled shortfall of anchored waypoints among `W_1` … `W_16`.
pub const REFUSE_TOO_FEW: &str = "too_few_waypoints_anchored";
pub const TOO_FEW_LINE: &str = "Too few waypoints among W_1 through W_16 are anchored.";

/// Budget exhausted while a measurement stayed `not read`.
pub const REFUSE_HELD_BOUND: &str = "held_bound";

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

/// `gl-admit-v` plus a positive integer. Bound to one table for its life.
pub const DEFAULT_POLICY_ID: &str = "gl-admit-v4";

/// `W_1` … `W_16`. `W_17` is never fetched and is not in the anchored count.
pub const ANCHORED_SPAN: u64 = 16;

/// Stage count on the object. Not the anchored count.
pub const WAYPOINTS_TOTAL: u64 = 18;

/// Spec rule: at least this many legs. Labelled as spec, not policy.
pub const SPEC_MIN_LEGS: u64 = 6;

/// Spec name published with the policy.
pub const POLICY_SPEC: &str = crate::verifier::SPEC_NAME;

/// A return line for one pair is written at most once per this many seconds.
pub const RETURN_WINDOW_S: u64 = 300;

/// Default pending caps (Rule B7 / F3). Over a cap the website outcome stays
/// the locked `pending` token and the nonce is not consumed.
pub const DEFAULT_PENDING_PER_SOURCE: u64 = 1;
pub const DEFAULT_PENDING_GLOBAL_FACTOR: u64 = 2;

/// `policy_id` matches `gl-admit-v` plus a positive integer.
pub fn parse_policy_id(raw: &str) -> Result<String, String> {
    let id = raw.trim();
    let Some(rest) = id.strip_prefix("gl-admit-v") else {
        return Err(format!(
            "policy_id {raw:?} must be gl-admit-v plus a positive integer"
        ));
    };
    if rest.is_empty() || !rest.bytes().all(|byte| byte.is_ascii_digit()) || rest.starts_with('0')
    {
        return Err(format!(
            "policy_id {raw:?} must be gl-admit-v plus a positive integer"
        ));
    }
    Ok(id.to_owned())
}

/// Integer leg-rate floor `p/q`. Meets when `q·N ≥ p·S`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateFloor {
    pub p: u64,
    pub q: u64,
}

/// One configured policy table. A different value is a different id.
///
/// The fields are the keys of `policy/gl-admit-v4.toml`, plus the pending
/// caps the plan names (F3) and the held-retry budgets §15.3 names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AdmissionPolicy {
    pub policy_id: String,
    pub leg_rate_floor: RateFloor,
    pub min_anchored_paths: u64,
    pub require_first_anchored: u64,
    pub require_last_anchored: u64,
    pub thorough_six_leg_runs: u64,
    pub thorough_early_stop: bool,
    /// `on` or `off`. `on` means the fast phase issues `provisional`.
    pub provisional: String,
    pub held_on_not_read: bool,
    pub ban_key_on_fake_leg: bool,
    /// Draw commitment spelling published with the table. The bytes are
    /// `H("pcoc-deep-v1" ‖ seq ‖ object_sha ‖ salt ‖ indices)`.
    pub draw_commitment: String,
    pub reverify_max_attempts: u64,
    pub max_held_blocks: u64,
    pub pending_per_source: u64,
    /// `None` means `max_concurrent × 2`, filled when the config is built.
    pub pending_global: u64,
    pub verify_cap_per_door_session: u64,
}

impl Default for AdmissionPolicy {
    fn default() -> Self {
        Self {
            policy_id: DEFAULT_POLICY_ID.to_owned(),
            leg_rate_floor: RateFloor { p: 1, q: 5 },
            min_anchored_paths: 6,
            require_first_anchored: 1,
            require_last_anchored: 16,
            thorough_six_leg_runs: 3,
            thorough_early_stop: true,
            provisional: "on".to_owned(),
            held_on_not_read: true,
            ban_key_on_fake_leg: true,
            draw_commitment: "sha256(draw || salt), revealed with the result".to_owned(),
            reverify_max_attempts: 3,
            max_held_blocks: 6,
            pending_per_source: DEFAULT_PENDING_PER_SOURCE,
            pending_global: 4,
            verify_cap_per_door_session: 3,
        }
    }
}

impl AdmissionPolicy {
    /// `{log_dir}/policy-bind.json` binds this id to a hash of the full table.
    ///
    /// The first start writes the file. A later start with the same id and
    /// a different table exits. A new id is a new binding. Old lines stay.
    pub fn bind(&self, log_dir: &std::path::Path) -> Result<(), String> {
        let path = log_dir.join("policy-bind.json");
        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .map_err(|err| format!("could not read {}: {err}", path.display()))?;
            let stored: serde_json::Value = serde_json::from_str(&raw)
                .map_err(|err| format!("{} is not JSON: {err}", path.display()))?;
            let key = self.bind_key();
            if let Some(previous) = stored.get(&self.policy_id) {
                if previous.as_str() != Some(key.as_str()) {
                    return Err(format!(
                        "policy_id {} is already bound to {previous}; a value change needs a new policy_id",
                        self.policy_id
                    ));
                }
                return Ok(());
            }
            let mut map = stored
                .as_object()
                .cloned()
                .unwrap_or_else(serde_json::Map::new);
            map.insert(self.policy_id.clone(), serde_json::Value::String(key));
            write_bind(&path, &serde_json::Value::Object(map))?;
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
        }
        let body = serde_json::json!({ self.policy_id.clone(): self.bind_key() });
        write_bind(&path, &body)
    }

    /// SHA-256 of the canonical full table. Two ids with the same table share
    /// this digest; a changed value does not.
    pub fn bind_key(&self) -> String {
        let value = serde_json::to_value(self).expect("policy table");
        let bytes = canon::jcs(&value).expect("policy jcs");
        hex::encode(canon::sha256(&bytes))
    }

    pub fn provisional_on(&self) -> bool {
        self.provisional == "on"
    }
}

fn write_bind(path: &std::path::Path, body: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(body).map_err(|err| err.to_string())?;
    std::fs::write(path, bytes).map_err(|err| format!("could not write {}: {err}", path.display()))
}

/// One gate check against a measurement. `min` is policy. `met` is the judgment.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyCheck {
    pub field: &'static str,
    pub min: serde_json::Value,
    pub met: bool,
}

pub fn policy_checks(measures: &crate::verifier::Measures, policy: &AdmissionPolicy) -> Vec<PolicyCheck> {
    let mut checks = Vec::new();
    if let (Some(n), Some(s)) = (measures.legs, measures.span_s) {
        checks.push(PolicyCheck {
            field: "leg_rate",
            min: serde_json::json!({
                "p": policy.leg_rate_floor.p,
                "q": policy.leg_rate_floor.q,
            }),
            met: !bookend_below_floor(
                i128::from(n),
                i128::from(s),
                policy.leg_rate_floor.p,
                policy.leg_rate_floor.q,
            ),
        });
    }
    if let Some(anchored) = measures.waypoints_fetched {
        checks.push(PolicyCheck {
            field: "anchored_total",
            min: serde_json::json!(policy.min_anchored_paths),
            met: anchored >= policy.min_anchored_paths,
        });
    }
    checks
}

/// Both floors were measured and both were met.
pub fn policy_admits(checks: &[PolicyCheck]) -> bool {
    let rate = checks.iter().any(|check| check.field == "leg_rate" && check.met);
    let waypoints = checks
        .iter()
        .any(|check| check.field == "anchored_total" && check.met);
    rate && waypoints && checks.iter().all(|check| check.met)
}

/// `below_policy` when a configured minimum is missed and the proof is otherwise sound.
pub fn below_policy(checks: &[PolicyCheck]) -> bool {
    checks.iter().any(|check| !check.met)
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
/// Compared in `u128` so a large pair does not wrap. `S` of 0 meets the
/// floor when `N` is not negative: `q·N ≥ 0`. A negative side, or a product
/// that does not fit in `u128`, is below the floor.
pub fn bookend_below_floor(n: i128, s: i128, p: u64, q: u64) -> bool {
    let (Ok(n), Ok(s)) = (u128::try_from(n), u128::try_from(s)) else {
        return true;
    };
    match (
        u128::from(q).checked_mul(n),
        u128::from(p).checked_mul(s),
    ) {
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

fn json_i128(value: &Value) -> Option<i128> {
    if let Some(number) = value.as_i64() {
        return Some(i128::from(number));
    }
    if let Some(number) = value.as_u64() {
        return Some(i128::from(number));
    }
    None
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

/// The public policy document. `source` says this table is a recommendation.
pub fn public_document(policy: &AdmissionPolicy) -> Value {
    let mut value = serde_json::to_value(policy).expect("policy table");
    if let Some(map) = value.as_object_mut() {
        map.insert(
            "source".to_owned(),
            Value::String("recommendation, not specification".to_owned()),
        );
        map.insert("spec".to_owned(), Value::String(POLICY_SPEC.to_owned()));
        map.insert(
            "anchored_span".to_owned(),
            serde_json::json!(ANCHORED_SPAN),
        );
    }
    value
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
            measures: crate::verifier::Measures {
                legs: None,
                span_s: None,
                leg_rate: None,
                waypoints_fetched: None,
                waypoints_total: WAYPOINTS_TOTAL,
                paths_included: None,
                legs_recomputed_ok: None,
            },
        }
    }

    /// Key order is the caller's. The judgment returns those bytes.
    fn pair_record(n: &str, s: &str) -> String {
        format!("{{\"profile\":\"production\",\"shape\":\"match\",\"N\":{n},\"S\":{s}}}\n")
    }

    #[test]
    fn a_configured_fraction_is_the_floor() {
        // 1/5: one leg every five seconds. Equality meets it.
        assert!(!bookend_below_floor(1, 5, 1, 5));
        assert!(bookend_below_floor(1, 6, 1, 5));
        assert!(!bookend_below_floor(2, 5, 1, 5));
        assert!(!bookend_below_floor(1, 0, 1, 5));
        assert!(bookend_below_floor(0, 5, 1, 5));
        // The same function applies a caller-chosen p/q. 7/20 is not a default.
        assert!(bookend_below_floor(1, 4, 7, 20));
        assert!(!bookend_below_floor(7, 20, 7, 20));
    }

    #[test]
    fn the_product_is_compared_in_u128() {
        let n = i128::from(u64::MAX);
        let s = i128::from(u64::MAX);
        assert!(!bookend_below_floor(n, s, 1, 5));
        assert!(bookend_below_floor(1, s, u64::MAX, 5));
    }

    #[test]
    fn a_numeric_pair_under_the_floor_is_refused() {
        let record = pair_record("1", "6");
        let judgment = judge_measured(&measured(&record, &[]), 1, 5);
        assert_eq!(
            judgment,
            Judgment::Refuse {
                reasons: vec!["leg_rate_floor".to_owned()],
                line: leg_rate_line(1, 5),
                record: record.clone(),
            }
        );
        assert!(record.ends_with('\n'));
        assert!(record.starts_with("{\"profile\":\"production\","));
    }

    #[test]
    fn a_configured_pq_is_applied_to_the_bookend_pair() {
        let meets = pair_record("1", "5");
        assert!(matches!(
            judge_measured(&measured(&meets, &[]), 1, 5),
            Judgment::Admit { record } if record == meets
        ));
        match judge_measured(&measured(&meets, &[]), 1, 4) {
            Judgment::Refuse {
                reasons,
                line,
                record,
            } => {
                assert_eq!(reasons, vec!["leg_rate_floor".to_owned()]);
                assert_eq!(line, leg_rate_line(1, 4));
                assert_eq!(record, meets);
            }
            other => panic!("1/4 should refuse N=1 S=5, got {other:?}"),
        }
        let wide = pair_record("1", "8");
        assert!(matches!(
            judge_measured(&measured(&wide, &[]), 1, 8),
            Judgment::Admit { .. }
        ));
        match judge_measured(&measured(&pair_record("1", "9"), &[]), 1, 8) {
            Judgment::Refuse { reasons, line, .. } => {
                assert_eq!(reasons, vec!["leg_rate_floor".to_owned()]);
                assert_eq!(line, leg_rate_line(1, 8));
            }
            other => panic!("1/8 should refuse N=1 S=9, got {other:?}"),
        }
    }

    #[test]
    fn an_object_settled_not_defined_pair_is_refused_without_a_floor_sentence() {
        let record = pair_record("\"not defined\"", "\"not defined\"");
        match judge_measured(&measured(&record, &["shape is false"]), 1, 5) {
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
                assert!(!line.contains("1/5"));
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
            match judge_measured(&measured(&record, &[]), 1, 5) {
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
        match judge_measured(&measured(&record, &["shape is false"]), 1, 5) {
            Judgment::Refuse {
                reasons,
                line,
                record: stored,
            } => {
                assert_eq!(
                    reasons,
                    vec!["leg_rate_floor".to_owned(), "shape is false".to_owned()]
                );
                assert!(line.contains("1/5"));
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

    #[test]
    fn the_bind_key_covers_the_whole_table() {
        let policy = AdmissionPolicy::default();
        let same = AdmissionPolicy::default();
        assert_eq!(policy.bind_key(), same.bind_key());
        let mut other = policy.clone();
        other.leg_rate_floor.q = 4;
        assert_ne!(policy.bind_key(), other.bind_key());
        let mut runs = policy.clone();
        runs.thorough_six_leg_runs = 1;
        assert_ne!(policy.bind_key(), runs.bind_key());
    }
}
