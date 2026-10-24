//! SPEC §14. The verify record, including Rule A5. No admission judgment.

use crate::checkpoint::checkpoint;
use crate::constants::{FILL, HEIGHT_MAX, KIND, SIX_LEG_COUNT, STAGES};
use crate::draw::{self, DrawStop};
use crate::hexutil::{hex_encode, reverse32};
use crate::indexdoc;
use crate::jsonutil::{unknown_kind_record, J};
use crate::object::{decode_object, DecodeError, HikeObject, Stage};
use crate::path::{self, PathBody};
use crate::profile::Profile;
use crate::step::leg_result;
use crate::terrain::{self, terrain_from_seed};
use crate::waypoint::{fold_checkpoints, waypoint};
use std::collections::HashMap;

/// SPEC §14. One read of `GET {index-url}/index/{height}`. `None` is not a document.
pub trait HeightIndex {
    fn read_height(&mut self, height: u64) -> Option<Vec<u8>>;
}

/// An index that returns no document.
pub struct EmptyIndex;

impl HeightIndex for EmptyIndex {
    fn read_height(&mut self, _height: u64) -> Option<Vec<u8>> {
        None
    }
}

/// Documents already in hand, keyed by height. Each call returns a fresh copy.
pub struct MapIndex {
    pub docs: HashMap<u64, Vec<u8>>,
}

impl HeightIndex for MapIndex {
    fn read_height(&mut self, height: u64) -> Option<Vec<u8>> {
        self.docs.get(&height).cloned()
    }
}

/// What verify produced. `Record` and `UnknownKind` are exit 0. `DecodeFailure` is exit 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verified {
    /// Section 14 record, no trailing newline.
    Record(String),
    /// Rule A5 record, no trailing newline.
    UnknownKind(String),
    /// SPEC §7.3 step 2. One well-formed item that is not a map, or a map with no kind text.
    DecodeFailure,
}

/// SPEC §14.7 / Rule A5 fast pre-check. Decode, kind, and shape only.
/// No terrain fill and no leg. Any kind other than `KIND` is Rule A5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Precheck {
    /// Section 14 shape of a `KIND` object, or of a malformed value that still parsed.
    Shape {
        shape: &'static str,
        shape_item: Vec<&'static str>,
    },
    /// Rule A5 record, no trailing newline.
    UnknownKind(String),
    /// SPEC §7.3 step 2. No record.
    DecodeFailure,
    /// SPEC §7.3 step 1. `shape_item` is exactly `malformed cbor`.
    Malformed,
}

/// SPEC §14.7. Decode plus kind plus shape. Does not fill a terrain and does not draw.
pub fn precheck(bytes: &[u8], profile: &Profile) -> Precheck {
    match decode_object(bytes) {
        Ok(obj) => {
            let shape_item = shape_items(&obj, profile);
            let shape = if shape_item.is_empty() {
                "match"
            } else {
                "mismatch"
            };
            Precheck::Shape { shape, shape_item }
        }
        Err(DecodeError::UnknownKind { kind_text }) => {
            Precheck::UnknownKind(unknown_kind_record(&kind_text))
        }
        Err(DecodeError::DecodeFailure) => Precheck::DecodeFailure,
        Err(DecodeError::Malformed) => Precheck::Malformed,
    }
}

/// SPEC §14. Verify one object. The six-leg draw reads the operating-system CSPRNG.
/// It takes no byte source, label, or seed (§14.6). The draw finishes before any leg is recomputed.
pub fn verify(bytes: &[u8], profile: &Profile, index: &mut dyn HeightIndex) -> Verified {
    verify_inner(bytes, profile, index, None)
}

/// Rig hook for `pcoc_testing::verify_from_bytes`. Not the release path.
/// The closure is the section 14.6 byte source. Production verify does not call this.
#[doc(hidden)]
pub fn verify_rig(
    bytes: &[u8],
    profile: &Profile,
    index: &mut dyn HeightIndex,
    draw_next: &mut dyn FnMut() -> Option<[u8; 8]>,
) -> Verified {
    verify_inner(bytes, profile, index, Some(draw_next))
}

fn verify_inner(
    bytes: &[u8],
    profile: &Profile,
    index: &mut dyn HeightIndex,
    draw_next: Option<&mut dyn FnMut() -> Option<[u8; 8]>>,
) -> Verified {
    let obj = match decode_object(bytes) {
        Ok(o) => o,
        Err(DecodeError::UnknownKind { kind_text }) => {
            return Verified::UnknownKind(unknown_kind_record(&kind_text));
        }
        Err(DecodeError::DecodeFailure) => return Verified::DecodeFailure,
        Err(DecodeError::Malformed) => {
            // Erratum 2 (E02). There is no stage and no path byte string.
            // Path fields and checkpoint counts are `not defined`. `skip_list` is `[]`.
            return Verified::Record(record_of(
                &blank_malformed(),
                profile,
                index,
                None,
                false,
                true,
            ));
        }
    };
    // A parsed map is measured, including trailing bytes (shape is exactly
    // `malformed cbor`). An unparsed input never reaches here, so a garbage
    // byte string does not fill a terrain.
    Verified::Record(record_of(&obj, profile, index, draw_next, true, false))
}

fn blank_malformed() -> HikeObject {
    HikeObject {
        kind: String::new(),
        fill: String::new(),
        slot_bytes: 0,
        n_slots: 0,
        sample_bytes: 0,
        steps_per_leg: 0,
        public_key: [0u8; 32],
        seed_beacon: [0u8; 32],
        stages: Vec::new(),
        last_interrupt: [0u8; 32],
        last_interrupt_height: 0,
        last_height_over: false,
        has_last: false,
        has_last_height: false,
        field_set: false,
        stage0_path: false,
        trailing: true,
        top_ok: false,
    }
}

