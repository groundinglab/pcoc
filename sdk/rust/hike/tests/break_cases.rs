//! rc.2 break pass: structural edits of the kat-small object, decoded and
//! re-encoded as CBOR values so one field, stage or width can be wrong while
//! everything else stays intact. Go and Java record these byte for byte the
//! same (sdk/tools/hikecases.py through sdk/tools/crosscheck.py); these tests
//! pin the section 14 facts in Rust.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
enum V {
    U(u64),
    B(Vec<u8>),
    T(String),
    A(Vec<V>),
    M(Vec<(String, V)>),
}

fn dec(b: &[u8], i: &mut usize) -> V {
    let ib = b[*i];
    *i += 1;
    let (mt, ai) = (ib >> 5, ib & 31);
    let n = match ai {
        0..=23 => u64::from(ai),
        24..=27 => {
            let w = 1usize << (ai - 24);
            let v = b[*i..*i + w]
                .iter()
                .fold(0u64, |a, x| (a << 8) | u64::from(*x));
            *i += w;
            v
        }
        _ => panic!("kat-small is definite"),
    };
    let take = |i: &mut usize| {
        let s = b[*i..*i + n as usize].to_vec();
        *i += n as usize;
        s
    };
    match mt {
        0 => V::U(n),
        2 => V::B(take(i)),
        3 => V::T(String::from_utf8(take(i)).unwrap()),
        4 => V::A((0..n).map(|_| dec(b, i)).collect()),
        5 => V::M(
            (0..n)
                .map(|_| match dec(b, i) {
                    V::T(k) => (k, dec(b, i)),
                    _ => panic!("text keys"),
                })
                .collect(),
        ),
        _ => panic!("major type {mt}"),
    }
}

fn head(mt: u8, n: u64) -> Vec<u8> {
    let mt = mt << 5;
    match n {
        0..=23 => vec![mt | n as u8],
        24..=0xff => vec![mt | 24, n as u8],
        0x100..=0xffff => [vec![mt | 25], (n as u16).to_be_bytes().to_vec()].concat(),
        0x1_0000..=0xffff_ffff => [vec![mt | 26], (n as u32).to_be_bytes().to_vec()].concat(),
        _ => [vec![mt | 27], n.to_be_bytes().to_vec()].concat(),
    }
}

fn enc(v: &V) -> Vec<u8> {
    match v {
        V::U(n) => head(0, *n),
        V::B(b) => [head(2, b.len() as u64), b.clone()].concat(),
        V::T(t) => [head(3, t.len() as u64), t.as_bytes().to_vec()].concat(),
        V::A(a) => [head(4, a.len() as u64), a.iter().flat_map(enc).collect()].concat(),
        V::M(m) => [
            head(5, m.len() as u64),
            m.iter()
                .flat_map(|(k, x)| [enc(&V::T(k.clone())), enc(x)].concat())
                .collect(),
        ]
        .concat(),
    }
}

impl V {
    fn map(&mut self) -> &mut Vec<(String, V)> {
        match self {
            V::M(m) => m,
            _ => panic!("not a map"),
        }
    }
    fn get(&mut self, key: &str) -> &mut V {
        &mut self.map().iter_mut().find(|(k, _)| k == key).expect(key).1
    }
    fn set(&mut self, key: &str, v: V) {
        *self.get(key) = v;
    }
    fn drop_key(&mut self, key: &str) {
        self.map().retain(|(k, _)| k != key);
    }
    fn arr(&mut self) -> &mut Vec<V> {
        match self {
            V::A(a) => a,
            _ => panic!("not an array"),
        }
    }
    fn stage(&mut self, g: usize) -> &mut V {
        &mut self.get("stages").arr()[g]
    }
    fn bytes(&self) -> Vec<u8> {
        match self {
            V::B(b) => b.clone(),
            _ => panic!("not bytes"),
        }
    }
}

