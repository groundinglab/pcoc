//! SPEC §8 / Rule A3 and §7.3. Hike object encode and decode.

use crate::cbor::{self, Cbor};
use crate::constants::{HEIGHT_MAX, KIND, STAGES};

/// One stage map. `path` is `None` when the key is absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stage {
    pub beacon: [u8; 32],
    pub beacon_height: u64,
    pub height_over: bool,
    pub leg_count: u64,
    pub waypoint: [u8; 32],
    pub path: Option<Vec<u8>>,
    pub checkpoints: Vec<Vec<u8>>,
    pub leg_results: Vec<Vec<u8>>,
    pub usable: bool,
}

/// A decoded object whose `kind` text was read. Value checks live in section 14.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HikeObject {
    pub kind: String,
    pub fill: String,
    pub slot_bytes: u64,
    pub n_slots: u64,
    pub sample_bytes: u64,
    pub steps_per_leg: u64,
    pub public_key: [u8; 32],
    pub seed_beacon: [u8; 32],
    pub stages: Vec<Stage>,
    pub last_interrupt: [u8; 32],
    pub last_interrupt_height: u64,
    pub last_height_over: bool,
    pub has_last: bool,
    pub has_last_height: bool,
    pub field_set: bool,
    pub stage0_path: bool,
    pub trailing: bool,
    pub top_ok: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// SPEC §7.3 step 2 (erratum E14). One well-formed item that is not a map,
    /// or a map with no `kind` text. No section 14 record. CLI exit 1.
    DecodeFailure,
    /// SPEC §7.3 step 1. Not one definite-length CBOR item, so there is no
    /// object to measure. The caller records `malformed cbor`.
    Malformed,
    /// SPEC Rule A5. Kind text is not `KIND`, and the input is one well-formed item.
    UnknownKind { kind_text: String },
}

fn as_bytes32(v: &Cbor) -> Option<[u8; 32]> {
    if let Cbor::Bytes(b) = v {
        if b.len() == 32 {
            let mut a = [0u8; 32];
            a.copy_from_slice(b);
            return Some(a);
        }
    }
    None
}

fn as_uint(v: &Cbor) -> Option<(u64, bool)> {
    if let Cbor::Uint(n) = v {
        return Some((*n, *n > HEIGHT_MAX));
    }
    None
}

fn blank_stage() -> Stage {
    Stage {
        beacon: [0u8; 32],
        beacon_height: 0,
        height_over: false,
        leg_count: 0,
        waypoint: [0u8; 32],
        path: None,
        checkpoints: Vec::new(),
        leg_results: Vec::new(),
        usable: false,
    }
}

fn parse_stage(v: &Cbor, index: usize) -> (Stage, bool, bool) {
    let Cbor::Map(map) = v else {
        return (blank_stage(), true, false);
    };
    let mut field_set = false;
    let mut stage0_path = false;
    let expect: &[&str] = if index == 0 {
        &[
            "beacon",
            "waypoint",
            "leg_count",
            "checkpoints",
            "leg_results",
            "beacon_height",
        ]
    } else {
        &[
            "path",
            "beacon",
            "waypoint",
            "leg_count",
            "checkpoints",
            "leg_results",
            "beacon_height",
        ]
    };
    for (k, _) in map {
        if !expect.contains(&k.as_str()) {
            field_set = true;
            if index == 0 && k == "path" {
                stage0_path = true;
            }
        }
    }
    for k in expect {
        if !map.iter().any(|(ek, _)| ek == k) {
            field_set = true;
        }
    }
    let mut st = blank_stage();
    if let Some(Cbor::Bytes(b)) = cbor::map_get(map, "beacon") {
        if b.len() == 32 {
            st.beacon.copy_from_slice(b);
        } else {
            field_set = true;
        }
    } else if cbor::map_get(map, "beacon").is_some() {
        field_set = true;
    }
    if let Some(Cbor::Bytes(b)) = cbor::map_get(map, "waypoint") {
        if b.len() == 32 {
            st.waypoint.copy_from_slice(b);
        } else {
            field_set = true;
        }
    } else if cbor::map_get(map, "waypoint").is_some() {
        field_set = true;
    }
    if let Some(Cbor::Uint(n)) = cbor::map_get(map, "leg_count") {
        st.leg_count = *n;
    } else if cbor::map_get(map, "leg_count").is_some() {
        field_set = true;
    }
    if let Some((n, over)) = cbor::map_get(map, "beacon_height").and_then(as_uint) {
        st.beacon_height = n;
        st.height_over = over;
    } else if cbor::map_get(map, "beacon_height").is_some() {
        field_set = true;
    }
    if index > 0 {
        if let Some(Cbor::Bytes(b)) = cbor::map_get(map, "path") {
            st.path = Some(b.clone());
        } else if cbor::map_get(map, "path").is_some() {
            field_set = true;
            st.path = Some(Vec::new());
        }
    } else if let Some(Cbor::Bytes(b)) = cbor::map_get(map, "path") {
        stage0_path = true;
        field_set = true;
        st.path = Some(b.clone());
    } else if cbor::map_get(map, "path").is_some() {
        stage0_path = true;
        field_set = true;
    }
    if let Some(Cbor::Array(items)) = cbor::map_get(map, "checkpoints") {
        for it in items {
            if let Cbor::Bytes(b) = it {
                st.checkpoints.push(b.clone());
            } else {
                field_set = true;
                st.checkpoints.push(Vec::new());
            }
        }
    } else if cbor::map_get(map, "checkpoints").is_some() {
        field_set = true;
    }
    if let Some(Cbor::Array(items)) = cbor::map_get(map, "leg_results") {
        for it in items {
            if let Cbor::Bytes(b) = it {
                st.leg_results.push(b.clone());
            } else {
                field_set = true;
                st.leg_results.push(Vec::new());
            }
        }
    } else if cbor::map_get(map, "leg_results").is_some() {
        field_set = true;
    }
    st.usable = !field_set;
    (st, field_set, stage0_path)
}

