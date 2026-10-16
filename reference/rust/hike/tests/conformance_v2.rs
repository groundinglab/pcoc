//! Conformance pack for Satoshi Hike v2.0, regenerated from this reference.
//!
//! `PCOC_REGEN_V2=1` writes `spec/conformance/v2.0/`. Without it, the test reads
//! those files and checks them against a fresh run of this library.

use hike::{
    encode_hike, encode_hike_sized, encode_path_in_block, parse_index_beacon, proxy_matches,
    structural_object, verify, verify_record_json, verify_sized, verify_with_bind_threads,
    AuditDraw, ChainRead, PathLocators, ANCHOR_WINDOW_OFFSETS, BEACON_BYTES,
    CHECKPOINT_PREIMAGE_LEN, DEFAULT_BIND_THREADS, EARLIER_KINDS, FILL, HASH_BYTES, KIND,
    LEG_RATE_FLOOR_DEFAULT, LEG_RATE_FLOOR_LENIENT, LEG_RATE_FLOOR_STRICT, LEG_RESULT_BYTES,
    MIX_ADD, MIX_MUL1, MIX_MUL2, N_SLOTS, OFFSET_MODULUS, PARENT_MUL, PUBLIC_KEY_BYTES,
    SAMPLE_BYTES, SEAL_DOMAIN, SEED_DOMAIN, SLOT_BYTES, SLOT_MUL, STAGES, STEPS_PER_LEG,
    TERRAIN_BYTES, WORDS_PER_SLOT, WORD_XOR,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn pack_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../spec/conformance/v2.0")
}

