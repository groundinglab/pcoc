//! SPEC preimages and the pinned section 14 record.
//!
//! Expected digests are SHA-256 of hand-assembled bytes (`sha2`). The terrain
//! builder is only an input when a sample or a slot is required. It is not the
//! oracle for the digest under test.
//!
//! main @ 546d24f, verify record L952–L1010.

#![allow(dead_code)]

use sha2::{Digest, Sha256};

pub const STAGES: usize = 18;

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// `seed = public_key ‖ seed_beacon`, 64 bytes. Seed value is SHA-256 of that.
pub fn seed_value(public_key: &[u8; 32], seed_beacon: &[u8; 32]) -> [u8; 32] {
    let mut seed = [0u8; 64];
    seed[..32].copy_from_slice(public_key);
    seed[32..].copy_from_slice(seed_beacon);
    sha256(&seed)
}

/// Library terrain, used only as the byte source for a sample or a slot.
pub fn terrain_input(public_key: &[u8; 32], seed_beacon: &[u8; 32]) -> Vec<u8> {
    let seed = hike::production_seed(public_key, seed_beacon);
    hike::fill_terrain(hike::fill_pin(&seed))
}

pub fn terrain_sample(terrain: &[u8], previous: &[u8; 32]) -> Vec<u8> {
    let offset =
        (u64::from_le_bytes(previous[..8].try_into().unwrap()) % hike::OFFSET_MODULUS) as usize;
    terrain[offset..offset + hike::SAMPLE_BYTES].to_vec()
}

pub fn slot_sample(terrain: &[u8], interrupt: &[u8; 32]) -> Vec<u8> {
    let index =
        (u64::from_le_bytes(interrupt[..8].try_into().unwrap()) % hike::N_SLOTS as u64) as usize;
    let start = index * hike::SLOT_BYTES;
    terrain[start..start + hike::SLOT_BYTES].to_vec()
}

/// §11.4: previous checkpoint ‖ stage limb ‖ terrain sample ‖ le64(leg result).
pub fn checkpoint_digest(
    previous: &[u8; 32],
    stage_limb: &[u8; 32],
    sample: &[u8],
    leg_result: u64,
) -> [u8; 32] {
    assert_eq!(sample.len(), hike::SAMPLE_BYTES);
    let mut pre = Vec::with_capacity(32 + 32 + hike::SAMPLE_BYTES + 8);
    pre.extend_from_slice(previous);
    pre.extend_from_slice(stage_limb);
    pre.extend_from_slice(sample);
    pre.extend_from_slice(&leg_result.to_le_bytes());
    assert_eq!(pre.len(), hike::CHECKPOINT_PREIMAGE_LEN);
    sha256(&pre)
}

pub fn fold_checkpoints(checkpoints: &[[u8; 32]]) -> [u8; 32] {
    let mut fold = [0u8; 32];
    for checkpoint in checkpoints {
        let mut pre = [0u8; 64];
        pre[..32].copy_from_slice(&fold);
        pre[32..].copy_from_slice(checkpoint);
        fold = sha256(&pre);
    }
    fold
}