fn record_of(
    obj: &HikeObject,
    profile: &Profile,
    index: &mut dyn HeightIndex,
    mut draw_next: Option<&mut dyn FnMut() -> Option<[u8; 8]>>,
    fill_terrain: bool,
    malformed: bool,
) -> String {
    let shape_item = shape_items(obj, profile);
    let shape = if shape_item.is_empty() { "match" } else { "mismatch" };

    let (seed_d, terrain_owned): ([u8; 32], Vec<u8>) = if fill_terrain {
        let d = terrain::seed(&obj.public_key, &obj.seed_beacon);
        let (terrain, _, _) = terrain_from_seed(&obj.public_key, &obj.seed_beacon, profile);
        (d, terrain)
    } else {
        ([0u8; 32], Vec::new())
    };
    let terrain = terrain_owned.as_slice();

    let mut header_rows = Vec::with_capacity(19);
    for idx in 0..19 {
        header_rows.push(measure_header(obj, idx, index));
    }

    let seed_value = if obj.stages.first().map(|s| s.waypoint == seed_d).unwrap_or(false) {
        "match"
    } else {
        "mismatch"
    };

    let checkpoints = measure_checkpoints(obj, &seed_d, &terrain, profile);
    let waypoints = measure_waypoints(obj, &terrain, profile);

    let mut issued_plants: HashMap<u64, Option<Vec<u8>>> = HashMap::new();
    let paths = if malformed {
        malformed_paths()
    } else {
        measure_paths(obj, index, &mut issued_plants, &header_rows)
    };
    let anchors = anchor_tokens(&paths);
    let anchored_total = anchors.iter().filter(|t| **t == "anchored").count();
    let skip_list: Vec<J> = paths
        .iter()
        .take(16)
        .enumerate()
        .filter(|(_, p)| p.skipped)
        .map(|(i, _)| J::Num((i as i128) + 1))
        .collect();

    let book = bookends(obj, &paths, &header_rows, index, &mut issued_plants);
    let analysis = analysis_of(obj, &header_rows, &book, malformed);

    let leg_total: u64 = obj.stages.iter().map(|s| s.checkpoints.len() as u64).sum();
    let six = measure_six(obj, &seed_d, &terrain, profile, leg_total, &mut draw_next);

    let mut pairs: Vec<(&'static str, J)> = Vec::new();
    pairs.push(("profile", J::s(profile.record_name())));
    if profile.record_name() == "conformance-small" {
        pairs.push(("sample_bytes", J::Num(profile.sample_bytes as i128)));
        pairs.push(("slot_bytes", J::Num(profile.slot_bytes as i128)));
        pairs.push(("terrain_bytes", J::Num(profile.terrain_bytes as i128)));
        pairs.push(("steps_per_leg", J::Num(profile.steps_per_leg as i128)));
    }
    pairs.push(("shape", J::s(shape)));
    pairs.push((
        "shape_item",
        J::Arr(shape_item.into_iter().map(J::s).collect()),
    ));
    pairs.push((
        "headers",
        J::Arr(header_rows.iter().map(|h| h.json()).collect()),
    ));
    pairs.push(("seed_value", J::s(seed_value)));
    pairs.push((
        "checkpoints",
        J::Arr(checkpoints.into_iter().map(|c| c.json()).collect()),
    ));
    pairs.push((
        "waypoints",
        J::Arr(waypoints.into_iter().map(|w| w.json()).collect()),
    ));
    pairs.push(("paths", J::Arr(paths.iter().map(|p| p.json()).collect())));
    pairs.push((
        "anchors",
        J::Arr(anchors.into_iter().map(J::s).collect()),
    ));
    pairs.push(("anchored_total", J::Num(anchored_total as i128)));
    pairs.push(("skip_list", J::Arr(skip_list)));
    pairs.push(("a", book.a));
    pairs.push(("b", book.b));
    pairs.push(("N", book.n));
    pairs.push(("S", book.s));
    pairs.push(("delta_h", book.delta_h));
    pairs.push(("fixed_start", book.fixed_start));
    pairs.push(("P_b", book.p_b));
    pairs.push(("height_P_b", book.height_p_b));
    pairs.push(("index_comparison_P_b", book.index_comparison_p_b));
    pairs.push(("T_P_b", book.t_p_b));
    pairs.push(("height_I_a_minus_1", book.height_i_a));
    pairs.push(("T_I_a_minus_1", book.t_i_a));
    pairs.push(("six_leg", six.six_leg));
    pairs.push(("six_leg_runs", six.six_leg_runs));
    pairs.push(("analysis", analysis));
    J::Obj(pairs).render()
}

fn shape_items(obj: &HikeObject, profile: &Profile) -> Vec<&'static str> {
    if obj.trailing {
        return vec!["malformed cbor"];
    }
    let mut items = Vec::new();
    if obj.field_set {
        items.push("field set");
    }
    if obj.stage0_path {
        items.push("stage-0 path key");
    }
    let w17_bad = match obj.stages.get(17) {
        Some(st) => match &st.path {
            Some(p) if p.is_empty() => false,
            _ => true,
        },
        None => true,
    };
    if w17_bad {
        items.push("W_17 path");
    }
    let constant_bad = obj.kind != KIND
        || obj.fill != FILL
        || obj.slot_bytes != profile.slot_bytes as u64
        || obj.n_slots != profile.n_slots
        || obj.sample_bytes != profile.sample_bytes as u64
        || obj.steps_per_leg != profile.steps_per_leg;
    if constant_bad {
        items.push("constant");
    }
    if obj.stages.len() != STAGES {
        items.push("stages length");
    }
    let leg_bad = obj.stages.len() != STAGES
        || obj.stages.iter().any(|st| {
            st.leg_count < 1
                || st.leg_count != st.checkpoints.len() as u64
                || st.checkpoints.len() != st.leg_results.len()
        });
    if leg_bad {
        items.push("leg_count");
    }
    if obj.stages.iter().any(|st| st.checkpoints.iter().any(|c| c.len() != 32)) {
        items.push("checkpoint width");
    }
    if obj
        .stages
        .iter()
        .any(|st| st.leg_results.iter().any(|c| c.len() != 8))
    {
        items.push("leg result width");
    }
    let seed_bad = obj
        .stages
        .first()
        .map(|s| s.beacon != obj.seed_beacon)
        .unwrap_or(true);
    if seed_bad {
        items.push("seed beacon");
    }
    let last_bad = match obj.stages.get(17) {
        Some(st17) => {
            !obj.has_last
                || !obj.has_last_height
                || obj.last_interrupt == st17.beacon
                || obj.last_height_over
                || st17.height_over
                || obj.last_interrupt_height <= st17.beacon_height
        }
        None => true,
    };
    if last_bad {
        items.push("last_interrupt");
    }
    if header_height_order_bad(obj) {
        items.push("header height order");
    }
    if header_hash_differ_bad(obj) {
        items.push("header hash differ");
    }
    if obj.last_height_over || obj.stages.iter().any(|s| s.height_over) {
        items.push("height_range");
    }
    items
}

