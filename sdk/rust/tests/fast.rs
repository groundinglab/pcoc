//! Lean checks for AC-C2, AC-C3, AC-C5, AC-C6 (honest), AC-C7. No PROD terrain.

use pcoc_sdk::cbor;
use pcoc_sdk::constants::{KIND, SEAL_DOMAIN};
use pcoc_sdk::draw::{draw_one, draw_six, DrawStop};
use pcoc_sdk::hash::sha256;
use pcoc_sdk::hashcash::{hashcash_digest, hashcash_solve, leading_zero_bits};
use pcoc_sdk::hexutil::{hex_decode_exact, hex_encode};
use pcoc_sdk::hike::produce_hike;
use pcoc_sdk::jsonutil::unknown_kind_record;
use pcoc_sdk::mix::{bitrev_parent, mul_parent};
use pcoc_sdk::object::{decode_object, encode_object, DecodeError};
use pcoc_sdk::pin::check_pin;
use pcoc_sdk::profile::Profile;
use pcoc_sdk::step::step;
use pcoc_sdk::terrain::{lanes, nl4_base_word, seed};
use pcoc_sdk::verify::{verify, EmptyIndex, Verified};
use pcoc_sdk::version;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn verify_bin() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_pcoc_verify")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
            root().join("target").join(profile).join("pcoc-verify")
        })
}

fn s1_profile() -> Profile {
    Profile::conformance_small(32, 64, 65536, 100).unwrap()
}

fn pk_a() -> [u8; 32] {
    hex_decode_exact("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9").unwrap()
}

fn seed_beacon() -> [u8; 32] {
    sha256(b"pcoc-pack-2.2/beacon/seed")
}

fn interrupts() -> [([u8; 32], u64); 18] {
    let mut out = [([0u8; 32], 0u64); 18];
    for k in 0..18 {
        let name = format!("pcoc-pack-2.2/beacon/{k}");
        out[k] = (sha256(name.as_bytes()), 1001 + k as u64);
    }
    out
}

fn honest_s1() -> (Profile, Vec<u8>) {
    let profile = s1_profile();
    let (terrain, _, _) = pcoc_sdk::terrain::terrain_from_seed(&pk_a(), &seed_beacon(), &profile);
    let legs = [1u32; 18];
    let (obj, _, _) = produce_hike(
        &profile,
        &pk_a(),
        &seed_beacon(),
        1000,
        &interrupts(),
        &legs,
        &terrain,
    );
    let bytes = encode_object(&obj);
    (profile, bytes)
}

fn draw_hex_0_to_5() -> String {
    let mut s = String::new();
    for i in 0..6u64 {
        s.push_str(&hex_encode(&i.to_le_bytes()));
    }
    s
}

