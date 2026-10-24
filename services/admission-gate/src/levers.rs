//! SPEC §15.4 policy file. The SDK has no policy reader. This module is the
//! gate's. A bad lever refuses the whole file. Keys outside `[levers]`, other
//! than `spec`, are the policy's (`policy_id`, `status`, `[policy_rules]`).
//! The loader reads `policy_id`, `draw_commitment`, `born_height`,
//! `expected_network`, and `sticky_refuse`. The first three have no unset
//! meaning and are required. An absent `expected_network` means no
//! index-network constraint. An absent `sticky_refuse` means false: a
//! resubmit of a refused pair is measured again. `status` and the other
//! `[policy_rules]` keys are not read.
//!
//! `submission_deadline_blocks` is retired. SPEC 2.3 has no Rule B10 and no
//! tip-versus-last-beacon deadline. The key is not required. If a file still
//! has it (gl-admit-v9, gl-admit-v10), the value is not read and is not
//! enforced. A key named `K` is not this name and is still an unknown lever.

use crate::policy::{self, AdmissionPolicy, IndexPolicy, RateFloor};
use std::collections::BTreeSet;

const KNOWN: &[&str] = &[
    "min_anchored_paths",
    "leg_rate_floor",
    // Retired. Kept so a file that still names it is not an unknown lever.
    // The value is ignored. See the module comment.
    "submission_deadline_blocks",
    "held_retry_seconds",
    "reverify_max_attempts",
    "max_held_blocks",
    "thorough_rounds",
    "provisional",
    "fast_accept_bound",
    "async_catch_budget",
    "verify_cap_per_object",
    "verify_cap_per_door_session",
    "fake_leg_ignore_seconds",
    "challenge_lifetime_seconds",
    "challenge_clock_skew_seconds",
    "result_retention_seconds",
];

/// Absent key refuses the file. No silent number is filled in.
/// `verify_cap_per_object`, `verify_cap_per_door_session`, and
/// `fake_leg_ignore_seconds` are not listed: an absent key means unset.
const REQUIRED: &[&str] = &[
    "min_anchored_paths",
    "leg_rate_floor",
    "held_retry_seconds",
    "reverify_max_attempts",
    "max_held_blocks",
    "thorough_rounds",
    "provisional",
    "fast_accept_bound",
    "async_catch_budget",
    "challenge_lifetime_seconds",
    "challenge_clock_skew_seconds",
    "result_retention_seconds",
];

/// Load a §15.4 file against the SDK pin in `sdk/SPEC_PIN`.
/// `max_concurrent` fills the gate's pending-global cap, which is not a lever.
pub fn load(raw: &str, max_concurrent: usize) -> Result<AdmissionPolicy, String> {
    load_for_pin(raw, max_concurrent, &pcoc::build_info().spec_tag)
}

