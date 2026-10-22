//! SPEC §14.6. Unbiased draw of 6 distinct global indices.

use crate::constants::SIX_LEG_COUNT;
use std::io::Read;

/// Why a draw did not yield indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawStop {
    /// Fewer than 6 legs. The byte source is not read.
    NotDefined,
    /// The byte source ended before 6 distinct indices.
    NotRead,
}

/// SPEC §14.6. `2^64 mod leg_total`, as a mathematical integer.
fn rem_pow2_64(leg_total: u64) -> u128 {
    (1u128 << 64) % (leg_total as u128)
}

/// SPEC §14.6. One unbiased index in `0 … leg_total − 1`.
/// An 8-byte draw with `u >= limit` is consumed and is not returned.
pub fn draw_one(leg_total: u64, next8: &mut dyn FnMut() -> Option<[u8; 8]>) -> Result<u64, DrawStop> {
    if leg_total == 0 {
        return Err(DrawStop::NotDefined);
    }
    let rem = rem_pow2_64(leg_total);
    let limit = (1u128 << 64) - rem;
    loop {
        let raw = next8().ok_or(DrawStop::NotRead)?;
        let u = u64::from_le_bytes(raw) as u128;
        if u < limit {
            return Ok((u % (leg_total as u128)) as u64);
        }
    }
}

/// SPEC §14.6. Six distinct indices, in draw order.
/// The caller must not invoke this when `leg_total < 6`.
pub fn draw_six(
    leg_total: u64,
    next8: &mut dyn FnMut() -> Option<[u8; 8]>,
) -> Result<Vec<u64>, DrawStop> {
    if leg_total < SIX_LEG_COUNT as u64 {
        return Err(DrawStop::NotDefined);
    }
    let mut chosen = Vec::with_capacity(SIX_LEG_COUNT);
    while chosen.len() < SIX_LEG_COUNT {
        let q = draw_one(leg_total, next8)?;
        if !chosen.contains(&q) {
            chosen.push(q);
        }
    }
    Ok(chosen)
}

/// SPEC §15.5. `D = 6 · thorough_rounds` indices, one draw from `next8`.
///
/// When `leg_total ≥ D` the indices are distinct. When `6 ≤ leg_total < D`,
/// an index repeats only after every index has been used, and the current
/// group of 6 stays distinct. `leg_total < 6` is `NotDefined` and reads nothing.
/// A short byte source is `NotRead`.
pub fn draw_thorough(
    leg_total: u64,
    thorough_rounds: u32,
    next8: &mut dyn FnMut() -> Option<[u8; 8]>,
) -> Result<Vec<u64>, DrawStop> {
    if leg_total < SIX_LEG_COUNT as u64 {
        return Err(DrawStop::NotDefined);
    }
    let d = (SIX_LEG_COUNT as u64).saturating_mul(u64::from(thorough_rounds));
    let mut phase: Vec<u64> = Vec::with_capacity(d as usize);
    while (phase.len() as u64) < d {
        let q = draw_one(leg_total, next8)?;
        let group_start = (phase.len() / SIX_LEG_COUNT) * SIX_LEG_COUNT;
        if phase[group_start..].contains(&q) {
            continue;
        }
        if phase.contains(&q) && (0..leg_total).any(|i| !phase.contains(&i)) {
            continue;
        }
        phase.push(q);
    }
    Ok(phase)
}

/// SPEC §14.6. Six distinct indices. Reads the operating-system CSPRNG.
/// Takes no byte source, label, or seed.
pub fn six_leg_draw(leg_total: u64) -> Result<Vec<u64>, DrawStop> {
    let mut next = os_csprng_next8;
    draw_six(leg_total, &mut next)
}

/// SPEC §15.5. Thorough-round indices. Reads the operating-system CSPRNG.
/// Takes no byte source, label, or seed.
pub fn thorough_draw(leg_total: u64, thorough_rounds: u32) -> Result<Vec<u64>, DrawStop> {
    let mut next = os_csprng_next8;
    draw_thorough(leg_total, thorough_rounds, &mut next)
}

/// SPEC §14.6. Read the next 8 bytes from `src`. Short reads are exhaustion.
pub fn next8_from(src: &mut dyn Read) -> Option<[u8; 8]> {
    let mut buf = [0u8; 8];
    let mut filled = 0;
    while filled < 8 {
        match src.read(&mut buf[filled..]) {
            Ok(0) => return None,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
    Some(buf)
}

/// SPEC §14.6. Production verify reads the operating-system CSPRNG.
pub fn os_csprng_next8() -> Option<[u8; 8]> {
    let mut f = std::fs::File::open("/dev/urandom").ok()?;
    next8_from(&mut f)
}