/// Rule A1.3: `SHA-256(I_g ‖ slot_sample(I_g) ‖ W_g ‖ fold_g)`.
pub fn waypoint_digest(
    interrupt: &[u8; 32],
    slot: &[u8],
    previous_w: &[u8; 32],
    fold: &[u8; 32],
) -> [u8; 32] {
    assert_eq!(slot.len(), hike::SLOT_BYTES);
    let mut pre = Vec::with_capacity(32 + hike::SLOT_BYTES + 32 + 32);
    pre.extend_from_slice(interrupt);
    pre.extend_from_slice(slot);
    pre.extend_from_slice(previous_w);
    pre.extend_from_slice(fold);
    assert_eq!(pre.len(), 16_480);
    sha256(&pre)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SixLegRecord {
    NotDefined,
    NotRead,
    Drawn {
        indices: Vec<u64>,
        results: Vec<&'static str>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Field<T> {
    Value(T),
    NotDefined,
    NotRead,
    /// The running verifier does not record this key yet.
    Absent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockRef {
    /// 64 lowercase hex digits, Bitcoin display order, as stored in the path body.
    pub block_hash: String,
    pub block_height: i128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderRow {
    /// 64 lowercase hex, natural byte order, or `not defined` when the hash is absent.
    /// main @ 546d24f L980.
    pub stored_hash: Field<String>,
    pub stored_height: Field<i128>,
    pub index_comparison: &'static str,
    pub t: Field<i128>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegRow {
    pub j: u128,
    pub result: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointRow {
    pub g: u32,
    /// `not defined` when that stage's checkpoints are unusable.
    /// Otherwise one row per checkpoint. main @ 546d24f L956 and L988.
    pub legs: Field<Vec<LegRow>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaypointRow {
    pub g: u32,
    pub result: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathRow {
    pub g: u32,
    pub path_state: Option<&'static str>,
    pub plant: Option<&'static str>,
    pub w17_path: Option<&'static str>,
    pub p_g: Field<BlockRef>,
    pub height_p_g: Field<i128>,
    pub d_g: Field<i128>,
    pub claimed_block: Field<BlockRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookends {
    pub a: Field<u32>,
    pub b: Field<u32>,
    pub n: Field<u128>,
    pub s: Field<i128>,
    pub delta_h: Field<i128>,
    pub p_b: Field<BlockRef>,
    pub height_p_b: Field<i128>,
    /// main @ 546d24f L1002. `not defined` under Rule A4.11.
    pub index_comparison_p_b: Field<String>,
    pub t_p_b: Field<i128>,
    pub height_i_a_minus_1: Field<i128>,
    pub t_i_a_minus_1: Field<i128>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageAnalysis {
    pub g: u32,
    /// Number of checkpoints in the stage. Not the object's `leg_count`.
    /// main @ 546d24f L1010.
    pub checkpoint_count: u128,
    pub span: Field<i128>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Analysis {
    pub whole_hike_legs: u128,
    pub whole_hike_span: Field<i128>,
    pub per_stage: Vec<StageAnalysis>,
    pub legs_outside: Field<u128>,
}

/// The section 14 record, parsed from `verify_record_json`.
/// `headers` has 19 rows. Index 18 is `last_interrupt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section14 {
    pub shape: String,
    pub shape_item: Option<Vec<String>>,
    pub headers: Option<Vec<HeaderRow>>,
    pub seed_value: Option<String>,
    pub checkpoints: Option<Vec<CheckpointRow>>,
    pub waypoints: Option<Vec<WaypointRow>>,
    pub paths: Option<Vec<PathRow>>,
    pub anchored_total: Option<u128>,
    pub skip_list: Option<Vec<u32>>,
    pub book: Option<Bookends>,
    pub six_leg: SixLegRecord,
    pub analysis: Option<Analysis>,
    /// #159 record key. Not asserted. Present or absent is not a mismatch.
    #[allow(dead_code)]
    pub profile: Option<String>,
    /// #159 `fixed_start` and `fixed_start.*`. Not asserted.
    #[allow(dead_code)]
    pub fixed_start: Option<String>,
}

/// The section 14 record `verify_record_json` writes. Harness callers read
/// that JSON. They do not rebuild a shorter table from `Measurements`.
pub fn section14(measurements: &hike::Measurements) -> Section14 {
    let text = hike::verify_record_json("production", measurements);
    parse_section14(&text)
}

fn parse_section14(text: &str) -> Section14 {
    let value: serde_json::Value =
        serde_json::from_str(text).unwrap_or_else(|err| panic!("section 14 record is not JSON: {err}"));
    if value.get("version").and_then(|item| item.as_str()) == Some("earlier version") {
        return Section14 {
            shape: String::new(),
            shape_item: None,
            headers: None,
            seed_value: None,
            checkpoints: None,
            waypoints: None,
            paths: None,
            anchored_total: None,
            skip_list: None,
            book: None,
            six_leg: SixLegRecord::NotRead,
            analysis: None,
            profile: None,
            fixed_start: None,
        };
    }
    let shape = value
        .get("shape")
        .and_then(|item| item.as_str())
        .unwrap_or_else(|| panic!("section 14 record has no shape"))
        .to_owned();
    Section14 {
        shape,
        shape_item: value.get("shape_item").map(string_list),
        headers: value.get("headers").map(parse_headers),
        seed_value: value
            .get("seed_value")
            .and_then(|item| item.as_str())
            .map(str::to_owned),
        checkpoints: value.get("checkpoints").map(parse_checkpoints),
        waypoints: value.get("waypoints").map(parse_waypoints),
        paths: value.get("paths").map(parse_paths),
        anchored_total: value.get("anchored_total").and_then(json_u128),
        skip_list: value.get("skip_list").and_then(|item| item.as_array()).map(|rows| {
            rows.iter()
                .map(|item| {
                    item.as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .unwrap_or_else(|| panic!("skip_list entry {item}"))
                })
                .collect()
        }),
        book: parse_book(&value),
        six_leg: parse_six(value.get("six_leg").unwrap_or_else(|| panic!("section 14 record has no six_leg"))),
        analysis: value.get("analysis").and_then(parse_analysis),
        profile: value
            .get("profile")
            .and_then(|item| item.as_str())
            .map(str::to_owned),
        fixed_start: value.get("fixed_start").map(|item| item.to_string()),
    }
}

fn string_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("expected a string array, got {value}"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("expected a string, got {item}"))
                .to_owned()
        })
        .collect()
}

fn static_token(value: &str) -> &'static str {
    match value {
        "match" => "match",
        "mismatch" => "mismatch",
        "present" => "present",
        "skip" => "skip",
        "not defined" => "not defined",
        "not read" => "not read",
        "not fetched by design" => "not fetched by design",
        "n/a" => "n/a",
        "anchored" => "anchored",
        "not anchored" => "not anchored",
        "earlier version" => "earlier version",
        other => panic!("unknown section 14 token {other}"),
    }
}

fn field_i128(value: &serde_json::Value) -> Field<i128> {
    match value {
        serde_json::Value::Number(number) => Field::Value(json_i128_number(number)),
        serde_json::Value::String(token) if token == "not defined" => Field::NotDefined,
        serde_json::Value::String(token) if token == "not read" => Field::NotRead,
        other => panic!("section 14 integer field {other}"),
    }
}

fn field_u128(value: &serde_json::Value) -> Field<u128> {
    match value {
        serde_json::Value::Number(number) => {
            Field::Value(number.as_u64().unwrap_or_else(|| panic!("unsigned {number}")) as u128)
        }
        serde_json::Value::String(token) if token == "not defined" => Field::NotDefined,
        serde_json::Value::String(token) if token == "not read" => Field::NotRead,
        other => panic!("section 14 count field {other}"),
    }
}

fn field_u32(value: &serde_json::Value) -> Field<u32> {
    match value {
        serde_json::Value::Number(number) => Field::Value(
            number
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or_else(|| panic!("stage index {number}")),
        ),
        serde_json::Value::String(token) if token == "not defined" => Field::NotDefined,
        serde_json::Value::String(token) if token == "not read" => Field::NotRead,
        other => panic!("section 14 index field {other}"),
    }
}

fn field_token_string(value: &serde_json::Value) -> Field<String> {
    match value {
        serde_json::Value::String(token) if token == "not defined" => Field::NotDefined,
        serde_json::Value::String(token) if token == "not read" => Field::NotRead,
        serde_json::Value::String(token) => Field::Value(token.clone()),
        other => panic!("section 14 token field {other}"),
    }
}

fn json_i128_number(number: &serde_json::Number) -> i128 {
    if let Some(value) = number.as_i64() {
        return i128::from(value);
    }
    if let Some(value) = number.as_u64() {
        return i128::from(value);
    }
    panic!("section 14 number {number}")
}

fn json_u128(value: &serde_json::Value) -> Option<u128> {
    value.as_u64().map(u128::from)
}

fn field_block(value: Option<&serde_json::Value>) -> Field<BlockRef> {
    let Some(value) = value else {
        return Field::Absent;
    };
    match value {
        serde_json::Value::String(token) if token == "not defined" => Field::NotDefined,
        serde_json::Value::String(token) if token == "not read" => Field::NotRead,
        serde_json::Value::Object(object) => Field::Value(BlockRef {
            block_hash: object
                .get("block_hash")
                .and_then(|item| item.as_str())
                .unwrap_or_else(|| panic!("block_hash {value}"))
                .to_owned(),
            block_height: object
                .get("block_height")
                .map(field_i128)
                .and_then(|field| match field {
                    Field::Value(height) => Some(height),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("block_height {value}")),
        }),
        other => panic!("section 14 block {other}"),
    }
}

fn optional_field_i128(value: Option<&serde_json::Value>) -> Field<i128> {
    match value {
        Some(value) => field_i128(value),
        None => Field::Absent,
    }
}

fn parse_headers(value: &serde_json::Value) -> Vec<HeaderRow> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("headers {value}"))
        .iter()
        .map(|row| HeaderRow {
            stored_hash: match row.get("stored_hash") {
                Some(serde_json::Value::String(token)) if token == "not defined" => Field::NotDefined,
                Some(serde_json::Value::String(token)) if token == "not read" => Field::NotRead,
                Some(serde_json::Value::String(hex)) => Field::Value(hex.clone()),
                Some(other) => panic!("stored_hash {other}"),
                None => Field::Absent,
            },
            stored_height: row
                .get("stored_height")
                .map(field_i128)
                .unwrap_or(Field::Absent),
            index_comparison: static_token(
                row.get("index_comparison")
                    .and_then(|item| item.as_str())
                    .unwrap_or_else(|| panic!("index_comparison {row}")),
            ),
            t: row.get("T").map(field_i128).unwrap_or(Field::Absent),
        })
        .collect()
}

fn parse_checkpoints(value: &serde_json::Value) -> Vec<CheckpointRow> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("checkpoints {value}"))
        .iter()
        .map(|row| CheckpointRow {
            g: row
                .get("g")
                .and_then(|item| item.as_u64())
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or_else(|| panic!("checkpoint g {row}")),
            legs: match row.get("legs") {
                Some(serde_json::Value::String(token)) if token == "not defined" => Field::NotDefined,
                Some(serde_json::Value::String(token)) if token == "not read" => Field::NotRead,
                Some(serde_json::Value::Array(legs)) => Field::Value(
                    legs.iter()
                        .map(|leg| LegRow {
                            j: leg
                                .get("j")
                                .and_then(|item| item.as_u64())
                                .map(u128::from)
                                .unwrap_or_else(|| panic!("leg j {leg}")),
                            result: leg
                                .get("result")
                                .and_then(|item| item.as_str())
                                .unwrap_or_else(|| panic!("leg result {leg}"))
                                .to_owned(),
                        })
                        .collect(),
                ),
                other => panic!("checkpoint legs {other:?}"),
            },
        })
        .collect()
}

fn parse_waypoints(value: &serde_json::Value) -> Vec<WaypointRow> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("waypoints {value}"))
        .iter()
        .map(|row| WaypointRow {
            g: row
                .get("g")
                .and_then(|item| item.as_u64())
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or_else(|| panic!("waypoint g {row}")),
            result: row
                .get("result")
                .and_then(|item| item.as_str())
                .unwrap_or_else(|| panic!("waypoint result {row}"))
                .to_owned(),
        })
        .collect()
}

fn parse_paths(value: &serde_json::Value) -> Vec<PathRow> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("paths {value}"))
        .iter()
        .map(|row| PathRow {
            g: row
                .get("g")
                .and_then(|item| item.as_u64())
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or_else(|| panic!("path g {row}")),
            path_state: row
                .get("path_state")
                .and_then(|item| item.as_str())
                .map(static_token),
            plant: row.get("plant").and_then(|item| item.as_str()).map(static_token),
            w17_path: row
                .get("w17_path")
                .and_then(|item| item.as_str())
                .map(static_token),
            p_g: field_block(row.get("P_g")),
            height_p_g: optional_field_i128(row.get("height_P_g")),
            d_g: optional_field_i128(row.get("d_g")),
            claimed_block: field_block(row.get("claimed_block")),
        })
        .collect()
}