fn repo_file(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(rel)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn quiet() -> hike::Hike {
    let mut hike = structural_object();
    for stage in &mut hike.stages {
        stage.leg_count = 0;
        stage.checkpoints.clear();
        stage.leg_results.clear();
        stage.path.clear();
    }
    hike
}

fn anchor_fixture() -> (String, Vec<u32>, Value) {
    let mut hike = quiet();
    hike.stages[3].beacon = [0x55; 32];
    let inside = hike.stages[1].waypoint;
    hike.stages[1].path = encode_path_in_block(&inside, 4);
    let outside = hike.stages[2].waypoint;
    hike.stages[2].path = encode_path_in_block(&outside, 100);
    let missing_header = hike.stages[4].waypoint;
    hike.stages[4].path = encode_path_in_block(&missing_header, 7);
    let bytes = encode_hike(&hike);
    // Height 4 is a plant document with no `header`. The header GETs return
    // no body. Those are separate reads of the same URL shape.
    let plant = plant_document(&fixture_locators(&inside, 4), "synthetic", None);
    let mut reads = Vec::new();
    let measurements = verify(
        &bytes,
        AuditDraw::PinnedSeed([0u8; 32]),
        |_| Err(()),
        |loc| {
            reads.push(loc.block_height);
            if loc.block_height == 4 {
                bind_plant(&plant, loc)
            } else {
                ChainRead::NotRead
            }
        },
    );
    let record = verify_record_json("production", &measurements);
    let value = json!({
        "id": "anchor-window",
        "cases": {
            "inside_the_window": "anchored",
            "outside_the_window": "not anchored",
            "header_not_in_the_object": "not anchored",
            "no_path": "not anchored",
        },
        "object_hex": hex_encode(&bytes),
        "sha256": sha256_hex(&bytes),
        "header_reads": null_header_reads(&hike),
        "plant_reads": [{"height": 4, "body": plant}],
        "index_reads": reads,
        "record": record.clone(),
    });
    (record, reads_of(&value), value)
}

fn w16_in_i17() -> Value {
    let mut hike = quiet();
    hike.last_interrupt = [0x55; 32];
    hike.last_interrupt_height = 19;
    hike.last_interrupt_absent = false;
    let waypoint = hike.stages[16].waypoint;
    hike.stages[16].path = encode_path_in_block(&waypoint, 19);
    let bytes = encode_hike(&hike);
    let plant = plant_document(&fixture_locators(&waypoint, 19), "main", None);
    let mut reads = Vec::new();
    let measurements = verify(
        &bytes,
        AuditDraw::PinnedSeed([0u8; 32]),
        |_| Err(()),
        |loc| {
            reads.push(loc.block_height);
            bind_plant(&plant, loc)
        },
    );
    let record = verify_record_json("production", &measurements);
    let value: Value = serde_json::from_str(record.trim()).unwrap();
    let anchors = value["anchors"].as_array().unwrap();
    assert_eq!(anchors[15], "anchored");
    assert!(anchors[..15].iter().all(|token| token == "not anchored"));
    assert_eq!(reads, vec![19]);
    json!({
        "id": "anchor-w16-i17",
        "case": "W_16 included in I_17",
        "object_hex": hex_encode(&bytes),
        "sha256": sha256_hex(&bytes),
        "header_reads": null_header_reads(&hike),
        "plant_reads": [{"height": 19, "body": plant}],
        "index_reads": reads,
        "record": record,
    })
}

fn bind_record() -> Value {
    let mut hike = quiet();
    hike.stages[3].leg_count = 4;
    hike.stages[3].checkpoints = (0..4u8).map(|j| [j; 32]).collect();
    hike.stages[3].leg_results = (0..4u64).map(|j| j.to_le_bytes()).collect();
    let bytes = encode_hike(&hike);
    let draw = AuditDraw::PinnedSeed([9u8; 32]);
    let one = verify_with_bind_threads(&bytes, draw, |_| Err(()), |_| Ok(()), 1);
    let draw = AuditDraw::PinnedSeed([9u8; 32]);
    let many =
        verify_with_bind_threads(&bytes, draw, |_| Err(()), |_| Ok(()), DEFAULT_BIND_THREADS);
    let left = verify_record_json("production", &one);
    let right = verify_record_json("production", &many);
    assert_eq!(left, right);
    json!({
        "id": "bind-threads",
        "threads": [1, DEFAULT_BIND_THREADS],
        "object_hex": hex_encode(&bytes),
        "sha256": sha256_hex(&bytes),
        "header_reads": null_header_reads(&hike),
        "plant_reads": [],
        "record": left,
    })
}

fn earlier_kind_object() -> Vec<u8> {
    let kind = b"satoshi-hike-v1.35-object";
    let mut out = vec![0xa1, 0x64];
    out.extend_from_slice(b"kind");
    out.push(0x78);
    out.push(kind.len() as u8);
    out.extend_from_slice(kind);
    out
}

fn earlier_record() -> Value {
    let rel = "spec/conformance/v2.0/earlier-kind.cbor";
    let bytes = earlier_kind_object();
    assert_eq!(
        fs::read(repo_file(rel)).expect("earlier kind object"),
        bytes
    );
    let measurements = verify(
        &bytes,
        AuditDraw::PinnedSeed([0u8; 32]),
        |_| panic!("an earlier object is not measured"),
        |_| -> ChainRead { panic!("an earlier object is not measured") },
    );
    let record = verify_record_json("production", &measurements);
    json!({
        "id": "earlier-run-b",
        "object": rel,
        "object_hex": hex_encode(&bytes),
        "sha256": sha256_hex(&bytes),
        "record": record,
    })
}

fn checkpoint_kat() -> Value {
    let previous = [0x11u8; 32];
    let limb = [0x22u8; 32];
    let mut sample = vec![0u8; SAMPLE_BYTES];
    sample[0] = 0x33;
    sample[SAMPLE_BYTES - 1] = 0x44;
    let leg = 7u64.to_le_bytes();
    let mut pre = Vec::with_capacity(CHECKPOINT_PREIMAGE_LEN);
    pre.extend_from_slice(&previous);
    pre.extend_from_slice(&limb);
    pre.extend_from_slice(&sample);
    pre.extend_from_slice(&leg);
    assert_eq!(pre.len(), CHECKPOINT_PREIMAGE_LEN);
    json!({
        "id": "checkpoint-preimage",
        "preimage_len": CHECKPOINT_PREIMAGE_LEN,
        "preimage_hex": hex_encode(&pre),
        "sha256": sha256_hex(&pre),
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// SPEC §16.1 TEST vector. Three legs, one per stage. Not an 18-stage object.
///
/// `legs_per_stage` and `draw_bytes_hex` are rig metadata. The draw is not
/// consumed: three legs is fewer than six, so `six_leg` is `not defined`.
/// The public key is the fixed pattern 0x42. It is not a BIP-340 key.
/// One path names the mock block `0x55…55` at height 102, which is stage 2's
/// stored beacon and sits in stage 1's anchor window (100 + 2). The index
/// read fails. The path chain returns success.
fn test_3leg() -> Value {
    let sizes = hike::Sizes::conformance_small();
    let public_key = [0x42u8; 32];
    let mut beacon0 = [0u8; 32];
    beacon0[0] = 1;
    let mut beacon1 = [0u8; 32];
    beacon1[0] = 2;
    let beacon2 = [0x55u8; 32];
    let mut last = [0u8; 32];
    last[0] = 4;
    let seed = hike::production_seed(&public_key, &beacon0);
    let seed_digest = Sha256::digest(seed);
    let mut seed_value = [0u8; 32];
    seed_value.copy_from_slice(&seed_digest);
    let terrain = hike::fill_terrain_sized(hike::fill_pin(&seed), sizes);

    let leg_of = |previous: &[u8; 32], limb: &[u8; 32]| {
        let leg = hike::leg_result_sized(&terrain, previous, sizes);
        let sample = hike::terrain_sample_sized(&terrain, previous, sizes);
        let digest = hike::checkpoint_sized(previous, limb, &sample, leg, sizes);
        (digest, leg.to_le_bytes())
    };
    let (c0, r0) = leg_of(&seed_value, &seed_value);
    let w1 = hike::successor_waypoint_sized(&terrain, &beacon1, &seed_value, &[c0], sizes);
    let (c1, r1) = leg_of(&w1, &w1);
    let w2 = hike::successor_waypoint_sized(&terrain, &beacon2, &w1, &[c1], sizes);
    let (c2, r2) = leg_of(&w2, &w2);
    let path = encode_path_in_block(&w1, 102);

    let object = hike::Hike {
        public_key,
        seed_beacon: beacon0,
        stages: vec![
            hike::Stage {
                beacon: beacon0,
                beacon_height: 90,
                leg_count: 1,
                waypoint: seed_value,
                path: Vec::new(),
                checkpoints: vec![c0],
                leg_results: vec![r0],
                checkpoint_rows_defined: true,
                leg_result_missing: Vec::new(),
            },
            hike::Stage {
                beacon: beacon1,
                beacon_height: 100,
                leg_count: 1,
                waypoint: w1,
                path,
                checkpoints: vec![c1],
                leg_results: vec![r1],
                checkpoint_rows_defined: true,
                leg_result_missing: Vec::new(),
            },
            hike::Stage {
                beacon: beacon2,
                beacon_height: 102,
                leg_count: 1,
                waypoint: w2,
                path: Vec::new(),
                checkpoints: vec![c2],
                leg_results: vec![r2],
                checkpoint_rows_defined: true,
                leg_result_missing: Vec::new(),
            },
        ],
        last_interrupt: last,
        last_interrupt_height: 110,
        last_interrupt_absent: false,
        last_interrupt_hash_absent: false,
        last_interrupt_height_absent: false,
    };
    let bytes = encode_hike_sized(&object, sizes);
    let draw = [0xA5u8; 48];
    let plant = plant_document(&fixture_locators(&w1, 102), "synthetic", None);
    let measurements = verify_sized(
        &bytes,
        AuditDraw::PinnedSeed([0xA5; 32]),
        |_| Err(()),
        |loc| bind_plant(&plant, loc),
        sizes,
    );
    let record = verify_record_json("conformance-small", &measurements);
    let value: Value = serde_json::from_str(record.trim_end()).unwrap();
    assert_eq!(value["profile"], "conformance-small");
    assert_eq!(value["sample_bytes"], 32);
    assert_eq!(value["slot_bytes"], 64);
    assert_eq!(value["terrain_bytes"], 256);
    assert_eq!(value["steps_per_leg"], 32);
    assert_eq!(value["shape"], "mismatch");
    assert_eq!(
        value["shape_item"],
        json!([
            "W_17 path",
            "stages length",
            "leg_count",
            "last_interrupt",
            "header height order",
            "header hash differ"
        ])
    );
    assert_eq!(value["seed_value"], "match");
    assert_eq!(value["checkpoints"][0]["legs"][0]["result"], "match");
    assert_eq!(value["checkpoints"][1]["legs"][0]["result"], "match");
    assert_eq!(value["checkpoints"][2]["legs"][0]["result"], "match");
    assert_eq!(value["checkpoints"][3]["legs"], "not defined");
    assert_eq!(value["waypoints"][0]["result"], "match");
    assert_eq!(value["waypoints"][1]["result"], "match");
    assert_eq!(value["waypoints"][2]["result"], "not defined");
    assert!(value["waypoints"]
        .as_array()
        .unwrap()
        .iter()
        .skip(2)
        .all(|row| row["result"] == "not defined"));
    assert_eq!(value["paths"][0]["path_state"], "present");
    assert_eq!(value["paths"][0]["plant"], "match");
    assert_eq!(value["paths"][0]["d_g"], "not defined");
    assert_eq!(value["anchors"][0], "anchored");
    assert!(value["anchors"]
        .as_array()
        .unwrap()
        .iter()
        .skip(1)
        .all(|token| token == "not anchored"));
    assert_eq!(value["paths"][1]["path_state"], "skip");
    assert_eq!(value["paths"][2]["path_state"], "not defined");
    assert_eq!(value["paths"][16]["w17_path"], "not defined");
    assert_eq!(value["skip_list"], json!([2]));
    assert_eq!(value["six_leg"], "not defined");
    assert_eq!(value["six_leg_runs"], json!([]));
    assert_eq!(value["N"], "not defined");
    assert_eq!(value["S"], "not defined");
    assert_eq!(value["analysis"]["whole_hike_legs"], "not defined");
    assert_eq!(value["analysis"]["per_stage"][0]["checkpoint_count"], 1);
    assert_eq!(value["analysis"]["per_stage"][1]["checkpoint_count"], 1);
    assert_eq!(
        value["analysis"]["per_stage"][3]["checkpoint_count"],
        "not defined"
    );
    assert!(record.ends_with('\n'));
    assert!(!record.contains("\"sample_bytes\":1024"));

    json!({
        "id": "test-3leg",
        "label": "TEST vector. Mock anchor. Not a real anchored PCOC. Not a public example.",
        "public_key_note": "32 bytes of 0x42. A fixed test pattern, not a BIP-340 key.",
        "legs_per_stage": [1, 1, 1],
        "draw_bytes_hex": hex_encode(&draw),
        "draw_consumed": false,
        "object_hex": hex_encode(&bytes),
        "sha256": sha256_hex(&bytes),
        "header_reads": null_header_reads(&object),
        "plant_reads": [{"height": 102, "body": plant}],
        "record": record,
    })
}

fn pack() -> Vec<(&'static str, Value)> {
    let (anchor_record, reads, anchor_doc) = anchor_fixture();
    let anchor_value: Value = serde_json::from_str(anchor_record.trim()).unwrap();
    let anchors = anchor_value["anchors"].as_array().unwrap();
    assert_eq!(anchors[0], "anchored");
    assert_eq!(anchors[1], "not anchored");
    assert_eq!(anchors[3], "not anchored");
    assert_eq!(anchors[4], "not anchored");
    assert_eq!(reads, vec![4]);
    assert_eq!(anchor_doc["record"], anchor_record);

    let w16 = w16_in_i17();
    let earlier = earlier_record();
    let bind = bind_record();

    vec![
        (
            "constants.json",
            json!({
                "id": "constants",
                "SLOT_BYTES": SLOT_BYTES,
                "WORDS_PER_SLOT": WORDS_PER_SLOT,
                "TERRAIN_BYTES": TERRAIN_BYTES,
                "N_SLOTS": N_SLOTS,
                "STAGES": STAGES,
                "HASH_BYTES": HASH_BYTES,
                "LEG_RESULT_BYTES": LEG_RESULT_BYTES,
                "PUBLIC_KEY_BYTES": PUBLIC_KEY_BYTES,
                "BEACON_BYTES": BEACON_BYTES,
                "SEAL_DOMAIN": hex_bytes(SEAL_DOMAIN),
                "SEED_DOMAIN": hex_bytes(SEED_DOMAIN),
                "FILL": FILL,
                "MIX_ADD": format!("0x{MIX_ADD:016X}"),
                "MIX_MUL1": format!("0x{MIX_MUL1:016X}"),
                "MIX_MUL2": format!("0x{MIX_MUL2:016X}"),
                "WORD_XOR": format!("0x{WORD_XOR:X}"),
                "PARENT_MUL": format!("0x{PARENT_MUL:016X}"),
                "SLOT_MUL": format!("0x{SLOT_MUL:016X}"),
                "SAMPLE_BYTES": SAMPLE_BYTES,
                "OFFSET_MODULUS": OFFSET_MODULUS,
                "STEPS_PER_LEG": STEPS_PER_LEG,
                "CHECKPOINT_PREIMAGE_LEN": CHECKPOINT_PREIMAGE_LEN,
                "KIND": KIND,
                "EARLIER_KINDS": EARLIER_KINDS,
                "ANCHOR_WINDOW_OFFSETS": ANCHOR_WINDOW_OFFSETS,
                "DEFAULT_BIND_THREADS": DEFAULT_BIND_THREADS,
                "DEFAULT_BIND_THREADS_NOTE": "non-normative reference default",
                "LEG_RATE_FLOOR": {
                    "strict": {"p": LEG_RATE_FLOOR_STRICT.0, "q": LEG_RATE_FLOOR_STRICT.1},
                    "default": {"p": LEG_RATE_FLOOR_DEFAULT.0, "q": LEG_RATE_FLOOR_DEFAULT.1},
                    "lenient": {"p": LEG_RATE_FLOOR_LENIENT.0, "q": LEG_RATE_FLOOR_LENIENT.1},
                },
                "LEG_RATE_FLOOR_NOTE": "Informative example values from §15.5 (Part B). Not a conformance target; policy sets the value.",
            }),
        ),
        ("checkpoint-preimage.json", checkpoint_kat()),
        ("anchor-window.json", anchor_doc),
        ("anchor-w16-i17.json", w16),
        ("earlier-run-b.json", earlier),
        ("bind-threads.json", bind),
        ("test-3leg.json", test_3leg()),
        ("kat-small.json", kat_small()),
    ]
}

fn reads_of(value: &Value) -> Vec<u32> {
    value["index_reads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_u64().unwrap() as u32)
        .collect()
}

fn null_header_reads(hike: &hike::Hike) -> Value {
    let mut rows = Vec::new();
    for stage in &hike.stages {
        rows.push(json!({"height": stage.beacon_height, "body": null}));
    }
    if !hike.last_interrupt_height_absent {
        rows.push(json!({"height": hike.last_interrupt_height, "body": null}));
    }
    Value::Array(rows)
}

fn display_hex(natural: &[u8; 32]) -> String {
    let mut displayed = *natural;
    displayed.reverse();
    hex_encode(&displayed)
}

/// One-leaf fixture body from `encode_path_in_block`: txid `0x44`, vout 2,
/// block hash `0x55`, root `SHA-256(0x1A ‖ waypoint)`.
fn fixture_locators(waypoint: &[u8; 32], block_height: u32) -> PathLocators {
    let mut pre = [0u8; 33];
    pre[0] = 0x1A;
    pre[1..].copy_from_slice(waypoint);
    PathLocators {
        root: sha2::Sha256::digest(pre).into(),
        txid: [0x44; 32],
        vout: 2,
        block_hash: [0x55; 32],
        block_height,
    }
}

fn plant_object(loc: &PathLocators) -> Value {
    json!({
        "root": hex_encode(&loc.root),
        "txid": display_hex(&loc.txid),
        "vout": loc.vout,
        "block_hash": display_hex(&loc.block_hash),
        "block_height": loc.block_height,
    })
}

/// Plant read body. `header` is omitted on purpose when `header` is `None`:
/// the header measurement of these bytes is an unsuccessful read.
fn plant_document(loc: &PathLocators, network: &str, header: Option<Value>) -> String {
    let mut doc = serde_json::Map::new();
    doc.insert("network".to_owned(), json!(network));
    if let Some(header) = header {
        doc.insert("header".to_owned(), header);
    }
    doc.insert("plant".to_owned(), plant_object(loc));
    serde_json::to_string(&Value::Object(doc)).unwrap()
}

fn bind_plant(document: &str, loc: &PathLocators) -> ChainRead {
    if proxy_matches(document, loc).is_ok() {
        ChainRead::Match
    } else {
        ChainRead::Mismatch
    }
}

fn header_document(natural: &[u8; 32], height: u64, time: u64, plants: Vec<Value>) -> String {
    let mut doc = serde_json::Map::new();
    doc.insert("network".to_owned(), json!("synthetic"));
    doc.insert(
        "header".to_owned(),
        json!({
            "hash": display_hex(natural),
            "height": height,
            "time": time,
        }),
    );
    if !plants.is_empty() {
        doc.insert("plants".to_owned(), Value::Array(plants));
    }
    serde_json::to_string(&Value::Object(doc)).unwrap()
}

fn pinned_stream(seed: &[u8; 32], nbytes: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut counter = 0u64;
    while out.len() < nbytes {
        let mut pre = [0u8; 40];
        pre[..32].copy_from_slice(seed);
        pre[32..].copy_from_slice(&counter.to_le_bytes());
        out.extend_from_slice(&sha2::Sha256::digest(pre));
        counter += 1;
    }
    out.truncate(nbytes);
    out
}

/// SPEC §16.1 known answers. Eighteen stages, one real leg each, two anchored
/// paths, numeric bookends, and one six-leg draw. Not a production-size object.
fn kat_small() -> Value {
    let sizes = hike::Sizes::conformance_small();
    let public_key = [0x7Au8; 32];
    let mut beacons = [[0u8; 32]; 18];
    let mut heights = [0u64; 18];
    for g in 0..18 {
        beacons[g][0] = g as u8 + 1;
        heights[g] = 1000 + g as u64;
    }
    beacons[4] = [0x55; 32];
    let seed_beacon = beacons[0];
    let seed = hike::production_seed(&public_key, &seed_beacon);
    let seed_digest = sha2::Sha256::digest(seed);
    let mut seed_value = [0u8; 32];
    seed_value.copy_from_slice(&seed_digest);
    let terrain = hike::fill_terrain_sized(hike::fill_pin(&seed), sizes);

    let mut limb = seed_value;
    let mut stages = Vec::with_capacity(18);
    let mut w1 = [0u8; 32];
    let mut stage0_sample = Vec::new();
    let mut stage0_leg = 0u64;
    let mut stage0_checkpoint = [0u8; 32];
    for g in 0..18 {
        let previous = limb;
        let leg = hike::leg_result_sized(&terrain, &previous, sizes);
        let sample = hike::terrain_sample_sized(&terrain, &previous, sizes);
        let digest = hike::checkpoint_sized(&previous, &limb, &sample, leg, sizes);
        let path = if g == 1 || g == 2 {
            encode_path_in_block(&limb, 1004)
        } else {
            Vec::new()
        };
        if g == 0 {
            stage0_sample = sample;
            stage0_leg = leg;
            stage0_checkpoint = digest;
        }
        stages.push(hike::Stage {
            beacon: beacons[g],
            beacon_height: heights[g],
            leg_count: 1,
            waypoint: limb,
            path,
            checkpoints: vec![digest],
            leg_results: vec![leg.to_le_bytes()],
            checkpoint_rows_defined: true,
            leg_result_missing: Vec::new(),
        });
        if g < 17 {
            limb = hike::successor_waypoint_sized(
                &terrain,
                &beacons[g + 1],
                &previous,
                &[digest],
                sizes,
            );
            if g == 0 {
                w1 = limb;
            }
        }
    }
    let mut last = [0u8; 32];
    last[0] = 0x63;
    let object = hike::Hike {
        public_key,
        seed_beacon,
        stages,
        last_interrupt: last,
        last_interrupt_height: 1020,
        last_interrupt_absent: false,
        last_interrupt_hash_absent: false,
        last_interrupt_height_absent: false,
    };
    let bytes = encode_hike_sized(&object, sizes);

    let loc_w1 = fixture_locators(&object.stages[1].waypoint, 1004);
    let loc_w2 = fixture_locators(&object.stages[2].waypoint, 1004);
    assert!(proxy_matches(&plant_document(&loc_w1, "synthetic", None), &loc_w1).is_ok());
    let mut documents = Vec::new();
    for stage in &object.stages {
        let time = 1_700_000_000 + stage.beacon_height;
        let plants = if stage.beacon_height == 1004 {
            vec![plant_object(&loc_w1), plant_object(&loc_w2)]
        } else {
            Vec::new()
        };
        documents.push(json!({
            "height": stage.beacon_height,
            "body": header_document(&stage.beacon, stage.beacon_height, time, plants),
        }));
    }
    documents.push(json!({
        "height": object.last_interrupt_height,
        "body": header_document(&object.last_interrupt, 1020, 1_700_000_000 + 1020, Vec::new()),
    }));
    let mut by_height = std::collections::HashMap::new();
    for row in &documents {
        by_height.insert(
            row["height"].as_u64().unwrap(),
            row["body"].as_str().unwrap().to_owned(),
        );
    }
    let draw_seed = [0x5Au8; 32];
    let draw = pinned_stream(&draw_seed, 256);
    let mut reads = Vec::new();
    let measurements = verify_sized(
        &bytes,
        AuditDraw::PinnedSeed(draw_seed),
        |height| {
            let body = by_height.get(&height).ok_or(())?;
            parse_index_beacon(body).map_err(|_| ())
        },
        |loc| {
            reads.push(loc.block_height);
            let Some(body) = by_height.get(&u64::from(loc.block_height)) else {
                return ChainRead::NotRead;
            };
            bind_plant(body, loc)
        },
        sizes,
    );
    let record = verify_record_json("conformance-small", &measurements);
    let parsed: Value = serde_json::from_str(record.trim_end()).unwrap();
    assert_eq!(parsed["shape"], "match");
    assert_eq!(parsed["seed_value"], "match");
    assert!(parsed["checkpoints"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["legs"][0]["result"] == "match"));
    assert!(parsed["waypoints"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["result"] == "match"));
    assert_eq!(parsed["anchors"][0], "anchored");
    assert_eq!(parsed["anchors"][1], "anchored");
    assert_eq!(parsed["a"], 1);
    assert_eq!(parsed["b"], 2);
    assert_eq!(parsed["N"], 1);
    assert_eq!(parsed["S"], 3);
    assert_eq!(parsed["delta_h"], 3);
    assert_eq!(reads, vec![1004, 1004]);
    let six = &parsed["six_leg"];
    assert_eq!(six["results"].as_array().unwrap().len(), 6);
    assert!(six["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(|token| token == "match"));
    assert_eq!(parsed["waypoints"][0]["result"], "match");
    assert_eq!(w1, object.stages[1].waypoint);

    json!({
        "id": "kat-small",
        "profile": "conformance-small",
        "label": "Known answers at conformance-small. One real leg per stage. Not a full-size object.",
        "public_key_hex": hex_encode(&public_key),
        "seed_beacon_hex": hex_encode(&seed_beacon),
        "sample_bytes": sizes.sample_bytes,
        "slot_bytes": sizes.slot_bytes,
        "terrain_bytes": sizes.terrain_bytes,
        "steps_per_leg": sizes.steps_per_leg,
        "terrain_hex": hex_encode(&terrain),
        "leg": {
            "previous_hex": hex_encode(&seed_value),
            "limb_hex": hex_encode(&seed_value),
            "sample_hex": hex_encode(&stage0_sample),
            "leg_result_hex": hex_encode(&stage0_leg.to_le_bytes()),
            "checkpoint_hex": hex_encode(&stage0_checkpoint),
        },
        "waypoint_w1": {
            "interrupt_hex": hex_encode(&beacons[1]),
            "limb_hex": hex_encode(&seed_value),
            "checkpoint_hex": hex_encode(&stage0_checkpoint),
            "digest_hex": hex_encode(&w1),
        },
        "object_hex": hex_encode(&bytes),
        "sha256": sha256_hex(&bytes),
        "draw_bytes_hex": hex_encode(&draw),
        "documents": documents,
        "index_reads": reads,
        "bookends": {"a": 1, "b": 2, "N": 1, "S": 3, "delta_h": 3},
        "six_leg": six.clone(),
        "record": record,
    })
}

fn write_pack(files: &[(&str, Value)]) {
    let root = pack_root();
    fs::create_dir_all(&root).unwrap();
    let mut ids = Vec::new();
    for (name, value) in files {
        ids.push(value["id"].as_str().unwrap());
        let text = serde_json::to_string_pretty(value).unwrap() + "\n";
        fs::write(root.join(name), text).unwrap();
    }
    let manifest = json!({
        "pack": "satoshi-hike-v2.0",
        "generator": "cargo test -p hike --test conformance_v2",
        "files": files.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        "ids": ids,
    });
    fs::write(
        root.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap() + "\n",
    )
    .unwrap();
}

#[test]
fn conformance_v2_pack() {
    let files = pack();
    if std::env::var("PCOC_REGEN_V2").ok().as_deref() == Some("1") {
        write_pack(&files);
    }
    let root = pack_root();
    for (name, value) in &files {
        let path = root.join(name);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        let got: Value = serde_json::from_str(&text).unwrap_or_else(|err| panic!("{name}: {err}"));
        assert_eq!(&got, value, "{name}");
        if let Some(record) = value["record"].as_str() {
            let stored = got["record"].as_str().expect("record string");
            assert_eq!(stored.as_bytes(), record.as_bytes(), "{name} record bytes");
            assert!(
                stored.ends_with('\n'),
                "{name} record is the CLI bytes, including the trailing newline"
            );
        }
    }
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let names: Vec<&str> = files.iter().map(|(name, _)| *name).collect();
    let listed: Vec<&str> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect();
    assert_eq!(listed, names);
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
