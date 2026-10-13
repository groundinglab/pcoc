use pcoc_bari::hashcash::{self, HASHCASH_BITS, HASHCASH_TAG};
use pcoc_bari::merkle::MerkleTree;
use pcoc_bari::proof::{commitment_script, CanonicalPathBody};

#[test]
fn public_merkle_and_wire_surfaces_match_the_frozen_literals() {
    let leaves = vec![[0x33; 32], [0x11; 32], [0x22; 32], [0x11; 32]];
    let tree = MerkleTree::build(leaves).unwrap();
    assert_eq!(
        hex::encode(tree.root()),
        "9c9a3f601444d1adc90e749e6a7972ea850c3029d2e8504ddfdaf111e4ecac90"
    );
    for leaf in tree.leaves() {
        tree.path(leaf).unwrap().verify(&tree.root()).unwrap();
    }

    let leaf = [0x11; 32];
    let one = MerkleTree::build(vec![leaf]).unwrap();
    let body = CanonicalPathBody {
        path: one.path(&leaf).unwrap(),
        root: one.root(),
        txid: [0x44; 32],
        vout: 2,
        block_hash: [0x55; 32],
        block_height: 123_456,
    };
    let bytes = body.encode().unwrap();
    assert_eq!(bytes.len(), 146);
    assert_eq!(CanonicalPathBody::decode(&bytes).unwrap(), body);
}

#[test]
fn public_commitment_script_has_no_alternate_framing() {
    let script = commitment_script(&[0xAB; 32]);
    assert_eq!(script.len(), 39);
    assert_eq!(&script[..7], b"\x6a\x25BARI\x02");
    assert_eq!(&script[7..], &[0xAB; 32]);
}

#[test]
fn public_hashcash_difficulty_is_21_bits_and_bound_to_the_live_tip() {
    assert_eq!(HASHCASH_BITS, 21);
    assert_eq!(HASHCASH_TAG, b"bari-hc-v1");
    let live = hashcash::challenge_from_tip(&[0x11; 32]);
    let other = hashcash::challenge_from_tip(&[0x12; 32]);
    assert_ne!(live, other);
    let waypoint = [0x22; 32];
    let nonce = hashcash::solve(&live, &waypoint, 8);
    assert!(hashcash::verify(&live, &waypoint, nonce, 8));
    assert!(hashcash::leading_zero_bits(&hashcash::hashcash_digest(&live, &waypoint, nonce)) >= 8);
    assert_ne!(
        hashcash::hashcash_digest(&live, &waypoint, nonce),
        hashcash::hashcash_digest(&other, &waypoint, nonce)
    );
}

/// Pinned vectors: every file under tests/vectors/ matches tests/vectors/SHA256SUMS.
#[test]
fn pinned_vectors_match_their_hashes() {
    use sha2::{Digest, Sha256};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    let sums = std::fs::read_to_string(dir.join("SHA256SUMS")).unwrap();
    let mut listed = Vec::new();
    for line in sums.lines() {
        let (want, name) = line.split_once("  ").expect("sha256sum line");
        let got = hex::encode(Sha256::digest(std::fs::read(dir.join(name)).unwrap()));
        assert_eq!(got, want, "{name}");
        listed.push(name.to_owned());
    }
    let mut present: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n != "SHA256SUMS")
        .collect();
    present.sort();
    listed.sort();
    assert_eq!(present, listed, "every vector is listed");
}