fn parse_book(value: &serde_json::Value) -> Option<Bookends> {
    if value.get("a").is_none() {
        return None;
    }
    Some(Bookends {
        a: field_u32(value.get("a").unwrap()),
        b: field_u32(value.get("b").unwrap()),
        n: field_u128(value.get("N").unwrap_or_else(|| panic!("bookend N"))),
        s: field_i128(value.get("S").unwrap_or_else(|| panic!("bookend S"))),
        delta_h: field_i128(value.get("delta_h").unwrap_or_else(|| panic!("bookend delta_h"))),
        p_b: field_block(value.get("P_b")),
        height_p_b: optional_field_i128(value.get("height_P_b")),
        index_comparison_p_b: value
            .get("index_comparison_P_b")
            .map(field_token_string)
            .unwrap_or(Field::Absent),
        t_p_b: optional_field_i128(value.get("T_P_b")),
        height_i_a_minus_1: optional_field_i128(value.get("height_I_a_minus_1")),
        t_i_a_minus_1: optional_field_i128(value.get("T_I_a_minus_1")),
    })
}

fn parse_six(value: &serde_json::Value) -> SixLegRecord {
    match value {
        serde_json::Value::String(token) if token == "not defined" => SixLegRecord::NotDefined,
        serde_json::Value::String(token) if token == "not read" => SixLegRecord::NotRead,
        serde_json::Value::Object(object) => SixLegRecord::Drawn {
            indices: object
                .get("indices")
                .and_then(|item| item.as_array())
                .unwrap_or_else(|| panic!("six_leg indices {value}"))
                .iter()
                .map(|item| item.as_u64().unwrap_or_else(|| panic!("six_leg index {item}")))
                .collect(),
            results: object
                .get("results")
                .and_then(|item| item.as_array())
                .unwrap_or_else(|| panic!("six_leg results {value}"))
                .iter()
                .map(|item| {
                    static_token(item.as_str().unwrap_or_else(|| panic!("six_leg result {item}")))
                })
                .collect(),
        },
        other => panic!("six_leg {other}"),
    }
}

/// Counts that are the string `not defined` leave analysis unset. A quiet
/// object has no numeric whole-hike count to invent.
fn parse_analysis(value: &serde_json::Value) -> Option<Analysis> {
    let whole = json_u128(value.get("whole_hike_legs")?)?;
    let rows = value.get("per_stage")?.as_array()?;
    let mut per_stage = Vec::with_capacity(rows.len());
    for row in rows {
        let checkpoint_count = json_u128(row.get("checkpoint_count")?)?;
        per_stage.push(StageAnalysis {
            g: row.get("g")?.as_u64().and_then(|n| u32::try_from(n).ok())?,
            checkpoint_count,
            span: field_i128(row.get("span")?),
        });
    }
    Some(Analysis {
        whole_hike_legs: whole,
        whole_hike_span: field_i128(value.get("whole_hike_span")?),
        per_stage,
        legs_outside: field_u128(value.get("legs_outside")?),
    })
}

pub fn verify_offline(bytes: &[u8]) -> hike::Measurements {
    hike::verify(
        bytes,
        hike::AuditDraw::PinnedSeed([0u8; 32]),
        |_| Err(()),
        |_| Ok(()),
    )
}

pub fn verify_with<F, C>(bytes: &[u8], beacon_at: F, path_chain: C) -> hike::Measurements
where
    F: FnMut(u64) -> Result<hike::IndexBeacon, ()>,
    C: FnMut(&hike::PathLocators) -> Result<(), ()>,
{
    hike::verify(
        bytes,
        hike::AuditDraw::PinnedSeed([0u8; 32]),
        beacon_at,
        path_chain,
    )
}

