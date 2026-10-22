//! SPEC §6 constants. `WORDS_PER_SLOT`, `N_SLOTS`, and `OFFSET_MODULUS` are formulas.

/// SPEC §6. `SLOT_BYTES` is 16384.
pub const SLOT_BYTES: usize = 16384;
/// SPEC §6. `TERRAIN_BYTES` is 268435456.
pub const TERRAIN_BYTES: usize = 268_435_456;
/// SPEC §6. `SAMPLE_BYTES` is 1024.
pub const SAMPLE_BYTES: usize = 1024;
/// SPEC §6. `STEPS_PER_LEG` is 2000000.
pub const STEPS_PER_LEG: u64 = 2_000_000;
/// SPEC §6. `STAGES` is 18.
pub const STAGES: usize = 18;
/// SPEC §6. `HASH_BYTES` is 32.
pub const HASH_BYTES: usize = 32;
/// SPEC §6. `LEG_RESULT_BYTES` is 8.
pub const LEG_RESULT_BYTES: usize = 8;
/// SPEC §6. `PUBLIC_KEY_BYTES` is 32.
pub const PUBLIC_KEY_BYTES: usize = 32;
/// SPEC §6. `BEACON_BYTES` is 32.
pub const BEACON_BYTES: usize = 32;

/// SPEC §6. `SEAL_DOMAIN` is the twelve ASCII bytes `pcoc-walk-v1`.
pub const SEAL_DOMAIN: &[u8; 12] = b"pcoc-walk-v1";
/// SPEC §6. `KIND` is `v2.2-pcoc-object`.
pub const KIND: &str = "v2.2-pcoc-object";
/// SPEC §6. `FILL` is `dag2`.
pub const FILL: &str = "dag2";

/// SPEC §6. `MIX_ADD`.
pub const MIX_ADD: u64 = 0x9E3779B97F4A7C15;
/// SPEC §6. `MIX_MUL1`.
pub const MIX_MUL1: u64 = 0xBF58476D1CE4E5B9;
/// SPEC §6. `MIX_MUL2`.
pub const MIX_MUL2: u64 = 0x94D049BB133111EB;
/// SPEC §6. `WORD_XOR`.
pub const WORD_XOR: u64 = 0xC0FFEE;
/// SPEC §6. `PARENT_MUL`.
pub const PARENT_MUL: u64 = 0xD1B54A32D192ED03;
/// SPEC §6. `SLOT_MUL`.
pub const SLOT_MUL: u64 = 0xA24BAED4963EE407;

/// SPEC §13.1. `ANCHOR_WINDOW_OFFSETS` is 2, 3, 4.
pub const ANCHOR_WINDOW_OFFSETS: [u64; 3] = [2, 3, 4];
/// SPEC §13.1. Hashcash difficulty is 21 leading zero bits.
pub const HASHCASH_DIFFICULTY: u32 = 21;
/// SPEC §14.6. The six-leg draw count is 6. It is not a section 6 constant.
pub const SIX_LEG_COUNT: usize = 6;
/// SPEC §9. A height above this bound is not a usable JSON integer.
pub const HEIGHT_MAX: u64 = (1_u64 << 53) - 1;

/// SPEC §6. `WORDS_PER_SLOT = SLOT_BYTES / 8` at the section 6 sizes.
pub const WORDS_PER_SLOT: usize = SLOT_BYTES / 8;
/// SPEC §6. `N_SLOTS = TERRAIN_BYTES / SLOT_BYTES` at the section 6 sizes.
pub const N_SLOTS: usize = TERRAIN_BYTES / SLOT_BYTES;
/// SPEC §6. `OFFSET_MODULUS = TERRAIN_BYTES − SAMPLE_BYTES + 1` at the section 6 sizes.
pub const OFFSET_MODULUS: u64 = (TERRAIN_BYTES - SAMPLE_BYTES + 1) as u64;

/// SPEC §13.1. The Hashcash challenge prefix is the ten ASCII bytes `bari-hc-v2`.
pub const HASHCASH_PREFIX: &[u8; 10] = b"bari-hc-v2";