/// Same as [`load`], with the pin tag supplied by the caller. Production
/// startup uses [`load`]. Tests pass `spec-2.2.x` and `spec-2.3.x` tags.
pub fn load_for_pin(
    raw: &str,
    max_concurrent: usize,
    pin: &str,
) -> Result<AdmissionPolicy, String> {
    let root: toml::Value =
        toml::from_str(raw).map_err(|err| format!("policy is not usable: {err}"))?;
    let spec = root
        .get("spec")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "policy spec must be the string \"2.2\" or \"2.3\"".to_owned())?;
    crate::verifier::accept_policy_spec(spec, pin)?;
    let levers = root
        .get("levers")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "policy has no [levers] table".to_owned())?;
    for key in levers.keys() {
        if !KNOWN.contains(&key.as_str()) {
            return Err(format!("unknown lever {key}"));
        }
    }
    let mut missing = Vec::new();
    for key in REQUIRED {
        if !levers.contains_key(*key) {
            missing.push(*key);
        }
    }
    if !missing.is_empty() {
        return Err(absent_levers(&missing));
    }
    let min_anchored_paths = int_range(levers, "min_anchored_paths", 2, 16)?;
    let (p, q) = floor(levers.get("leg_rate_floor"))?;
    // `submission_deadline_blocks` is in KNOWN and is not read.
    let held_retry_seconds = int_at_least(levers, "held_retry_seconds", 1)?;
    let reverify_max_attempts = int_at_least(levers, "reverify_max_attempts", 1)?;
    let max_held_blocks = int_at_least(levers, "max_held_blocks", 1)?;
    let thorough_rounds = int_at_least(levers, "thorough_rounds", 1)?;
    let provisional = match levers.get("provisional") {
        Some(toml::Value::String(text)) if text == "on" || text == "off" => text.clone(),
        Some(_) => return Err("provisional has the wrong type".to_owned()),
        None => return Err("lever provisional is absent and has no default".to_owned()),
    };
    let fast_accept_bound_ms = positive_seconds(levers, "fast_accept_bound")?;
    let async_catch_budget_ms = positive_seconds(levers, "async_catch_budget")?;
    let verify_cap_per_object = optional_unset_int(levers, "verify_cap_per_object")?;
    let verify_session = optional_unset_int(levers, "verify_cap_per_door_session")?;
    let fake_leg_ignore_seconds = optional_unset_int(levers, "fake_leg_ignore_seconds")?;
    let challenge_lifetime_seconds = int_at_least(levers, "challenge_lifetime_seconds", 1)?;
    let challenge_clock_skew_seconds = int_at_least(levers, "challenge_clock_skew_seconds", 0)?;
    let result_retention_seconds = int_at_least(levers, "result_retention_seconds", 0)?;
    let rules = match root.get("policy_rules") {
        None => None,
        Some(value) => Some(
            value
                .as_table()
                .ok_or_else(|| "policy_rules has the wrong type".to_owned())?,
        ),
    };
    let (policy_id, draw_commitment, born_height) = policy_keys(&root, rules)?;
    // Absent means no index-network constraint. Older policy ids omit the key
    // so their bind hash stays the same. It is not a filled-in network name.
    let expected = rules
        .and_then(|table| table.get("expected_network"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    if let Some(network) = &expected {
        if !policy::NETWORKS.contains(&network.as_str()) {
            return Err(format!(
                "expected_network {network:?} is not a known network"
            ));
        }
    }
    // Absent means off. Not a §15.4 lever, and not part of the bind hash.
    let sticky_refuse = match rules.and_then(|table| table.get("sticky_refuse")) {
        None => false,
        Some(toml::Value::Boolean(on)) => *on,
        Some(_) => return Err("sticky_refuse has the wrong type".to_owned()),
    };
    let pending_global =
        (max_concurrent as u64).saturating_mul(policy::DEFAULT_PENDING_GLOBAL_FACTOR);
    Ok(AdmissionPolicy {
        policy_id,
        leg_rate_floor: RateFloor { p, q },
        min_anchored_paths,
        require_first_anchored: None,
        require_last_anchored: None,
        thorough_six_leg_runs: thorough_rounds,
        thorough_early_stop: true,
        provisional,
        held_on_not_read: true,
        ban_key_on_fake_leg: true,
        draw_commitment,
        reverify_max_attempts,
        max_held_blocks,
        pending_per_source: policy::DEFAULT_PENDING_PER_SOURCE,
        pending_global,
        verify_cap_per_door_session: verify_session,
        held_retry_seconds,
        fast_accept_bound_ms,
        async_catch_budget_ms,
        verify_cap_per_object,
        fake_leg_ignore_seconds,
        challenge_lifetime_seconds,
        challenge_clock_skew_seconds,
        result_retention_seconds,
        born_height,
        index: expected.map(|expected_network| IndexPolicy { expected_network }),
        sticky_refuse,
    })
}

fn absent_levers(missing: &[&str]) -> String {
    if missing.len() == 1 {
        format!("lever {} is absent and has no default", missing[0])
    } else {
        format!(
            "levers {} are absent and have no default",
            missing.join(", ")
        )
    }
}

/// `policy_id`, `draw_commitment`, and `born_height` are required. A missing
/// key is named. A present value of the wrong type is a type error.
fn policy_keys(
    root: &toml::Value,
    rules: Option<&toml::map::Map<String, toml::Value>>,
) -> Result<(String, String, u64), String> {
    let mut missing = Vec::new();
    let policy_id = match root.get("policy_id") {
        None => {
            missing.push("policy_id");
            None
        }
        Some(toml::Value::String(text)) => Some(text.clone()),
        Some(_) => return Err("policy_id has the wrong type".to_owned()),
    };
    let draw_commitment = match rules.and_then(|table| table.get("draw_commitment")) {
        None => {
            missing.push("draw_commitment");
            None
        }
        Some(toml::Value::String(text)) => Some(text.clone()),
        Some(_) => return Err("draw_commitment has the wrong type".to_owned()),
    };
    let born_height = match rules.and_then(|table| table.get("born_height")) {
        None => {
            missing.push("born_height");
            None
        }
        Some(toml::Value::Integer(n)) if *n >= 0 => Some(*n as u64),
        Some(toml::Value::Integer(_)) => return Err("born_height is out of range".to_owned()),
        Some(_) => return Err("born_height has the wrong type".to_owned()),
    };
    if !missing.is_empty() {
        return Err(absent_policy_keys(&missing));
    }
    let (Some(policy_id), Some(draw_commitment), Some(born_height)) =
        (policy_id, draw_commitment, born_height)
    else {
        return Err(absent_policy_keys(&missing));
    };
    Ok((
        policy::parse_policy_id(&policy_id)?,
        draw_commitment,
        born_height,
    ))
}

fn absent_policy_keys(missing: &[&str]) -> String {
    if missing.len() == 1 {
        format!("{} is absent and has no default", missing[0])
    } else {
        format!("{} are absent and have no default", missing.join(", "))
    }
}

fn int_range(
    levers: &toml::map::Map<String, toml::Value>,
    key: &str,
    lo: u64,
    hi: u64,
) -> Result<u64, String> {
    let value = match levers.get(key) {
        None => return Err(format!("lever {key} is absent and has no default")),
        Some(toml::Value::Integer(n)) => {
            u64::try_from(*n).map_err(|_| format!("{key} is out of range"))?
        }
        Some(_) => return Err(format!("{key} has the wrong type")),
    };
    if value < lo || value > hi {
        return Err(format!("{key} is out of range"));
    }
    Ok(value)
}

fn int_at_least(
    levers: &toml::map::Map<String, toml::Value>,
    key: &str,
    lo: u64,
) -> Result<u64, String> {
    let value = match levers.get(key) {
        None => return Err(format!("lever {key} is absent and has no default")),
        Some(toml::Value::Integer(n)) => {
            u64::try_from(*n).map_err(|_| format!("{key} is out of range"))?
        }
        Some(_) => return Err(format!("{key} has the wrong type")),
    };
    if value < lo {
        return Err(format!("{key} is out of range"));
    }
    Ok(value)
}

fn floor(value: Option<&toml::Value>) -> Result<(u64, u64), String> {
    let Some(value) = value else {
        return Err("lever leg_rate_floor is absent and has no default".to_owned());
    };
    let Some(table) = value.as_table() else {
        return Err("leg_rate_floor has the wrong type".to_owned());
    };
    let mut keys: BTreeSet<&str> = table.keys().map(String::as_str).collect();
    if keys.len() != 2 || !keys.remove("p") || !keys.remove("q") || !keys.is_empty() {
        return Err("leg_rate_floor has the wrong type".to_owned());
    }
    let p = table
        .get("p")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| "leg_rate_floor has the wrong type".to_owned())?;
    let q = table
        .get("q")
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| "leg_rate_floor has the wrong type".to_owned())?;
    if p < 1 || q < 1 {
        return Err("leg_rate_floor is out of range".to_owned());
    }
    Ok((p as u64, q as u64))
}