#[derive(Clone, Debug)]
pub struct SpecStage {
    pub beacon: [u8; 32],
    pub beacon_height: u64,
    pub waypoint: [u8; 32],
    /// `None` omits the path key. Stage 0 does that.
    pub path: Option<Vec<u8>>,
    pub checkpoints: Vec<[u8; 32]>,
    pub leg_results: Vec<[u8; 8]>,
    /// Replaces the checkpoint array when set, including a short byte string.
    pub checkpoint_blobs: Option<Vec<Vec<u8>>>,
    /// Replaces the leg-result array when set.
    pub leg_result_blobs: Option<Vec<Vec<u8>>>,
    /// Writes this `leg_count` instead of the checkpoint array length.
    pub leg_count_override: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct SpecObject {
    pub kind: String,
    pub fill: String,
    pub public_key: [u8; 32],
    pub seed_beacon: [u8; 32],
    pub stages: Vec<SpecStage>,
    pub last_interrupt: Option<[u8; 32]>,
    pub last_interrupt_height: Option<u64>,
    pub slot_bytes: u64,
    pub n_slots: u64,
    pub sample_bytes: u64,
    pub steps_per_leg: u64,
}

/// One object that meets Rule A3 aside from whatever a test then changes.
pub fn baseline_spec() -> SpecObject {
    let public_key = [0x11u8; 32];
    let mut seed_beacon = [0u8; 32];
    seed_beacon[0] = 1;
    let opening = seed_value(&public_key, &seed_beacon);
    let mut stages = Vec::with_capacity(STAGES);
    for g in 0..STAGES {
        let mut beacon = [0u8; 32];
        beacon[0] = (g as u8) + 1;
        stages.push(SpecStage {
            beacon,
            beacon_height: 1_000 + g as u64,
            waypoint: if g == 0 { opening } else { [g as u8; 32] },
            path: if g == 0 { None } else { Some(Vec::new()) },
            checkpoints: vec![[0xAB; 32]],
            leg_results: vec![1u64.to_le_bytes()],
            checkpoint_blobs: None,
            leg_result_blobs: None,
            leg_count_override: None,
        });
    }
    let mut last_interrupt = [0u8; 32];
    last_interrupt[0] = 99;
    SpecObject {
        kind: hike::KIND.to_owned(),
        fill: "dag2".to_owned(),
        public_key,
        seed_beacon,
        stages,
        last_interrupt: Some(last_interrupt),
        last_interrupt_height: Some(1_000 + STAGES as u64),
        slot_bytes: hike::SLOT_BYTES as u64,
        n_slots: hike::N_SLOTS as u64,
        sample_bytes: hike::SAMPLE_BYTES as u64,
        steps_per_leg: hike::STEPS_PER_LEG,
    }
}

pub fn encode_spec(object: &SpecObject) -> Vec<u8> {
    let stages = object.stages.iter().map(encode_stage).collect::<Vec<_>>();
    let mut pairs = vec![
        ("fill", cbor_text(&object.fill)),
        ("kind", cbor_text(&object.kind)),
        ("n_slots", cbor_uint(object.n_slots)),
        ("public_key", cbor_bytes(&object.public_key)),
        ("sample_bytes", cbor_uint(object.sample_bytes)),
        ("seed_beacon", cbor_bytes(&object.seed_beacon)),
        ("slot_bytes", cbor_uint(object.slot_bytes)),
        ("stages", cbor_array(stages)),
        ("steps_per_leg", cbor_uint(object.steps_per_leg)),
    ];
    if let Some(last) = &object.last_interrupt {
        pairs.push(("last_interrupt", cbor_bytes(last)));
    }
    if let Some(height) = object.last_interrupt_height {
        pairs.push(("last_interrupt_height", cbor_uint(height)));
    }
    cbor_map(pairs)
}

fn encode_stage(stage: &SpecStage) -> Vec<u8> {
    let checkpoint_items = match &stage.checkpoint_blobs {
        Some(blobs) => blobs.iter().map(|c| cbor_bytes(c)).collect(),
        None => stage.checkpoints.iter().map(|c| cbor_bytes(c)).collect(),
    };
    let leg_items = match &stage.leg_result_blobs {
        Some(blobs) => blobs.iter().map(|c| cbor_bytes(c)).collect(),
        None => stage.leg_results.iter().map(|c| cbor_bytes(c)).collect(),
    };
    let leg_count = stage
        .leg_count_override
        .unwrap_or(match &stage.checkpoint_blobs {
            Some(blobs) => blobs.len() as u64,
            None => stage.checkpoints.len() as u64,
        });
    let mut pairs = vec![
        ("beacon", cbor_bytes(&stage.beacon)),
        ("beacon_height", cbor_uint(stage.beacon_height)),
        ("checkpoints", cbor_array(checkpoint_items)),
        ("leg_count", cbor_uint(leg_count)),
        ("leg_results", cbor_array(leg_items)),
        ("waypoint", cbor_bytes(&stage.waypoint)),
    ];
    if let Some(path) = &stage.path {
        pairs.push(("path", cbor_bytes(path)));
    }
    cbor_map(pairs)
}

fn cbor_uint(n: u64) -> Vec<u8> {
    if n < 24 {
        vec![n as u8]
    } else if n < 256 {
        vec![0x18, n as u8]
    } else if n < 65_536 {
        let b = (n as u16).to_be_bytes();
        vec![0x19, b[0], b[1]]
    } else if n <= u32::MAX as u64 {
        let b = (n as u32).to_be_bytes();
        let mut out = vec![0x1a];
        out.extend_from_slice(&b);
        out
    } else {
        let b = n.to_be_bytes();
        let mut out = vec![0x1b];
        out.extend_from_slice(&b);
        out
    }
}

fn head(major: u8, len: usize) -> Vec<u8> {
    let mut info = cbor_uint(len as u64);
    info[0] |= major << 5;
    info
}

fn cbor_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = head(2, bytes.len());
    out.extend_from_slice(bytes);
    out
}

fn cbor_text(text: &str) -> Vec<u8> {
    let mut out = head(3, text.len());
    out.extend_from_slice(text.as_bytes());
    out
}

fn cbor_array(items: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out = head(4, items.len());
    for item in items {
        out.extend_from_slice(&item);
    }
    out
}

fn cbor_map(pairs: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut out = head(5, pairs.len());
    for (key, value) in pairs {
        out.extend_from_slice(&cbor_text(key));
        out.extend_from_slice(&value);
    }
    out
}

pub fn quiet_hike() -> hike::Hike {
    let mut hike = hike::structural_object();
    for stage in &mut hike.stages {
        stage.leg_count = 0;
        stage.checkpoints.clear();
        stage.leg_results.clear();
        stage.path.clear();
    }
    hike
}

/// Natural-order lowercase hex, byte 0 first.
pub fn hex_natural(bytes: &[u8; 32]) -> String {
    hike::encode_hex32(bytes)
}