/// SPEC §7.3 / §8. Decode one object. Erratum E14 orders the outcomes.
///
/// 1. `decode_item` failing (empty, truncated, not CBOR, invalid UTF-8 text,
///    or a malformed item) is `Malformed`. A well-formed non-map with bytes
///    after it is `Malformed` too. SPEC §7.3 (erratum E14), first sort.
/// 2. One well-formed non-map with nothing after it, or a map with no `kind`
///    text and nothing after it, is `DecodeFailure`. No section 14 record.
///    CLI exit 1. SPEC §7.3 second sort; §18.
/// 3. A map whose kind text is not `KIND`, with nothing after it, is Rule A5.
/// 4. Every other map, including `KIND` with bytes after it and a foreign kind
///    with bytes after it, is measured. Those bytes set `HikeObject::trailing`,
///    and shape then reports only `malformed cbor`. Later measurements still run.
pub fn decode_object(data: &[u8]) -> Result<HikeObject, DecodeError> {
    let (item, used) = match cbor::decode_item(data) {
        Ok(v) => v,
        Err(_) => return Err(DecodeError::Malformed),
    };
    let trailing = used != data.len();
    let Cbor::Map(map) = item else {
        return if trailing {
            Err(DecodeError::Malformed)
        } else {
            Err(DecodeError::DecodeFailure)
        };
    };
    let kind = match cbor::map_get(&map, "kind") {
        Some(Cbor::Text(t)) => t.clone(),
        _ => {
            if trailing {
                String::new()
            } else {
                return Err(DecodeError::DecodeFailure);
            }
        }
    };
    if kind != KIND && !trailing {
        return Err(DecodeError::UnknownKind { kind_text: kind });
    }
    let top_keys = [
        "fill",
        "kind",
        "stages",
        "n_slots",
        "public_key",
        "slot_bytes",
        "seed_beacon",
        "sample_bytes",
        "steps_per_leg",
        "last_interrupt",
        "last_interrupt_height",
    ];
    let mut field_set = false;
    for (k, _) in &map {
        if !top_keys.contains(&k.as_str()) {
            field_set = true;
        }
    }
    for k in top_keys {
        if !map.iter().any(|(ek, _)| ek == k) {
            field_set = true;
        }
    }
    let fill = match cbor::map_get(&map, "fill") {
        Some(Cbor::Text(t)) => t.clone(),
        Some(_) => {
            field_set = true;
            String::new()
        }
        None => String::new(),
    };
    let slot_bytes = match cbor::map_get(&map, "slot_bytes") {
        Some(Cbor::Uint(n)) => *n,
        Some(_) => {
            field_set = true;
            0
        }
        None => 0,
    };
    let n_slots = match cbor::map_get(&map, "n_slots") {
        Some(Cbor::Uint(n)) => *n,
        Some(_) => {
            field_set = true;
            0
        }
        None => 0,
    };
    let sample_bytes = match cbor::map_get(&map, "sample_bytes") {
        Some(Cbor::Uint(n)) => *n,
        Some(_) => {
            field_set = true;
            0
        }
        None => 0,
    };
    let steps_per_leg = match cbor::map_get(&map, "steps_per_leg") {
        Some(Cbor::Uint(n)) => *n,
        Some(_) => {
            field_set = true;
            0
        }
        None => 0,
    };
    let public_key = match cbor::map_get(&map, "public_key").and_then(as_bytes32) {
        Some(b) => b,
        None => {
            if cbor::map_get(&map, "public_key").is_some() {
                field_set = true;
            }
            [0u8; 32]
        }
    };
    let seed_beacon = match cbor::map_get(&map, "seed_beacon").and_then(as_bytes32) {
        Some(b) => b,
        None => {
            if cbor::map_get(&map, "seed_beacon").is_some() {
                field_set = true;
            }
            [0u8; 32]
        }
    };
    let (last_interrupt, has_last) = match cbor::map_get(&map, "last_interrupt").and_then(as_bytes32)
    {
        Some(b) => (b, true),
        None => {
            if cbor::map_get(&map, "last_interrupt").is_some() {
                field_set = true;
            }
            ([0u8; 32], false)
        }
    };
    let (last_interrupt_height, last_height_over, has_last_height) =
        match cbor::map_get(&map, "last_interrupt_height").and_then(as_uint) {
            Some((n, over)) => (n, over, true),
            None => {
                if cbor::map_get(&map, "last_interrupt_height").is_some() {
                    field_set = true;
                }
                (0, false, false)
            }
        };
    let mut stages = Vec::new();
    let mut stage0_path = false;
    let mut top_ok = !field_set;
    match cbor::map_get(&map, "stages") {
        Some(Cbor::Array(items)) => {
            for (i, it) in items.iter().enumerate() {
                let (st, fs, s0) = parse_stage(it, i);
                if fs {
                    field_set = true;
                }
                if s0 {
                    stage0_path = true;
                }
                if !st.usable {
                    top_ok = false;
                }
                stages.push(st);
            }
        }
        Some(_) => {
            field_set = true;
            top_ok = false;
        }
        None => {
            top_ok = false;
        }
    }
    if field_set {
        top_ok = false;
    }
    Ok(HikeObject {
        kind,
        fill,
        slot_bytes,
        n_slots,
        sample_bytes,
        steps_per_leg,
        public_key,
        seed_beacon,
        stages,
        last_interrupt,
        last_interrupt_height,
        last_height_over,
        has_last,
        has_last_height,
        field_set,
        stage0_path,
        trailing,
        top_ok,
    })
}

