//! Satoshi Hike 2.3 construction and verifier.
//! Behaviour is `spec/satoshi-hike-v2.3/SPEC.md`. Errata 1 and 2 are folded into that file.
//! Each public function below names the section it implements (Rule I2).
#![deny(unsafe_code)]

pub mod br1;
pub mod cbor;
pub mod checkpoint;
pub mod compare;
pub mod constants;
pub mod draw;
pub mod fast_record;
pub mod hash;
pub mod hashcash;
pub mod hexutil;
pub mod hike;
pub mod indexdoc;
pub mod jsonutil;
pub mod mix;
pub mod object;
pub mod path;
pub mod pin;
pub mod profile;
pub mod signin;
pub mod step;
pub mod terrain;
pub mod verify;
pub mod waypoint;

/// SPEC Rule I5. SDK version. Equals `sdk/VERSION`.
pub const VERSION: &str = "2.3.1";

const VERSION_FILE: &str = include_str!("../../../VERSION");
const SPEC_PIN_FILE: &str = include_str!("../../../SPEC_PIN");

/// SPEC §6 / Rule I5. Reports the SDK version.
pub fn version() -> &'static str {
    VERSION
}

/// SPEC Rule I5. Version, spec tag, pack pin, and the SHA-256 backend in use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: &'static str,
    pub spec_tag: String,
    pub pack_pin: String,
    pub sha_backend: &'static str,
}

/// SPEC Rule I5 and §11.1. `sha_backend` is the backend actually selected at runtime.
pub fn build_info() -> BuildInfo {
    let pin = pin::read_pin(SPEC_PIN_FILE).expect("sdk/SPEC_PIN");
    debug_assert_eq!(VERSION_FILE.trim(), VERSION);
    BuildInfo {
        version: VERSION,
        spec_tag: pin.tag,
        pack_pin: pin.vector_sha256,
        sha_backend: hash::sha_backend(),
    }
}

/// SPEC §10. `D = SHA-256(public_key ‖ seed_beacon)`.
pub use terrain::seed;

/// SPEC §10. The 64-byte preimage `public_key ‖ seed_beacon`.
pub use terrain::seed_preimage;

/// SPEC §10.2. Four little-endian lanes of `D`.
pub use terrain::lanes;

/// SPEC §10.4. Fill `terrain` once from `lane`. Every slot, every word.
pub use terrain::fill_terrain;

/// SPEC §10.4. Allocate and fill one terrain from a public key and a seed beacon.
pub use terrain::terrain_from_seed;

/// SPEC §11.1. One step. Loads the whole slot and folds every word with `S`.
pub use step::step;

/// SPEC §11.2. `leg_result` after `STEPS_PER_LEG` steps.
pub use step::leg_result;

/// SPEC §11.4. Checkpoint digest.
pub use checkpoint::checkpoint;

/// SPEC §12.2. Waypoint `W_{g+1}`.
pub use waypoint::waypoint;

/// SPEC §12.2. Fold of a stage's checkpoints.
pub use waypoint::fold_checkpoints;

/// SPEC §13.1. Double SHA-256 of the 80-byte fee header, natural order.
pub use hashcash::header_hash;

/// SPEC §13.1. Hashcash challenge.
pub use hashcash::hashcash_challenge;

/// SPEC §13.1. Hashcash digest.
pub use hashcash::hashcash_digest;

/// SPEC §13.1. First nonce from 0 whose digest meets `difficulty`.
pub use hashcash::hashcash_solve;

/// SPEC §13.1. Leading zero bits, high bit of the first byte first.
pub use hashcash::leading_zero_bits;

/// SPEC §13.6 Rule BR1. `204` when the waypoint post is paid, `402` otherwise.
pub use br1::br1_status;

/// SPEC §13.6. Current, before, after, or outside, before the Hashcash check.
pub use br1::br1_window;

/// SPEC §9 and §13.6. Compact `nBits` an "after" fee header must carry.
pub use br1::required_bits;

/// SPEC §7.3 / §8. Deterministic object bytes.
pub use object::encode_object;

/// SPEC §7.3 / §8. Decode one object (erratum E14 order).
pub use object::decode_object;

/// SPEC §14. Verify record, including Rule A5. The six-leg draw reads the OS CSPRNG.
pub use verify::verify;

/// SPEC §14.7. Decode, kind, and shape. No terrain fill. Any other kind is Rule A5.
pub use verify::precheck;

/// SPEC §14.6. Six distinct leg indices from the OS CSPRNG. No byte source, label, or seed.
pub use draw::six_leg_draw;

/// SPEC §15.5. `D = 6 · thorough_rounds` indices from the OS CSPRNG. No byte source, label, or seed.
pub use draw::thorough_draw;

/// SPEC Rule B2. Integer rate comparison. `meets` or `refuse`.
pub use compare::leg_rate_floor;

/// SPEC §14 fast-phase hike pair. No terrain fill and no leg.
pub use fast_record::{hike_legs, hike_span, mtp_at};

/// SPEC §16.1 Anchor comparison. Integer window only.
pub use compare::anchor_window;

/// SPEC §15.1. Challenge bytes for the sign-in wire.
pub use signin::challenge_bytes;

/// SPEC §15.1. `SHA-256(challenge_bytes)`, the BIP-340 message. Not hashed again.
pub use signin::bip340_message;

/// SPEC §15.1. Exact origin comparison.
pub use signin::origin_served;

/// SPEC §15.1. Challenge-window comparison, wide integers.
pub use signin::window_accepts;

/// SPEC §11.1. SHA-256 backend actually in use: `sha-ni` or `software`.
pub use hash::sha_backend;