#[test]
fn section_7_4_pins() {
    assert_eq!(1u64.to_le_bytes(), [0x01, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        0x0102030405060708u64.to_le_bytes(),
        [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    assert_eq!(
        hex_encode(&sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let mut pre = [0u8; 20];
    pre[..12].copy_from_slice(SEAL_DOMAIN);
    let e = sha256(&pre);
    assert_eq!(
        hex_encode(&e),
        "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533"
    );
    let s_prime = u64::from_le_bytes(e[..8].try_into().unwrap());
    assert_eq!(s_prime, 0x76042423afbd1648);

    let mut seed_pre = [0x11u8; 64];
    seed_pre[32..].fill(0x22);
    let d = sha256(&seed_pre);
    assert_eq!(
        hex_encode(&d),
        "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c"
    );
    let p = lanes(&d);
    assert_eq!(p[0], 0x545dfe297dc78951);
    assert_eq!(p[1], 0x27858669c45e046a);
    assert_eq!(p[2], 0x9cdac73ac1a5fe85);
    assert_eq!(p[3], 0x7c81df6efbf55f11);
    assert_eq!(nl4_base_word(&p, 0, 0), 0xc7582082a632c2c8);
    assert_eq!(seed(&[0x11u8; 32], &[0x22u8; 32]), d);
}

#[test]
fn parents_stay_below_i() {
    for i in 1..4096u64 {
        assert!(bitrev_parent(i) < i);
        assert!(mul_parent(i) < i);
    }
}

#[test]
fn version_is_2_2_0() {
    assert_eq!(version(), "2.2.0");
    assert_eq!(pcoc_sdk::VERSION, "2.2.0");
    let file = std::fs::read_to_string(root().join("../VERSION")).unwrap();
    assert_eq!(file, "2.2.0\n");
    let out = Command::new(verify_bin())
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "2.2.0\n");
}

#[test]
fn pin_matches_manifest_and_rejects_one_byte() {
    let pin = std::fs::read_to_string(root().join("../SPEC_PIN")).unwrap();
    let manifest = std::fs::read(root().join("../../spec/conformance/pack/manifest.json")).unwrap();
    let got = check_pin(&pin, &manifest).expect("pin");
    assert_eq!(got.tag, "spec-2.2.0");
    let mut flipped = manifest.clone();
    flipped[0] ^= 0x01;
    assert!(check_pin(&pin, &flipped).is_err());
}

#[test]
fn object_round_trip_s1() {
    let (_profile, bytes) = honest_s1();
    let decoded = decode_object(&bytes).expect("decode");
    assert_eq!(decoded.kind, KIND);
    let again = encode_object(&decoded);
    assert_eq!(bytes, again);
}

#[test]
fn unknown_kind_records_and_nothing_else() {
    let labels = [
        "v2.1-pcoc-object",
        "satoshi-hike-v2.0-object",
        "",
        "v2.2-pcoc-objecT",
    ];
    let expect = [
        r#"{"kind":"unknown kind","kind_text":"v2.1-pcoc-object"}"#,
        r#"{"kind":"unknown kind","kind_text":"satoshi-hike-v2.0-object"}"#,
        r#"{"kind":"unknown kind","kind_text":""}"#,
        r#"{"kind":"unknown kind","kind_text":"v2.2-pcoc-objecT"}"#,
    ];
    let profile = s1_profile();
    for (label, want) in labels.iter().zip(expect) {
        assert_eq!(unknown_kind_record(label), want);
        let bytes = cbor::encode_map(&[("kind".into(), cbor::encode_text(label))]);
        match verify(&bytes, &profile, &mut EmptyIndex, None) {
            Verified::UnknownKind(got) => assert_eq!(got, want),
            other => panic!("expected A5, got {other:?}"),
        }
    }
    match decode_object(&[0x00]) {
        Err(DecodeError::Unreadable) => {}
        other => panic!("expected unreadable, got {other:?}"),
    }
    match verify(&[0x00], &profile, &mut EmptyIndex, None) {
        Verified::Unreadable => {}
        other => panic!("expected unreadable, got {other:?}"),
    }
}

#[test]
fn honest_six_leg_matches_and_tamper_mismatches() {
    let (profile, bytes) = honest_s1();
    let mut raw = Vec::new();
    for i in 0..6u64 {
        raw.extend_from_slice(&i.to_le_bytes());
    }
    let mut pos = 0;
    let mut next = || {
        if pos + 8 > raw.len() {
            return None;
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&raw[pos..pos + 8]);
        pos += 8;
        Some(b)
    };
    let rec = match verify(&bytes, &profile, &mut EmptyIndex, Some(&mut next)) {
        Verified::Record(s) => s,
        other => panic!("expected record, got {other:?}"),
    };
    assert!(rec.contains(r#""shape":"match""#), "{rec}");
    assert!(
        rec.contains(r#""results":["match","match","match","match","match","match"]"#),
        "{rec}"
    );
    let mut obj = decode_object(&bytes).unwrap();
    obj.stages[0].leg_results[0][0] ^= 0x01;
    let bad = encode_object(&obj);
    let mut pos = 0;
    let mut next = || {
        if pos + 8 > raw.len() {
            return None;
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&raw[pos..pos + 8]);
        pos += 8;
        Some(b)
    };
    let rec = match verify(&bad, &profile, &mut EmptyIndex, Some(&mut next)) {
        Verified::Record(s) => s,
        other => panic!("{other:?}"),
    };
    assert!(rec.contains(r#""results":["mismatch","match","match","match","match","match"]"#), "{rec}");
}

#[test]
fn trailing_kind_is_malformed_cbor_only() {
    let (profile, mut bytes) = honest_s1();
    bytes.push(0x00);
    let mut raw = Vec::new();
    for i in 0..6u64 {
        raw.extend_from_slice(&i.to_le_bytes());
    }
    let mut pos = 0;
    let mut next = || {
        if pos + 8 > raw.len() {
            return None;
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&raw[pos..pos + 8]);
        pos += 8;
        Some(b)
    };
    let rec = match verify(&bytes, &profile, &mut EmptyIndex, Some(&mut next)) {
        Verified::Record(s) => s,
        other => panic!("{other:?}"),
    };
    assert!(rec.contains(r#""shape_item":["malformed cbor"]"#), "{rec}");
    assert!(!rec.contains("field set"), "{rec}");
}

#[test]
fn step_folds_every_word() {
    let profile = Profile::conformance_small(1, 16384, 16384, 1).unwrap();
    assert_eq!(profile.words_per_slot, 2048);
    let terrain = vec![0u8; profile.terrain_bytes];
    let (acc0, _) = step(&terrain, &profile, 0);
    for w in 0..profile.words_per_slot {
        let mut flipped = terrain.clone();
        flipped[w * 8] ^= 0x01;
        let (acc, _) = step(&flipped, &profile, 0);
        assert_ne!(acc, acc0, "word {w} did not move the accumulator");
    }
}

#[test]
fn hashcash_difficulty_8_rejects_smaller_nonces() {
    let challenge = sha256(b"pcoc-pack-2.2/hashcash/fast-challenge");
    let waypoint = sha256(b"pcoc-pack-2.2/hashcash/fast-waypoint");
    let (nonce, digest) = hashcash_solve(&challenge, &waypoint, 8).unwrap();
    assert!(leading_zero_bits(&digest) >= 8);
    for n in 0..nonce {
        assert!(leading_zero_bits(&hashcash_digest(&challenge, &waypoint, n)) < 8);
    }
}

#[test]
fn draw_rejects_over_limit_and_duplicates() {
    let mut pulls = vec![u64::MAX.to_le_bytes(), 0u64.to_le_bytes()];
    pulls.reverse();
    let mut next = || pulls.pop();
    assert_eq!(draw_one(3, &mut next).unwrap(), 0);
    assert!(pulls.is_empty());

    let mut seq = Vec::new();
    for n in (1..6u64).rev() {
        seq.push(n.to_le_bytes());
    }
    for _ in 0..3 {
        seq.push(0u64.to_le_bytes());
    }
    let mut next = || seq.pop();
    let got = draw_six(8, &mut next).unwrap();
    assert_eq!(got, vec![0, 1, 2, 3, 4, 5]);

    let mut none = || None;
    assert_eq!(draw_six(8, &mut none), Err(DrawStop::NotRead));
    let mut unused = || Some([0u8; 8]);
    assert_eq!(draw_six(5, &mut unused), Err(DrawStop::NotDefined));
}

#[test]
fn cli_exits() {
    let bin = verify_bin();
    let dir = std::env::temp_dir().join("pcoc-sdk-fast");
    let _ = std::fs::create_dir_all(&dir);
    let (_profile, bytes) = honest_s1();
    let path = dir.join("s1.cbor");
    std::fs::write(&path, &bytes).unwrap();

    let missing = Command::new(&bin)
        .args(["--profile", "conformance-small", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));

    let unknown = Command::new(&bin).arg("--nope").output().unwrap();
    assert_eq!(unknown.status.code(), Some(2));

    let bad = dir.join("nope.cbor");
    std::fs::write(&bad, [0x00u8]).unwrap();
    let unread = Command::new(&bin)
        .args([
            "--profile",
            "conformance-small",
            "--sample-bytes",
            "32",
            "--slot-bytes",
            "64",
            "--terrain-bytes",
            "65536",
            "--steps-per-leg",
            "100",
            bad.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(unread.status.code(), Some(1));

    let ok = Command::new(&bin)
        .args([
            "--profile",
            "conformance-small",
            "--sample-bytes",
            "32",
            "--slot-bytes",
            "64",
            "--terrain-bytes",
            "65536",
            "--steps-per-leg",
            "100",
            "--draw-hex",
            &draw_hex_0_to_5(),
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(ok.status.code(), Some(0), "{}", String::from_utf8_lossy(&ok.stderr));
    let stdout = String::from_utf8(ok.stdout).unwrap();
    assert!(stdout.ends_with('\n'));
    assert_eq!(stdout.chars().filter(|c| *c == '\n').count(), 1);
    assert!(!stdout.contains("pass"));
    assert!(!stdout.contains("fail"));
    assert!(stdout.contains(r#""results":["match","match","match","match","match","match"]"#));

    let kind = cbor::encode_map(&[("kind".into(), cbor::encode_text("v2.1-pcoc-object"))]);
    let kpath = dir.join("old.cbor");
    let mut f = std::fs::File::create(&kpath).unwrap();
    f.write_all(&kind).unwrap();
    let a5 = Command::new(&bin)
        .args([
            "--profile",
            "conformance-small",
            "--sample-bytes",
            "32",
            "--slot-bytes",
            "64",
            "--terrain-bytes",
            "65536",
            "--steps-per-leg",
            "100",
            kpath.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(a5.status.code(), Some(0));
    let text = String::from_utf8(a5.stdout).unwrap();
    assert_eq!(
        text,
        "{\"kind\":\"unknown kind\",\"kind_text\":\"v2.1-pcoc-object\"}\n"
    );
}
