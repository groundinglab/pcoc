//! AC-A11. Fast phase, OS-CSPRNG draw, and the §15.4 policy file.
//!
//! Nothing here fills a production terrain or dials a live index.

#![forbid(unsafe_code)]

use pcoc_admission::fast::FastDecision;
use pcoc_admission::levers;
use pcoc_admission::thorough;
use std::path::PathBuf;
use std::time::Duration;

fn policy_text() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../policy/groundinglab-admission/gl-admit-v9.toml");
    std::fs::read_to_string(path).expect("gl-admit-v9.toml")
}

fn other_kind() -> Vec<u8> {
    fn text(value: &str) -> Vec<u8> {
        let bytes = value.as_bytes();
        let mut out = vec![0x78, u8::try_from(bytes.len()).unwrap()];
        out.extend_from_slice(bytes);
        out
    }
    let mut out = vec![0xa2];
    out.extend(text("kind"));
    out.extend(text("not-a-pcoc-object"));
    out.extend(text("public_key"));
    out.push(0x58);
    out.push(32);
    out.extend_from_slice(&[9u8; 32]);
    out
}

#[test]
fn other_kind_is_unknown_kind_from_the_fast_phase() {
    let policy = levers::load(&policy_text(), 2).expect("v9");
    let index = pcoc_admission::verifier::IndexClient::open("http://127.0.0.1:9", Duration::from_secs(1));
    match pcoc_admission::fast::decide(&other_kind(), &policy, &index) {
        FastDecision::Refuse { reasons, website, .. } => {
            assert_eq!(reasons, vec!["unknown_kind".to_owned()]);
            assert_eq!(website, "rejected");
        }
        other => panic!("fast phase did not refuse: {other:?}"),
    }
}

#[test]
fn two_thorough_draws_differ() {
    let first = thorough::os_draw(64, 3).expect("draw");
    let second = thorough::os_draw(64, 3).expect("draw");
    assert_ne!(first, second);
    assert!(first.iter().all(|run| run.len() == 6));
}

#[test]
fn gl_admit_v9_loads() {
    let policy = levers::load(&policy_text(), 2).expect("v9");
    assert_eq!(policy.policy_id, "gl-admit-v9");
    assert_eq!(policy.leg_rate_floor.p, 1);
    assert_eq!(policy.leg_rate_floor.q, 5);
    assert_eq!(policy.min_anchored_paths, 6);
    assert_eq!(policy.thorough_six_leg_runs, 3);
    assert_eq!(policy.provisional, "on");
    assert_eq!(policy.reverify_max_attempts, 3);
    assert_eq!(policy.max_held_blocks, 6);
    assert_eq!(policy.expected_network(), Some("main"));
}

fn base_levers() -> String {
    r#"
policy_id = "gl-admit-v1"
spec = "2.2"
[levers]
min_anchored_paths = 6
leg_rate_floor = { p = 1, q = 5 }
submission_deadline_blocks = 3
held_retry_seconds = 600
thorough_rounds = 3
challenge_lifetime_seconds = 120
challenge_clock_skew_seconds = 5
result_retention_seconds = 600
reverify_max_attempts = 3
max_held_blocks = 6
provisional = "on"
fast_accept_bound = 2
async_catch_budget = 22.05
"#
    .to_owned()
}

#[test]
fn a_bad_lever_file_is_refused() {
    let unknown = base_levers() + "\nsurprise = 1\n";
    let err = levers::refuse_reason(&unknown).unwrap_err();
    assert!(err.contains("unknown lever"), "{err}");

    let wrong_type = base_levers().replace("provisional = \"on\"", "provisional = 1");
    let err = levers::refuse_reason(&wrong_type).unwrap_err();
    assert!(err.contains("wrong type"), "{err}");

    let p_zero = base_levers().replace("{ p = 1, q = 5 }", "{ p = 0, q = 5 }");
    let err = levers::refuse_reason(&p_zero).unwrap_err();
    assert!(err.contains("out of range"), "{err}");

    let deadline = base_levers().replace(
        "submission_deadline_blocks = 3",
        "submission_deadline_blocks = 7",
    );
    let err = levers::refuse_reason(&deadline).unwrap_err();
    assert!(err.contains("out of range"), "{err}");

    let absent = base_levers().replace("reverify_max_attempts = 3\n", "");
    let err = levers::refuse_reason(&absent).unwrap_err();
    assert!(err.contains("absent"), "{err}");
}
