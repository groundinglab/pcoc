//! AC-C4 re-run, and AC-C6 negatives against the committed candidate bytes.

use pcoc_sdk::hash::sha256;
use pcoc_sdk::hexutil::{hex_decode, hex_decode_exact};
use pcoc_sdk::hike::produce_hike;
use pcoc_sdk::object::{decode_object, encode_object};
use pcoc_sdk::profile::Profile;
use pcoc_sdk::verify::{verify, EmptyIndex, Verified};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

const FILES: &[&str] = &[
    "seed.txt",
    "nl4.txt",
    "terrain.txt",
    "fold.txt",
    "kat-small.txt",
    "test-3leg.txt",
    "prod.txt",
    "wes.txt",
    "rate.txt",
    "anchor.txt",
    "draw.txt",
    "unknown-kind.txt",
    "hashcash.txt",
    "negatives.txt",
];

fn committed() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("candidates")
}

fn load(path: &std::path::Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(text.is_ascii(), "{}", path.display());
    assert!(!text.contains('\r'), "{}", path.display());
    assert!(text.ends_with('\n') && !text.ends_with("\n\n"), "{}", path.display());
    let mut map = BTreeMap::new();
    let mut prev = String::new();
    for line in text.lines() {
        assert!(!line.is_empty(), "blank {}", path.display());
        assert!(!line.ends_with(' ') && !line.ends_with('\t'), "{}", path.display());
        let (k, v) = line.split_once('\t').unwrap_or_else(|| panic!("tab {}", path.display()));
        assert!(!v.contains('\t'), "{k}");
        assert!(prev.as_bytes() < k.as_bytes(), "order {prev} then {k}");
        prev = k.to_string();
        assert!(map.insert(k.to_string(), v.to_string()).is_none(), "dup {k}");
    }
    map
}

fn pk_a() -> [u8; 32] {
    hex_decode_exact("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9").unwrap()
}

fn honest_bytes() -> (Profile, Vec<u8>) {
    let profile = Profile::conformance_small(32, 64, 65536, 100).unwrap();
    let beacon = sha256(b"pcoc-pack-2.2/beacon/seed");
    let mut interrupts = [([0u8; 32], 0u64); 18];
    for k in 0..18 {
        let name = format!("pcoc-pack-2.2/beacon/{k}");
        interrupts[k] = (sha256(name.as_bytes()), 1001 + k as u64);
    }
    let (terrain, _, _) = pcoc_sdk::terrain::terrain_from_seed(&pk_a(), &beacon, &profile);
    let (obj, _, _) = produce_hike(&profile, &pk_a(), &beacon, 1000, &interrupts, &[1u32; 18], &terrain);
    (profile, encode_object(&obj))
}

fn six(profile: &Profile, bytes: &[u8]) -> String {
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
    match verify(bytes, profile, &mut EmptyIndex, Some(&mut next)) {
        Verified::Record(s) => s,
        other => panic!("{other:?}"),
    }
}

#[test]
fn generator_matches_committed_files() {
    if cfg!(debug_assertions) {
        panic!("run cargo test --release --test candidates");
    }
    let dir = std::env::temp_dir().join(format!("pcoc-candidates-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bin = std::env::var_os("CARGO_BIN_EXE_gen_candidates")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(profile)
                .join("gen-candidates")
        });
    let out = Command::new(bin)
        .arg("--out")
        .arg(&dir)
        .output()
        .expect("gen-candidates");
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprint!("{}", String::from_utf8_lossy(&out.stderr));
    for name in FILES {
        let got = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let want = std::fs::read(committed().join(name)).unwrap_or_else(|e| panic!("committed {name}: {e}"));
        assert_eq!(got, want, "{name} changed");
        let _ = load(&committed().join(name));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn negatives_mismatch_under_six_leg() {
    if cfg!(debug_assertions) {
        panic!("run cargo test --release --test candidates");
    }
    let neg = load(&committed().join("negatives.txt"));
    let terrain = load(&committed().join("terrain.txt"));
    let prod = load(&committed().join("prod.txt"));
    let honest_digest = terrain.get("terrain.S1.pk_A.digest").unwrap();
    let honest_prod = prod.get("prod.PROD.pk_A.stage.0.leg.0.legs_le_hex").unwrap();
    let (profile, honest) = honest_bytes();
    let rec = six(&profile, &honest);
    assert!(
        rec.contains(r#""results":["match","match","match","match","match","match"]"#),
        "{rec}"
    );

    let leg_names = [
        "slot_summary_fold",
        "xor_accumulator_fold",
        "xor_w_fold",
        "s_plus_w_fold",
        "prefold_hoist",
        "partial_slot_read",
        "cached_leg",
        "partial_slot_read_2047",
    ];
    for name in leg_names {
        let key = if name == "partial_slot_read_2047" {
            "negatives.PROD.pk_A.partial_slot_read_2047.legs_le_hex".to_string()
        } else {
            format!("negatives.S1.pk_A.{name}.legs_le_hex")
        };
        let flag = if name == "partial_slot_read_2047" {
            "negatives.PROD.pk_A.partial_slot_read_2047.matches_honest".to_string()
        } else {
            format!("negatives.S1.pk_A.{name}.matches_honest")
        };
        assert_eq!(neg.get(&flag).map(String::as_str), Some("false"), "{name}");
        let le = hex_decode(neg.get(&key).unwrap_or_else(|| panic!("missing {key}"))).unwrap();
        assert_eq!(le.len(), 8, "{name}");
        let mut obj = decode_object(&honest).unwrap();
        for st in &mut obj.stages {
            for lr in &mut st.leg_results {
                *lr = le.clone();
            }
        }
        let rec = six(&profile, &encode_object(&obj));
        assert!(
            rec.contains(r#""results":["mismatch","mismatch","mismatch","mismatch","mismatch","mismatch"]"#),
            "{name}: {rec}"
        );
    }
    assert_ne!(
        neg.get("negatives.PROD.pk_A.partial_slot_read_2047.legs_le_hex").map(String::as_str),
        Some(honest_prod.as_str())
    );

    for name in ["one_lane_flip", "all4_kernel", "f3_shared_terrain", "squeezed_seed_terrain"] {
        let dig = neg.get(&format!("negatives.S1.pk_A.{name}.digest")).unwrap();
        let flag = neg.get(&format!("negatives.S1.pk_A.{name}.matches_honest")).unwrap();
        assert_eq!(flag, "false");
        assert_ne!(dig, honest_digest, "{name}");
    }
}