fn stored_headers(obj: &HikeObject) -> Vec<Option<([u8; 32], u64, bool)>> {
    let mut v = Vec::with_capacity(19);
    for g in 0..18 {
        if let Some(st) = obj.stages.get(g) {
            v.push(Some((st.beacon, st.beacon_height, st.height_over)));
        } else {
            v.push(None);
        }
    }
    if obj.has_last && obj.has_last_height {
        v.push(Some((
            obj.last_interrupt,
            obj.last_interrupt_height,
            obj.last_height_over,
        )));
    } else if obj.has_last {
        v.push(Some((
            obj.last_interrupt,
            obj.last_interrupt_height,
            true,
        )));
    } else {
        v.push(None);
    }
    v
}

fn header_height_order_bad(obj: &HikeObject) -> bool {
    let h = stored_headers(obj);
    for w in h.windows(2) {
        match (&w[0], &w[1]) {
            (Some((_, a, ao)), Some((_, b, bo))) if !ao && !bo && *a <= HEIGHT_MAX && *b <= HEIGHT_MAX => {
                if *b <= *a {
                    return true;
                }
            }
            _ => return true,
        }
    }
    false
}

fn header_hash_differ_bad(obj: &HikeObject) -> bool {
    let h = stored_headers(obj);
    for w in h.windows(2) {
        match (&w[0], &w[1]) {
            (Some((a, _, _)), Some((b, _, _))) => {
                if a == b {
                    return true;
                }
            }
            _ => return true,
        }
    }
    false
}

struct HeaderRow {
    stored_hash: J,
    stored_height: J,
    index_comparison: String,
    t: J,
    /// Unix time when `index_comparison` is match and time follows section 9.
    time: Option<i128>,
}

impl HeaderRow {
    fn json(&self) -> J {
        J::Obj(vec![
            ("stored_hash", self.stored_hash.clone()),
            ("stored_height", self.stored_height.clone()),
            ("index_comparison", J::s(&self.index_comparison)),
            ("T", self.t.clone()),
        ])
    }
}

fn measure_header(obj: &HikeObject, idx: usize, index: &mut dyn HeightIndex) -> HeaderRow {
    let src = stored_headers(obj);
    let Some(Some((hash, height, over))) = src.get(idx) else {
        return HeaderRow {
            stored_hash: J::s("not defined"),
            stored_height: J::s("not defined"),
            index_comparison: "not read".into(),
            t: J::s("not read"),
            time: None,
        };
    };
    if *over || *height > HEIGHT_MAX {
        return HeaderRow {
            stored_hash: J::s(&hex_encode(hash)),
            stored_height: J::s("not defined"),
            index_comparison: "not read".into(),
            t: J::s("not read"),
            time: None,
        };
    }
    let (cmp, time) = match index.read_height(*height) {
        None => ("not read".to_string(), None),
        Some(bytes) => match indexdoc::parse_document(&bytes).and_then(|d| indexdoc::read_header(&d))
        {
            None => ("not read".to_string(), None),
            Some(doc) => {
                if doc.natural == *hash && doc.height == *height {
                    ("match".to_string(), doc.time.map(|t| t as i128))
                } else {
                    ("mismatch".to_string(), None)
                }
            }
        },
    };
    let t = match (&cmp[..], time) {
        ("match", Some(tv)) => J::Num(tv),
        _ => J::s("not read"),
    };
    HeaderRow {
        stored_hash: J::s(&hex_encode(hash)),
        stored_height: J::Num(*height as i128),
        index_comparison: cmp,
        t,
        time,
    }
}