/// Positive seconds, read as the nearest millisecond. Zero or less is out of range.
fn positive_seconds(
    levers: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<u64, String> {
    let Some(value) = levers.get(key) else {
        return Err(format!("lever {key} is absent and has no default"));
    };
    let seconds = match value {
        toml::Value::Integer(n) => *n as f64,
        toml::Value::Float(n) => {
            if !fraction_digits_ok(*n) {
                return Err(format!("{key} has more than 3 fractional digits"));
            }
            *n
        }
        _ => return Err(format!("{key} has the wrong type")),
    };
    if !seconds.is_finite() {
        return Err(format!("{key} is out of range"));
    }
    let ms = (seconds * 1000.0).round();
    if ms < 1.0 || ms > u64::MAX as f64 {
        return Err(format!("{key} is out of range"));
    }
    Ok(ms as u64)
}

fn fraction_digits_ok(n: f64) -> bool {
    if !n.is_finite() {
        return false;
    }
    let scaled = (n * 1000.0).round();
    (scaled / 1000.0 - n).abs() < 1e-9
}

/// Absent means unset. A present value must be an integer ≥ 1.
fn optional_unset_int(
    levers: &toml::map::Map<String, toml::Value>,
    key: &str,
) -> Result<Option<u64>, String> {
    match levers.get(key) {
        None => Ok(None),
        Some(toml::Value::Integer(n)) if *n >= 1 => Ok(Some(*n as u64)),
        Some(toml::Value::Integer(_)) => Err(format!("{key} is out of range")),
        Some(_) => Err(format!("{key} has the wrong type")),
    }
}

/// Used by the refusal tests so a bad document is a string the loader sees.
pub fn refuse_reason(raw: &str) -> Result<(), String> {
    load(raw, 2).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::RateFloor;
    use crate::verifier::spec_identity;

    fn v10_text() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../policy/groundinglab-admission/gl-admit-v10.toml");
        std::fs::read_to_string(path).expect("gl-admit-v10.toml")
    }

    fn v11_text() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../policy/groundinglab-admission/gl-admit-v11.toml");
        std::fs::read_to_string(path).expect("gl-admit-v11.toml")
    }

    fn complete(spec: &str) -> String {
        format!(
            "\
policy_id = \"gl-admit-v10\"
spec = \"{spec}\"
[levers]
min_anchored_paths = 6
leg_rate_floor = {{ p = 1, q = 5 }}
held_retry_seconds = 600
thorough_rounds = 3
challenge_lifetime_seconds = 120
challenge_clock_skew_seconds = 5
result_retention_seconds = 600
reverify_max_attempts = 3
max_held_blocks = 6
provisional = \"on\"
fast_accept_bound = 2
async_catch_budget = 22.05
[policy_rules]
draw_commitment = \"sha256(draw || salt), revealed with the result\"
born_height = 972685
"
        )
    }

    fn without(raw: &str, key: &str) -> String {
        raw.lines()
            .filter(|line| {
                let rest = line.trim().strip_prefix(key);
                !rest.is_some_and(|rest| rest.trim_start().starts_with('='))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn v10_loads_on_a_2_2_pin_and_a_2_3_pin() {
        let raw = v10_text();
        assert!(
            !raw.lines().any(|line| {
                let line = line.trim();
                !line.starts_with('#') && line.starts_with("sticky_refuse")
            }),
            "gl-admit-v10.toml stays pinned; an absent sticky_refuse means off"
        );
        for pin in ["spec-2.2.0", "spec-2.2", "spec-2.3.0", "spec-2.3"] {
            let loaded = load_for_pin(&raw, 2, pin).unwrap_or_else(|err| panic!("{pin}: {err}"));
            assert_eq!(loaded.policy_id, "gl-admit-v10", "{pin}");
            assert_eq!(loaded.min_anchored_paths, 6, "{pin}");
            assert_eq!(loaded.leg_rate_floor, RateFloor { p: 1, q: 5 }, "{pin}");
            let table = serde_json::to_value(&loaded).expect("policy json");
            assert!(
                table.get("submission_deadline_blocks").is_none(),
                "{pin}: retired lever was stored"
            );
            assert_eq!(loaded.held_retry_seconds, 600, "{pin}");
            assert_eq!(loaded.thorough_six_leg_runs, 3, "{pin}");
            assert_eq!(loaded.challenge_lifetime_seconds, 120, "{pin}");
            assert_eq!(loaded.challenge_clock_skew_seconds, 5, "{pin}");
            assert_eq!(loaded.result_retention_seconds, 600, "{pin}");
            assert_eq!(loaded.reverify_max_attempts, 3, "{pin}");
            assert_eq!(loaded.max_held_blocks, 6, "{pin}");
            assert_eq!(loaded.provisional, "on", "{pin}");
            assert_eq!(loaded.fast_accept_bound_ms, 2_000, "{pin}");
            assert_eq!(loaded.async_catch_budget_ms, 22_050, "{pin}");
            assert_eq!(loaded.verify_cap_per_door_session, Some(3), "{pin}");
            assert_eq!(loaded.verify_cap_per_object, None, "{pin}");
            assert_eq!(loaded.fake_leg_ignore_seconds, None, "{pin}");
            assert_eq!(loaded.born_height, 972685, "{pin}");
            assert!(!loaded.sticky_refuse, "{pin}");
            assert_eq!(
                loaded.draw_commitment, "sha256(draw || salt), revealed with the result",
                "{pin}"
            );
            let value = serde_json::to_value(&loaded).expect("policy json");
            assert_eq!(value["verify_cap_per_door_session"], 3, "{pin}");
            assert!(value.get("sticky_refuse").is_none(), "{pin}");
        }
        let live = load(&raw, 2).expect("compiled pin");
        assert_eq!(live.policy_id, "gl-admit-v10");
        assert!(!live.sticky_refuse);
        assert_eq!(pcoc::build_info().spec_tag, "spec-2.3.0");
    }

    /// gl-admit-v11 sets the testing switch off. `true` is the re-enable.
    /// The value is not part of the bind hash. A non-boolean refuses the file.
    #[test]
    fn v11_sticky_refuse_defaults_off_and_stays_out_of_the_bind_hash() {
        let raw = v11_text();
        assert!(
            raw.lines().any(|line| {
                let line = line.trim();
                !line.starts_with('#')
                    && line.split_whitespace().collect::<Vec<_>>().join(" ")
                        == "sticky_refuse = false"
            }),
            "gl-admit-v11.toml must keep sticky_refuse = false"
        );
        let loaded = load_for_pin(&raw, 2, "spec-2.3.0").expect("v11");
        assert_eq!(loaded.policy_id, "gl-admit-v11");
        assert_eq!(loaded.leg_rate_floor, RateFloor { p: 1, q: 10 });
        assert!(!loaded.sticky_refuse);
        assert!(serde_json::to_value(&loaded)
            .expect("policy json")
            .get("sticky_refuse")
            .is_none());
        let enabled = load_for_pin(&with_sticky(&raw, "true"), 2, "spec-2.3.0").expect("sticky on");
        assert!(enabled.sticky_refuse);
        assert_eq!(loaded.bind_key(), enabled.bind_key());
        let err = load_for_pin(&with_sticky(&raw, "\"off\""), 2, "spec-2.3.0").expect_err("type");
        assert!(
            err.contains("sticky_refuse") && err.contains("wrong type"),
            "{err}"
        );
        let absent = raw
            .lines()
            .filter(|line| !line.trim_start().starts_with("sticky_refuse"))
            .collect::<Vec<_>>()
            .join("\n");
        let omitted = load_for_pin(&absent, 2, "spec-2.3.0").expect("absent");
        assert!(!omitted.sticky_refuse);
    }

    fn with_sticky(raw: &str, value: &str) -> String {
        raw.lines()
            .map(|line| {
                if !line.trim_start().starts_with('#')
                    && line.trim_start().starts_with("sticky_refuse")
                {
                    format!("sticky_refuse = {value}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn spec_2_3_loads_on_a_2_3_pin_and_is_rejected_on_a_2_2_pin() {
        let raw = complete("2.3");
        assert!(load_for_pin(&raw, 2, "spec-2.3.0").is_ok());
        assert!(load_for_pin(&raw, 2, "spec-2.3").is_ok());
        for pin in ["spec-2.2.0", "spec-2.2"] {
            let err = load_for_pin(&raw, 2, pin).expect_err(pin);
            assert!(err.contains("2.3"), "{pin}: {err}");
            assert!(err.contains(pin), "{pin}: {err}");
        }
    }

    #[test]
    fn each_required_lever_is_named_when_absent() {
        let raw = complete("2.2");
        assert_eq!(REQUIRED.len(), 12);
        for key in REQUIRED {
            let stripped = without(&raw, key);
            assert_ne!(stripped, raw, "{key} was not removed");
            let err = load_for_pin(&stripped, 2, "spec-2.2.0").expect_err(key);
            assert!(err.contains(key), "{key}: {err}");
            assert!(err.contains("absent"), "{key}: {err}");
        }
        let both = without(&without(&raw, "min_anchored_paths"), "held_retry_seconds");
        let err = load_for_pin(&both, 2, "spec-2.3.0").expect_err("two levers");
        assert!(err.contains("min_anchored_paths"), "{err}");
        assert!(err.contains("held_retry_seconds"), "{err}");
    }

    /// SPEC 2.3 retired the lever. Omitting it loads. A present value is not
    /// read, whatever its type or range. The alias `K` is still unknown.
    #[test]
    fn retired_submission_deadline_is_ignored() {
        let raw = complete("2.3");
        assert!(
            !raw.contains("submission_deadline_blocks"),
            "the fixture still requires the retired lever"
        );
        let omitted = load_for_pin(&raw, 2, "spec-2.3.0").expect("omit");
        assert!(serde_json::to_value(&omitted)
            .expect("json")
            .get("submission_deadline_blocks")
            .is_none());
        for value in ["0", "3", "6", "7", "\"K\"", "{ p = 1 }"] {
            let with = raw.replace(
                "[policy_rules]",
                &format!("submission_deadline_blocks = {value}\n[policy_rules]"),
            );
            let loaded =
                load_for_pin(&with, 2, "spec-2.3.0").unwrap_or_else(|err| panic!("{value}: {err}"));
            assert!(
                serde_json::to_value(&loaded)
                    .expect("json")
                    .get("submission_deadline_blocks")
                    .is_none(),
                "{value}"
            );
        }
        let alias = raw.replace("[policy_rules]", "K = 3\n[policy_rules]");
        let err = load_for_pin(&alias, 2, "spec-2.3.0").expect_err("K");
        assert!(err.contains("unknown lever"), "{err}");
        assert!(err.contains('K'), "{err}");
    }

    #[test]
    fn unknown_spec_strings_and_unknown_pins_are_rejected() {
        for spec in ["2.1", "2.4", "3.0", "", "2.2.0"] {
            let err = load_for_pin(&complete(spec), 2, "spec-2.3.0").expect_err(spec);
            assert!(err.contains("2.2") && err.contains("2.3"), "{spec}: {err}");
            if !spec.is_empty() {
                assert!(err.contains(spec), "{spec}: {err}");
            }
        }
        for pin in [
            "spec-2.1.0",
            "spec-2.4.0",
            "spec-3.0.0",
            "spec-2.20",
            "spec-2.30",
            "",
        ] {
            let err = load_for_pin(&complete("2.2"), 2, pin).expect_err(pin);
            assert!(err.contains("pin"), "{pin}: {err}");
        }
        let err = load_for_pin(&complete("2.3"), 2, "not-a-pin").expect_err("pin");
        assert!(err.contains("not-a-pin"), "{err}");
    }

    #[test]
    fn absent_unset_levers_stay_unset() {
        let loaded = load_for_pin(&complete("2.2"), 2, "spec-2.2.0").expect("load");
        assert_eq!(loaded.verify_cap_per_object, None);
        assert_eq!(loaded.verify_cap_per_door_session, None);
        assert_eq!(loaded.fake_leg_ignore_seconds, None);
        let set = complete("2.3").replace(
            "[policy_rules]",
            "verify_cap_per_object = 2\n\
             verify_cap_per_door_session = 3\n\
             fake_leg_ignore_seconds = 60\n\
             [policy_rules]",
        );
        let loaded = load_for_pin(&set, 2, "spec-2.3.0").expect("set");
        assert_eq!(loaded.verify_cap_per_object, Some(2));
        assert_eq!(loaded.verify_cap_per_door_session, Some(3));
        assert_eq!(loaded.fake_leg_ignore_seconds, Some(60));
    }

    #[test]
    fn each_required_policy_key_is_named_when_absent() {
        let raw = complete("2.2");
        for key in ["policy_id", "draw_commitment", "born_height"] {
            let stripped = without(&raw, key);
            assert_ne!(stripped, raw, "{key} was not removed");
            let err = load_for_pin(&stripped, 2, "spec-2.3.0").expect_err(key);
            assert!(err.contains(key), "{key}: {err}");
            assert!(err.contains("absent"), "{key}: {err}");
        }
        let raw = complete("2.2");
        let id = raw.replace("policy_id = \"gl-admit-v10\"", "policy_id = 10");
        let err = load_for_pin(&id, 2, "spec-2.2.0").expect_err("policy_id type");
        assert!(
            err.contains("policy_id") && err.contains("wrong type"),
            "{err}"
        );
        let draw = raw.replace(
            "draw_commitment = \"sha256(draw || salt), revealed with the result\"",
            "draw_commitment = 1",
        );
        let err = load_for_pin(&draw, 2, "spec-2.2.0").expect_err("draw type");
        assert!(
            err.contains("draw_commitment") && err.contains("wrong type"),
            "{err}"
        );
        let born = raw.replace("born_height = 972685", "born_height = \"972685\"");
        let err = load_for_pin(&born, 2, "spec-2.2.0").expect_err("born type");
        assert!(
            err.contains("born_height") && err.contains("wrong type"),
            "{err}"
        );
        let born = raw.replace("born_height = 972685", "born_height = -1");
        let err = load_for_pin(&born, 2, "spec-2.2.0").expect_err("born range");
        assert!(
            err.contains("born_height") && err.contains("out of range"),
            "{err}"
        );
    }

    #[test]
    fn v9_omits_born_height_and_is_refused_on_both_pins() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../policy/groundinglab-admission/gl-admit-v9.toml");
        let raw = std::fs::read_to_string(path).expect("gl-admit-v9.toml");
        assert!(
            !raw.lines().any(|line| {
                let line = line.trim();
                !line.starts_with('#') && line.starts_with("born_height")
            }),
            "gl-admit-v9.toml gained a born_height key"
        );
        for pin in ["spec-2.2.0", "spec-2.3.0"] {
            let err = load_for_pin(&raw, 2, pin).expect_err(pin);
            assert!(err.contains("born_height"), "{pin}: {err}");
            assert!(err.contains("absent"), "{pin}: {err}");
        }
        // The on-disk file is not edited. An explicit 0 is the old cutoff,
        // and that text still matches both pins.
        let with_born = format!("{raw}\nborn_height = 0\n");
        for pin in ["spec-2.2.0", "spec-2.3.0"] {
            let loaded =
                load_for_pin(&with_born, 2, pin).unwrap_or_else(|err| panic!("{pin}: {err}"));
            assert_eq!(loaded.policy_id, "gl-admit-v9", "{pin}");
            assert_eq!(loaded.born_height, 0, "{pin}");
            assert_eq!(
                loaded.draw_commitment, "sha256(draw || salt), revealed with the result",
                "{pin}"
            );
        }
    }

    #[test]
    fn health_name_follows_the_pin() {
        assert_eq!(
            spec_identity("spec-2.2.0").unwrap(),
            ("Satoshi Hike 2.2", "2.2")
        );
        assert_eq!(
            spec_identity("spec-2.2").unwrap(),
            ("Satoshi Hike 2.2", "2.2")
        );
        assert_eq!(
            spec_identity("spec-2.3.0").unwrap(),
            ("Satoshi Hike 2.3", "2.3")
        );
        assert_eq!(
            spec_identity("spec-2.3").unwrap(),
            ("Satoshi Hike 2.3", "2.3")
        );
        assert!(spec_identity("spec-2.4.0").is_err());
        assert!(spec_identity("spec-2.20").is_err());
        assert_eq!(crate::verifier::SPEC_KIND, "v2.2-pcoc-object");
        let tag = pcoc::build_info().spec_tag;
        assert_eq!(tag, "spec-2.3.0");
        assert_eq!(spec_identity(&tag).unwrap(), ("Satoshi Hike 2.3", "2.3"));
        let (name, label) = crate::verifier::health_spec();
        assert_eq!(name, "Satoshi Hike 2.3");
        assert_eq!(label, "2.3");
    }
}