/// Big-endian height at the end of a single-leaf canonical path body.
pub fn path_body_block(path: &[u8]) -> Option<BlockRef> {
    if path.len() < 36 {
        return None;
    }
    let hash = path[path.len() - 36..path.len() - 4].try_into().ok()?;
    let height = u32::from_be_bytes(path[path.len() - 4..].try_into().ok()?);
    Some(BlockRef {
        block_hash: hex_natural(&hash),
        block_height: i128::from(height),
    })
}

#[derive(Clone, Debug)]
pub struct PlantInput {
    pub g: u32,
    pub path_state: &'static str,
    pub plant: &'static str,
    pub height_p: i128,
    pub height_i_prev: i128,
    /// `T(I_{g−1})` when that header time was read.
    pub t_i_prev: Option<i128>,
    pub block_hash: String,
}

/// Index read at `height_P_b`. main @ 546d24f L1002 and L1213.
/// `header_time` is `Some` only when `header.time` follows section 9.
#[derive(Clone, Copy, Debug)]
pub struct PbRead {
    pub index_comparison: &'static str,
    pub header_time: Option<i128>,
}

pub fn d_g(plant: &PlantInput) -> Field<i128> {
    if plant.path_state == "present" && plant.plant == "match" {
        Field::Value(plant.height_p - plant.height_i_prev)
    } else {
        Field::NotDefined
    }
}

/// Bookend block from plain inputs. `checkpoint_count[g]` is
/// `len(stages[g].checkpoints)`. It is not the object's `leg_count` field.
/// `T_P_b` is an integer only when `pb.index_comparison` is `match` and the
/// time follows section 9. main @ 546d24f L1006 and L1201.
pub fn bookends(
    plants: &[PlantInput],
    checkpoint_count: &[u128; STAGES],
    pb: PbRead,
) -> (Bookends, u128, Vec<u32>) {
    let mut anchored = Vec::new();
    let mut skip_list = Vec::new();
    for plant in plants {
        if !(1..=16).contains(&plant.g) {
            continue;
        }
        if plant.path_state == "skip" {
            skip_list.push(plant.g);
        } else if plant.path_state == "present" && plant.plant == "match" {
            anchored.push(plant.clone());
        }
    }
    skip_list.sort_unstable();
    let anchored_total = anchored.len() as u128;
    if anchored.len() < 2 {
        return (
            Bookends {
                a: Field::NotDefined,
                b: Field::NotDefined,
                n: Field::NotDefined,
                s: Field::NotDefined,
                delta_h: Field::NotDefined,
                p_b: Field::NotDefined,
                height_p_b: Field::NotDefined,
                index_comparison_p_b: Field::NotDefined,
                t_p_b: Field::NotDefined,
                height_i_a_minus_1: Field::NotDefined,
                t_i_a_minus_1: Field::NotDefined,
            },
            anchored_total,
            skip_list,
        );
    }
    anchored.sort_by_key(|plant| plant.g);
    let first = anchored[0].clone();
    let last = anchored[anchored.len() - 1].clone();
    let a = first.g;
    let b = last.g;
    let n = checkpoint_count[a as usize..=(b as usize - 1)].iter().sum();
    let delta_h = last.height_p - first.height_i_prev;
    let t_p_b = if pb.index_comparison == "match" {
        pb.header_time.map(Field::Value).unwrap_or(Field::NotRead)
    } else {
        Field::NotRead
    };
    let s = match (&t_p_b, first.t_i_prev) {
        (Field::Value(t_p), Some(t_i)) => Field::Value(t_p - t_i),
        _ => Field::NotRead,
    };
    (
        Bookends {
            a: Field::Value(a),
            b: Field::Value(b),
            n: Field::Value(n),
            s,
            delta_h: Field::Value(delta_h),
            p_b: Field::Value(BlockRef {
                block_hash: last.block_hash.clone(),
                block_height: last.height_p,
            }),
            height_p_b: Field::Value(last.height_p),
            index_comparison_p_b: Field::Value(pb.index_comparison.to_owned()),
            t_p_b,
            height_i_a_minus_1: Field::Value(first.height_i_prev),
            t_i_a_minus_1: first.t_i_prev.map(Field::Value).unwrap_or(Field::NotRead),
        },
        anchored_total,
        skip_list,
    )
}

/// `times[0]` is `T(seed_beacon)`, `times[1]` is `T(I_0)`, …, `times[18]` is `T(I_17)`.
pub fn analysis(
    checkpoint_count: &[u128; STAGES],
    times: &[Option<i128>; 19],
    book: &Bookends,
) -> Analysis {
    let whole_hike_legs = checkpoint_count[1..].iter().sum();
    let whole_hike_span = span(times[18], times[1]);
    let mut per_stage = Vec::with_capacity(STAGES);
    for g in 0..STAGES {
        let span = if g == 0 {
            span(times[1], times[0])
        } else if g == 17 {
            span(times[18], times[17])
        } else {
            span(times[g + 1], times[g])
        };
        per_stage.push(StageAnalysis {
            g: g as u32,
            checkpoint_count: checkpoint_count[g],
            span,
        });
    }
    let legs_outside = match (&book.a, &book.b) {
        (Field::Value(a), Field::Value(b)) => {
            let inside: u128 = checkpoint_count[*a as usize..=(*b as usize - 1)]
                .iter()
                .sum();
            let all: u128 = checkpoint_count.iter().sum();
            Field::Value(all - inside)
        }
        _ => Field::NotDefined,
    };
    Analysis {
        whole_hike_legs,
        whole_hike_span,
        per_stage,
        legs_outside,
    }
}

fn span(later: Option<i128>, earlier: Option<i128>) -> Field<i128> {
    match (later, earlier) {
        (Some(later), Some(earlier)) => Field::Value(later - earlier),
        _ => Field::NotRead,
    }
}

/// Stderr line Rule A3.4 names for `W_g`, g in 1..=16.
/// main @ 546d24f L880, L892. One step after the cut-off, ascending `g`.
pub fn path_skip_line(g: u32) -> String {
    format!("path_skip {g}\n")
}

/// main @ 546d24f L858, L892, L900. Last line of that same step.
pub fn w17_stderr_line() -> String {
    "not fetched by design 17\n".to_owned()
}

/// Capture of the step before the cut-off. This is not a producer run.
/// main @ 546d24f L882: nothing for those paths is written before the
/// final step. The capture stays empty until that step exists.
pub fn stderr_capture_before_cutoff_step(_skipped: &[u32]) -> String {
    String::new()
}

/// Capture of the one step after the cut-off. This is not a producer run.
/// The reference producer does not emit Rule A3.4, and a full drive is one
/// million steps per leg, so this capture stays empty.
pub fn stderr_capture_of_cutoff_step(_skipped: &[u32]) -> String {
    String::new()
}

