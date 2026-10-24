//! Ticket 386 AC1–AC5 and ticket 383 AC2b.
//!
//! Policy numbers come from the gate. Measures come from the verifier.
//! A return line skips measurement. An unknown tip is null.

#![forbid(unsafe_code)]

use pcoc_admission::log::{DecisionLog, DecisionWrite};
use pcoc_admission::policy::{self, AdmissionPolicy};
use pcoc_admission::reasons::{self, Reason};
use pcoc_admission::verifier::{Measures, MEASURE_THRESHOLD_KEYS};
use pcoc_admission::{Config, Door};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn scratch(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "pcoc-gate-policy-{label}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn measures(rate: f64, waypoints: u64) -> Measures {
    let span_s = 100u64;
    let legs = (rate * span_s as f64).round() as u64;
    Measures {
        legs: Some(legs),
        span_s: Some(span_s),
        leg_rate: Some(rate),
        waypoints_fetched: Some(waypoints),
        waypoints_total: 18,
        paths_included: Some(0),
        legs_recomputed_ok: Some(6),
    }
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy::default()
}

fn write_line(log: &mut DecisionLog, write: &DecisionWrite) -> Value {
    log.append(write).unwrap();
    log.lines().last().unwrap().value.clone()
}

/// AC1. Measures hold verifier numbers. A threshold key fails the schema.
#[test]
fn ac1_measures_hold_no_threshold() {
    let sample = measures(0.25, 6);
    let value = sample.to_json();
    let object = value.as_object().unwrap();
    for key in MEASURE_THRESHOLD_KEYS {
        assert!(object.get(*key).is_none(), "{key} leaked into measures");
    }
    assert_eq!(object.len(), 7);
    assert_eq!(value["waypoints_total"], 18);
    assert!(value["leg_rate"].as_f64().unwrap() >= 0.25);
    let text = value.to_string();
    assert!(!text.contains("min_leg_rate"));
    assert!(!text.contains("policy_id"));
    assert!(!text.contains("threshold"));
}

/// A configured p/q is applied. 1/5 refuses 19 hike legs over a span of 100 and admits 20.
/// Five anchored of sixteen refuses. Six admits.
#[test]
fn ac2_configured_floor_is_enforced() {
    let policy = policy();
    let slow = measures(0.19, 6);
    let checks = policy::policy_checks(&slow, &policy);
    assert!(
        !checks
            .iter()
            .find(|check| check.field == "leg_rate")
            .unwrap()
            .met
    );
    let reason = policy_miss(&checks, &policy);
    assert_eq!(reason.class, reasons::CLASS_BELOW_POLICY);
    assert_eq!(reason.code, reasons::CODE_BELOW_LEG_RATE);

    let few = measures(0.25, 5);
    let checks = policy::policy_checks(&few, &policy);
    assert!(
        checks
            .iter()
            .find(|check| check.field == "leg_rate")
            .unwrap()
            .met
    );
    assert!(
        !checks
            .iter()
            .find(|check| check.field == "anchored_total")
            .unwrap()
            .met
    );
    let reason = policy_miss(&checks, &policy);
    assert_eq!(reason.class, reasons::CLASS_BELOW_POLICY);
    assert_eq!(reason.code, reasons::CODE_BELOW_WAYPOINTS);

    let ok = measures(0.20, 6);
    let checks = policy::policy_checks(&ok, &policy);
    assert!(checks.iter().all(|check| check.met));

    let source =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/door.rs"))
            .unwrap();
    assert!(
        !source.contains("enforces_waypoint_bar"),
        "the hardcoded waypoint bar flag is gone"
    );
}

fn policy_miss(checks: &[policy::PolicyCheck], policy: &AdmissionPolicy) -> Reason {
    let report = pcoc_admission::verifier::Report::Read(pcoc_admission::verifier::Measured {
        earlier_version: false,
        kind: "satoshi-hike-v2.0-object".to_owned(),
        record: "{}".to_owned(),
        verifier_notes: Vec::new(),
        measures: measures(0.25, 6),
    });
    reasons::from_report(&report, checks, policy)
}

