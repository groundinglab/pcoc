//! Produce, in-process, on the pinned `hike` crate (DESIGN-v2 sections 6, 7.3, 7.4).

pub mod adapter;
pub mod cells;
pub mod estimate;
pub mod eventlog;
pub mod events;
pub mod live;
pub mod model;

/// `STAGES`, section 6. Numbered 0–17.
pub const STAGES: usize = 18;
/// `TERRAIN_BYTES`, section 6. 256 MiB, held for the run.
pub const TERRAIN_BYTES: u64 = 268_435_456;
/// `STEPS_PER_LEG`, section 6. Shown once, in Learn mode, as the reason steps are not progress.
pub const STEPS_PER_LEG: u64 = hike::STEPS_PER_LEG;
/// Expected seconds between Bitcoin headers. The estimate's only input besides the count.
pub const HEADER_INTERVAL_S: u64 = 600;
