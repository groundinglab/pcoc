/// Constants from Satoshi Hike v2.0 section 6.

pub const SLOT_BYTES: usize = 16384;
pub const WORDS_PER_SLOT: usize = 2048;
pub const TERRAIN_BYTES: usize = 268_435_456;
pub const N_SLOTS: usize = 16384;
pub const SAMPLE_BYTES: usize = 1024;
pub const OFFSET_MODULUS: u64 = 268_434_433;
pub const STEPS_PER_LEG: u64 = 2_000_000;
pub const STAGES: usize = 18;
/// Section 9 and section 14. A JSON integer on the verify record stays inside this bound.
pub(crate) const JSON_INT_MAX: u64 = (1 << 53) - 1;
pub const HASH_BYTES: usize = 32;
pub const LEG_RESULT_BYTES: usize = 8;
pub const PUBLIC_KEY_BYTES: usize = 32;
pub const BEACON_BYTES: usize = 32;

pub const SEAL_DOMAIN: &[u8; 12] = b"pcoc-walk-v1";
pub const KIND: &str = "satoshi-hike-v2.1-object";
/// Rule A5. v2.0 is a hard cut: that kind is an earlier version, with v1.35.
pub const EARLIER_KINDS: &[&str] = &[
    "satoshi-hike-v2.0-object",
    "satoshi-hike-v1.35-object",
];
pub const FILL: &str = "dag2";

/// Section 13.1. The only anchor-window offsets. Producer and verifier both use this list.
pub const ANCHOR_WINDOW_OFFSETS: [u64; 3] = [2, 3, 4];

/// How many Rule A4.3 checkpoint comparisons run at once when `PCOC_BIND_THREADS` is unset.
pub const DEFAULT_BIND_THREADS: usize = 6;

// No leg-rate floor here: the spec defines measurements, a policy sets the
// values. The section 15.5 examples (7/20, 1/4, 1/8) live in test fixtures and
// in spec/conformance/v2.0/constants.json; the admission gate's floor is its
// policy file (services/admission-gate/policy).

pub const MIX_ADD: u64 = 0x9E3779B97F4A7C15;
pub const MIX_MUL1: u64 = 0xBF58476D1CE4E5B9;
pub const MIX_MUL2: u64 = 0x94D049BB133111EB;
pub const WORD_XOR: u64 = 0xC0FFEE;
pub const PARENT_MUL: u64 = 0xD1B54A32D192ED03;
pub const SLOT_MUL: u64 = 0xA24BAED4963EE407;

pub const CHECKPOINT_PREIMAGE_LEN: usize = 32 + 32 + SAMPLE_BYTES + 8;

/// Live Bari. Filled only by the explicit live profile.
pub const LIVE_BARI_URL: &str = "https://bari.groundinglab.io";
/// Live index. The admission door's source default stays this origin.
/// Verify fills it only for the explicit live profile.
pub const LIVE_INDEX_URL: &str = "https://index.groundinglab.io";

/// Live origin kept under the old name. Produce does not fill it unless `--profile live`.
pub const DEFAULT_BARI_URL: &str = LIVE_BARI_URL;
pub const DEFAULT_INDEX_URL: &str = LIVE_INDEX_URL;

const _: () = assert!(WORDS_PER_SLOT == SLOT_BYTES / 8);
const _: () = assert!(N_SLOTS == TERRAIN_BYTES / SLOT_BYTES);
const _: () = assert!(TERRAIN_BYTES == N_SLOTS * SLOT_BYTES);
const _: () = assert!(OFFSET_MODULUS == (TERRAIN_BYTES - SAMPLE_BYTES + 1) as u64);
const _: () = assert!(CHECKPOINT_PREIMAGE_LEN == 1096);
const _: () = assert!(STEPS_PER_LEG == 2_000_000);
const _: () = assert!(STAGES == 18);
const _: () = assert!(ANCHOR_WINDOW_OFFSETS[0] == 2);
const _: () = assert!(ANCHOR_WINDOW_OFFSETS[1] == ANCHOR_WINDOW_OFFSETS[0] + 1);
const _: () = assert!(ANCHOR_WINDOW_OFFSETS[2] == ANCHOR_WINDOW_OFFSETS[1] + 1);