/// AC3. A new decision line carries policy_id and policy_checks.
#[test]
fn ac3_every_new_line_names_its_policy() {
    let dir = scratch("ac3");
    let mut log = DecisionLog::open(&dir).unwrap();
    let sample = measures(0.25, 6);
    let checks = policy::policy_checks(&sample, &policy());
    let mut write = DecisionWrite::bare(1_700_000_000, None, "thorough", "accept");
    write.object = b"object".to_vec();
    write.object_sha256 = pcoc_admission::canon::sha256(&write.object);
    write.summary = None;
    write.measures = Some(sample.to_json());
    write.policy_id = Some(policy().policy_id);
    write.policy_checks = Some(json!(checks
        .iter()
        .map(|check| json!({"field": check.field, "min": check.min, "met": check.met}))
        .collect::<Vec<_>>()));
    let line = write_line(&mut log, &write);
    assert_eq!(line["policy_id"], "gl-admit-v10");
    assert_eq!(line["policy_checks"][0]["field"], "leg_rate");
    assert_eq!(line["policy_checks"][0]["min"]["p"], 1);
    assert_eq!(line["policy_checks"][0]["min"]["q"], 5);
    assert_eq!(line["policy_checks"][0]["met"], true);
    assert_eq!(line["policy_checks"][1]["field"], "anchored_total");
    assert_eq!(line["policy_checks"][1]["min"], 6);
    assert!(line.get("verify_summary").is_none(), "{line}");
    assert_eq!(line["measures"]["waypoints_total"], 18);
    assert!(line["bitcoin_tip_height"].is_null(), "{line}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC4. Every inventory code is one snake_case reason, and rejected is never written.
#[test]
fn ac4_one_reason_code_per_refusal() {
    let dir = scratch("ac4");
    let mut log = DecisionLog::open(&dir).unwrap();
    for (code, class) in reasons::INVENTORY {
        assert_ne!(*code, "rejected");
        let mut write = DecisionWrite::bare(1_700_000_000, None, "thorough", "refuse");
        write.object = code.as_bytes().to_vec();
        write.object_sha256 = pcoc_admission::canon::sha256(&write.object);
        write.summary = None;
        write.reason_code = Some((*code).to_owned());
        write.class = Some((*class).to_owned());
        write.reason = Some(format!("plain {code}"));
        let line = write_line(&mut log, &write);
        assert_eq!(line["reason_code"], *code, "{line}");
        assert_eq!(line["class"], *class, "{line}");
        assert_ne!(line["reason_code"], "rejected");
        let raw = String::from_utf8(log.lines().last().unwrap().raw.clone()).unwrap();
        assert!(!raw.contains("\"reason\":\"rejected\""), "{raw}");
        assert!(!raw.contains("\"reason_code\":\"rejected\""), "{raw}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC5. An unknown tip is null. Public health omits log_dir and config_path.
#[test]
fn ac5_unknown_tip_is_null_and_health_hides_paths() {
    let dir = scratch("ac5");
    let mut log = DecisionLog::open(&dir).unwrap();
    let mut write = DecisionWrite::bare(1_700_000_000, None, "thorough", "accept");
    write.object = b"tip".to_vec();
    write.object_sha256 = pcoc_admission::canon::sha256(&write.object);
    let line = write_line(&mut log, &write);
    assert!(line["bitcoin_tip_height"].is_null(), "{line}");
    assert_ne!(line["bitcoin_tip_height"], 0);

    let cfg = Config::for_tests(dir.clone(), "http://127.0.0.1:9");
    let door = Door::open(
        &cfg.log_dir,
        cfg.policy(),
        &cfg.index_url,
        cfg.index_timeout,
        cfg.served_origins.clone(),
    )
    .unwrap();
    let _ = door;
    let health = serde_json::to_value(json!({
        "config": {
            "bind": cfg.bind.to_string(),
            "policy_id": cfg.policy.policy_id,
        }
    }))
    .unwrap();
    let rendered = format!("{health}");
    assert!(!rendered.contains("log_dir"));
    assert!(!rendered.contains("config_path"));
    let source =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/http.rs"))
            .unwrap();
    let health_fn = source
        .split("async fn health")
        .nth(1)
        .unwrap()
        .split("async fn ")
        .next()
        .unwrap();
    assert!(!health_fn.contains("log_dir"));
    assert!(!health_fn.contains("config_path"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// An old line with a numeric tip and verify_summary still opens.
#[test]
fn an_old_numeric_tip_still_opens() {
    let dir = scratch("old-tip");
    let mut log = DecisionLog::open(&dir).unwrap();
    let mut write = DecisionWrite::bare(1_700_000_000, Some(4), "fast", "provisional");
    write.object = b"old".to_vec();
    write.object_sha256 = pcoc_admission::canon::sha256(&write.object);
    let line = write_line(&mut log, &write);
    assert_eq!(line["bitcoin_tip_height"], 4);
    assert!(line.get("measures").is_none());
    assert_eq!(line["verify_summary"], json!({}));
    drop(log);
    let reopened = DecisionLog::open(&dir).unwrap();
    assert_eq!(reopened.lines().len(), 1);
    assert_eq!(reopened.lines()[0].value["bitcoin_tip_height"], 4);
    let _ = std::fs::remove_dir_all(&dir);
}

/// AC2b. Return, echo, a new key, and a different object.
#[test]
fn ac2b_return_line_skips_measurement() {
    let dir = scratch("ac2b");
    let cfg = Config::for_tests(dir.clone(), "http://127.0.0.1:9");
    let door = Door::open(
        &cfg.log_dir,
        cfg.policy(),
        &cfg.index_url,
        cfg.index_timeout,
        cfg.served_origins.clone(),
    )
    .unwrap();
    let now = 1_800_000_000u64;
    door.set_now_unix_s(Some(now));

    let key = [9u8; 32];
    let other_key = [8u8; 32];
    let object = [4u8; 32];
    let other_object = [5u8; 32];

    // Seed a thorough accept with policy_id by appending through the same log
    // the door holds. The door's log is private, so the seed is a second open
    // only before the door starts. Write it first, then open.
    drop(door);
    {
        let mut log = DecisionLog::open(&dir).unwrap();
        let mut accept = DecisionWrite::bare(now - 10, None, "thorough", "accept");
        accept.public_key = key;
        accept.object = b"same-object".to_vec();
        accept.object_sha256 = object;
        accept.reasons = vec!["accepted".to_owned()];
        accept.summary = None;
        accept.measures = Some(measures(0.25, 6).to_json());
        accept.policy_id = Some("gl-admit-v10".to_owned());
        accept.policy_checks = Some(json!([
            {"field": "leg_rate", "min": {"p": 1, "q": 5}, "met": true},
            {"field": "waypoints_fetched", "min": 6, "met": true}
        ]));
        log.append(&accept).unwrap();
    }
    let door = Door::open(
        &cfg.log_dir,
        cfg.policy(),
        &cfg.index_url,
        cfg.index_timeout,
        cfg.served_origins.clone(),
    )
    .unwrap();
    door.set_now_unix_s(Some(now));

    // (a) same pair writes one return, with the accept ref, and no measurement.
    let job = thorough(&door, key, object, 1);
    door.consume_for_test(&job.nonce, object, now);
    assert!(job.prior_accept.is_some());
    door.settle_locked(job, Err("must not be read".to_owned()));
    let lines = chain(&dir);
    assert_eq!(lines.len(), 2, "{lines:?}");
    let ret = &lines[1];
    assert_eq!(ret["decision"], "return");
    assert_eq!(ret["phase"], "return");
    assert_eq!(ret["policy_id"], "gl-admit-v10");
    assert_eq!(ret["access_ref_seq"], 0);
    assert!(ret.get("measures").is_none(), "{ret}");
    assert!(ret.get("legs").is_none(), "{ret}");
    assert!(ret.get("span_s").is_none(), "{ret}");
    assert!(ret["bitcoin_tip_height"].is_null());

    // (d) a second sign-in inside 300 s echoes and writes no new line.
    let again = thorough(&door, key, object, 2);
    door.set_now_unix_s(Some(now + 1));
    door.consume_for_test(&again.nonce, object, now + 1);
    door.settle_locked(again, Err("echo must not measure".to_owned()));
    assert_eq!(chain(&dir).len(), 2);

    // (b) a new key writes no return line. It falls through. The fixture
    // object is not a v2.0 proof, so the line is a refusal, not a return.
    door.set_now_unix_s(Some(now + 2));
    let fresh = thorough(&door, other_key, object, 3);
    assert!(fresh.prior_accept.is_none());

    // (c) a different object is not a return.
    let different = thorough(&door, key, other_object, 4);
    assert!(
        different.prior_accept.is_none(),
        "a different object is measured"
    );

    // A prior accept with no policy_id opens nothing.
    drop(door);
    {
        let mut log = DecisionLog::open(&dir).unwrap();
        let mut bare = DecisionWrite::bare(now - 20, Some(3), "thorough", "accept");
        bare.public_key = [7u8; 32];
        bare.object_sha256 = [6u8; 32];
        bare.object = b"old".to_vec();
        log.append(&bare).unwrap();
    }
    let door = Door::open(
        &cfg.log_dir,
        cfg.policy(),
        &cfg.index_url,
        cfg.index_timeout,
        cfg.served_origins.clone(),
    )
    .unwrap();
    let old = thorough(&door, [7u8; 32], [6u8; 32], 5);
    assert!(
        old.prior_accept.is_none(),
        "a line without policy_id opens nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn thorough(
    door: &Door,
    key: [u8; 32],
    object_sha: [u8; 32],
    nonce_byte: u8,
) -> pcoc_admission::door::LockedThorough {
    static N: AtomicU64 = AtomicU64::new(1);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let mut nonce = [0u8; 32];
    nonce[0] = nonce_byte;
    nonce[1] = n as u8;
    pcoc_admission::door::LockedThorough {
        nonce,
        origin: "https://groundinglab.io".to_owned(),
        object: b"body".to_vec(),
        submission: [2u8; 32],
        public_key: key,
        object_sha,
        prior_accept: door.prior_accept_ref(&key, &object_sha),
        pending_epoch: 0,
        flush_ticket: 0,
    }
}

fn chain(dir: &std::path::Path) -> Vec<Value> {
    let log = DecisionLog::open(dir).unwrap();
    log.lines().iter().map(|line| line.value.clone()).collect()
}
