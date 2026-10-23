//! Produce. Offline runs use the scripted rig. Live runs use Bitcoin gossip.

pub mod adapter;
pub mod cells;
pub mod estimate;
pub mod eventlog;
pub mod events;
pub mod live;
pub mod model;

/// `STAGES`, section 6. Numbered 0–17.
pub const STAGES: usize = pcoc::constants::STAGES;
/// `TERRAIN_BYTES`, section 6. 256 MiB, held for the run.
pub const TERRAIN_BYTES: u64 = pcoc::constants::TERRAIN_BYTES as u64;
/// `STEPS_PER_LEG`, section 6. Shown once, in Learn mode, as the reason steps are not progress.
pub const STEPS_PER_LEG: u64 = pcoc::constants::STEPS_PER_LEG;
/// Expected seconds between Bitcoin headers. The estimate's only input besides the count.
pub const HEADER_INTERVAL_S: u64 = 600;
