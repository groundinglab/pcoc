//! AC-A5. Regenerated candidates match the committed files and the pinned
//! SHA-256 values. Produced objects match the pinned objects. Each object verifies.

#[path = "../../support/repo.rs"]
mod repo;
#[path = "../../support/round2.rs"]
mod round2;

use pcoc::hash::sha256;
use pcoc::hashcash::header_hash;
use pcoc::hexutil::hex_encode;
use pcoc_produce::{encode_produced, produce_on_rig, Case, ProduceOutcome};
use repo::repo_root;
use round2::{
    assert_committed_pins, assert_objects_verify, assert_regenerated_candidates, OBJECTS,
};
use std::path::Path;
use std::sync::atomic::AtomicBool;

const PK: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
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
    let root = repo_root();
    assert_committed_pins(&root);
    let committed = root.join("sdk/rust/candidates");

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

    assert_regenerated_candidates(&root, |dir| {
        pcoc_gen::write_candidates(dir).unwrap_or_else(|e| panic!("regenerate candidates: {e}"));
    });

    let objects = root.join("sdk/rust/objects");
    for (case, name, pin) in [
        (Case::S1, OBJECTS[0].0, OBJECTS[0].1),
        (Case::S2, OBJECTS[1].0, OBJECTS[1].1),
        (Case::Prod, OBJECTS[2].0, OBJECTS[2].1),
    ] {
        let bytes = produce(case);
        assert_eq!(hex_encode(&sha256(&bytes)), pin, "object pin {name}");
        assert_eq!(bytes, read(&objects.join(name)), "{name} committed bytes");
    }
    assert_objects_verify(&root);
}
