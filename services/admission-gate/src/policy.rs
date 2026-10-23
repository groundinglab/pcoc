//! Gate policy. The verifier reports measurements. This module judges them.
//!
//! `policy_id` names one exact table for its whole life. A change of any
//! value is a new id. The admission rate is SPEC §15.4 Rule B2 on
//! `(hike_legs, hike_span)` from the §14 `analysis` object. The comparison
//! is `pcoc::leg_rate_floor` (`sdk/rust/pcoc/src/compare.rs`). Bookend `N`
//! and `S` are measurements. They drive no admission comparison (Rule B6).
//! The waypoint count compares `anchored_total` among `W_1` … `W_16`.
//!
//! v1 history: through gate 0.4.1 the code default was `gl-admit-v1`, a float
//! `min_leg_rate` of 0.25 leg/s and `min_waypoints` 6 of 18. That float and
//! the named floors (`strict` 7/20, `default` 1/4, `lenient` 1/8) are gone.
//! Those three fractions stay informative examples in spec §15.5. They are
//! not a policy key. The code default is `gl-admit-v10`.
//!
//! `hike_span` ≤ 0 is `refuse`. It is not `held`, and it does not meet the
//! floor. A numeric shortfall is `refuse`. `hike_span` `not read` (or an
//! analysis block that does not carry it) is not a comparison: with no other
//! verifier note the state is `held`; another note is still a refusal and
//! that note wins. Bookend `N` `not defined` is not, by itself, an admission
//! refuse. An object the verifier reports as an earlier version is refused
//! as `earlier_version`.

use crate::canon;
use crate::verifier::{Measured, Report, EARLIER_VERSION_NOTE};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const REFUSE_EARLIER: &str = "earlier_version";
pub const REFUSE_LEG_RATE: &str = "leg_rate_floor";
/// Kept for the reason inventory. Rule B0 does not refuse admission because
/// bookend `N` is `not defined`, so the judge does not emit this code.
pub const REFUSE_BOOKENDS: &str = "bookends_not_defined";
pub const BOOKENDS_LINE: &str = "N and S are not defined.";

/// Kept for the reason inventory. A pure `hike_span` `not read` is
/// [`Judgment::Held`], not this reason. The website outcome for that hold
/// stays the locked `pending` token. The judge does not emit this code:
/// another verifier note refuses on that note alone.
pub const REFUSE_BOOKENDS_NOT_READ: &str = "bookends_not_read";
pub const BOOKENDS_NOT_READ_LINE: &str =
    "The bookend headers were not read from the index. Submit again.";

/// Settled shortfall of anchored waypoints among `W_1` … `W_16`.
pub const REFUSE_TOO_FEW: &str = "too_few_waypoints_anchored";
pub const TOO_FEW_LINE: &str = "Too few waypoints among W_1 through W_16 are anchored.";

/// The too-few line with the count, e.g. `Waypoints anchored: 4 of 16, needs 6.`
pub fn too_few_line(anchored: u64, min: u64) -> String {
    format!("Waypoints anchored: {anchored} of {ANCHORED_SPAN}, needs {min}.")
}

/// Budget exhausted while a measurement stayed `not read`.
pub const REFUSE_HELD_BOUND: &str = "held_bound";

/// A stored or anchored height is below [`AdmissionPolicy::born_height`].
/// Terminal. Not `held`.
pub const BEFORE_BORN: &str = "before_born_height";

/// Sentence for [`BEFORE_BORN`]. The numbers are the policy value that
/// applied and the lowest offending height.
pub fn before_born_line(born_height: u64, lowest: u64) -> String {
    format!(
        "This object is before the born height {born_height}. The lowest offending height is {lowest}. The decision is final."
    )
}

/// Verify-job record for [`BEFORE_BORN`]. Not a section 14 record: the gate
/// does not read the index for this decision.
pub fn before_born_record(born_height: u64, lowest: u64) -> String {
    serde_json::json!({
        "reason": BEFORE_BORN,
        "born_height": born_height,
        "lowest_height": lowest,
    })
    .to_string()
}