fn cp32(st: &Stage, j: usize) -> Option<[u8; 32]> {
    let b = st.checkpoints.get(j)?;
    if b.len() != 32 {
        return None;
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(b);
    Some(a)
}

fn limb_of(obj: &HikeObject, g: usize, seed_d: &[u8; 32]) -> [u8; 32] {
    if g == 0 {
        *seed_d
    } else {
        obj.stages.get(g).map(|s| s.waypoint).unwrap_or([0u8; 32])
    }
}

struct LegRow {
    j: usize,
    result: &'static str,
}

struct StageRows {
    g: usize,
    legs: Option<Vec<LegRow>>,
}

impl StageRows {
    fn json(&self) -> J {
        let legs = match &self.legs {
            None => J::s("not defined"),
            Some(ls) => J::Arr(
                ls.iter()
                    .map(|l| {
                        J::Obj(vec![
                            ("j", J::Num(l.j as i128)),
                            ("result", J::s(l.result)),
                        ])
                    })
                    .collect(),
            ),
        };
        J::Obj(vec![("g", J::Num(self.g as i128)), ("legs", legs)])
    }
}

fn measure_checkpoints(
    obj: &HikeObject,
    seed_d: &[u8; 32],
    terrain: &[u8],
    profile: &Profile,
) -> Vec<StageRows> {
    let mut out = Vec::with_capacity(18);
    for g in 0..18 {
        let Some(st) = obj.stages.get(g) else {
            out.push(StageRows { g, legs: None });
            continue;
        };
        if st.checkpoints.iter().any(|c| c.len() != 32) {
            out.push(StageRows { g, legs: None });
            continue;
        }
        let limb = limb_of(obj, g, seed_d);
        let mut legs = Vec::new();
        for j in 0..st.checkpoints.len() {
            let prev = if j == 0 {
                if g == 0 {
                    Some(*seed_d)
                } else {
                    Some(st.waypoint)
                }
            } else {
                cp32(st, j - 1)
            };
            let result = match (prev, st.leg_results.get(j)) {
                (Some(previous), Some(raw)) if raw.len() == 8 => {
                    let mut b = [0u8; 8];
                    b.copy_from_slice(raw);
                    let lr = u64::from_le_bytes(b);
                    let got = checkpoint(&previous, &limb, terrain, profile, lr);
                    let mut want = [0u8; 32];
                    want.copy_from_slice(&st.checkpoints[j]);
                    if got == want {
                        "match"
                    } else {
                        "mismatch"
                    }
                }
                _ => "mismatch",
            };
            legs.push(LegRow { j, result });
        }
        out.push(StageRows {
            g,
            legs: Some(legs),
        });
    }
    out
}

struct WpRow {
    g: usize,
    result: &'static str,
}

impl WpRow {
    fn json(&self) -> J {
        J::Obj(vec![
            ("g", J::Num(self.g as i128)),
            ("result", J::s(self.result)),
        ])
    }
}

fn measure_waypoints(obj: &HikeObject, terrain: &[u8], profile: &Profile) -> Vec<WpRow> {
    let mut out = Vec::with_capacity(17);
    for g in 0..17 {
        let result = waypoint_result(obj, g, terrain, profile);
        out.push(WpRow { g: g + 1, result });
    }
    out
}

fn waypoint_result(
    obj: &HikeObject,
    g: usize,
    terrain: &[u8],
    profile: &Profile,
) -> &'static str {
    let (Some(st), Some(next)) = (obj.stages.get(g), obj.stages.get(g + 1)) else {
        return "not defined";
    };
    if st.checkpoints.iter().any(|c| c.len() != 32) {
        return "not defined";
    }
    let mut cps = Vec::with_capacity(st.checkpoints.len());
    for c in &st.checkpoints {
        let mut a = [0u8; 32];
        a.copy_from_slice(c);
        cps.push(a);
    }
    let fold = fold_checkpoints(&cps);
    let got = waypoint(&next.beacon, terrain, profile, &st.waypoint, &fold);
    if got == next.waypoint {
        "match"
    } else {
        "mismatch"
    }
}

struct PathRow {
    g: usize,
    w17: bool,
    path_state: &'static str,
    plant: &'static str,
    p_g: J,
    height_p_g: J,
    d_g: J,
    claimed: J,
    w17_path: &'static str,
    skipped: bool,
    anchored: bool,
    /// Validated present path, for the bookend "present" count.
    validated: bool,
    /// An issued plant GET returned no document.
    issued_miss: bool,
    body_height: Option<u64>,
    body_hash_natural: Option<[u8; 32]>,
}

impl PathRow {
    fn json(&self) -> J {
        if self.w17 {
            J::Obj(vec![
                ("g", J::Num(17)),
                ("w17_path", J::s(self.w17_path)),
                ("claimed_block", self.claimed.clone()),
            ])
        } else {
            J::Obj(vec![
                ("g", J::Num(self.g as i128)),
                ("path_state", J::s(self.path_state)),
                ("plant", J::s(self.plant)),
                ("P_g", self.p_g.clone()),
                ("height_P_g", self.height_p_g.clone()),
                ("d_g", self.d_g.clone()),
                ("claimed_block", self.claimed.clone()),
            ])
        }
    }
}

fn claimed_from_natural(hash: &[u8; 32], height: u64) -> J {
    J::Obj(vec![
        ("block_hash", J::s(&hex_encode(&reverse32(hash)))),
        ("block_height", J::Num(height as i128)),
    ])
}

/// Erratum 2 (E02). A malformed CBOR value has no stage and no path byte string.
/// `path_state` and `plant` are `not defined`. The row is not a skip: `skip` is
/// only an empty path byte string on a decoded stage (Rule A4.5).
fn malformed_paths() -> Vec<PathRow> {
    let mut rows = Vec::with_capacity(17);
    for g in 1..=16 {
        rows.push(not_defined_path(g, false));
    }
    rows.push(not_defined_path(17, true));
    rows
}

fn not_defined_path(g: usize, w17: bool) -> PathRow {
    PathRow {
        g,
        w17,
        path_state: "not defined",
        plant: "not defined",
        p_g: J::s("not defined"),
        height_p_g: J::s("not defined"),
        d_g: J::s("not defined"),
        claimed: J::s("not defined"),
        w17_path: "not defined",
        skipped: false,
        anchored: false,
        validated: false,
        issued_miss: false,
        body_height: None,
        body_hash_natural: None,
    }
}

fn measure_paths(
    obj: &HikeObject,
    index: &mut dyn HeightIndex,
    issued_plants: &mut HashMap<u64, Option<Vec<u8>>>,
    headers: &[HeaderRow],
) -> Vec<PathRow> {
    let mut rows = Vec::with_capacity(17);
    for g in 1..=16 {
        rows.push(measure_one_path(obj, g, index, issued_plants, headers));
    }
    rows.push(measure_w17(obj));
    rows
}

