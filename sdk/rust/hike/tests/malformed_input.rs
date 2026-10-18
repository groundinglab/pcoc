//! Malformed untrusted bytes must give a typed error, never a panic.
//!
//! Every public entry point that reads bytes from outside (a submitted object,
//! a path body from the plant or index, an index reply) is fed truncations,
//! single-byte flips, splices and random bytes, derived from the conformance
//! pack objects. Each call runs under `catch_unwind`; any panic fails the test.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

fn pack() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../spec/conformance/v2.0")
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Deterministic splitmix64, so a failure names a reproducible case number.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn seeds() -> Vec<Vec<u8>> {
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(pack().join("test-3leg.json")).unwrap())
            .unwrap();
    let mut out = vec![hex(doc["object_hex"].as_str().unwrap())];
    out.push(std::fs::read(pack().join("earlier-kind.cbor")).unwrap());
    out.push(hike::encode_hike(&hike::structural_object()));
    out
}

fn mutants(seed: &[u8], rng: &mut Rng, count: usize) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(count + seed.len().min(512));
    // Every truncation of the first 512 bytes, plus the empty input.
    for cut in 0..seed.len().min(512) {
        out.push(seed[..cut].to_vec());
    }
    for _ in 0..count {
        let mut m = seed.to_vec();
        match rng.below(5) {
            0 => {
                let i = rng.below(m.len());
                m[i] ^= 1 << rng.below(8);
            }
            1 => {
                let i = rng.below(m.len());
                m[i] = [
                    0x00, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1f, 0x5b, 0x9b, 0xbb, 0xff,
                ][rng.below(11)];
            }
            2 => {
                let i = rng.below(m.len());
                m.truncate(i);
            }
            3 => {
                let i = rng.below(m.len());
                let n = 1 + rng.below(16);
                let extra: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
                m.splice(i..i, extra);
            }
            _ => {
                m = (0..rng.below(256)).map(|_| rng.next() as u8).collect();
            }
        }
        out.push(m);
    }
    out
}

fn no_panic(what: &str, case: usize, f: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(f)).is_err() {
        panic!("{what}: case {case} panicked on malformed input");
    }
}

#[test]
fn object_decoders_never_panic_on_malformed_objects() {
    let mut rng = Rng(0x5eed_0001);
    let mut cases = 0usize;
    for seed in seeds() {
        for (i, m) in mutants(&seed, &mut rng, 1500).into_iter().enumerate() {
            no_panic("decoded_hike", i, || {
                let _ = hike::decoded_hike(&m);
            });
            no_panic("object_public_key", i, || {
                let _ = hike::object_public_key(&m);
            });
            no_panic("canonical_object", i, || {
                let _ = hike::canonical_object(&m);
            });
            no_panic("earlier_object_kind", i, || {
                let _ = hike::earlier_object_kind(&m);
            });
            no_panic("yielded_public_key", i, || {
                let _ = hike::yielded_public_key(&m);
            });
            no_panic("screen_fast_sized", i, || {
                let _ = hike::screen_fast_sized(
                    &m,
                    |_| hike::ChainRead::NotRead,
                    hike::Sizes::conformance_small(),
                );
            });
            if i % 4 == 0 {
                no_panic("verify_fast_sized", i, || {
                    let _ = hike::verify_fast_sized(
                        &m,
                        |_| Err(()),
                        |_| hike::ChainRead::NotRead,
                        hike::Sizes::conformance_small(),
                    );
                });
            }
            cases += 1;
        }
    }
    assert!(cases > 4000, "{cases}");
}

#[test]
fn truncated_objects_are_typed_errors() {
    for seed in seeds().into_iter().take(1) {
        for cut in [0, 1, 2, 9, seed.len() / 2, seed.len() - 1] {
            assert!(hike::decoded_hike(&seed[..cut]).is_err(), "cut {cut}");
            assert!(hike::object_public_key(&seed[..cut]).is_err(), "cut {cut}");
        }
    }
}

/// CBOR heads whose length argument is cut short (additional info 25, 26, 27).
#[test]
fn short_cbor_length_arguments_are_typed_errors() {
    for head in [
        0x19u8, 0x1a, 0x1b, 0x59, 0x5a, 0x5b, 0x79, 0x7a, 0x7b, 0xb9, 0xba, 0xbb,
    ] {
        for extra in 0..8usize {
            let mut bytes = vec![head];
            bytes.extend(std::iter::repeat(0xffu8).take(extra));
            no_panic("canonical_object short head", extra, || {
                let _ = hike::canonical_object(&bytes);
            });
            assert!(hike::decoded_hike(&bytes).is_err());
        }
    }
}

#[test]
fn path_bodies_never_panic_and_bad_lengths_are_typed_errors() {
    let waypoint = [0x2au8; 32];
    let good = hike::encode_path_in_block(&waypoint, 840_000);
    assert!(hike::path_locators(&good).is_ok());
    let mut rng = Rng(0x5eed_0002);
    for (i, m) in mutants(&good, &mut rng, 4000).into_iter().enumerate() {
        no_panic("path_locators", i, || {
            let _ = hike::path_locators(&m);
        });
    }
    for cut in 0..good.len() {
        assert!(
            matches!(
                hike::path_locators(&good[..cut]),
                Err(hike::PathError::Truncated | hike::PathError::WrongLength { .. })
            ),
            "cut {cut}"
        );
    }
    // A sibling count that promises more bytes than are present.
    let mut lying = good.clone();
    lying[41] = 3;
    assert!(matches!(
        hike::path_locators(&lying),
        Err(hike::PathError::WrongLength { .. })
    ));
}

#[test]
fn index_replies_never_panic() {
    let kat: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(pack().join("kat-small.json")).unwrap())
            .unwrap();
    let good = kat["documents"][0]["body"].as_str().unwrap().to_owned();
    assert!(
        hike::parse_index_beacon(&good).is_ok(),
        "seed document must parse"
    );
    let mut rng = Rng(0x5eed_0003);
    for (i, m) in mutants(good.as_bytes(), &mut rng, 4000)
        .into_iter()
        .enumerate()
    {
        let text = String::from_utf8_lossy(&m).into_owned();
        no_panic("parse_index_beacon", i, || {
            let _ = hike::parse_index_beacon(&text);
        });
        no_panic("parse_index_document", i, || {
            let _ = hike::parse_index_document(&text);
        });
    }
    for bad in [
        "",
        "{",
        "[]",
        "null",
        r#"{"height":-1}"#,
        r#"{"height":1e999}"#,
    ] {
        assert!(hike::parse_index_beacon(bad).is_err(), "{bad}");
    }
}