/// Sentence the current website shows for [`REFUSE_EARLIER`].
///
/// The website sign-in page blanks a snake_case reason it does not know
/// and shows a `reason` string that contains a space. This sentence is that
/// line. It is not "could not be read".
pub const EARLIER_LINE: &str =
    "This object is an earlier version. This gate checks the pinned Satoshi Hike kind only.";

pub const UNREADABLE_REASON: &str = "unreadable";
pub const UNREADABLE_LINE: &str = "This file is not an object of the pinned Satoshi Hike kind.";

pub const NO_KEY_REASON: &str = "no_public_key";
pub const NO_KEY_LINE: &str = "This object has no public key the gate can check.";

/// Plain decimal with at most `places` places, trailing zeros dropped.
fn decimal(x: f64, places: usize) -> String {
    let text = format!("{x:.places$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

/// `this gate needs at least 0.2 legs/s (one leg every 5 s)` for `p/q = 1/5`.
fn floor_words(p: u64, q: u64) -> String {
    if p == 0 || q == 0 {
        return format!("this gate's floor is {p}/{q} legs/s");
    }
    format!(
        "this gate needs at least {} legs/s (one leg every {} s)",
        decimal(p as f64 / q as f64, 3),
        decimal(q as f64 / p as f64, 1)
    )
}

/// Too-slow sentence with the measured hike pair, `hike_legs` checkpoints in
/// `hike_span` seconds, and the configured floor `p/q`. Example: `Too slow:
/// 1499 legs in 9557 s = 0.157 legs/s (6.4 s per leg); this gate needs at
/// least 0.2 legs/s (one leg every 5 s).`
pub fn leg_rate_line(n: i128, s: i128, p: u64, q: u64) -> String {
    let need = floor_words(p, q);
    if n > 0 && s > 0 {
        let (legs, secs) = (n as f64, s as f64);
        format!(
            "Too slow: {n} legs in {s} s = {} legs/s ({} s per leg); {need}.",
            decimal(legs / secs, 3),
            decimal(secs / legs, 1)
        )
    } else {
        format!("Too slow: {n} legs in {s} s; {need}.")
    }
}

/// Too-slow sentence when the hike pair was not measured as two numbers.
pub fn leg_rate_floor_line(p: u64, q: u64) -> String {
    format!("Too slow; {}.", floor_words(p, q))
}

/// `gl-admit-v` plus a positive integer. Bound to one table for its life.
pub const DEFAULT_POLICY_ID: &str = "gl-admit-v10";

/// `W_1` … `W_16`. `W_17` is never fetched and is not in the anchored count.
pub const ANCHORED_SPAN: u64 = 16;

/// Stage count on the object. Not the anchored count.
pub const WAYPOINTS_TOTAL: u64 = 18;

/// Spec rule: at least this many legs. Labelled as spec, not policy.
pub const SPEC_MIN_LEGS: u64 = 6;

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
    if rest.is_empty() || !rest.bytes().all(|byte| byte.is_ascii_digit()) || rest.starts_with('0') {
        return Err(format!(
            "policy_id {raw:?} must be gl-admit-v plus a positive integer"
        ));
    }
    Ok(id.to_owned())
}

/// Integer leg-rate floor `p/q`. Meets when `hike_span` > 0 and
/// `q · hike_legs ≥ p · hike_span` (SPEC §15.4 Rule B2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateFloor {
    pub p: u64,
    pub q: u64,
}

/// `[index]` table of a policy (gl-admit-v7 on). Hashed with the table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IndexPolicy {
    /// `main` or `synthetic`. Every index document a decision rests on,
    /// header and plant reads alike, must carry this `network`.
    pub expected_network: String,
}

/// The networks an `[index] expected_network` may name.
pub const NETWORKS: &[&str] = &["main", "synthetic"];

/// Plain wording of `index_network_not_expected` for a mainnet policy.
pub const NETWORK_NOT_MAIN_LINE: &str =
    "The index evidence is not from the Bitcoin mainnet, so it cannot support admission here.";