fn measure_w17(obj: &HikeObject) -> PathRow {
    let empty = PathRow {
        g: 17,
        w17: true,
        path_state: "skip",
        plant: "n/a",
        p_g: J::s("not defined"),
        height_p_g: J::s("not defined"),
        d_g: J::s("not defined"),
        claimed: J::s("not defined"),
        w17_path: "not fetched by design",
        skipped: false,
        anchored: false,
        validated: false,
        issued_miss: false,
        body_height: None,
        body_hash_natural: None,
    };
    let Some(st) = obj.stages.get(17) else {
        return PathRow {
            w17_path: "not defined",
            ..empty
        };
    };
    match &st.path {
        Some(p) if p.is_empty() => empty,
        Some(p) => {
            let claimed = match path::peek_claimed(p) {
                Some((h, ht)) => claimed_from_natural(&h, ht as u64),
                None => J::s("not defined"),
            };
            PathRow {
                claimed,
                w17_path: "not defined",
                ..empty
            }
        }
        None => PathRow {
            w17_path: "not defined",
            ..empty
        },
    }
}

fn in_window(stage_h: u64, block_h: u64) -> bool {
    block_h > stage_h && matches!(block_h - stage_h, 2 | 3 | 4)
}

fn interrupt_match(obj: &HikeObject, height: u64, natural: &[u8; 32]) -> bool {
    for g in 1..obj.stages.len().min(18) {
        let st = &obj.stages[g];
        if !st.height_over && st.beacon_height == height && st.beacon == *natural {
            return true;
        }
    }
    obj.has_last
        && obj.has_last_height
        && !obj.last_height_over
        && obj.last_interrupt_height == height
        && obj.last_interrupt == *natural
}

fn measure_one_path(
    obj: &HikeObject,
    g: usize,
    index: &mut dyn HeightIndex,
    issued_plants: &mut HashMap<u64, Option<Vec<u8>>>,
    headers: &[HeaderRow],
) -> PathRow {
    let base = PathRow {
        g,
        w17: false,
        path_state: "skip",
        plant: "n/a",
        p_g: J::s("not defined"),
        height_p_g: J::s("not defined"),
        d_g: J::s("not defined"),
        claimed: J::s("not defined"),
        w17_path: "not defined",
        skipped: true,
        anchored: false,
        validated: false,
        issued_miss: false,
        body_height: None,
        body_hash_natural: None,
    };
    let Some(st) = obj.stages.get(g) else {
        return PathRow {
            path_state: "present",
            plant: "mismatch",
            skipped: false,
            ..base
        };
    };
    let Some(bytes) = st.path.as_ref() else {
        return PathRow {
            path_state: "present",
            plant: "mismatch",
            skipped: false,
            ..base
        };
    };
    if bytes.is_empty() {
        return base;
    }
    let Some(body) = path::decode_path(bytes) else {
        return PathRow {
            path_state: "present",
            plant: "mismatch",
            skipped: false,
            ..base
        };
    };
    if !path::validate_for(&body, &st.waypoint) {
        return PathRow {
            path_state: "present",
            plant: "mismatch",
            skipped: false,
            ..base
        };
    }
    let claimed = claimed_from_natural(&body.block_hash, body.block_height as u64);
    let stage_h = st.beacon_height;
    let bh = body.block_height as u64;
    if st.height_over || !in_window(stage_h, bh) || !interrupt_match(obj, bh, &body.block_hash) {
        return PathRow {
            path_state: "present",
            plant: "not read",
            claimed,
            skipped: false,
            validated: true,
            body_height: Some(bh),
            body_hash_natural: Some(body.block_hash),
            ..base
        };
    }
    let doc_bytes = if let Some(prior) = issued_plants.get(&bh) {
        prior.clone()
    } else {
        let got = index.read_height(bh);
        issued_plants.insert(bh, got.clone());
        got
    };
    let Some(raw) = doc_bytes else {
        return PathRow {
            path_state: "present",
            plant: "not read",
            claimed,
            skipped: false,
            validated: true,
            issued_miss: true,
            body_height: Some(bh),
            body_hash_natural: Some(body.block_hash),
            ..base
        };
    };
    let parsed = indexdoc::parse_document(&raw);
    let plants = parsed.as_ref().and_then(indexdoc::read_plants);
    let matched = match plants {
        Some(p) if p.network_ok => p.plants.into_iter().find(|pl| plant_eq(pl, &body)),
        _ => None,
    };
    if matched.is_some() && parsed.is_some() {
        let height_p = body.block_height as i128;
        let d_g = if headers.get(g).map(|h| h.index_comparison == "match").unwrap_or(false) {
            if let Some(hdoc) = headers.get(g).and_then(|h| h.time.map(|_| h)) {
                let _ = hdoc;
            }
            match header_height_of(obj, g, headers) {
                Some(hh) => J::Num(height_p - hh),
                None => J::s("not defined"),
            }
        } else {
            J::s("not defined")
        };
        PathRow {
            path_state: "present",
            plant: "match",
            p_g: claimed_from_natural(&body.block_hash, body.block_height as u64),
            height_p_g: J::Num(height_p),
            d_g,
            claimed,
            skipped: false,
            anchored: true,
            validated: true,
            body_height: Some(bh),
            body_hash_natural: Some(body.block_hash),
            ..base
        }
    } else {
        let plant = if parsed.is_none() { "not read" } else { "mismatch" };
        let issued_miss = parsed.is_none();
        PathRow {
            path_state: "present",
            plant,
            claimed,
            skipped: false,
            validated: true,
            issued_miss,
            body_height: Some(bh),
            body_hash_natural: Some(body.block_hash),
            ..base
        }
    }
}

fn header_height_of(obj: &HikeObject, g: usize, headers: &[HeaderRow]) -> Option<i128> {
    if headers.get(g).map(|h| h.index_comparison == "match") != Some(true) {
        return None;
    }
    obj.stages.get(g).map(|s| s.beacon_height as i128)
}

fn plant_eq(pl: &indexdoc::PlantFact, body: &PathBody) -> bool {
    pl.root == body.root
        && pl.txid == body.txid
        && pl.vout == body.vout
        && pl.block_hash == body.block_hash
        && pl.block_height == body.block_height
}

fn anchor_tokens(paths: &[PathRow]) -> Vec<&'static str> {
    paths
        .iter()
        .take(16)
        .map(|p| if p.anchored { "anchored" } else { "not anchored" })
        .collect()
}

