//! SPEC §15.4 policy file. The SDK has no policy reader. This module is the
//! gate's. A bad lever refuses the whole file. Keys outside `[levers]`, other
//! than `spec`, are the policy's (`policy_id`, `status`, `[policy_rules]`).

use crate::policy::{self, AdmissionPolicy, IndexPolicy, RateFloor};
use std::collections::BTreeSet;

const KNOWN: &[&str] = &[
    "min_anchored_paths",
    "leg_rate_floor",
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
    "submission_deadline_blocks",
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
    let policy_id = root
        .get("policy_id")
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| policy::DEFAULT_POLICY_ID.to_owned());
    let policy_id = policy::parse_policy_id(&policy_id)?;
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
    let submission_deadline_blocks = int_range(levers, "submission_deadline_blocks", 0, 6)?;
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
    let rules = root.get("policy_rules");
    let draw_commitment = rules
        .and_then(|table| table.get("draw_commitment"))
        .and_then(toml::Value::as_str)
        .unwrap_or("sha256(draw || salt), revealed with the result")
        .to_owned();
    // Absent means 0 so a partial test file still loads. The shipped file sets it.
    let born_height = match rules.and_then(|table| table.get("born_height")) {
        None => 0,
        Some(toml::Value::Integer(n)) if *n >= 0 => *n as u64,
        Some(toml::Value::Integer(_)) => return Err("born_height is out of range".to_owned()),
        Some(_) => return Err("born_height has the wrong type".to_owned()),
    };
    let expected = root
        .get("policy_rules")
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
        submission_deadline_blocks,
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

    fn complete(spec: &str) -> String {
        format!(
            "\
policy_id = \"gl-admit-v10\"
spec = \"{spec}\"
[levers]
min_anchored_paths = 6
leg_rate_floor = {{ p = 1, q = 5 }}
submission_deadline_blocks = 3
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
        for pin in ["spec-2.2.0", "spec-2.2", "spec-2.3.0", "spec-2.3"] {
            let loaded = load_for_pin(&raw, 2, pin).unwrap_or_else(|err| panic!("{pin}: {err}"));
            assert_eq!(loaded.policy_id, "gl-admit-v10", "{pin}");
            assert_eq!(loaded.min_anchored_paths, 6, "{pin}");
            assert_eq!(loaded.leg_rate_floor, RateFloor { p: 1, q: 5 }, "{pin}");
            assert_eq!(loaded.submission_deadline_blocks, 3, "{pin}");
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
            let value = serde_json::to_value(&loaded).expect("policy json");
            assert_eq!(value["verify_cap_per_door_session"], 3, "{pin}");
        }
        let live = load(&raw, 2).expect("compiled pin");
        assert_eq!(live.policy_id, "gl-admit-v10");
        assert_eq!(pcoc::build_info().spec_tag, "spec-2.2.0");
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
    fn each_formerly_defaulted_lever_is_named_when_absent() {
        let raw = complete("2.2");
        let keys = [
            "min_anchored_paths",
            "leg_rate_floor",
            "submission_deadline_blocks",
            "held_retry_seconds",
            "thorough_rounds",
            "challenge_lifetime_seconds",
            "challenge_clock_skew_seconds",
            "result_retention_seconds",
        ];
        for key in keys {
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
    fn an_absent_door_session_cap_is_unset() {
        let loaded = load_for_pin(&complete("2.2"), 2, "spec-2.2.0").expect("load");
        assert_eq!(loaded.verify_cap_per_door_session, None);
        let set = complete("2.2") + "verify_cap_per_door_session = 3\n";
        let loaded = load_for_pin(&set, 2, "spec-2.3.0").expect("set");
        assert_eq!(loaded.verify_cap_per_door_session, Some(3));
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
        assert_eq!(tag, "spec-2.2.0");
        assert_eq!(spec_identity(&tag).unwrap(), ("Satoshi Hike 2.2", "2.2"));
        let (name, label) = crate::verifier::health_spec();
        assert_eq!(name, "Satoshi Hike 2.2");
        assert_eq!(label, "2.2");
    }
}
