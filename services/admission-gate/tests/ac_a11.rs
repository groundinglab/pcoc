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
        .join("../../policy/groundinglab-admission/gl-admit-v10.toml");
    std::fs::read_to_string(path).expect("gl-admit-v10.toml")
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
    let policy = levers::load(&policy_text(), 2).expect("v10");
    let index =
        pcoc_admission::verifier::IndexClient::open("http://127.0.0.1:9", Duration::from_secs(1));
    match pcoc_admission::fast::decide(&other_kind(), &policy, &index) {
        FastDecision::Refuse {
            reasons, website, ..
        } => {
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
fn gl_admit_v10_loads() {
    let policy = levers::load(&policy_text(), 2).expect("v10");
    assert_eq!(policy.policy_id, "gl-admit-v10");
    assert_eq!(policy.leg_rate_floor.p, 1);
    assert_eq!(policy.leg_rate_floor.q, 5);
    assert_eq!(policy.min_anchored_paths, 6);
    assert_eq!(policy.thorough_six_leg_runs, 3);
    assert_eq!(policy.provisional, "on");
    assert_eq!(policy.reverify_max_attempts, 3);
    assert_eq!(policy.max_held_blocks, 6);
    assert_eq!(policy.expected_network(), Some("main"));
    assert_eq!(policy.born_height, 972685);
    assert!(!policy.sticky_refuse);
}

/// The previous id stays as it was. It has no `born_height` key. Load
/// refuses that file and names the key. This test reads the file. It sets
/// nothing on the process.
#[test]
fn gl_admit_v9_file_is_unchanged() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../policy/groundinglab-admission/gl-admit-v9.toml");
    let text = std::fs::read_to_string(&path).expect("gl-admit-v9.toml");
    assert!(
        !text.lines().any(|line| {
            let line = line.trim();
            !line.starts_with('#') && line.starts_with("born_height")
        }),
        "gl-admit-v9.toml gained a born_height key"
    );
    let err = levers::load(&text, 2).expect_err("v9");
    assert!(err.contains("born_height"), "{err}");
    assert!(err.contains("absent"), "{err}");
}

fn base_levers() -> String {
    r#"
policy_id = "gl-admit-v1"
spec = "2.2"
[levers]
min_anchored_paths = 6
leg_rate_floor = { p = 1, q = 5 }
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
[policy_rules]
draw_commitment = "sha256(draw || salt), revealed with the result"
born_height = 972685
"#
    .to_owned()
}

#[test]
fn a_bad_lever_file_is_refused() {
    let unknown = base_levers().replace("[policy_rules]", "surprise = 1\n[policy_rules]");
    let err = levers::refuse_reason(&unknown).unwrap_err();
    assert!(err.contains("unknown lever"), "{err}");

    let wrong_type = base_levers().replace("provisional = \"on\"", "provisional = 1");
    let err = levers::refuse_reason(&wrong_type).unwrap_err();
    assert!(err.contains("wrong type"), "{err}");

    let p_zero = base_levers().replace("{ p = 1, q = 5 }", "{ p = 0, q = 5 }");
    let err = levers::refuse_reason(&p_zero).unwrap_err();
    assert!(err.contains("out of range"), "{err}");

    let absent = base_levers().replace("reverify_max_attempts = 3\n", "");
    let err = levers::refuse_reason(&absent).unwrap_err();
    assert!(err.contains("absent"), "{err}");
}

/// The retired tip-deadline key is not required. A present value, including
/// one outside the old 0…6 range, does not refuse the file. The alias `K`
/// is not that key.
#[test]
fn retired_submission_deadline_key_is_ignored() {
    assert!(levers::refuse_reason(&base_levers()).is_ok());
    let present = base_levers().replace(
        "[policy_rules]",
        "submission_deadline_blocks = 7\n[policy_rules]",
    );
    assert!(levers::refuse_reason(&present).is_ok(), "{present}");
    let named_k = base_levers().replace("[policy_rules]", "K = 3\n[policy_rules]");
    let err = levers::refuse_reason(&named_k).unwrap_err();
    assert!(err.contains("unknown lever"), "{err}");
}