fn kat() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/conformance/v2.0/kat-small.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn object_bytes() -> Vec<u8> {
    let s = kat()["object_hex"].as_str().unwrap().to_owned();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn object() -> V {
    let raw = object_bytes();
    let mut i = 0;
    let v = dec(&raw, &mut i);
    assert_eq!(i, raw.len());
    assert_eq!(enc(&v), raw, "kat-small round-trips");
    v
}

fn record_seeded(bytes: &[u8], seed: [u8; 32]) -> Value {
    let docs: HashMap<u64, String> = kat()["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["height"].as_u64().unwrap(),
                d["body"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let measurements = hike::verify_sized(
        bytes,
        hike::AuditDraw::PinnedSeed(seed),
        |height| hike::parse_index_beacon(docs.get(&height).ok_or(())?).map_err(|_| ()),
        |loc: &hike::PathLocators| match docs.get(&u64::from(loc.block_height)) {
            Some(doc) => hike::proxy_matches(doc, loc).into(),
            None => hike::ChainRead::NotRead,
        },
        hike::Sizes::conformance_small(),
    );
    let line = hike::verify_record_json("conformance-small", &measurements);
    serde_json::from_str(line.trim_end()).unwrap()
}

fn record(bytes: &[u8]) -> Value {
    record_seeded(bytes, [0x11; 32])
}

fn items(rec: &Value) -> Vec<&str> {
    rec["shape_item"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}

fn undecoded() -> Value {
    record(&[])
}

/// A 31-byte checkpoint in stage 2: legs and W_3 are `not defined`; every
/// count is still the array length (sections 14.2, 14.4, 14.6).
#[test]
fn a_short_checkpoint_counts_and_leaves_w3_not_defined() {
    let mut v = object();
    let cp = &mut v.stage(2).get("checkpoints").arr()[0];
    *cp = V::B(cp.bytes()[..31].to_vec());
    let rec = record(&enc(&v));
    assert_eq!(items(&rec), ["checkpoint width"]);
    assert_eq!(rec["checkpoints"][2]["legs"], "not defined");
    assert_eq!(rec["waypoints"][2]["g"], 3);
    assert_eq!(rec["waypoints"][2]["result"], "not defined");
    assert_eq!(rec["waypoints"][1]["result"], "match");
    assert_eq!(rec["analysis"]["per_stage"][2]["checkpoint_count"], 1);
    assert_eq!(rec["analysis"]["whole_hike_legs"], 17);
    assert_eq!(rec["analysis"]["legs_outside"], 17);
    assert_eq!(rec["N"], 1);
    assert_eq!(rec["fixed_start"]["N"], 1);
    assert_eq!(rec["six_leg"], record(&object_bytes())["six_leg"]);
}

/// leg_count is its own condition: a bad width does not hide it.
#[test]
fn a_bad_width_does_not_hide_leg_count() {
    let mut v = object();
    let s = v.stage(3);
    let cp = &mut s.get("checkpoints").arr()[0];
    *cp = V::B(cp.bytes()[..31].to_vec());
    s.set("leg_count", V::U(2));
    assert_eq!(items(&record(&enc(&v))), ["leg_count", "checkpoint width"]);
}

/// An absent checkpoints array: field set, leg_count and checkpoint width;
/// the hike is measured on, legs and W_6 are `not defined`.
#[test]
fn absent_checkpoints_measure_on() {
    let mut v = object();
    v.stage(5).drop_key("checkpoints");
    let rec = record(&enc(&v));
    assert_eq!(items(&rec), ["field set", "leg_count", "checkpoint width"]);
    assert_eq!(rec["checkpoints"][5]["legs"], "not defined");
    assert_eq!(rec["waypoints"][5]["result"], "not defined");
    assert_eq!(rec["analysis"]["per_stage"][5]["checkpoint_count"], 0);
    assert_eq!(rec["seed_value"], "match");
    assert_eq!(rec["headers"][0]["index_comparison"], "match");
}

/// A checkpoints value that is not an array is a wrong value type: field set.
#[test]
fn mistyped_checkpoints_and_leg_results_are_field_set() {
    for key in ["checkpoints", "leg_results"] {
        let mut v = object();
        v.stage(5).set(key, V::T("x".into()));
        let rec = record(&enc(&v));
        assert_eq!(items(&rec)[0], "field set", "{key}");
    }
    // No usable checkpoints: legs are `not defined`. Checkpoints with no
    // usable leg result: each leg is `mismatch` (section 14.2).
    let mut v = object();
    v.stage(5).set("checkpoints", V::T("x".into()));
    assert_eq!(record(&enc(&v))["checkpoints"][5]["legs"], "not defined");
    let mut v = object();
    v.stage(5).set("leg_results", V::T("x".into()));
    assert_eq!(
        record(&enc(&v))["checkpoints"][5]["legs"][0]["result"],
        "mismatch"
    );
}

/// An absent or mistyped stage-17 path is `W_17 path`.
#[test]
fn absent_or_text_stage17_path_is_w17_path() {
    let mut v = object();
    v.stage(17).drop_key("path");
    assert_eq!(items(&record(&enc(&v))), ["field set", "W_17 path"]);
    let mut v = object();
    v.stage(17).set("path", V::T("x".into()));
    assert_eq!(items(&record(&enc(&v))), ["field set", "W_17 path"]);
}

/// No public key: the seed and every leg are `mismatch`; headers are still read.
#[test]
fn absent_public_key_measures_on() {
    let mut v = object();
    v.drop_key("public_key");
    let rec = record(&enc(&v));
    assert_eq!(items(&rec), ["field set"]);
    assert_eq!(rec["seed_value"], "mismatch");
    assert_eq!(rec["headers"][0]["index_comparison"], "match");
    let results = rec["six_leg"]["results"].as_array().unwrap();
    assert!(results.iter().all(|r| r == "mismatch"), "{rec}");
}

/// A stored hash that is not 32 bytes is `not defined`.
#[test]
fn a_short_beacon_has_no_stored_hash() {
    let mut v = object();
    let b = v.stage(8).get("beacon").bytes();
    v.stage(8).set("beacon", V::B(b[..31].to_vec()));
    let rec = record(&enc(&v));
    assert_eq!(rec["headers"][8]["stored_hash"], "not defined");
    assert_eq!(rec["headers"][8]["stored_height"], 1008);
}

/// 19 stages: the draw reads stages 0..17 only (section 14.6, leg_total = start_18).
#[test]
fn a_nineteenth_stage_is_not_drawn() {
    let mut v = object();
    let extra = v.stage(17).clone();
    v.get("stages").arr().push(extra);
    let rec = record(&enc(&v));
    assert_eq!(items(&rec), ["stages length"]);
    assert_eq!(rec["six_leg"], record(&object_bytes())["six_leg"]);
    assert_eq!(rec["analysis"]["whole_hike_legs"], 17);
}

/// No stages at all: the seed value compares against nothing and is `mismatch`.
#[test]
fn no_stages_is_seed_mismatch() {
    let mut v = object();
    v.set("stages", V::A(vec![]));
    let rec = record(&enc(&v));
    assert_eq!(rec["seed_value"], "mismatch");
    assert_eq!(rec["six_leg"], "not defined");
}

/// Six-leg previous for leg j >= 1 is the stored checkpoints[j-1], first 8
/// bytes (sections 11.2 and 14.6). Seed 01*32 draws stage 3 leg 1.
#[test]
fn six_leg_previous_reads_the_first_eight_bytes_of_the_stored_checkpoint() {
    let leg = hex8("0594e6d98cd90356");
    let edit = |width: usize, drop_waypoint: bool| {
        let mut v = object();
        let s = v.stage(3);
        let cp0 = s.get("checkpoints").arr()[0].bytes();
        let lr0 = s.get("leg_results").arr()[0].clone();
        s.set(
            "checkpoints",
            V::A(vec![V::B(cp0[..width].to_vec()), V::B(cp0.clone())]),
        );
        s.set("leg_results", V::A(vec![lr0, V::B(leg.to_vec())]));
        s.set("leg_count", V::U(2));
        if drop_waypoint {
            s.drop_key("waypoint");
        }
        let rec = record_seeded(&enc(&v), [0x01; 32]);
        assert_eq!(rec["six_leg"]["indices"][1], 4, "{rec}");
        rec["six_leg"]["results"][1].as_str().unwrap().to_owned()
    };
    assert_eq!(edit(32, false), "match");
    assert_eq!(edit(31, false), "match");
    assert_eq!(edit(8, false), "match");
    assert_eq!(edit(7, false), "mismatch");
    assert_eq!(edit(32, true), "match");
}

fn hex8(s: &str) -> [u8; 8] {
    let mut out = [0u8; 8];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

/// rc.2 (approved change 1): a top level that is not a map is the undecoded
/// ["malformed cbor"] record, byte for byte the record of no bytes at all.
#[test]
fn a_top_level_that_is_not_a_map_is_malformed_cbor() {
    let none = undecoded();
    assert_eq!(items(&none), ["malformed cbor"]);
    for bytes in [&[0x01u8][..], &[0x80], &[0x60], &[0x40], &[0x01, 0x00]] {
        assert!(!hike::top_level_map(bytes), "{bytes:?}");
        assert_eq!(record(bytes), none, "{bytes:?}");
    }
    // The fast screen (the gate's path) records the same: nothing measured.
    for bytes in [&[][..], &[0x01u8], &[0x80], &[0x60], &[0x40], &[0x01, 0x00]] {
        let fast = hike::screen_fast_sized(
            bytes,
            |_: &hike::PathLocators| hike::ChainRead::NotRead,
            hike::Sizes::conformance_small(),
        );
        assert!(!fast.shape, "{bytes:?}");
        assert!(
            fast.record_body.contains("[\"malformed cbor\"]"),
            "{bytes:?}"
        );
        assert!(!fast.record_body.contains("field set"), "{bytes:?}");
        let quick = hike::verify_fast_sized(
            bytes,
            |_| Err(()),
            |_: &hike::PathLocators| hike::ChainRead::NotRead,
            hike::Sizes::conformance_small(),
        );
        assert!(
            quick.record_body.contains("[\"malformed cbor\"]"),
            "{bytes:?}"
        );
        assert!(!quick.record_body.contains("field set"), "{bytes:?}");
    }
    assert!(hike::top_level_map(&[0xa0]));
    assert!(hike::top_level_map(&[0xa0, 0x00]));
    assert!(!hike::top_level_map(&[0x20]));
    assert!(!hike::top_level_map(&[]));
}

/// F9: a checkpoint or leg-result element that is not bytes lists field set
/// with the width condition.
#[test]
fn a_non_byte_checkpoint_element_is_field_set() {
    for (key, value, width) in [
        ("checkpoints", V::T("x".repeat(32)), "checkpoint width"),
        ("checkpoints", V::U(7), "checkpoint width"),
        ("checkpoints", V::M(vec![]), "checkpoint width"),
        ("checkpoints", V::A(vec![]), "checkpoint width"),
        ("leg_results", V::T("short".to_owned()), "leg result width"),
        ("leg_results", V::U(7), "leg result width"),
        ("leg_results", V::M(vec![]), "leg result width"),
        ("leg_results", V::A(vec![]), "leg result width"),
    ] {
        let mut v = object();
        v.stage(4).get(key).arr()[0] = value;
        let rec = record(&enc(&v));
        let got = items(&rec);
        assert!(got.contains(&"field set"), "{key} {got:?}");
        assert!(got.contains(&width), "{key} {got:?}");
    }
}

/// F7: text last_interrupt and text last_interrupt_height are wrong value
/// types, so field set is listed with the conditions those fields already fail.
#[test]
fn a_text_last_interrupt_is_field_set() {
    let mut hash = object();
    hash.set("last_interrupt", V::T("x".to_owned()));
    let rec = record(&enc(&hash));
    let got = items(&rec);
    assert!(got.contains(&"field set"), "{got:?}");
    assert!(got.contains(&"last_interrupt"), "{got:?}");
    assert!(got.contains(&"header hash differ"), "{got:?}");

    let mut height = object();
    height.set("last_interrupt_height", V::T("not-a-height".to_owned()));
    let rec = record(&enc(&height));
    let got = items(&rec);
    assert!(got.contains(&"field set"), "{got:?}");
    assert!(got.contains(&"last_interrupt"), "{got:?}");
    assert!(got.contains(&"header height order"), "{got:?}");
}

/// short-li: a 31-byte last_interrupt is a byte string of the wrong width.
/// That is last_interrupt (section 14.1 row 11), not field set.
#[test]
fn a_short_last_interrupt_is_not_field_set() {
    let mut v = object();
    let short = v.get("last_interrupt").bytes();
    v.set("last_interrupt", V::B(short[..31].to_vec()));
    let rec = record(&enc(&v));
    assert_eq!(items(&rec), ["last_interrupt", "header hash differ"]);
}

/// F6: more than 18 stages still lists every other violated condition over
/// the stages that are present. A duplicated stage-3 header is header height
/// order and header hash differ, with stages length.
#[test]
fn nineteen_stages_keep_the_other_shape_conditions() {
    let mut v = object();
    let extra = v.stage(17).clone();
    v.get("stages").arr().push(extra);
    let beacon = v.stage(2).get("beacon").clone();
    let height = v.stage(2).get("beacon_height").clone();
    v.stage(3).set("beacon", beacon);
    v.stage(3).set("beacon_height", height);
    let rec = record(&enc(&v));
    assert_eq!(
        items(&rec),
        ["stages length", "header height order", "header hash differ"]
    );
}

/// Q-j: an integer above 2^53−1 that is not a height does not list
/// height_range here. `leg_count` stays `leg_count`. `n_slots` stays `constant`.
#[test]
fn a_non_height_integer_above_the_json_bound_is_not_height_range() {
    let mut legs = object();
    legs.stage(5).set("leg_count", V::U(1 << 53));
    assert_eq!(items(&record(&enc(&legs))), ["leg_count"]);
    let mut slots = object();
    slots.set("n_slots", V::U(u64::MAX));
    assert_eq!(items(&record(&enc(&slots))), ["constant"]);
}

/// F5 / Q-a: last_interrupt_height above 2^53−1 is `height_range` and is
/// written `not defined`. Header height order compares the exact integer, so
/// 2^53 and 2^64−1, both greater than stage 17's height, do not list
/// `header height order`. `last_interrupt` is not listed either.
#[test]
fn an_out_of_range_interrupt_height_keeps_exact_order() {
    for n in [1u64 << 53, u64::MAX] {
        let mut v = object();
        v.set("last_interrupt_height", V::U(n));
        let rec = record(&enc(&v));
        assert_eq!(items(&rec), ["height_range"], "height {n}");
        assert_eq!(rec["headers"][18]["stored_height"], "not defined");
    }
}

/// F5: every header pair uses the exact integer. Stage 17 at 2^53 is still
/// below last_interrupt_height 2^64−1, so the order holds. Stage 16 at
/// 2^64−1 is not below the next height, so the order fails.
#[test]
fn header_pairs_order_over_range_heights_by_exact_value() {
    let mut below = object();
    below.stage(17).set("beacon_height", V::U(1 << 53));
    below.set("last_interrupt_height", V::U(u64::MAX));
    let rec = record(&enc(&below));
    assert_eq!(items(&rec), ["height_range"]);
    assert_eq!(rec["headers"][17]["stored_height"], "not defined");
    assert_eq!(rec["headers"][18]["stored_height"], "not defined");

    let mut gap = object();
    gap.stage(16).set("beacon_height", V::U(u64::MAX));
    let rec = record(&enc(&gap));
    assert_eq!(items(&rec), ["header height order", "height_range"]);
    assert_eq!(rec["headers"][16]["stored_height"], "not defined");
}

/// No index document (the connection was refused, or the height is absent)
/// records plant `not read` on an issued path, not `match`.
#[test]
fn a_missing_index_document_records_plant_not_read() {
    let measurements = hike::verify_sized(
        &object_bytes(),
        hike::AuditDraw::PinnedSeed([0x11; 32]),
        |_| Err(()),
        |_| hike::ChainRead::NotRead,
        hike::Sizes::conformance_small(),
    );
    let line = hike::verify_record_json("conformance-small", &measurements);
    let rec: Value = serde_json::from_str(line.trim_end()).unwrap();
    let paths = rec["paths"].as_array().unwrap();
    assert!(paths.iter().any(|row| row.get("plant").and_then(|p| p.as_str()) == Some("not read")));
    assert!(paths.iter().all(|row| row.get("plant").and_then(|p| p.as_str()) != Some("match")));
}

/// F2: a declared byte, text, array or map length past the bytes left is
/// the undecoded ["malformed cbor"] record. The same heads the Go and Java
/// decoders reject without allocating from the declared length.
#[test]
fn an_oversized_length_is_malformed_cbor() {
    let none = undecoded();
    let inputs: &[&[u8]] = &[
        &[0x5b, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        &[0x7b, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        &[0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        &[0xba, 0xff, 0xff, 0xff, 0xff],
        &[0x9a, 0x3b, 0x9a, 0xca, 0x00],
    ];
    for bytes in inputs {
        assert!(!hike::top_level_map(bytes), "{bytes:?}");
        assert_eq!(record(bytes), none, "{bytes:?}");
    }
}

/// rc.2 (approved change 3): decoded_hike refuses trailing bytes.
#[test]
fn decoded_hike_refuses_trailing_bytes() {
    let mut bytes = object_bytes();
    assert!(hike::decoded_hike(&bytes).is_ok());
    bytes.push(0x00);
    assert!(hike::decoded_hike(&bytes).is_err());
    assert!(hike::decoded_hike(&[0x01]).is_err());
}

/// A trailing byte: a strictly usable map is measured with exactly
/// ["malformed cbor"]; any other map gets the undecoded record.
#[test]
fn a_trailing_byte_measures_only_a_strictly_usable_map() {
    let none = undecoded();
    let trailing = |v: &V| record(&[enc(v), vec![0x00]].concat());
    let rec = trailing(&object());
    assert_eq!(items(&rec), ["malformed cbor"]);
    assert_eq!(rec["seed_value"], "match");
    let mut v = object();
    v.map().push(("zzz".into(), V::U(1)));
    assert_eq!(trailing(&v)["seed_value"], "match");
    let edits: [fn(&mut V); 4] = [
        |v| v.drop_key("public_key"),
        |v| v.stage(5).drop_key("checkpoints"),
        |v| v.stage(3).set("leg_count", V::U(2)),
        |v| v.stage(5).map().push(("zzz".into(), V::U(1))),
    ];
    for (i, edit) in edits.iter().enumerate() {
        let mut v = object();
        edit(&mut v);
        assert_eq!(trailing(&v), none, "edit {i}");
    }
}
