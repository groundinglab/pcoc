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

/// Absent key refuses the file. SPEC §15.4 default "none".
const REQUIRED: &[&str] = &[
    "reverify_max_attempts",
    "max_held_blocks",
    "provisional",
    "fast_accept_bound",
    "async_catch_budget",
];

/// SPEC §15.4 default of `verify_cap_per_door_session` is unset. This constant
/// applies only when the key is absent. `gl-admit-v10.toml` sets the key to 3,
/// and `load` reads that value.
pub const VERIFY_CAP_PER_DOOR_SESSION_AWAITING_POLICY: u64 = 3;

/// The door-session cap from the file. An absent key uses the constant above.
pub fn door_session_cap(parsed: Option<u64>) -> u64 {
    parsed.unwrap_or(VERIFY_CAP_PER_DOOR_SESSION_AWAITING_POLICY)
}

/// Load a §15.4 file. `max_concurrent` fills the gate's pending-global cap,
/// which is not a lever.
pub fn load(raw: &str, max_concurrent: usize) -> Result<AdmissionPolicy, String> {
    let root: toml::Value =
        toml::from_str(raw).map_err(|err| format!("policy is not usable: {err}"))?;
    let spec = root
        .get("spec")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| "policy spec must be the string \"2.2\"".to_owned())?;
    if spec != "2.2" {
        return Err(format!("policy spec {spec:?} is not \"2.2\""));
    }
    let pin = pcoc::build_info().spec_tag;
    if !spec_matches_pin(spec, &pin) {
        return Err(format!("policy spec {spec:?} is not the SDK pin {pin}"));
    }
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
    for key in REQUIRED {
        if !levers.contains_key(*key) {
            return Err(format!("lever {key} is absent and has no default"));
        }
    }
    let min_anchored_paths = int_range(levers, "min_anchored_paths", 6, 2, 16)?;
    let (p, q) = floor(levers.get("leg_rate_floor"))?;
    let submission_deadline_blocks = int_range(levers, "submission_deadline_blocks", 3, 0, 6)?;
    let held_retry_seconds = int_at_least(levers, "held_retry_seconds", 600, 1)?;
    let reverify_max_attempts = int_at_least(levers, "reverify_max_attempts", 0, 1)?;
    let max_held_blocks = int_at_least(levers, "max_held_blocks", 0, 1)?;
    let thorough_rounds = int_at_least(levers, "thorough_rounds", 3, 1)?;
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
    let challenge_lifetime_seconds = int_at_least(levers, "challenge_lifetime_seconds", 120, 1)?;
    let challenge_clock_skew_seconds = int_at_least(levers, "challenge_clock_skew_seconds", 5, 0)?;
    let result_retention_seconds = int_at_least(levers, "result_retention_seconds", 600, 0)?;
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
        verify_cap_per_door_session: door_session_cap(verify_session),
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

/// `spec` is `"2.2"` and that is the version named by the SDK pin tag.
fn spec_matches_pin(spec: &str, tag: &str) -> bool {
    spec == "2.2" && (tag == "spec-2.2" || tag.starts_with("spec-2.2."))
}

fn int_range(
    levers: &toml::map::Map<String, toml::Value>,
    key: &str,
    default: u64,
    lo: u64,
    hi: u64,
) -> Result<u64, String> {
    let value = match levers.get(key) {
        None => default,
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
    default: u64,
    lo: u64,
) -> Result<u64, String> {
    let value = match levers.get(key) {
        None => default,
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
        return Ok((1, 5));
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