struct Book {
    a: J,
    b: J,
    n: J,
    s: J,
    delta_h: J,
    fixed_start: J,
    p_b: J,
    height_p_b: J,
    index_comparison_p_b: J,
    t_p_b: J,
    height_i_a: J,
    t_i_a: J,
    outside: J,
}

fn nd_book(outside: J) -> Book {
    let nd = J::s("not defined");
    Book {
        a: nd.clone(),
        b: nd.clone(),
        n: nd.clone(),
        s: nd.clone(),
        delta_h: nd.clone(),
        fixed_start: J::Obj(vec![
            ("N", nd.clone()),
            ("S", nd.clone()),
            ("delta_h", nd),
        ]),
        p_b: J::s("not defined"),
        height_p_b: J::s("not defined"),
        index_comparison_p_b: J::s("not defined"),
        t_p_b: J::s("not defined"),
        height_i_a: J::s("not defined"),
        t_i_a: J::s("not defined"),
        outside,
    }
}

fn bookends(
    obj: &HikeObject,
    paths: &[PathRow],
    headers: &[HeaderRow],
    index: &mut dyn HeightIndex,
    issued_plants: &mut HashMap<u64, Option<Vec<u8>>>,
) -> Book {
    let present = paths.iter().take(16).filter(|p| p.validated).count();
    if present < 2 {
        return nd_book(J::s("not defined"));
    }
    if paths.iter().take(16).any(|p| p.issued_miss) {
        let nr = J::s("not read");
        let mut b = nd_book(nr.clone());
        b.a = nr.clone();
        b.b = nr.clone();
        b.n = nr.clone();
        b.s = nr.clone();
        b.delta_h = nr.clone();
        b.fixed_start = J::Obj(vec![
            ("N", nr.clone()),
            ("S", nr.clone()),
            ("delta_h", nr.clone()),
        ]);
        b.p_b = nr.clone();
        b.height_p_b = nr.clone();
        b.index_comparison_p_b = nr.clone();
        b.t_p_b = nr.clone();
        b.height_i_a = nr.clone();
        b.t_i_a = nr.clone();
        b.outside = nr;
        return b;
    }
    let anchored: Vec<usize> = paths
        .iter()
        .take(16)
        .enumerate()
        .filter(|(_, p)| p.anchored)
        .map(|(i, _)| i + 1)
        .collect();
    if anchored.len() < 2 {
        return nd_book(J::s("not defined"));
    }
    let a = *anchored.first().unwrap();
    let b = *anchored.last().unwrap();
    let mut n: u64 = 0;
    for g in a..b {
        n += obj.stages.get(g).map(|s| s.checkpoints.len() as u64).unwrap_or(0);
    }
    let path_b = &paths[b - 1];
    let pb_height = path_b.body_height;
    let pb_hash = path_b.body_hash_natural;
    let (cmp_pb, t_pb, height_pb) = match pb_height {
        Some(h) => read_pb(index, issued_plants, h, pb_hash),
        None => ("not read".to_string(), None, None),
    };
    let header_a = headers.get(a);
    let a_not_read = header_a.map(|h| h.index_comparison == "not read").unwrap_or(true);
    let pb_not_read = cmp_pb == "not read";
    let no_block = pb_hash.is_none();
    if a_not_read || no_block || pb_not_read {
        let nr = J::s("not read");
        return Book {
            a: nr.clone(),
            b: nr.clone(),
            n: nr.clone(),
            s: nr.clone(),
            delta_h: nr.clone(),
            fixed_start: J::Obj(vec![
                ("N", nr.clone()),
                ("S", nr.clone()),
                ("delta_h", nr.clone()),
            ]),
            p_b: nr.clone(),
            height_p_b: nr.clone(),
            index_comparison_p_b: nr.clone(),
            t_p_b: nr.clone(),
            height_i_a: nr.clone(),
            t_i_a: nr.clone(),
            outside: nr,
        };
    }
    let p_b = match (pb_hash, pb_height) {
        (Some(h), Some(ht)) => claimed_from_natural(&h, ht),
        _ => J::s("not defined"),
    };
    let t_i = header_a.and_then(|h| h.time);
    let s = match (t_pb, t_i) {
        (Some(tb), Some(ti)) => J::Num(tb - ti),
        _ => J::s("not read"),
    };
    let height_i = if header_a.map(|h| h.index_comparison == "match") == Some(true) {
        obj.stages.get(a).map(|st| J::Num(st.beacon_height as i128)).unwrap_or_else(|| J::s("not defined"))
    } else {
        J::s("not defined")
    };
    let height_i_num = if header_a.map(|h| h.index_comparison == "match") == Some(true) {
        obj.stages.get(a).map(|st| st.beacon_height as i128)
    } else {
        None
    };
    let delta = match (height_pb, height_i_num) {
        (Some(hp), Some(hi)) => J::Num(hp - hi),
        _ => J::s("not defined"),
    };
    let t_i_j = match t_i {
        Some(t) => J::Num(t),
        None => J::s("not read"),
    };
    let t_pb_j = match t_pb {
        Some(t) => J::Num(t),
        None => J::s("not read"),
    };
    let height_pb_j = match height_pb {
        Some(h) => J::Num(h),
        None => J::s("not defined"),
    };
    let mut outside: u64 = 0;
    for g in 0..obj.stages.len().min(18) {
        if g < a || g >= b {
            outside += obj.stages[g].checkpoints.len() as u64;
        }
    }
    let fs_s = match (headers.get(1).and_then(|h| h.time), t_pb) {
        (Some(t1), Some(tp)) => J::Num(tp - t1),
        _ => J::s("not read"),
    };
    let mut fs_n: u64 = 0;
    for g in 1..b {
        fs_n += obj.stages.get(g).map(|s| s.checkpoints.len() as u64).unwrap_or(0);
    }
    let fs_dh = match (
        height_pb,
        headers.get(1).map(|h| h.index_comparison == "match"),
    ) {
        (Some(hp), Some(true)) => {
            let h1 = obj.stages.get(1).map(|s| s.beacon_height as i128);
            match h1 {
                Some(v) => J::Num(hp - v),
                None => J::s("not defined"),
            }
        }
        (None, _) | (_, Some(false)) => J::s("not defined"),
        _ => J::s("not read"),
    };
    Book {
        a: J::Num(a as i128),
        b: J::Num(b as i128),
        n: J::Num(n as i128),
        s,
        delta_h: delta,
        fixed_start: J::Obj(vec![
            ("N", J::Num(fs_n as i128)),
            ("S", fs_s),
            ("delta_h", fs_dh),
        ]),
        p_b,
        height_p_b: height_pb_j,
        index_comparison_p_b: J::s(&cmp_pb),
        t_p_b: t_pb_j,
        height_i_a: height_i,
        t_i_a: t_i_j,
        outside: J::Num(outside as i128),
    }
}