/// Section 6 sizes, or the one SPEC §16.1 `conformance-small` set.
///
/// Derived integers stay the section 6 formulas. This type does not set
/// `STAGES`, the domains, `KIND`, `FILL`, the mix constants, or the draw count.
/// Produce and the produce CLI do not select [`Sizes::conformance_small`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizes {
    pub sample_bytes: usize,
    pub slot_bytes: usize,
    pub terrain_bytes: usize,
    pub steps_per_leg: u64,
}

impl Sizes {
    pub const fn production() -> Self {
        Self {
            sample_bytes: SAMPLE_BYTES,
            slot_bytes: SLOT_BYTES,
            terrain_bytes: TERRAIN_BYTES,
            steps_per_leg: STEPS_PER_LEG,
        }
    }

    /// SPEC §16.1. The only size set other than section 6.
    ///
    /// Same integers as the Go and Java `ConformanceSmallV2` profile:
    /// sample 32, slot 64, terrain 256, steps 32.
    /// Words per slot is 8, `N_SLOTS` is 4, `OFFSET_MODULUS` is 225.
    pub const fn conformance_small() -> Self {
        Self {
            sample_bytes: 32,
            slot_bytes: 64,
            terrain_bytes: 256,
            steps_per_leg: 32,
        }
    }

    pub const fn is_production(self) -> bool {
        self.sample_bytes == SAMPLE_BYTES
            && self.slot_bytes == SLOT_BYTES
            && self.terrain_bytes == TERRAIN_BYTES
            && self.steps_per_leg == STEPS_PER_LEG
    }

    pub const fn words_per_slot(self) -> usize {
        self.slot_bytes / 8
    }

    pub const fn n_slots(self) -> usize {
        self.terrain_bytes / self.slot_bytes
    }

    pub const fn offset_modulus(self) -> u64 {
        (self.terrain_bytes - self.sample_bytes + 1) as u64
    }

    /// SPEC §16.1 bounds on the four free integers. `SLOT_BYTES` is at least 8
    /// and a multiple of 8, at most the section 6 value. `TERRAIN_BYTES` is a
    /// multiple of the slot, at least the sample, at most section 6.
    /// `SAMPLE_BYTES` and `STEPS_PER_LEG` are at least 1 and at most section 6.
    pub const fn conformance_small_checked(
        sample_bytes: usize,
        slot_bytes: usize,
        terrain_bytes: usize,
        steps_per_leg: u64,
    ) -> Option<Self> {
        if slot_bytes < 8 || slot_bytes % 8 != 0 || slot_bytes > SLOT_BYTES {
            return None;
        }
        if sample_bytes < 1 || sample_bytes > SAMPLE_BYTES || terrain_bytes < sample_bytes {
            return None;
        }
        if terrain_bytes > TERRAIN_BYTES || terrain_bytes % slot_bytes != 0 {
            return None;
        }
        if steps_per_leg < 1 || steps_per_leg > STEPS_PER_LEG {
            return None;
        }
        Some(Self {
            sample_bytes,
            slot_bytes,
            terrain_bytes,
            steps_per_leg,
        })
    }
}

// Section 6 small-size profile, checked at compile time.
const _: () = {
    let small = Sizes::conformance_small();
    assert!(small.slot_bytes >= 8 && small.slot_bytes % 8 == 0);
    assert!(small.slot_bytes <= SLOT_BYTES);
    assert!(small.steps_per_leg >= 1 && small.steps_per_leg <= STEPS_PER_LEG);
    assert!(small.sample_bytes >= 1 && small.sample_bytes <= SAMPLE_BYTES);
    assert!(small.terrain_bytes <= TERRAIN_BYTES);
    assert!(small.terrain_bytes % small.slot_bytes == 0);
    assert!(small.offset_modulus() >= 1);
    assert!(small.words_per_slot() == 8);
    assert!(small.n_slots() == 4);
    assert!(small.offset_modulus() == 225);
};

