//! SPEC §6 and §16.1. The profile in force.

use crate::constants::{
    SAMPLE_BYTES, SLOT_BYTES, STEPS_PER_LEG, TERRAIN_BYTES,
};

/// SPEC §14. The profile name on the verify record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileName {
    /// Section 6 constants.
    Production,
    /// SPEC §16.1. The four integers are verifier inputs.
    ConformanceSmall,
}

/// Sizes for one run. Derived fields use the section 6 formulas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub name: ProfileName,
    pub slot_bytes: usize,
    pub sample_bytes: usize,
    pub terrain_bytes: usize,
    pub steps_per_leg: u64,
    pub words_per_slot: usize,
    pub n_slots: u64,
    pub offset_modulus: u64,
}

impl Profile {
    /// SPEC §6. The production profile.
    pub fn production() -> Self {
        Self::from_sizes(
            ProfileName::Production,
            SLOT_BYTES,
            SAMPLE_BYTES,
            TERRAIN_BYTES,
            STEPS_PER_LEG,
        )
        .expect("section 6 sizes meet section 16.1")
    }

    /// SPEC §16.1. All four integers are required. No section 6 substitute.
    pub fn conformance_small(
        sample_bytes: usize,
        slot_bytes: usize,
        terrain_bytes: usize,
        steps_per_leg: u64,
    ) -> Result<Self, String> {
        Self::from_sizes(
            ProfileName::ConformanceSmall,
            slot_bytes,
            sample_bytes,
            terrain_bytes,
            steps_per_leg,
        )
    }

    fn from_sizes(
        name: ProfileName,
        slot_bytes: usize,
        sample_bytes: usize,
        terrain_bytes: usize,
        steps_per_leg: u64,
    ) -> Result<Self, String> {
        if slot_bytes < 8 || slot_bytes > SLOT_BYTES || slot_bytes % 8 != 0 {
            return Err("slot_bytes is outside the section 16.1 range".into());
        }
        if !(1..=2_000_000).contains(&steps_per_leg) {
            return Err("steps_per_leg is outside the section 16.1 range".into());
        }
        if sample_bytes < 1 || sample_bytes > SAMPLE_BYTES {
            return Err("sample_bytes is outside the section 16.1 range".into());
        }
        if terrain_bytes > TERRAIN_BYTES
            || terrain_bytes < slot_bytes
            || terrain_bytes % slot_bytes != 0
        {
            return Err("terrain_bytes is outside the section 16.1 range".into());
        }
        if terrain_bytes < sample_bytes {
            return Err("OFFSET_MODULUS would be below 1".into());
        }
        let n_slots = (terrain_bytes / slot_bytes) as u64;
        let offset_modulus = (terrain_bytes - sample_bytes + 1) as u64;
        Ok(Self {
            name,
            slot_bytes,
            sample_bytes,
            terrain_bytes,
            steps_per_leg,
            words_per_slot: slot_bytes / 8,
            n_slots,
            offset_modulus,
        })
    }

    /// SPEC §14. Record name: `production` or `conformance-small`.
    pub fn record_name(&self) -> &'static str {
        match self.name {
            ProfileName::Production => "production",
            ProfileName::ConformanceSmall => "conformance-small",
        }
    }
}
