//! Ask cells, DESIGN-v2 section 4 and section 13.1.
//!
//! Offsets are H+2, H+3 and H+4 from the stage beacon height H. A served
//! path stops later asks. Nothing is asked above the cut-off. `W_17` is
//! posted and never asked. A cell that has not been asked is not a miss.

use super::events::AskOutcome;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Served,
    /// Asked, and the ask was a miss. Hatched.
    Miss,
    /// The next ask in order, at the next header or already due.
    StillDue,
    NotYetDue,
    /// Not asked, because the path is already stored.
    NotAskedStored,
    /// Not asked, because that height is above the cut-off.
    AboveCutOff,
    /// Not asked for another reason, such as a jumped header.
    NotAsked,
}

impl Cell {
    /// The accessible name and tooltip.
    pub fn words(self) -> &'static str {
        match self {
            Cell::Served => "served",
            Cell::Miss => "asked, miss",
            Cell::StillDue => "still due",
            Cell::NotYetDue => "not yet due",
            Cell::NotAskedStored => "not asked, path already stored",
            Cell::AboveCutOff => "above the cut-off",
            Cell::NotAsked => "not asked",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// Not taken to post yet.
    NotPosted,
    /// No cells: the POST did not connect, or was not appended, so no ask is due.
    Skip,
    Cells([Cell; 3]),
    /// Present, but the record has no `d_g` to place the serve.
    PresentUnplaced,
    /// `W_17`.
    NotFetchedByDesign,
}

pub const OFFSETS: [u64; 3] = [2, 3, 4];

/// Produce: cells from the post, the asks so far, the tip and the cut-off.
pub fn produce_row(
    g: u64,
    posted: Option<bool>,
    beacon_height: Option<u64>,
    asks: &[(u64, AskOutcome)],
    tip: Option<u64>,
    cut_off: Option<u64>,
) -> Row {
    let Some(appended) = posted else {
        return Row::NotPosted;
    };
    if g == 17 {
        return Row::NotFetchedByDesign;
    }
    if !appended {
        return Row::Skip;
    }
    let Some(h) = beacon_height else {
        return Row::Skip;
    };
    let served_at = asks
        .iter()
        .find(|(_, o)| *o == AskOutcome::Served)
        .map(|(off, _)| *off);
    let mut cells = [Cell::NotYetDue; 3];
    let mut next_marked = false;
    for (i, &off) in OFFSETS.iter().enumerate() {
        let height = h + off;
        let above = cut_off.is_some_and(|c| height > c);
        cells[i] = if let Some((_, outcome)) = asks.iter().find(|(o, _)| *o == off) {
            match outcome {
                AskOutcome::Served => Cell::Served,
                AskOutcome::Miss(_) => Cell::Miss,
            }
        } else if above {
            Cell::AboveCutOff
        } else if served_at.is_some_and(|s| s < off) {
            Cell::NotAskedStored
        } else if cut_off.is_some()
            && tip.is_some_and(|t| t >= cut_off.unwrap_or(0))
            && tip.is_some_and(|t| height < t)
        {
            // Settled past this height without an ask.
            Cell::NotAsked
        } else if !next_marked && tip.is_some_and(|t| height <= t + 1) {
            next_marked = true;
            Cell::StillDue
        } else {
            next_marked = true;
            Cell::NotYetDue
        };
    }
    Row::Cells(cells)
}

/// Verify: cells from the record's path row. `d_g` is the serving offset.
pub fn record_row(
    path_state: &str,
    d_g: Option<u64>,
    beacon_height: Option<u64>,
    cut_off: Option<u64>,
) -> Row {
    if path_state != "present" {
        return Row::Skip;
    }
    let Some(d) = d_g.filter(|d| OFFSETS.contains(d)) else {
        return Row::PresentUnplaced;
    };
    let mut cells = [Cell::NotAskedStored; 3];
    for (i, &off) in OFFSETS.iter().enumerate() {
        let above = matches!((beacon_height, cut_off), (Some(h), Some(c)) if h + off > c);
        cells[i] = if off < d {
            Cell::Miss
        } else if off == d {
            Cell::Served
        } else if above {
            Cell::AboveCutOff
        } else {
            Cell::NotAskedStored
        };
    }
    Row::Cells(cells)
}

/// A skip whose three asks are known to have missed, from the `.events` sibling.
pub fn skip_with_asks(asks: &[(u64, AskOutcome)]) -> Row {
    if asks.len() == 3 && asks.iter().all(|(_, o)| matches!(o, AskOutcome::Miss(_))) {
        Row::Cells([Cell::Miss; 3])
    } else {
        Row::Skip
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Cell::*;

    const SERVED: AskOutcome = AskOutcome::Served;
    fn miss() -> AskOutcome {
        AskOutcome::Miss(None)
    }

    /// DESIGN-v2 section 2.5, tip 971,210, cut-off not known yet.
    #[test]
    fn moment_r_cells() {
        let tip = Some(971_210);
        assert_eq!(
            produce_row(1, Some(true), Some(971_205), &[(2, SERVED)], tip, None),
            Row::Cells([Served, NotAskedStored, NotAskedStored])
        );
        assert_eq!(
            produce_row(2, Some(false), Some(971_206), &[], tip, None),
            Row::Skip
        );
        assert_eq!(
            produce_row(4, Some(true), Some(971_208), &[(2, SERVED)], tip, None),
            Row::Cells([Served, NotAskedStored, NotAskedStored])
        );
        assert_eq!(
            produce_row(5, Some(true), Some(971_209), &[], tip, None),
            Row::Cells([StillDue, NotYetDue, NotYetDue])
        );
        assert_eq!(
            produce_row(6, Some(true), Some(971_210), &[], tip, None),
            Row::Cells([NotYetDue, NotYetDue, NotYetDue])
        );
        assert_eq!(produce_row(7, None, None, &[], tip, None), Row::NotPosted);
    }

    /// DESIGN-v2 section 2.7, cut-off 971,222.
    #[test]
    fn moment_f_cells() {
        let (tip, cut) = (Some(971_222), Some(971_222));
        assert_eq!(
            produce_row(
                5,
                Some(true),
                Some(971_209),
                &[(2, miss()), (3, SERVED)],
                tip,
                cut
            ),
            Row::Cells([Miss, Served, NotAskedStored])
        );
        assert_eq!(
            produce_row(
                9,
                Some(true),
                Some(971_213),
                &[(2, miss()), (3, miss()), (4, miss())],
                tip,
                cut
            ),
            Row::Cells([Miss, Miss, Miss])
        );
        assert_eq!(
            produce_row(15, Some(true), Some(971_219), &[(2, SERVED)], tip, cut),
            Row::Cells([Served, NotAskedStored, AboveCutOff])
        );
        assert_eq!(
            produce_row(16, Some(true), Some(971_220), &[(2, SERVED)], tip, cut),
            Row::Cells([Served, AboveCutOff, AboveCutOff])
        );
        assert_eq!(
            produce_row(17, Some(true), Some(971_221), &[], tip, cut),
            Row::NotFetchedByDesign
        );
    }

    #[test]
    fn verify_cells_from_d_g() {
        let cut = Some(971_222);
        assert_eq!(
            record_row("present", Some(2), Some(971_205), cut),
            Row::Cells([Served, NotAskedStored, NotAskedStored])
        );
        assert_eq!(
            record_row("present", Some(3), Some(971_216), cut),
            Row::Cells([Miss, Served, NotAskedStored])
        );
        assert_eq!(
            record_row("present", Some(2), Some(971_220), cut),
            Row::Cells([Served, AboveCutOff, AboveCutOff])
        );
        assert_eq!(record_row("skip", None, Some(971_206), cut), Row::Skip);
        assert_eq!(
            record_row("present", None, Some(1), cut),
            Row::PresentUnplaced
        );
        assert_eq!(
            skip_with_asks(&[(2, miss()), (3, miss()), (4, miss())]),
            Row::Cells([Miss; 3])
        );
        assert_eq!(skip_with_asks(&[]), Row::Skip);
    }
}