fn stage_map(stage: &Stage, index: usize) -> Vec<u8> {
    let mut pairs: Vec<(String, Vec<u8>)> = Vec::new();
    pairs.push(("beacon".into(), cbor::encode_bytes(&stage.beacon)));
    pairs.push((
        "beacon_height".into(),
        cbor::encode_uint(stage.beacon_height),
    ));
    pairs.push(("leg_count".into(), cbor::encode_uint(stage.leg_count)));
    pairs.push(("waypoint".into(), cbor::encode_bytes(&stage.waypoint)));
    let cps: Vec<Vec<u8>> = stage
        .checkpoints
        .iter()
        .map(|c| cbor::encode_bytes(c))
        .collect();
    pairs.push(("checkpoints".into(), cbor::encode_array(&cps)));
    let lrs: Vec<Vec<u8>> = stage
        .leg_results
        .iter()
        .map(|c| cbor::encode_bytes(c))
        .collect();
    pairs.push(("leg_results".into(), cbor::encode_array(&lrs)));
    if index > 0 {
        let path = stage.path.as_deref().unwrap_or(b"");
        pairs.push(("path".into(), cbor::encode_bytes(path)));
    }
    cbor::encode_map(&pairs)
}

/// SPEC §7.3 / §8. Canonical object bytes. Stage 0 omits `path`.
pub fn encode_object(obj: &HikeObject) -> Vec<u8> {
    debug_assert_eq!(obj.stages.len(), STAGES);
    let stages: Vec<Vec<u8>> = obj
        .stages
        .iter()
        .enumerate()
        .map(|(i, s)| stage_map(s, i))
        .collect();
    let pairs = vec![
        ("kind".into(), cbor::encode_text(&obj.kind)),
        ("fill".into(), cbor::encode_text(&obj.fill)),
        ("slot_bytes".into(), cbor::encode_uint(obj.slot_bytes)),
        ("n_slots".into(), cbor::encode_uint(obj.n_slots)),
        ("sample_bytes".into(), cbor::encode_uint(obj.sample_bytes)),
        (
            "steps_per_leg".into(),
            cbor::encode_uint(obj.steps_per_leg),
        ),
        ("public_key".into(), cbor::encode_bytes(&obj.public_key)),
        ("seed_beacon".into(), cbor::encode_bytes(&obj.seed_beacon)),
        ("stages".into(), cbor::encode_array(&stages)),
        (
            "last_interrupt".into(),
            cbor::encode_bytes(&obj.last_interrupt),
        ),
        (
            "last_interrupt_height".into(),
            cbor::encode_uint(obj.last_interrupt_height),
        ),
    ];
    cbor::encode_map(&pairs)
}

/// Build a clean object for produce. Checkpoints and leg results are raw bytes.
pub fn make_object(
    fill: &str,
    profile_slot: u64,
    profile_slots: u64,
    profile_sample: u64,
    profile_steps: u64,
    public_key: [u8; 32],
    seed_beacon: [u8; 32],
    stages: Vec<Stage>,
    last_interrupt: [u8; 32],
    last_interrupt_height: u64,
) -> HikeObject {
    HikeObject {
        kind: KIND.to_string(),
        fill: fill.to_string(),
        slot_bytes: profile_slot,
        n_slots: profile_slots,
        sample_bytes: profile_sample,
        steps_per_leg: profile_steps,
        public_key,
        seed_beacon,
        stages,
        last_interrupt,
        last_interrupt_height,
        last_height_over: last_interrupt_height > HEIGHT_MAX,
        has_last: true,
        has_last_height: true,
        field_set: false,
        stage0_path: false,
        trailing: false,
        top_ok: true,
    }
}