/// Section 13.1. The one height H: the stage's stored beacon height.
///
/// Swap this body if a later choice stores the Hashcash post height instead.
pub fn anchor_window_origin(stage_beacon_height: u64) -> u64 {
    stage_beacon_height
}

/// Heights `H + offset` for each offset in [`ANCHOR_WINDOW_OFFSETS`].
pub fn anchor_window(stage_beacon_height: u64) -> Option<[u64; 3]> {
    let origin = anchor_window_origin(stage_beacon_height);
    let mut out = [0u64; 3];
    for (i, offset) in ANCHOR_WINDOW_OFFSETS.iter().enumerate() {
        out[i] = origin.checked_add(*offset)?;
    }
    Some(out)
}

pub fn height_in_anchor_window(post_height: u64, height: u64) -> bool {
    anchor_window(post_height).is_some_and(|window| window.contains(&height))
}

/// `PCOC_BIND_THREADS`, or [`DEFAULT_BIND_THREADS`] when unset, blank, or not a positive integer.
pub fn bind_threads_from(text: Option<&str>) -> usize {
    text.and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|count| *count >= 1)
        .unwrap_or(DEFAULT_BIND_THREADS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Section 15.5 example floors, as test fixtures (policy values, not SDK constants).
    const FLOOR_EXAMPLES: [(u64, u64); 3] = [(7, 20), (1, 4), (1, 8)];

    #[test]
    fn leg_rate_floor_examples_scale_with_the_leg() {
        let higher = |left: (u64, u64), right: (u64, u64)| left.0 * right.1 > right.0 * left.1;
        assert!(higher(FLOOR_EXAMPLES[0], FLOOR_EXAMPLES[1]));
        assert!(higher(FLOOR_EXAMPLES[1], FLOOR_EXAMPLES[2]));
    }

    /// The seal preimage starts with these 12 bytes. The seed has no domain label.
    #[test]
    fn p6_13_seal_domain_and_seed_has_no_label() {
        assert_eq!(SEAL_DOMAIN, b"pcoc-walk-v1");
        assert_eq!(SEAL_DOMAIN.len(), 12);
    }

    /// SPEC §16.1. The small set keeps the section 6 formulas and stays inside the bounds.
    #[test]
    fn p6_23_conformance_small_keeps_the_section_6_formulas() {
        let small = Sizes::conformance_small();
        assert_eq!(
            (
                small.sample_bytes,
                small.slot_bytes,
                small.terrain_bytes,
                small.steps_per_leg
            ),
            (32, 64, 256, 32)
        );
        assert_eq!(small.words_per_slot(), small.slot_bytes / 8);
        assert_eq!(small.n_slots(), small.terrain_bytes / small.slot_bytes);
        assert_eq!(
            small.offset_modulus(),
            (small.terrain_bytes - small.sample_bytes + 1) as u64
        );
        assert_eq!(small.words_per_slot(), 8);
        assert_eq!(small.n_slots(), 4);
        assert_eq!(small.offset_modulus(), 225);
        assert!(small.slot_bytes >= 8 && small.slot_bytes % 8 == 0);
        assert!(small.slot_bytes <= SLOT_BYTES);
        assert!(small.steps_per_leg >= 1 && small.steps_per_leg <= STEPS_PER_LEG);
        assert!(small.sample_bytes >= 1 && small.sample_bytes <= SAMPLE_BYTES);
        assert!(small.terrain_bytes <= TERRAIN_BYTES);
        assert!(small.terrain_bytes >= small.sample_bytes);
        assert_ne!(small, Sizes::production());
        assert!(Sizes::production().is_production());
        assert!(!small.is_production());
    }

    /// Rule A4.9 and Rule B2. The record holds integers. The comparison is `q·N ≥ p·S`.
    /// The three pairs are §15.5 informative examples.
    #[test]
    fn a4_9_rate_floor_compares_integers() {
        let below = |p: u64, q: u64, n: u64, s: u64| q * n < p * s;
        let meets = |p: u64, q: u64, n: u64, s: u64| q * n >= p * s;
        let floors = FLOOR_EXAMPLES;
        for (p, q) in floors {
            assert!(below(p, q, 0, 1));
            assert!(meets(p, q, p, q));
            assert!(meets(p, q, p, q - 1));
            assert!(below(p, q, p - 1, q));
        }
        // 7/20 is stricter than 1/4, which is stricter than 1/8, on the same span.
        assert!(below(7, 20, 1, 4));
        assert!(meets(1, 4, 1, 4));
        assert!(meets(1, 8, 1, 4));
    }
}