/// Final stderr step: `path_skip <g>` in ascending `g`, then `not fetched by design 17`.
/// main @ 546d24f L858, L880, L892, L900.
pub fn expected_final_stderr(skipped: &[u32]) -> String {
    let mut gs: Vec<u32> = skipped
        .iter()
        .copied()
        .filter(|g| (1..=16).contains(g))
        .collect();
    gs.sort_unstable();
    gs.dedup();
    let mut out = String::new();
    for g in gs {
        out.push_str(&path_skip_line(g));
    }
    out.push_str(&w17_stderr_line());
    out
}

/// Closed `shape_item` strings, table order. main @ 546d24f L956–L972.
pub const SHAPE_ITEMS: [&str; 13] = [
    "malformed cbor",
    "field set",
    "stage-0 path key",
    "W_17 path",
    "constant",
    "stages length",
    "leg_count",
    "checkpoint width",
    "leg result width",
    "seed beacon",
    "last_interrupt",
    "header height order",
    "header hash differ",
];

/// 64 hex digits, either letter case, to 32 bytes. No reversal.
pub fn hex_bytes(spelling: &str) -> Option<[u8; 32]> {
    if spelling.len() != 64 || !spelling.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&spelling[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Bitcoin display order of a natural-order header hash.
pub fn display_bytes(natural: &[u8; 32]) -> [u8; 32] {
    let mut display = [0u8; 32];
    for i in 0..32 {
        display[i] = natural[31 - i];
    }
    display
}

/// One plant a portable-index document can record. `tree_size` is stored and
/// is not part of the plant comparison. main @ 546d24f L1153.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexPlant {
    pub root: [u8; 32],
    pub txid: [u8; 32],
    pub vout: u32,
    pub block_hash: [u8; 32],
    pub block_height: u32,
    pub tree_size: u32,
}

/// Canonical path body fields this batch reads from the wire bytes.
/// Offsets follow the single-leaf layout: version, leaf, leaf index, tree
/// size, sibling count, siblings, root, txid, vout, block hash, block height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathBody {
    pub leaf: [u8; 32],
    pub leaf_index: u32,
    pub tree_size: u32,
    pub siblings: usize,
    pub root: [u8; 32],
    pub txid: [u8; 32],
    pub vout: u32,
    pub block_hash: [u8; 32],
    pub block_height: u32,
}

fn array32(bytes: &[u8]) -> Option<[u8; 32]> {
    bytes.try_into().ok()
}

/// Decode a canonical path body without calling the library decoder.
pub fn path_body(bytes: &[u8]) -> Option<PathBody> {
    if bytes.first() != Some(&0x01) || bytes.len() < 146 {
        return None;
    }
    let siblings = bytes[41] as usize;
    if siblings > 32 || bytes.len() != 146 + siblings * 32 {
        return None;
    }
    let leaf = array32(&bytes[1..33])?;
    let leaf_index = u32::from_be_bytes(bytes[33..37].try_into().ok()?);
    let tree_size = u32::from_be_bytes(bytes[37..41].try_into().ok()?);
    let mut offset = 42 + siblings * 32;
    let root = array32(&bytes[offset..offset + 32])?;
    offset += 32;
    let txid = array32(&bytes[offset..offset + 32])?;
    offset += 32;
    let vout = u32::from_be_bytes(bytes[offset..offset + 4].try_into().ok()?);
    offset += 4;
    let block_hash = array32(&bytes[offset..offset + 32])?;
    offset += 32;
    let block_height = u32::from_be_bytes(bytes[offset..offset + 4].try_into().ok()?);
    Some(PathBody {
        leaf,
        leaf_index,
        tree_size,
        siblings,
        root,
        txid,
        vout,
        block_hash,
        block_height,
    })
}

/// Merkle leaf node: SHA-256(0x1A ‖ leaf). One-leaf fixtures commit `root` to that.
pub fn merkle_leaf_root(leaf: &[u8; 32]) -> [u8; 32] {
    let mut pre = [0u8; 33];
    pre[0] = 0x1A;
    pre[1..].copy_from_slice(leaf);
    sha256(&pre)
}

/// Step 1 of the plant measurement for a one-leaf body: the leaf is the
/// waypoint and the Merkle path commits that leaf to `root`.
pub fn one_leaf_validates_for(bytes: &[u8], waypoint: &[u8; 32]) -> bool {
    let Some(body) = path_body(bytes) else {
        return false;
    };
    body.leaf == *waypoint
        && body.siblings == 0
        && body.tree_size == 1
        && body.leaf_index == 0
        && body.root == merkle_leaf_root(&body.leaf)
}

fn json_string(text: &str) -> String {
    format!("\"{text}\"")
}

/// Portable-index document. `plants` can hold more than one plant at this height.
/// `header.time` is unix seconds. `timestamp_unix_ms` is a different member.
pub fn index_document(
    network: &str,
    header_hash_display: &str,
    height: u64,
    time: Option<i128>,
    timestamp_unix_ms: Option<u64>,
    plants: &[IndexPlant],
) -> String {
    let mut header = format!(
        "\"hash\":{},\"height\":{height}",
        json_string(header_hash_display)
    );
    if let Some(time) = time {
        header.push_str(&format!(",\"time\":{time}"));
    }
    if let Some(millis) = timestamp_unix_ms {
        header.push_str(&format!(",\"timestamp_unix_ms\":{millis}"));
    }
    let mut plant_items = Vec::new();
    for plant in plants {
        plant_items.push(format!(
            "{{\"root\":{},\"txid\":{},\"vout\":{},\"block_hash\":{},\"block_height\":{},\"tree_size\":{}}}",
            json_string(&hex_natural(&plant.root)),
            json_string(&hex_natural(&plant.txid)),
            plant.vout,
            json_string(&hex_natural(&plant.block_hash)),
            plant.block_height,
            plant.tree_size,
        ));
    }
    format!(
        "{{\"network\":{},\"header\":{{{header}}},\"plants\":[{}]}}",
        json_string(network),
        plant_items.join(",")
    )
}

fn object_after<'a>(doc: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("\"{key}\":");
    let start = doc.find(&marker)? + marker.len();
    Some(&doc[start..])
}

fn json_int(text: &str) -> Option<i128> {
    let rest = text.trim_start();
    if rest.starts_with('-') {
        return None;
    }
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    rest[..end].parse().ok()
}

/// `header.time` when it is a JSON integer in `0 … 2^32 − 1`. main @ 546d24f L420.
pub fn header_time(doc: &str) -> Option<i128> {
    let header = object_after(doc, "header")?;
    let time_at = header.find("\"time\":")?;
    let time = json_int(&header[time_at + "\"time\":".len()..])?;
    if (0..=u32::MAX as i128).contains(&time) {
        Some(time)
    } else {
        None
    }
}