/// The refusal sentence for an `expected_network` miss.
pub fn network_line(expected: &str) -> String {
    if expected == "main" {
        NETWORK_NOT_MAIN_LINE.to_owned()
    } else {
        format!(
            "The index evidence is not from the {expected} network this gate expects, so it cannot support admission here."
        )
    }
}

/// How this binary was built. A testing build must not claim mainnet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateBuild {
    Production,
    /// Built with the cargo feature `testing` (conformance-small helpers,
    /// synthetic fixtures).
    Testing,
}

/// The build of this binary: `Testing` when the cargo feature `testing` is
/// on. A release build (`cargo build --release`) is `Production`.
pub const fn gate_build() -> GateBuild {
    if cfg!(feature = "testing") {
        GateBuild::Testing
    } else {
        GateBuild::Production
    }
}

/// Startup check. A policy that expects mainnet evidence does not start in
/// a testing build.
pub fn check_build(policy: &AdmissionPolicy, build: GateBuild) -> Result<(), String> {
    if policy.expected_network() == Some("main") && build == GateBuild::Testing {
        return Err(format!(
            "policy {} expects index network \"main\", and this is a testing build; a testing build serves only a synthetic policy",
            policy.policy_id
        ));
    }
    Ok(())
}

/// One configured policy table. A different value is a different id.
///
/// The fields are the keys of `policy/gl-admit-v6.toml`, plus the pending
/// caps the plan names (F3) and the held-retry budgets §15.3 names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AdmissionPolicy {
    pub policy_id: String,
    pub leg_rate_floor: RateFloor,
    pub min_anchored_paths: u64,
    /// Optional first/last anchored rule. `None` (gl-admit-v6) means no
    /// specific waypoint is required; only the count matters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_first_anchored: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_last_anchored: Option<u64>,
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
    /// Open sign-ins per source. Since gate 0.5.4 the source is the
    /// submitter's public key, not the signed site origin. The value and
    /// its key name are unchanged, so the policy hash is unchanged.
    pub pending_per_source: u64,
    /// `None` means `max_concurrent × 2`, filled when the config is built.
    pub pending_global: u64,
    /// SPEC §15.4. `None` means the key is absent (unset: no session cap).
    /// [`AdmissionPolicy::default`] uses `Some(3)` to mirror gl-admit-v10.
    /// That constructor is not a policy-file load path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_cap_per_door_session: Option<u64>,
    /// SPEC §15.4 `K`. `tip − last_beacon > K` is Rule B10.
    pub submission_deadline_blocks: u64,
    /// Seconds between held retries. Read from the policy file.
    pub held_retry_seconds: u64,
    /// `fast_accept_bound` as integer milliseconds.
    pub fast_accept_bound_ms: u64,
    /// `async_catch_budget` as integer milliseconds.
    pub async_catch_budget_ms: u64,
    /// Unset when the policy omits the key (SPEC default unset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_cap_per_object: Option<u64>,
    /// Unset when the policy omits the key. A ban then has no expiry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fake_leg_ignore_seconds: Option<u64>,
    pub challenge_lifetime_seconds: u64,
    pub challenge_clock_skew_seconds: u64,
    pub result_retention_seconds: u64,
    /// `[policy_rules] born_height`. Heights below this are testing.
    /// The policy file must set it. An explicit `0` admits every height.
    /// Not a SPEC lever.
    pub born_height: u64,
    /// `[index]` (gl-admit-v7 on). Absent in gl-admit-v6 and earlier, and
    /// then not serialized, so their bind hash and policy document stay the same.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<IndexPolicy>,
}