/// SPEC v2.0 section 6, rules P6.1-P6.25, and the section 16.1
/// conformance-small sizes. Changing or removing any of these fails CI.
#[cfg(test)]
mod spec6_pins {
    use super::*;

    #[test]
    fn spec6_constants_are_pinned() {
        assert_eq!(SLOT_BYTES, 16384, "P6.1");
        assert_eq!(SLOT_BYTES / 8, 2048, "P6.2 WORDS_PER_SLOT");
        assert_eq!(SLOT_BYTES % 8, 0, "P6.23");
        assert_eq!(TERRAIN_BYTES, 268_435_456, "P6.3");
        assert_eq!(N_SLOTS, 16384, "P6.4");
        assert_eq!(N_SLOTS * SLOT_BYTES, TERRAIN_BYTES, "P6.24");
        assert_eq!(SAMPLE_BYTES, 1024, "P6.5");
        assert_eq!(OFFSET_MODULUS, 268_434_433, "P6.6");
        assert_eq!(
            OFFSET_MODULUS,
            (TERRAIN_BYTES - SAMPLE_BYTES + 1) as u64,
            "P6.25"
        );
        assert_eq!(STEPS_PER_LEG, 2_000_000, "P6.7");
        assert_eq!(STAGES, 18, "P6.8");
        assert_eq!(HASH_BYTES, 32, "P6.9");
        assert_eq!(SEAL_DOMAIN, b"pcoc-walk-v1", "P6.13");
        assert_eq!(KIND, "satoshi-hike-v2.1-object", "P6.15");
        assert_eq!(KIND.len(), 24);
        assert_eq!(FILL, "dag2", "P6.16");
        assert_eq!(MIX_ADD, 0x9E3779B97F4A7C15, "P6.17");
        assert_eq!(MIX_MUL1, 0xBF58476D1CE4E5B9, "P6.18");
        assert_eq!(MIX_MUL2, 0x94D049BB133111EB, "P6.19");
        assert_eq!(WORD_XOR, 0xC0FFEE, "P6.20");
        assert_eq!(PARENT_MUL, 0xD1B54A32D192ED03, "P6.21");
        assert_eq!(SLOT_MUL, 0xA24BAED4963EE407, "P6.22");
        assert_eq!(
            Sizes::production(),
            Sizes {
                sample_bytes: 1024,
                slot_bytes: 16384,
                terrain_bytes: 268_435_456,
                steps_per_leg: 2_000_000
            }
        );
    }

    #[test]
    fn conformance_small_sizes_are_pinned() {
        let s = Sizes::conformance_small();
        assert_eq!(
            s,
            Sizes {
                sample_bytes: 32,
                slot_bytes: 64,
                terrain_bytes: 256,
                steps_per_leg: 32
            },
            "16.1"
        );
        assert_eq!(s.slot_bytes / 8, 8, "P6.23 small");
        assert_eq!(s.terrain_bytes / s.slot_bytes, 4, "P6.24 small");
        assert_eq!(s.terrain_bytes - s.sample_bytes + 1, 225, "P6.25 small");
        assert!(!s.is_production());
    }
}