fn read_pb(
    index: &mut dyn HeightIndex,
    issued_plants: &mut HashMap<u64, Option<Vec<u8>>>,
    height: u64,
    natural_hash: Option<[u8; 32]>,
) -> (String, Option<i128>, Option<i128>) {
    let raw = if let Some(c) = issued_plants.get(&height) {
        c.clone()
    } else {
        let got = index.read_height(height);
        issued_plants.insert(height, got.clone());
        got
    };
    let Some(raw) = raw else {
        return ("not read".into(), None, None);
    };
    let Some(doc) = indexdoc::parse_document(&raw) else {
        return ("not read".into(), None, None);
    };
    let Some(header) = indexdoc::read_header(&doc) else {
        return ("not read".into(), None, None);
    };
    let display = natural_hash.map(|b| reverse32(&b));
    if display == Some(header.display) && header.height == height {
        (
            "match".into(),
            header.time.map(|t| t as i128),
            Some(header.height as i128),
        )
    } else {
        ("mismatch".into(), None, None)
    }
}

fn analysis_of(
    obj: &HikeObject,
    headers: &[HeaderRow],
    book: &Book,
    malformed: bool,
) -> J {
    let complete = obj.stages.len() == 18;
    // Erratum 2 (E02). A malformed value has no stage, so these counts are
    // `not defined`. A decoded empty checkpoint list still counts as zero (SPEC L961).
    // `hike_legs` and `whole_hike_legs` count the same checkpoints: stages 1–17.
    // Stage 0 is before the first interrupt and is in neither count.
    let counted = if malformed || !complete {
        J::s("not defined")
    } else {
        let n: usize = obj.stages[1..].iter().map(|s| s.checkpoints.len()).sum();
        J::Num(n as i128)
    };
    let whole = counted.clone();
    let hike_legs = counted;
    let span = |ia: usize, ib: usize| -> J {
        match (headers.get(ia).and_then(|h| h.time), headers.get(ib).and_then(|h| h.time)) {
            (Some(a), Some(b)) => J::Num(b - a),
            _ => J::s("not read"),
        }
    };
    // Same integer as whole_hike_span: nTime(I_17) − nTime(I_0).
    let whole_span = span(1, 18);
    let hike_span = whole_span.clone();
    let mut per = Vec::with_capacity(18);
    for g in 0..18 {
        let count = if malformed {
            J::s("not defined")
        } else {
            let n = obj.stages.get(g).map(|s| s.checkpoints.len()).unwrap_or(0);
            J::Num(n as i128)
        };
        let sp = if g == 0 {
            span(0, 1)
        } else if g < 17 {
            span(g, g + 1)
        } else {
            span(17, 18)
        };
        per.push(J::Obj(vec![
            ("g", J::Num(g as i128)),
            ("checkpoint_count", count),
            ("span", sp),
        ]));
    }
    J::Obj(vec![
        ("whole_hike_legs", whole),
        ("whole_hike_span", whole_span),
        ("per_stage", J::Arr(per)),
        ("legs_outside", book.outside.clone()),
        ("hike_legs", hike_legs),
        ("hike_span", hike_span),
    ])
}

struct Six {
    six_leg: J,
    six_leg_runs: J,
}

fn measure_six(
    obj: &HikeObject,
    seed_d: &[u8; 32],
    terrain: &[u8],
    profile: &Profile,
    leg_total: u64,
    draw_next: &mut Option<&mut dyn FnMut() -> Option<[u8; 8]>>,
) -> Six {
    if leg_total < SIX_LEG_COUNT as u64 {
        return Six {
            six_leg: J::s("not defined"),
            six_leg_runs: J::Arr(vec![]),
        };
    }
    let drawn = {
        let mut pull = | | -> Option<[u8; 8]> {
            if let Some(src) = draw_next.as_mut() {
                src()
            } else {
                draw::os_csprng_next8()
            }
        };
        draw::draw_six(leg_total, &mut pull)
    };
    let indices = match drawn {
        Ok(v) => v,
        Err(DrawStop::NotRead) | Err(DrawStop::NotDefined) => {
            return Six {
                six_leg: J::s("not read"),
                six_leg_runs: J::s("not read"),
            };
        }
    };
    // SPEC §14.6. The draw has finished. Recompute at most six legs at once.
    let mut results = Vec::with_capacity(indices.len());
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(indices.len());
        for q in &indices {
            let q = *q;
            handles.push(scope.spawn(move || recompute_leg(obj, seed_d, terrain, profile, q)));
        }
        for handle in handles {
            results.push(handle.join().expect("leg recompute"));
        }
    });
    let obj = J::Obj(vec![
        (
            "indices",
            J::Arr(indices.iter().map(|q| J::Num(*q as i128)).collect()),
        ),
        (
            "results",
            J::Arr(results.iter().map(|r| J::s(r)).collect()),
        ),
    ]);
    Six {
        six_leg: obj.clone(),
        six_leg_runs: J::Arr(vec![obj]),
    }
}