/// Mirrors gl-admit-v10, including door-session cap 3. `Config::load` does
/// not call this. A policy file goes through `levers::load`, which refuses
/// a missing required lever and leaves an absent door-session cap unset.
/// Unit tests compile this impl. Integration tests compile it through the
/// `test-util` feature. A release build compiles neither, so the service
/// has no built-in policy table.
#[cfg(any(test, feature = "test-util"))]
impl Default for AdmissionPolicy {
    fn default() -> Self {
        Self {
            policy_id: DEFAULT_POLICY_ID.to_owned(),
            leg_rate_floor: RateFloor { p: 1, q: 5 },
            min_anchored_paths: 6,
            require_first_anchored: None,
            require_last_anchored: None,
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
            verify_cap_per_door_session: Some(3),
            submission_deadline_blocks: 3,
            held_retry_seconds: 600,
            fast_accept_bound_ms: 2_000,
            async_catch_budget_ms: 22_050,
            verify_cap_per_object: None,
            fake_leg_ignore_seconds: None,
            challenge_lifetime_seconds: 120,
            challenge_clock_skew_seconds: 5,
            result_retention_seconds: 600,
            born_height: 972685,
            index: Some(IndexPolicy {
                expected_network: "main".to_owned(),
            }),
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

    /// `[index] expected_network`, when this policy sets one.
    pub fn expected_network(&self) -> Option<&str> {
        self.index
            .as_ref()
            .map(|index| index.expected_network.as_str())
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

pub fn policy_checks(
    measures: &crate::verifier::Measures,
    policy: &AdmissionPolicy,
) -> Vec<PolicyCheck> {
    let mut checks = Vec::new();
    if let (Some(n), Some(s)) = (measures.legs, measures.span_s) {
        // `Measures.legs` / `span_s` are the hike pair when the verifier
        // filled them. Bookend `N`/`S` are not this check. `span_s` is `u64`,
        // so a negative span is judged from the record in [`judge`].
        checks.push(PolicyCheck {
            field: "leg_rate",
            min: serde_json::json!({
                "p": policy.leg_rate_floor.p,
                "q": policy.leg_rate_floor.q,
            }),
            met: rate_meets(
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
    let rate = checks
        .iter()
        .any(|check| check.field == "leg_rate" && check.met);
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
    /// `hike_span` is `not read`. Not a refusal. Not a decision-log event.
    Held {
        /// Section 14 record bytes, including the trailing newline.
        record: String,
    },
}

/// SPEC §14 `hike_span` is a JSON number in ±(2^53−1). A wider integer is not
/// that measurement. The caller refuses it; it does not hold.
const SPEC_JSON_INT_MAX: i128 = (1i128 << 53) - 1;

/// Rule B2 via the SDK. `meets` only when `hike_span` > 0 and
/// `q · hike_legs ≥ p · hike_span`. `hike_span` ≤ 0 is `refuse`.
fn rate_meets(hike_legs: i128, hike_span: i128, p: u64, q: u64) -> bool {
    pcoc::leg_rate_floor(hike_legs, hike_span, i128::from(p), i128::from(q)) == "meets"
}

enum HikeRate {
    /// `hike_span` is absent or the token `not read`. No comparison.
    NotRead,
    /// Integers for [`pcoc::leg_rate_floor`]. A non-integer span that is not
    /// `not read`, and a non-integer `hike_legs` beside a positive span, are
    /// passed as values the SDK refuses (span 0, or 0 legs).
    Compare { legs: i128, span: i128 },
}

/// Read `analysis.hike_legs` and `analysis.hike_span`. Bookend `N` and `S`
/// are not read. The record text is not rewritten.
fn hike_rate(record: &str) -> HikeRate {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(record) else {
        return HikeRate::NotRead;
    };
    let Some(analysis) = value.get("analysis") else {
        return HikeRate::NotRead;
    };
    match analysis.get("hike_span") {
        None => HikeRate::NotRead,
        Some(Value::String(token)) if token == "not read" => HikeRate::NotRead,
        Some(span_value) => {
            let span = spec_int(span_value).unwrap_or(0);
            let legs = analysis.get("hike_legs").and_then(spec_int).unwrap_or(0);
            HikeRate::Compare { legs, span }
        }
    }
}

/// An integer JSON number inside ±(2^53−1). Anything else is not a §14 span.
fn spec_int(value: &Value) -> Option<i128> {
    let number = if let Some(number) = value.as_i64() {
        i128::from(number)
    } else if let Some(number) = value.as_u64() {
        i128::from(number)
    } else {
        return None;
    };
    if number.abs() > SPEC_JSON_INT_MAX {
        return None;
    }
    Some(number)
}

/// Stored beacon heights, `last_interrupt_height`, and claimed path block
/// heights. Absent keys are not heights. A kind other than the current one
/// yields nothing.
pub fn object_heights(object: &[u8]) -> Vec<u64> {
    let Ok((item, _)) = pcoc::cbor::decode_item(object) else {
        return Vec::new();
    };
    let pcoc::cbor::Cbor::Map(map) = item else {
        return Vec::new();
    };
    let Some(pcoc::cbor::Cbor::Text(kind)) = pcoc::cbor::map_get(&map, "kind") else {
        return Vec::new();
    };
    if kind != pcoc::constants::KIND {
        return Vec::new();
    }
    let mut heights = Vec::new();
    push_height(
        &mut heights,
        pcoc::cbor::map_get(&map, "last_interrupt_height"),
    );
    if let Some(pcoc::cbor::Cbor::Array(stages)) = pcoc::cbor::map_get(&map, "stages") {
        for stage in stages {
            let pcoc::cbor::Cbor::Map(stage) = stage else {
                continue;
            };
            push_height(&mut heights, pcoc::cbor::map_get(stage, "beacon_height"));
            if let Some(pcoc::cbor::Cbor::Bytes(path)) = pcoc::cbor::map_get(stage, "path") {
                if let Some((_, height)) = pcoc::path::peek_claimed(path) {
                    heights.push(u64::from(height));
                }
            }
        }
    }
    heights
}

fn push_height(out: &mut Vec<u64>, value: Option<&pcoc::cbor::Cbor>) {
    let Some(pcoc::cbor::Cbor::Uint(height)) = value else {
        return;
    };
    if *height <= pcoc::constants::HEIGHT_MAX {
        out.push(*height);
    }
}

/// Lowest stored or claimed height strictly below `born_height`.
/// `None` when every such height is at or above the boundary, or when the
/// bytes are not a current object.
pub fn lowest_before_born(object: &[u8], born_height: u64) -> Option<u64> {
    object_heights(object)
        .into_iter()
        .filter(|height| *height < born_height)
        .min()
}

/// Lowest numeric height in a section 14 record that is below `born_height`.
/// Looks at stored header heights and claimed or anchored path heights.
pub fn lowest_in_record(record: &str, born_height: u64) -> Option<u64> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(record) else {
        return None;
    };
    let mut lowest = None;
    let mut note = |item: &serde_json::Value| {
        let Some(height) = item.as_u64() else {
            return;
        };
        if height < born_height {
            lowest = Some(lowest.map_or(height, |cur: u64| cur.min(height)));
        }
    };
    if let Some(headers) = value.get("headers").and_then(|item| item.as_array()) {
        for header in headers {
            if let Some(stored) = header.get("stored_height") {
                note(stored);
            }
        }
    }
    if let Some(paths) = value.get("paths").and_then(|item| item.as_array()) {
        for path in paths {
            if let Some(height) = path.pointer("/claimed_block/block_height") {
                note(height);
            }
            if let Some(height) = path.pointer("/P_g/block_height") {
                note(height);
            }
            if let Some(height) = path.get("height_P_g") {
                note(height);
            }
        }
    }
    lowest
}

pub fn judge(report: &Report, floor_p: u64, floor_q: u64, born_height: u64) -> Judgment {
    match report {
        Report::Unreadable => Judgment::Refuse {
            reasons: vec![UNREADABLE_REASON.to_owned()],
            line: UNREADABLE_LINE.to_owned(),
            record: "{}".to_owned(),
        },
        Report::Read(measured) => {
            if let Some(lowest) = lowest_in_record(&measured.record, born_height) {
                return Judgment::Refuse {
                    reasons: vec![BEFORE_BORN.to_owned()],
                    line: before_born_line(born_height, lowest),
                    record: before_born_record(born_height, lowest),
                };
            }
            judge_measured(measured, floor_p, floor_q)
        }
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
    // Rule A5. The kind is the reason. The hike pair is not read on another kind.
    if measured
        .verifier_notes
        .iter()
        .any(|note| note == crate::reasons::CODE_UNKNOWN_KIND)
    {
        return Judgment::Refuse {
            reasons: vec![crate::reasons::CODE_UNKNOWN_KIND.to_owned()],
            line: "This object is not the v2.2 kind. The gate refuses it.".to_owned(),
            record,
        };
    }
    let mut reasons: Vec<String> = measured
        .verifier_notes
        .iter()
        .map(|note| (*note).to_owned())
        .collect();
    match hike_rate(&record) {
        HikeRate::Compare { legs, span } if !rate_meets(legs, span, floor_p, floor_q) => {
            reasons.insert(0, REFUSE_LEG_RATE.to_owned());
            return Judgment::Refuse {
                reasons,
                line: leg_rate_line(legs, span, floor_p, floor_q),
                record,
            };
        }
        HikeRate::Compare { .. } => {}
        // A pure `not read` is held. Another verifier note is still a refusal.
        HikeRate::NotRead => {
            if reasons.is_empty() {
                return Judgment::Held { record };
            }
            let line = reasons.join(" ");
            return Judgment::Refuse {
                reasons,
                line,
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
        let (spec_name, _) = crate::verifier::health_spec();
        map.insert("spec".to_owned(), Value::String(spec_name));
        map.insert("anchored_span".to_owned(), serde_json::json!(ANCHORED_SPAN));
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
    /// Bookend `N`/`S` are on the record and are not the comparison.
    fn hike_record(legs: &str, span: &str) -> String {
        format!(
            "{{\"profile\":\"production\",\"shape\":\"match\",\"N\":0,\"S\":0,\"analysis\":{{\"hike_legs\":{legs},\"hike_span\":{span}}}}}\n"
        )
    }

    #[test]
    fn a_configured_fraction_is_the_floor() {
        // 1/5: one leg every five seconds. Equality meets it. Span 0 refuses.
        assert!(rate_meets(1, 5, 1, 5));
        assert!(!rate_meets(1, 6, 1, 5));
        assert!(rate_meets(2, 5, 1, 5));
        assert!(!rate_meets(1, 0, 1, 5));
        assert!(!rate_meets(0, 5, 1, 5));
        assert!(!rate_meets(1, -1, 1, 5));
        // The same function applies a caller-chosen p/q. 7/20 is not a default.
        assert!(!rate_meets(1, 4, 7, 20));
        assert!(rate_meets(7, 20, 7, 20));
        assert_eq!(pcoc::leg_rate_floor(1, 5, 1, 5), "meets");
        assert_eq!(pcoc::leg_rate_floor(1, 0, 1, 5), "refuse");
    }

    #[test]
    fn the_too_slow_line_states_the_measured_numbers_and_the_floor() {
        assert_eq!(
            leg_rate_line(1499, 9557, 1, 5),
            "Too slow: 1499 legs in 9557 s = 0.157 legs/s (6.4 s per leg); this gate needs at least 0.2 legs/s (one leg every 5 s)."
        );
        assert_eq!(
            leg_rate_line(0, 30, 1, 4),
            "Too slow: 0 legs in 30 s; this gate needs at least 0.25 legs/s (one leg every 4 s)."
        );
        assert_eq!(
            leg_rate_floor_line(1, 5),
            "Too slow; this gate needs at least 0.2 legs/s (one leg every 5 s)."
        );
        assert_eq!(too_few_line(4, 6), "Waypoints anchored: 4 of 16, needs 6.");
        for line in [leg_rate_line(1499, 9557, 1, 5), too_few_line(4, 6)] {
            assert!(
                !line.contains("1/5") && !line.contains("leg per second"),
                "{line}"
            );
        }
    }

    #[test]
    fn a_numeric_pair_under_the_floor_is_refused() {
        let record = hike_record("1", "6");
        let judgment = judge_measured(&measured(&record, &[]), 1, 5);
        assert_eq!(
            judgment,
            Judgment::Refuse {
                reasons: vec!["leg_rate_floor".to_owned()],
                line: leg_rate_line(1, 6, 1, 5),
                record: record.clone(),
            }
        );
        assert!(record.ends_with('\n'));
        assert!(record.starts_with("{\"profile\":\"production\","));
    }

    #[test]
    fn a_configured_pq_is_applied_to_the_hike_pair() {
        let meets = hike_record("1", "5");
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
                assert_eq!(line, leg_rate_line(1, 5, 1, 4));
                assert_eq!(record, meets);
            }
            other => panic!("1/4 should refuse 1 leg in 5 s, got {other:?}"),
        }
        let wide = hike_record("1", "8");
        assert!(matches!(
            judge_measured(&measured(&wide, &[]), 1, 8),
            Judgment::Admit { .. }
        ));
        match judge_measured(&measured(&hike_record("1", "9"), &[]), 1, 8) {
            Judgment::Refuse { reasons, line, .. } => {
                assert_eq!(reasons, vec!["leg_rate_floor".to_owned()]);
                assert_eq!(line, leg_rate_line(1, 9, 1, 8));
            }
            other => panic!("1/8 should refuse 1 leg in 9 s, got {other:?}"),
        }
    }

    #[test]
    fn bookend_not_defined_does_not_refuse_when_the_hike_pair_meets() {
        // Bookend tokens sit on the record. Rule B0 does not refuse them.
        // A verifier note still refuses, and it does not become bookends_not_defined.
        let record = "{\"profile\":\"production\",\"N\":\"not defined\",\"S\":\"not defined\",\"analysis\":{\"hike_legs\":5,\"hike_span\":5}}\n";
        assert!(matches!(
            judge_measured(&measured(&record, &[]), 1, 5),
            Judgment::Admit { .. }
        ));
        match judge_measured(&measured(&record, &["shape is false"]), 1, 5) {
            Judgment::Refuse {
                reasons,
                line,
                record: stored,
            } => {
                assert_eq!(reasons, vec!["shape is false".to_owned()]);
                assert_eq!(line, "shape is false");
                assert!(!line.contains("1/5"));
                assert!(!reasons
                    .iter()
                    .any(|reason| reason == "bookends_not_defined"));
                assert_eq!(stored, record);
            }
            other => panic!("expected the verifier note, got {other:?}"),
        }
    }

    #[test]
    fn a_not_read_span_is_held() {
        for span in ["\"not read\"", "null"] {
            let record = if span == "null" {
                "{\"profile\":\"production\",\"analysis\":{\"hike_legs\":4}}\n".to_owned()
            } else {
                hike_record("4", span)
            };
            match judge_measured(&measured(&record, &[]), 1, 5) {
                Judgment::Held { record: stored } => {
                    assert_eq!(stored, record, "{span}");
                    assert!(!stored.contains("bookends_not_read"), "{span}");
                }
                other => panic!("expected held for span {span}, got {other:?}"),
            }
        }
    }

    #[test]
    fn bookend_not_defined_with_span_not_read_is_held() {
        // SPEC Rule B0: bookend N not defined is not an admission refuse.
        // hike_span not read is held.
        let mixed = "{\"profile\":\"production\",\"N\":\"not defined\",\"S\":\"not read\",\"analysis\":{\"hike_legs\":\"not defined\",\"hike_span\":\"not read\"}}\n";
        match judge_measured(&measured(mixed, &[]), 7, 20) {
            Judgment::Held { record } => {
                assert_eq!(record, mixed);
                assert!(
                    !record.contains("bookends_not_defined")
                        || record.contains("\"N\":\"not defined\"")
                );
            }
            other => panic!("expected held, got {other:?}"),
        }
    }

    #[test]
    fn a_floor_refusal_keeps_the_other_notes() {
        let record = hike_record("0", "5");
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
                assert_eq!(
                    line,
                    "Too slow: 0 legs in 5 s; this gate needs at least 0.2 legs/s (one leg every 5 s)."
                );
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
