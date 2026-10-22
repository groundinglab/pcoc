//! AC-A5. Candidates match the round-2 files. The scripted rig writes O1–O3.

use pcoc::hash::sha256;
use pcoc::hashcash::header_hash;
use pcoc::hexutil::hex_encode;
use pcoc_produce::{encode_produced, produce_on_rig, Case, ProduceOutcome};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

const FILES: [&str; 14] = [
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

const PK: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn find_up(start: &Path, rel: &str) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(rel);
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn produce(case: Case) -> Vec<u8> {
    let key = pcoc::hexutil::hex_decode_exact::<32>(PK).unwrap();
    let profile = pcoc_produce::cases_profile(case);
    let rig = pcoc_produce::cases_rig(case);
    let cancel = AtomicBool::new(false);
    match produce_on_rig(&key, &profile, &rig, &cancel, |_| {}) {
        ProduceOutcome::Object(obj) => encode_produced(&obj),
        ProduceOutcome::Cancelled => panic!("cancelled"),
    }
}

#[test]
fn candidates_and_objects() {
    let gen_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let committed = gen_dir.join("../candidates");
    let scratch = std::env::temp_dir().join(format!("pcoc-ac-a5-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    pcoc_gen::write_candidates(&scratch).unwrap();
    let again = scratch.join("again");
    pcoc_gen::write_candidates(&again).unwrap();
    for name in FILES {
        let got = read(&scratch.join(name));
        assert_eq!(got, read(&again.join(name)), "rerun {name}");
        assert_eq!(got, read(&committed.join(name)), "committed {name}");
    }
    if let Some(reference) = find_up(&gen_dir, "ref-round2/C-grok/candidates") {
        for name in FILES {
            assert_eq!(
                read(&scratch.join(name)),
                read(&reference.join(name)),
                "reference {name}"
            );
        }
    }
    let before = FILES.map(|n| read(&committed.join(n)));
    pcoc_gen::write_candidates(&committed).unwrap();
    for (name, prior) in FILES.iter().zip(before) {
        assert_eq!(read(&committed.join(name)), prior, "in place {name}");
    }
    let prod = String::from_utf8(read(&committed.join("prod.txt"))).unwrap();
    assert!(prod.contains("prod.PROD.stage.0.leg.0.legs_le_hex\t88a04ac5beacb8a7\n"));

    let mut fee = Vec::new();
    fee.extend_from_slice(&sha256(b"pcoc-pack-2.2/hashcash/fee_header/0"));
    fee.extend_from_slice(&sha256(b"pcoc-pack-2.2/hashcash/fee_header/1"));
    fee.extend_from_slice(&sha256(b"pcoc-pack-2.2/hashcash/fee_header/2")[..16]);
    let once = hex_encode(&sha256(&fee));
    let twice = hex_encode(&header_hash(&fee));
    assert_ne!(once, twice);
    let hashcash = String::from_utf8(read(&committed.join("hashcash.txt"))).unwrap();
    assert!(hashcash.contains(&format!("hashcash.header_hash\t{twice}\n")));
    assert!(!hashcash.contains(&format!("hashcash.header_hash\t{once}\n")));

    let objects = gen_dir.join("../objects");
    let pins = [
        (Case::S1, "O1-S1.cbor", "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246"),
        (Case::S2, "O2-S2.cbor", "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0"),
        (Case::Prod, "O3-PROD.cbor", "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7"),
    ];
    for (case, name, pin) in pins {
        let bytes = produce(case);
        assert_eq!(hex_encode(&sha256(&bytes)), pin, "{name}");
        assert_eq!(bytes, read(&objects.join(name)), "{name} committed bytes");
    }
    let _ = std::fs::remove_dir_all(&scratch);
}
