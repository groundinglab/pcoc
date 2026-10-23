//! SPEC §6 constants and the §16.1 conformance-small profile formulas.

pub const SLOT_BYTES: u64 = 16384;
pub const WORDS_PER_SLOT: u64 = 2048;
pub const TERRAIN_BYTES: u64 = 268435456;
pub const N_SLOTS: u64 = 16384;
pub const SAMPLE_BYTES: u64 = 1024;
pub const OFFSET_MODULUS: u64 = 268434433;
pub const STEPS_PER_LEG: u64 = 2000000;
pub const STAGES: u64 = 18;
pub const HASH_BYTES: u64 = 32;
pub const LEG_RESULT_BYTES: u64 = 8;
pub const PUBLIC_KEY_BYTES: u64 = 32;
pub const BEACON_BYTES: u64 = 32;
pub const SEAL_DOMAIN = "pcoc-walk-v1";
pub const KIND = "v2.2-pcoc-object";
pub const FILL = "dag2";

pub const MIX_ADD: u64 = 0x9E3779B97F4A7C15;
pub const MIX_MUL1: u64 = 0xBF58476D1CE4E5B9;
pub const MIX_MUL2: u64 = 0x94D049BB133111EB;
pub const WORD_XOR: u64 = 0xC0FFEE;
pub const PARENT_MUL: u64 = 0xD1B54A32D192ED03;
pub const SLOT_MUL: u64 = 0xA24BAED4963EE407;

pub const ANCHOR_WINDOW = [_]u64{ 2, 3, 4 };
pub const HASHCASH_BITS: u32 = 21;
pub const HEIGHT_MAX: u64 = (1 << 53) - 1;

/// Scalar is the plain loop. Fast is the SIMD fold plus SHA-NI (via std, when the target has it).
pub const Engine = enum { scalar, fast };

pub const Profile = struct {
    slot_bytes: u64,
    terrain_bytes: u64,
    sample_bytes: u64,
    steps_per_leg: u64,

    pub fn wordsPerSlot(self: Profile) u64 {
        return self.slot_bytes / 8;
    }
    pub fn nSlots(self: Profile) u64 {
        return self.terrain_bytes / self.slot_bytes;
    }
    pub fn offsetModulus(self: Profile) u64 {
        return self.terrain_bytes - self.sample_bytes + 1;
    }
};

pub const production = Profile{
    .slot_bytes = SLOT_BYTES,
    .terrain_bytes = TERRAIN_BYTES,
    .sample_bytes = SAMPLE_BYTES,
    .steps_per_leg = STEPS_PER_LEG,
};

pub const s1 = Profile{
    .slot_bytes = 64,
    .terrain_bytes = 65536,
    .sample_bytes = 32,
    .steps_per_leg = 100,
};

pub const s2 = Profile{
    .slot_bytes = 256,
    .terrain_bytes = 1048576,
    .sample_bytes = 64,
    .steps_per_leg = 1000,
};

pub fn profileOk(p: Profile) bool {
    if (p.slot_bytes < 8 or p.slot_bytes > SLOT_BYTES or p.slot_bytes % 8 != 0) return false;
    if (p.steps_per_leg < 1 or p.steps_per_leg > STEPS_PER_LEG) return false;
    if (p.sample_bytes < 1 or p.sample_bytes > SAMPLE_BYTES) return false;
    if (p.terrain_bytes > TERRAIN_BYTES or p.terrain_bytes < p.sample_bytes) return false;
    if (p.terrain_bytes % p.slot_bytes != 0) return false;
    if (p.offsetModulus() < 1) return false;
    return true;
}