/// `header.timestamp_unix_ms` when it is a JSON integer. Not a `T` source.
pub fn header_timestamp_unix_ms(doc: &str) -> Option<u64> {
    let header = object_after(doc, "header")?;
    let at = header.find("\"timestamp_unix_ms\":")?;
    let n = json_int(&header[at + "\"timestamp_unix_ms\":".len()..])?;
    u64::try_from(n).ok()
}

pub fn header_hash_bytes(doc: &str) -> Option<[u8; 32]> {
    let header = object_after(doc, "header")?;
    let at = header.find("\"hash\":\"")?;
    let spelling = &header[at + "\"hash\":\"".len()..];
    let spelling = &spelling[..spelling.find('"')?];
    hex_bytes(spelling)
}

pub fn document_network(doc: &str) -> Option<String> {
    let at = doc.find("\"network\":\"")?;
    let spelling = &doc[at + "\"network\":\"".len()..];
    let spelling = &spelling[..spelling.find('"')?];
    Some(spelling.to_owned())
}

pub fn document_plants(doc: &str) -> Vec<IndexPlant> {
    let Some(array) = object_after(doc, "plants") else {
        return Vec::new();
    };
    if !array.starts_with('[') {
        return Vec::new();
    }
    let mut plants = Vec::new();
    for item in array[1..].split("},{") {
        let root = match quoted_hex(item, "root") {
            Some(root) => root,
            None => continue,
        };
        let Some(txid) = quoted_hex(item, "txid") else {
            continue;
        };
        let Some(block_hash) = quoted_hex(item, "block_hash") else {
            continue;
        };
        let Some(vout_at) = item.find("\"vout\":") else {
            continue;
        };
        let Some(vout) = json_int(&item[vout_at + "\"vout\":".len()..]) else {
            continue;
        };
        let Some(height_at) = item.find("\"block_height\":") else {
            continue;
        };
        let Some(block_height) = json_int(&item[height_at + "\"block_height\":".len()..]) else {
            continue;
        };
        let tree_size = item
            .find("\"tree_size\":")
            .and_then(|at| json_int(&item[at + "\"tree_size\":".len()..]))
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0);
        let Ok(vout) = u32::try_from(vout) else {
            continue;
        };
        let Ok(block_height) = u32::try_from(block_height) else {
            continue;
        };
        plants.push(IndexPlant {
            root,
            txid,
            vout,
            block_hash,
            block_height,
            tree_size,
        });
    }
    plants
}

fn quoted_hex(item: &str, key: &str) -> Option<[u8; 32]> {
    let marker = format!("\"{key}\":\"");
    let at = item.find(&marker)?;
    let spelling = &item[at + marker.len()..];
    let spelling = &spelling[..spelling.find('"')?];
    hex_bytes(spelling)
}

/// Plant token for one present path. main @ 546d24f L1103–L1157.
/// `document` is `None` when `GET /index/{block_height}` does not return one.
pub fn plant_token(bytes: &[u8], waypoint: &[u8; 32], document: Option<&str>) -> &'static str {
    if !one_leaf_validates_for(bytes, waypoint) {
        return "mismatch";
    }
    let Some(document) = document else {
        return "not read";
    };
    if document_network(document).as_deref() != Some("main") {
        return "mismatch";
    }
    let Some(body) = path_body(bytes) else {
        return "mismatch";
    };
    let matched = document_plants(document).into_iter().any(|plant| {
        plant.root == body.root
            && plant.txid == body.txid
            && plant.vout == body.vout
            && plant.block_hash == body.block_hash
            && plant.block_height == body.block_height
    });
    if matched {
        "match"
    } else {
        "mismatch"
    }
}

#[derive(Clone, Debug)]
enum CborVal {
    Uint(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<CborVal>),
    Map(Vec<(String, CborVal)>),
}

fn take(input: &[u8], n: usize) -> Result<(&[u8], &[u8]), ()> {
    if input.len() < n {
        return Err(());
    }
    Ok((&input[..n], &input[n..]))
}

fn decode_cbor(input: &[u8]) -> Result<(CborVal, &[u8]), ()> {
    let (head, rest) = take(input, 1)?;
    let major = head[0] >> 5;
    let info = head[0] & 0x1f;
    let (len, rest) = match info {
        n if n < 24 => (n as u64, rest),
        24 => {
            let (b, rest) = take(rest, 1)?;
            (b[0] as u64, rest)
        }
        25 => {
            let (b, rest) = take(rest, 2)?;
            (u16::from_be_bytes([b[0], b[1]]) as u64, rest)
        }
        26 => {
            let (b, rest) = take(rest, 4)?;
            (u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64, rest)
        }
        27 => {
            let (b, rest) = take(rest, 8)?;
            (
                u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
                rest,
            )
        }
        _ => return Err(()),
    };
    match major {
        0 => Ok((CborVal::Uint(len), rest)),
        2 => {
            let (bytes, rest) = take(rest, len as usize)?;
            Ok((CborVal::Bytes(bytes.to_vec()), rest))
        }
        3 => {
            let (bytes, rest) = take(rest, len as usize)?;
            let text = std::str::from_utf8(bytes).map_err(|_| ())?;
            Ok((CborVal::Text(text.to_owned()), rest))
        }
        4 => {
            let mut items = Vec::with_capacity(len as usize);
            let mut rest = rest;
            for _ in 0..len {
                let (item, next) = decode_cbor(rest)?;
                items.push(item);
                rest = next;
            }
            Ok((CborVal::Array(items), rest))
        }
        5 => {
            let mut pairs = Vec::with_capacity(len as usize);
            let mut rest = rest;
            for _ in 0..len {
                let (key, next) = decode_cbor(rest)?;
                let CborVal::Text(key) = key else {
                    return Err(());
                };
                let (value, next) = decode_cbor(next)?;
                pairs.push((key, value));
                rest = next;
            }
            Ok((CborVal::Map(pairs), rest))
        }
        _ => Err(()),
    }
}

fn map_get<'a>(pairs: &'a [(String, CborVal)], key: &str) -> Option<&'a CborVal> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn as_bytes32(value: &CborVal) -> Result<[u8; 32], ()> {
    let CborVal::Bytes(bytes) = value else {
        return Err(());
    };
    bytes.as_slice().try_into().map_err(|_| ())
}

fn as_uint(value: &CborVal) -> Result<u64, ()> {
    match value {
        CborVal::Uint(n) => Ok(*n),
        _ => Err(()),
    }
}