fn recompute_leg(
    obj: &HikeObject,
    seed_d: &[u8; 32],
    terrain: &[u8],
    profile: &Profile,
    q: u64,
) -> &'static str {
    let mut start = 0u64;
    for (g, st) in obj.stages.iter().enumerate() {
        let n = st.checkpoints.len() as u64;
        if q < start + n {
            let j = (q - start) as usize;
            let prev = if j == 0 {
                if g == 0 {
                    *seed_d
                } else {
                    st.waypoint
                }
            } else {
                match cp32(st, j - 1) {
                    Some(p) => p,
                    None => return "mismatch",
                }
            };
            let recomputed = leg_result(terrain, profile, &prev);
            let Some(raw) = st.leg_results.get(j) else {
                return "mismatch";
            };
            if raw.len() != 8 {
                return "mismatch";
            }
            let mut b = [0u8; 8];
            b.copy_from_slice(raw);
            return if u64::from_le_bytes(b) == recomputed {
                "match"
            } else {
                "mismatch"
            };
        }
        start += n;
    }
    "mismatch"
}

#[cfg(test)]
mod admit_pair_tests {
    use super::*;

    fn stage(height: u64, checkpoints: usize) -> Stage {
        Stage {
            beacon: [0x11; 32],
            beacon_height: height,
            height_over: false,
            leg_count: checkpoints as u64,
            waypoint: [0x22; 32],
            path: Some(Vec::new()),
            checkpoints: vec![vec![0x33; 32]; checkpoints],
            leg_results: Vec::new(),
            usable: true,
        }
    }

    fn row(time: Option<i128>) -> HeaderRow {
        HeaderRow {
            stored_hash: J::s("00"),
            stored_height: J::Num(0),
            index_comparison: if time.is_some() {
                "match".into()
            } else {
                "not read".into()
            },
            t: match time {
                Some(t) => J::Num(t),
                None => J::s("not read"),
            },
            time,
        }
    }

    /// Linear `nTime`, with the last block spiked ahead. The admit span
    /// keeps that spike. A seed-start span would include the wait before `I_0`.
    fn ntime(h: u64, last: u64) -> i128 {
        let base = 1000 * i128::from(h);
        if h == last {
            base + 500_000
        } else {
            base
        }
    }

    #[test]
    fn admit_pair_starts_at_the_first_interrupt_not_the_seed() {
        // The admit pair starts at I_0 (stages[1].beacon), not the seed.
        // Stage 0 checkpoints are not in hike_legs. hike_span is raw nTime,
        // the same integer as whole_hike_span, including a last-block spike.
        let seed = 1000u64;
        let i0 = 1100u64;
        let last = 1117u64;
        let mut stages = Vec::with_capacity(18);
        for g in 0..18u64 {
            let (height, n) = if g == 0 {
                (seed, 90)
            } else if g == 17 {
                (last - 1, 140)
            } else {
                (i0 + g - 1, 50)
            };
            stages.push(stage(height, n));
        }
        let obj = HikeObject {
            kind: String::new(),
            fill: String::new(),
            slot_bytes: 0,
            n_slots: 0,
            sample_bytes: 0,
            steps_per_leg: 0,
            public_key: [0; 32],
            seed_beacon: [0x44; 32],
            stages,
            last_interrupt: [0x55; 32],
            last_interrupt_height: last,
            last_height_over: false,
            has_last: true,
            has_last_height: true,
            field_set: true,
            stage0_path: false,
            trailing: false,
            top_ok: true,
        };
        assert_eq!(crate::hike_legs(&obj), Some(940));

        let mut headers = Vec::with_capacity(19);
        headers.push(row(Some(ntime(seed, last))));
        for g in 1..18 {
            headers.push(row(Some(ntime(obj.stages[g].beacon_height, last))));
        }
        headers.push(row(Some(ntime(last, last))));
        assert_eq!(headers.len(), 19);

        let book = Book {
            a: J::s("not defined"),
            b: J::s("not defined"),
            n: J::s("not defined"),
            s: J::s("not defined"),
            delta_h: J::s("not defined"),
            fixed_start: J::Obj(vec![
                ("N", J::s("not defined")),
                ("S", J::s("not defined")),
                ("delta_h", J::s("not defined")),
            ]),
            p_b: J::s("not defined"),
            height_p_b: J::s("not defined"),
            index_comparison_p_b: J::s("not defined"),
            t_p_b: J::s("not defined"),
            height_i_a: J::s("not defined"),
            t_i_a: J::s("not defined"),
            outside: J::s("not defined"),
        };
        let analysis = analysis_of(&obj, &headers, &book, false);
        let v: serde_json::Value = serde_json::from_str(&analysis.render()).unwrap();
        assert_eq!(v["hike_legs"], 940);
        assert_eq!(v["whole_hike_legs"], 940);
        assert_ne!(
            v["hike_legs"], 1030,
            "stage 0's 90 checkpoints are not admission legs"
        );
        // nTime(I_0) = 1_100_000. nTime(last) = 1_117_000 + 500_000.
        // The last block's spike is the admit clock. A seed start is longer.
        assert_eq!(v["hike_span"], 517_000);
        assert_eq!(
            v["whole_hike_span"], 517_000,
            "hike_span and whole_hike_span are the same nTime delta"
        );
        assert_eq!(
            v["legs_outside"], "not defined",
            "bookends are not the admit pair"
        );

        let mut asked = Vec::new();
        let admit = crate::hike_span(i0, last, |h| {
            asked.push(h);
            Some(ntime(h, last))
        })
        .unwrap();
        assert_eq!(asked, vec![i0, last], "the span reads only the two endpoint nTimes");
        let seeded = crate::hike_span(seed, last, |h| Some(ntime(h, last))).unwrap();
        assert_eq!(admit, 517_000);
        assert_eq!(seeded, 617_000);
        assert_ne!(
            v["hike_span"].as_i64(),
            Some(i64::try_from(seeded).unwrap())
        );
    }
}