/// Stored fields of an object the current encoder writes. This reads bytes.
/// It does not recompute a digest.
#[derive(Clone, Debug)]
pub struct StoredStage {
    pub beacon: [u8; 32],
    pub beacon_height: u64,
    pub leg_count: u64,
    pub waypoint: [u8; 32],
    pub path: Vec<u8>,
    pub checkpoints: Vec<[u8; 32]>,
    pub leg_results: Vec<[u8; 8]>,
}

#[derive(Clone, Debug)]
pub struct StoredHike {
    pub public_key: [u8; 32],
    pub seed_beacon: [u8; 32],
    pub stages: Vec<StoredStage>,
}

pub fn read_stored_hike(bytes: &[u8]) -> Result<StoredHike, ()> {
    let (value, rest) = decode_cbor(bytes)?;
    if !rest.is_empty() {
        return Err(());
    }
    let CborVal::Map(top) = value else {
        return Err(());
    };
    let stages = match map_get(&top, "stages") {
        Some(CborVal::Array(items)) => items,
        _ => return Err(()),
    };
    let mut out = Vec::with_capacity(stages.len());
    for stage in stages {
        let CborVal::Map(fields) = stage else {
            return Err(());
        };
        let checkpoints = match map_get(fields, "checkpoints") {
            Some(CborVal::Array(items)) => items
                .iter()
                .map(as_bytes32)
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(()),
        };
        let leg_results = match map_get(fields, "leg_results") {
            Some(CborVal::Array(items)) => items
                .iter()
                .map(|item| {
                    let CborVal::Bytes(bytes) = item else {
                        return Err(());
                    };
                    bytes.as_slice().try_into().map_err(|_| ())
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(()),
        };
        let path = match map_get(fields, "path") {
            Some(CborVal::Bytes(bytes)) => bytes.clone(),
            // Stage 0 omits the key. A later stage that omits it is still empty bytes here.
            None => Vec::new(),
            _ => return Err(()),
        };
        out.push(StoredStage {
            beacon: as_bytes32(map_get(fields, "beacon").ok_or(())?)?,
            beacon_height: as_uint(map_get(fields, "beacon_height").ok_or(())?)?,
            leg_count: as_uint(map_get(fields, "leg_count").ok_or(())?)?,
            waypoint: as_bytes32(map_get(fields, "waypoint").ok_or(())?)?,
            path,
            checkpoints,
            leg_results,
        });
    }
    Ok(StoredHike {
        public_key: as_bytes32(map_get(&top, "public_key").ok_or(())?)?,
        seed_beacon: as_bytes32(map_get(&top, "seed_beacon").ok_or(())?)?,
        stages: out,
    })
}

/// One stage, including a missing `path` key. Stage 0 omits that key.
/// main @ 546d24f L353 and L384.
#[derive(Clone, Debug)]
pub struct WireStage {
    pub beacon: [u8; 32],
    pub beacon_height: u64,
    pub leg_count: u64,
    pub waypoint: [u8; 32],
    pub path: Option<Vec<u8>>,
    pub checkpoints: Vec<[u8; 32]>,
    pub leg_results: Vec<[u8; 8]>,
}

/// Object bytes, including a stage map that has no `path` key and a top-level
/// map that has no `last_interrupt`.
#[derive(Clone, Debug)]
pub struct WireHike {
    pub public_key: [u8; 32],
    pub seed_beacon: [u8; 32],
    pub stages: Vec<WireStage>,
    pub last_interrupt: Option<[u8; 32]>,
    pub last_interrupt_height: Option<u64>,
}

/// Reads the stored object without requiring a `path` key or `last_interrupt`.
pub fn read_wire_hike(bytes: &[u8]) -> Result<WireHike, ()> {
    let (value, rest) = decode_cbor(bytes)?;
    if !rest.is_empty() {
        return Err(());
    }
    let CborVal::Map(top) = value else {
        return Err(());
    };
    let stages = match map_get(&top, "stages") {
        Some(CborVal::Array(items)) => items,
        _ => return Err(()),
    };
    let mut out = Vec::with_capacity(stages.len());
    for stage in stages {
        let CborVal::Map(fields) = stage else {
            return Err(());
        };
        let checkpoints = match map_get(fields, "checkpoints") {
            Some(CborVal::Array(items)) => items
                .iter()
                .map(as_bytes32)
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(()),
        };
        let leg_results = match map_get(fields, "leg_results") {
            Some(CborVal::Array(items)) => items
                .iter()
                .map(|item| {
                    let CborVal::Bytes(bytes) = item else {
                        return Err(());
                    };
                    bytes.as_slice().try_into().map_err(|_| ())
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(()),
        };
        let path = match map_get(fields, "path") {
            Some(CborVal::Bytes(bytes)) => Some(bytes.clone()),
            None => None,
            _ => return Err(()),
        };
        out.push(WireStage {
            beacon: as_bytes32(map_get(fields, "beacon").ok_or(())?)?,
            beacon_height: as_uint(map_get(fields, "beacon_height").ok_or(())?)?,
            leg_count: as_uint(map_get(fields, "leg_count").ok_or(())?)?,
            waypoint: as_bytes32(map_get(fields, "waypoint").ok_or(())?)?,
            path,
            checkpoints,
            leg_results,
        });
    }
    let last_interrupt = match map_get(&top, "last_interrupt") {
        Some(value) => Some(as_bytes32(value)?),
        None => None,
    };
    let last_interrupt_height = match map_get(&top, "last_interrupt_height") {
        Some(value) => Some(as_uint(value)?),
        None => None,
    };
    Ok(WireHike {
        public_key: as_bytes32(map_get(&top, "public_key").ok_or(())?)?,
        seed_beacon: as_bytes32(map_get(&top, "seed_beacon").ok_or(())?)?,
        stages: out,
        last_interrupt,
        last_interrupt_height,
    })
}

pub fn stored_as_hike(stored: &StoredHike) -> hike::Hike {
    hike::Hike {
        public_key: stored.public_key,
        seed_beacon: stored.seed_beacon,
        stages: stored
            .stages
            .iter()
            .map(|stage| hike::Stage {
                beacon: stage.beacon,
                beacon_height: stage.beacon_height,
                leg_count: stage.leg_count,
                waypoint: stage.waypoint,
                path: stage.path.clone(),
                checkpoints: stage.checkpoints.clone(),
                leg_results: stage.leg_results.clone(),
                checkpoint_rows_defined: true,
                leg_result_missing: Vec::new(),
            })
            .collect(),
        last_interrupt: [0u8; 32],
        last_interrupt_height: 0,
        last_interrupt_absent: true,
        last_interrupt_hash_absent: true,
        last_interrupt_height_absent: true,
    }
}

/// Hand numbers for the bookend examples. Stages 0..17.
pub fn prime_leg_counts() -> [u128; STAGES] {
    [
        2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61,
    ]
}
