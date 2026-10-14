//! What the Produce screen and its status bar show, folded from events.
//! The unit is the leg. There is no step counter and no percentage.

use super::cells::{self, Row};
use super::estimate::{estimate, Estimate};
use super::events::{AskOutcome, ProduceEvent};
use super::STAGES;
use crate::format::{duration_hms, thousands, three_places};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    /// Seed beacon not yet seen.
    Starting,
    Recording,
    /// Every leg is stored. Waiting on the last asks.
    Settling,
    Written,
    Stopped,
    Failed(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StageCol {
    /// Stored checkpoints. `None` is a future stage, drawn as an em dash.
    pub legs: Option<u64>,
    pub open: bool,
    pub interrupt_height: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaypointState {
    pub appended: Option<bool>,
    pub status: Option<u16>,
    /// `(offset, outcome)` with offset 2, 3 or 4.
    pub asks: Vec<(u64, AskOutcome)>,
    pub miss_phrases: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bari {
    /// Terrain fill, before any request.
    NotAskedYet,
    /// The last POST or path GET got an HTTP status. The POST status shown
    /// is the last POST that was answered.
    Reachable { last_post_status: Option<u16> },
    /// The last request did not connect.
    Unreachable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectFacts {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: [u8; 32],
    pub at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProduceView {
    pub phase: Phase,
    pub output: Option<PathBuf>,
    pub public_key_hex: Option<String>,
    pub started_ms: Option<u64>,
    pub seed_height: Option<u64>,
    /// Bitcoin header `nTime` of the seed, unix seconds. Absent when unread.
    pub seed_time_unix_s: Option<u32>,
    pub stages: Vec<StageCol>,
    pub open_stage: usize,
    pub legs_total: u64,
    pub first_leg_ms: Option<u64>,
    pub last_leg_ms: Option<u64>,
    pub tip_height: Option<u64>,
    pub tip_natural: Option<[u8; 32]>,
    pub tip_seen_ms: Option<u64>,
    pub peers: Option<u32>,
    pub waypoints: BTreeMap<u64, WaypointState>,
    pub bari: Bari,
    pub last_post_status: Option<u16>,
    pub path_lines: Vec<String>,
    pub object: Option<ObjectFacts>,
    pub log: Vec<String>,
}

impl Default for ProduceView {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            output: None,
            public_key_hex: None,
            started_ms: None,
            seed_height: None,
            seed_time_unix_s: None,
            stages: vec![StageCol::default(); STAGES],
            open_stage: 0,
            legs_total: 0,
            first_leg_ms: None,
            last_leg_ms: None,
            tip_height: None,
            tip_natural: None,
            tip_seen_ms: None,
            peers: None,
            waypoints: BTreeMap::new(),
            bari: Bari::NotAskedYet,
            last_post_status: None,
            path_lines: Vec::new(),
            object: None,
            log: Vec::new(),
        }
    }
}

const LOG_LINES: usize = 400;

/// Shown from Start until the first Observe returns the tip already on the
/// best chain. §9 P9.14: that header is the seed. It is not a header mined later.
pub const READING_THE_TIP: &str = "Reading the Bitcoin tip…";

/// One chip for a waypoint. The desktop reports the POST and the path fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaypointChip {
    /// Not taken to post yet.
    NotDueYet,
    /// The POST was answered and no path ask has started.
    Posted,
    /// Posted, and a path ask has not returned a path.
    WaitingForAnchor,
    /// A path was fetched.
    Anchored,
    /// The asks did not return a path.
    Skipped,
    /// HTTP 5xx, 408, or the POST did not connect.
    PostError,
}

impl WaypointChip {
    pub fn css_class(self) -> &'static str {
        match self {
            WaypointChip::NotDueYet => "wp-not-due",
            WaypointChip::Posted => "wp-posted",
            WaypointChip::WaitingForAnchor => "wp-waiting",
            WaypointChip::Anchored => "wp-anchored",
            WaypointChip::Skipped => "wp-skipped",
            WaypointChip::PostError => "wp-post-error",
        }
    }

    /// The words on the chip.
    pub fn label(self) -> &'static str {
        match self {
            WaypointChip::NotDueYet => "not due yet",
            WaypointChip::Posted => "posted to Bari",
            WaypointChip::WaitingForAnchor => "posted, waiting for anchor path",
            WaypointChip::Anchored => "anchored",
            WaypointChip::Skipped => "skipped",
            WaypointChip::PostError => "post error",
        }
    }

    /// Tooltip. States only what this desktop has seen.
    pub fn tooltip(self) -> &'static str {
        match self {
            WaypointChip::NotDueYet => "Not due yet. This waypoint has not been taken to post.",
            WaypointChip::Posted => {
                "Posted to Bari. The POST was answered. No path has been asked yet."
            }
            WaypointChip::WaitingForAnchor => {
                "Posted, waiting for anchor path. A path has been asked and has not been fetched."
            }
            WaypointChip::Anchored => "Anchored. The path was fetched.",
            WaypointChip::Skipped => "Skipped. The asks did not return a path.",
            WaypointChip::PostError => "Post error. Bari answered 5xx, or the POST timed out.",
        }
    }
}

impl ProduceView {
    pub fn apply(&mut self, event: ProduceEvent) {
        match event {
            ProduceEvent::Started {
                output,
                public_key_hex,
                at_ms,
            } => {
                *self = Self {
                    phase: Phase::Starting,
                    output: Some(output),
                    public_key_hex: Some(public_key_hex),
                    started_ms: Some(at_ms),
                    ..Self::default()
                };
                self.stages[0].open = true;
                self.stages[0].legs = Some(0);
            }
            ProduceEvent::SeedBeacon {
                height,
                header_time_unix_s,
                ..
            } => {
                self.seed_height = Some(height);
                if header_time_unix_s != 0 {
                    self.seed_time_unix_s = Some(header_time_unix_s);
                }
                self.phase = Phase::Recording;
            }
            ProduceEvent::TerrainFilled { .. } => {}
            ProduceEvent::LegRecorded {
                stage,
                stored_in_stage,
                at_ms,
                ..
            } => {
                if stage < STAGES {
                    self.stages[stage].legs = Some(stored_in_stage);
                    self.stages[stage].open = true;
                    self.open_stage = stage;
                }
                self.legs_total += 1;
                self.first_leg_ms.get_or_insert(at_ms);
                self.last_leg_ms = Some(at_ms);
            }
            ProduceEvent::Tip {
                height,
                natural_hash,
                peers,
                header_seen_unix_ms,
            } => {
                if self.tip_height.is_none_or(|h| height >= h) {
                    self.tip_height = Some(height);
                    self.tip_natural = Some(natural_hash);
                    self.tip_seen_ms = Some(header_seen_unix_ms);
                }
                if peers.is_some() {
                    self.peers = peers;
                }
            }
            ProduceEvent::Gossip { peers } => {
                self.peers = Some(peers);
            }
            ProduceEvent::StageClosed {
                stage,
                legs,
                interrupt_height,
            } => {
                if stage < STAGES {
                    self.stages[stage] = StageCol {
                        legs: Some(legs),
                        open: false,
                        interrupt_height: Some(interrupt_height),
                    };
                    if stage + 1 < STAGES {
                        self.open_stage = stage + 1;
                        self.stages[stage + 1].open = true;
                        self.stages[stage + 1].legs = Some(0);
                    }
                }
            }
            ProduceEvent::WaypointPosted {
                g,
                appended,
                status,
            } => {
                let w = self.waypoints.entry(g).or_default();
                w.appended = Some(appended);
                w.status = status;
                self.bari = match status {
                    Some(status) => Bari::Reachable {
                        last_post_status: Some(status),
                    },
                    None => Bari::Unreachable,
                };
                if let Some(status) = status {
                    self.last_post_status = Some(status);
                }
            }
            ProduceEvent::PathAsk {
                g,
                at_height,
                outcome,
            } => {
                let h = self.beacon_height(g as usize);
                let w = self.waypoints.entry(g).or_default();
                let previous = w.asks.last().map(|(o, _)| *o).unwrap_or(1);
                let measured = h
                    .map(|h| at_height.saturating_sub(h))
                    .unwrap_or(previous + 1);
                let offset = measured.clamp(previous + 1, 4).max(2);
                if let AskOutcome::Miss(Some(phrase)) = &outcome {
                    w.miss_phrases.push(phrase.clone());
                }
                if w.asks.len() < 3 {
                    w.asks.push((offset, outcome));
                }
                self.bari = Bari::Reachable {
                    last_post_status: self.last_post_status,
                };
            }
            ProduceEvent::Settling => self.phase = Phase::Settling,
            ProduceEvent::PathLines(lines) => self.path_lines = lines,
            ProduceEvent::ObjectWritten {
                path,
                bytes,
                sha256,
                at_ms,
            } => {
                self.phase = Phase::Written;
                self.object = Some(ObjectFacts {
                    path,
                    bytes,
                    sha256,
                    at_ms,
                });
                for stage in &mut self.stages {
                    stage.open = false;
                }
            }
            ProduceEvent::Log(line) => {
                self.log.push(line);
                if self.log.len() > LOG_LINES {
                    self.log.remove(0);
                }
            }
            ProduceEvent::Stopped => self.phase = Phase::Stopped,
            ProduceEvent::Failed { message } => self.phase = Phase::Failed(message),
        }
    }

    pub fn running(&self) -> bool {
        matches!(
            self.phase,
            Phase::Starting | Phase::Recording | Phase::Settling
        )
    }

    /// Stages with a stored interrupt.
    pub fn stages_recorded(&self) -> usize {
        self.stages
            .iter()
            .filter(|s| s.interrupt_height.is_some())
            .count()
    }

    /// H for `W_g`: `stages[g].beacon_height`, the height of `I_{g-1}`.
    pub fn beacon_height(&self, g: usize) -> Option<u64> {
        match g {
            0 => self.seed_height,
            g if g < STAGES => self.stages[g - 1].interrupt_height,
            _ => None,
        }
    }

    pub fn cut_off(&self) -> Option<u64> {
        self.stages[STAGES - 1].interrupt_height
    }

    /// `0.308` legs/s: stored legs over the time since the first stored leg.
    /// Terrain fill is before the first leg and is not in it. Recording only.
    pub fn rate(&self) -> Option<(String, u64)> {
        if self.phase != Phase::Recording {
            return None;
        }
        let span_s = (self.last_leg_ms? - self.first_leg_ms?) / 1000;
        Some((three_places(self.legs_total, span_s)?, span_s))
    }

    pub fn tip_age_s(&self, now_ms: u64) -> Option<u64> {
        Some(now_ms.saturating_sub(self.tip_seen_ms?) / 1000)
    }

    pub fn headers_to_come(&self) -> u64 {
        (STAGES - self.stages_recorded()) as u64
    }

    pub fn estimate(&self, now_ms: u64) -> Option<Estimate> {
        if !self.running() {
            return None;
        }
        estimate(self.headers_to_come(), self.tip_age_s(now_ms).unwrap_or(0))
    }

    /// `Stage 6`, `leg 37 in progress`, `6 of 18 stages recorded`.
    pub fn hero_stage(&self) -> (String, String, String) {
        let g = self.open_stage;
        let in_stage = self.stages[g].legs.unwrap_or(0);
        (
            format!("Stage {g}"),
            format!("leg {} in progress", in_stage + 1),
            format!("{} of {STAGES} stages recorded", self.stages_recorded()),
        )
    }

    /// One chip for `W_g`, from the post and the asks stored on this view.
    pub fn waypoint_chip(&self, g: u64) -> WaypointChip {
        let Some(w) = self.waypoints.get(&g) else {
            return WaypointChip::NotDueYet;
        };
        if w.appended.is_none() {
            return WaypointChip::NotDueYet;
        }
        if w.appended == Some(false) || w.status.is_some_and(post_error_status) {
            return WaypointChip::PostError;
        }
        if w.asks.iter().any(|(_, o)| *o == AskOutcome::Served) {
            return WaypointChip::Anchored;
        }
        let misses = w
            .asks
            .iter()
            .filter(|(_, o)| matches!(o, AskOutcome::Miss(_)))
            .count();
        if !w.asks.is_empty() && misses == w.asks.len() && w.asks.len() == 3 {
            return WaypointChip::Skipped;
        }
        if !w.asks.is_empty() {
            return WaypointChip::WaitingForAnchor;
        }
        WaypointChip::Posted
    }

    pub fn waypoint_row(&self, g: u64) -> Row {
        let w = self.waypoints.get(&g);
        let row = cells::produce_row(
            g,
            w.and_then(|w| w.appended),
            self.beacon_height(g as usize),
            w.map(|w| w.asks.as_slice()).unwrap_or(&[]),
            self.tip_height,
            self.cut_off(),
        );
        if row == Row::Skip && w.is_some_and(|w| w.appended == Some(true)) {
            return cells::skip_with_asks(&w.unwrap().asks);
        }
        row
    }

    pub fn posts_taken(&self) -> u64 {
        self.waypoints
            .values()
            .filter(|w| w.appended.is_some())
            .count() as u64
    }

    pub fn posts_appended(&self) -> u64 {
        self.waypoints
            .values()
            .filter(|w| w.appended == Some(true))
            .count() as u64
    }

    pub fn not_connected(&self) -> Vec<u64> {
        self.waypoints
            .iter()
            .filter(|(_, w)| w.appended == Some(false))
            .map(|(g, _)| *g)
            .collect()
    }

    /// `5 of 6 posts appended 204` while recording. `16 of 17 waypoints posted · W2 did not connect` after.
    pub fn post_summary(&self) -> String {
        let missed = self.not_connected();
        let missed_text = missed
            .iter()
            .map(|g| format!("W{g}"))
            .collect::<Vec<_>>()
            .join(", ");
        if self.phase == Phase::Written {
            let mut out = format!("{} of 17 waypoints posted", self.posts_appended());
            if !missed.is_empty() {
                out.push_str(&format!(" · {missed_text} did not connect"));
            }
            return out;
        }
        if self.posts_taken() == 0 {
            return "no post taken yet".to_owned();
        }
        let statuses: Vec<u16> = self
            .waypoints
            .values()
            .filter(|w| w.appended == Some(true))
            .filter_map(|w| w.status)
            .collect();
        let status = match statuses.as_slice() {
            [first, rest @ ..] if rest.iter().all(|s| s == first) => format!(" {first}"),
            _ => String::new(),
        };
        format!(
            "{} of {} posts appended{status}",
            self.posts_appended(),
            self.posts_taken()
        )
    }

    pub fn wall_s(&self) -> Option<u64> {
        Some((self.object.as_ref()?.at_ms - self.started_ms?) / 1000)
    }

    /// Status bar while producing. Display-order tip, last block age, Bari.
    pub fn status_line(&self, now_ms: u64, narrow: bool) -> (String, String) {
        let mut gossip = String::from("Bitcoin gossip");
        if let Some(peers) = self.peers {
            gossip.push_str(&format!(" · {peers} peers"));
        }
        match (self.tip_height, self.tip_natural) {
            (Some(height), Some(natural)) => {
                gossip.push_str(&format!(" · tip {}", thousands(height)));
                if !narrow {
                    let display = crate::hex::encode(&crate::hex::display_order(&natural));
                    gossip.push_str(&format!(" · {} display", crate::hex::short_tip(&display)));
                }
                if let Some(age) = self.tip_age_s(now_ms) {
                    if narrow {
                        gossip.push_str(&format!(" · {}", duration_hms(age)));
                    } else {
                        gossip.push_str(&format!(" · last block {} ago", duration_hms(age)));
                    }
                }
            }
            _ => gossip.push_str(" · connecting"),
        }
        let bari = match self.bari {
            Bari::NotAskedYet => "Bari not asked yet".to_owned(),
            Bari::Reachable {
                last_post_status: Some(status),
            } if !narrow => {
                format!("Bari reachable · last answered POST {status}")
            }
            Bari::Reachable { .. } => "Bari reachable".to_owned(),
            Bari::Unreachable => "Bari did not answer the last request".to_owned(),
        };
        (gossip, bari)
    }

    /// Minutes from the seed header's `nTime` to `now_ms`. Absent when unread.
    pub fn mined_minutes(&self, now_ms: u64) -> Option<u64> {
        let n_time = self.seed_time_unix_s?;
        if n_time == 0 {
            return None;
        }
        Some(now_ms.saturating_sub(u64::from(n_time).saturating_mul(1000)) / 60_000)
    }

    /// "Started on block N (mined X min ago)" once the seed header is known.
    pub fn started_line(&self, now_ms: u64) -> Option<String> {
        let height = self.seed_height?;
        Some(match self.mined_minutes(now_ms) {
            Some(mins) => format!("Started on block {height} (mined {mins} min ago)"),
            None => format!("Started on block {height}"),
        })
    }

    /// "Stage k of 18 · from block H · tip T".
    pub fn stage_line(&self) -> String {
        let g = self.open_stage;
        let from = self.beacon_height(g).unwrap_or(0);
        let tip = self.tip_height.unwrap_or(from);
        format!("Stage {g} of {STAGES} · from block {from} · tip {tip}")
    }

    /// "Stage 0 from block N" once that header is known.
    pub fn stage_from_block_line(&self) -> Option<String> {
        self.seed_height
            .map(|height| format!("Stage 0 from block {height}"))
    }

    /// The activity phrase. Empty when nothing is running.
    pub fn status_word(&self, now_ms: u64) -> String {
        let _ = now_ms;
        match &self.phase {
            Phase::Idle => String::new(),
            Phase::Starting | Phase::Recording if self.seed_height.is_none() => {
                READING_THE_TIP.to_owned()
            }
            Phase::Starting | Phase::Recording if self.legs_total == 0 => self
                .seed_height
                .map(|height| format!("Seed height {}", thousands(height)))
                .unwrap_or_else(|| READING_THE_TIP.to_owned()),
            Phase::Starting | Phase::Recording => self.stage_line(),
            Phase::Settling if self.tip_height.is_none() => READING_THE_TIP.to_owned(),
            Phase::Settling => "Asking the last paths".to_owned(),
            Phase::Written => "Object written".to_owned(),
            Phase::Stopped => "Stopped · no object written".to_owned(),
            Phase::Failed(_) => "Stopped · no object written".to_owned(),
        }
    }

    /// The one status-bar line. No header yet is the connecting sentence.
    /// After that, gossip, Bari and the activity phrase are one line.
    pub fn status_bar_line(&self, now_ms: u64, narrow: bool) -> String {
        if matches!(
            self.phase,
            Phase::Starting | Phase::Recording | Phase::Settling
        ) && self.tip_height.is_none()
        {
            return READING_THE_TIP.to_owned();
        }
        if self.phase == Phase::Idle {
            return String::new();
        }
        let (gossip, bari) = self.status_line(now_ms, narrow);
        let activity = self.status_word(now_ms);
        let mut parts = Vec::new();
        if self.tip_height.is_some() {
            parts.push(gossip);
            parts.push(bari);
        }
        if !activity.is_empty() && activity != READING_THE_TIP {
            parts.push(activity);
        }
        parts.join(" · ")
    }

    /// Peers, the latest header, and when this process saw it.
    pub fn gossip_lines(&self) -> (String, String, String) {
        let peers = match self.peers {
            Some(n) => format!("{n} connected"),
            None => "0 connected".to_owned(),
        };
        let header = match (self.tip_height, self.tip_natural) {
            (Some(height), Some(natural)) => {
                format!("{} · {}", thousands(height), crate::hex::prefix(&natural))
            }
            _ => READING_THE_TIP.to_owned(),
        };
        let seen = match self.tip_seen_ms {
            Some(ms) => format!("{} UTC", crate::format::clock(ms / 1000)),
            None => "—".to_owned(),
        };
        (peers, header, seen)
    }

    pub fn sidebar_badge(&self) -> Option<String> {
        self.running()
            .then(|| format!("{} of {STAGES}", self.stages_recorded()))
    }
}

/// 5xx, or 408. A missing status on a failed POST is a timeout and is handled
/// by the caller (`appended == Some(false)`).
fn post_error_status(status: u16) -> bool {
    status >= 500 || status == 408
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produce_says_what_it_is_doing_from_the_first_moment() {
        let mut view = ProduceView {
            phase: Phase::Starting,
            ..ProduceView::default()
        };
        assert_eq!(view.status_word(0), READING_THE_TIP);

        view.phase = Phase::Recording;
        view.seed_height = Some(970_726);
        view.seed_time_unix_s = Some(1_000);
        view.tip_height = Some(970_726);
        view.stages[0].legs = Some(0);
        let now = (1_000 + 16 * 60) * 1000;
        assert_eq!(view.status_word(now), "Seed height 970,726");
        assert_eq!(
            view.stage_line(),
            "Stage 0 of 18 · from block 970726 · tip 970726"
        );

        view.legs_total = 3;
        view.tip_height = Some(970_727);
        assert_eq!(
            view.status_word(now),
            "Stage 0 of 18 · from block 970726 · tip 970727"
        );
    }

    #[test]
    fn stop_then_restart_is_setup_then_a_running_hike() {
        let mut view = ProduceView {
            phase: Phase::Recording,
            output: Some(PathBuf::from("/tmp/hike/pcoc.cbor")),
            seed_height: Some(971_174),
            started_ms: Some(1),
            ..ProduceView::default()
        };
        assert!(view.running());
        view.apply(ProduceEvent::Stopped);
        assert_eq!(view.phase, Phase::Stopped);
        assert!(!view.running());
        view = ProduceView::default();
        assert_eq!(view.phase, Phase::Idle);
        view.apply(ProduceEvent::Started {
            output: PathBuf::from("/tmp/hike-2/pcoc.cbor"),
            public_key_hex: "aa".into(),
            at_ms: 2,
        });
        assert_eq!(view.phase, Phase::Starting);
        assert!(view.running());
        assert_eq!(view.status_word(0), READING_THE_TIP);
        view.apply(ProduceEvent::SeedBeacon {
            height: 971_174,
            natural_hash: [0x22; 32],
            header_time_unix_s: 1_700_000_000,
            at_ms: 3,
        });
        assert_eq!(view.status_word(0), "Seed height 971,174");
    }

    #[test]
    fn gossip_panel_names_peers_the_header_and_when_it_was_seen() {
        let mut view = ProduceView {
            phase: Phase::Starting,
            ..ProduceView::default()
        };
        let (peers, header, seen) = view.gossip_lines();
        assert_eq!(peers, "0 connected");
        assert_eq!(header, READING_THE_TIP);
        assert_eq!(seen, "—");
        view.apply(ProduceEvent::Gossip { peers: 1 });
        view.apply(ProduceEvent::Tip {
            height: 971_174,
            natural_hash: [0x11; 32],
            peers: None,
            header_seen_unix_ms: 1_700_000_000_000,
        });
        let (peers, header, seen) = view.gossip_lines();
        assert_eq!(peers, "1 connected");
        assert!(header.starts_with("971,174 · "), "{header}");
        assert!(header.contains(&crate::hex::prefix(&[0x11; 32])));
        assert!(seen.ends_with("UTC"), "{seen}");
    }

    /// AC5. One line per state. No header is the connecting sentence, with or
    /// without peers. A known tip never says it is still waiting for a header.
    #[test]
    fn status_bar_maps_each_gossip_and_bari_state() {
        let mut view = ProduceView {
            phase: Phase::Starting,
            ..ProduceView::default()
        };
        assert!(view.peers.is_none());
        let connecting = view.status_bar_line(0, false);
        assert_eq!(connecting, READING_THE_TIP);
        assert!(!connecting.contains('\n'));

        view.peers = Some(4);
        assert!(view.tip_height.is_none());
        assert_eq!(view.status_bar_line(0, false), READING_THE_TIP);

        view.tip_height = Some(970_867);
        view.tip_natural = Some([0xab; 32]);
        view.tip_seen_ms = Some(0);
        view.seed_height = Some(970_867);
        view.phase = Phase::Recording;
        view.bari = Bari::NotAskedYet;
        let tip_known = view.status_bar_line(60_000, false);
        assert!(tip_known.contains("Bari not asked yet"));
        assert!(tip_known.contains("970,867") || tip_known.contains("970867"));
        assert!(!tip_known.contains("Waiting for latest"));
        assert!(!tip_known.contains(READING_THE_TIP));
        assert!(!tip_known.contains('\n'));
        assert_eq!(tip_known.lines().count(), 1);

        view.bari = Bari::Reachable {
            last_post_status: Some(204),
        };
        let reachable = view.status_bar_line(60_000, false);
        assert!(reachable.contains("Bari reachable"));
        assert!(reachable.contains("204"));
        assert!(!reachable.contains("Waiting for latest"));
        assert!(!reachable.contains('\n'));
        assert_eq!(reachable.lines().count(), 1);
    }

    /// AC7. Every waypoint state has one chip, and the blue label does not
    /// claim anything about a pool this desktop cannot see.
    #[test]
    fn waypoint_chip_maps_every_state() {
        use AskOutcome::{Miss, Served};

        let mut view = ProduceView::default();
        assert_eq!(view.waypoint_chip(1), WaypointChip::NotDueYet);

        view.waypoints.insert(
            1,
            WaypointState {
                appended: Some(true),
                status: Some(204),
                ..WaypointState::default()
            },
        );
        assert_eq!(view.waypoint_chip(1), WaypointChip::Posted);

        view.waypoints.insert(
            5,
            WaypointState {
                appended: Some(true),
                status: Some(204),
                asks: vec![(2, Miss(None))],
                ..WaypointState::default()
            },
        );
        assert_eq!(view.waypoint_chip(5), WaypointChip::WaitingForAnchor);

        view.waypoints.insert(
            4,
            WaypointState {
                appended: Some(true),
                status: Some(204),
                asks: vec![(2, Served)],
                ..WaypointState::default()
            },
        );
        assert_eq!(view.waypoint_chip(4), WaypointChip::Anchored);

        view.waypoints.insert(
            9,
            WaypointState {
                appended: Some(true),
                status: Some(204),
                asks: vec![(2, Miss(None)), (3, Miss(None)), (4, Miss(None))],
                ..WaypointState::default()
            },
        );
        assert_eq!(view.waypoint_chip(9), WaypointChip::Skipped);

        view.waypoints.insert(
            2,
            WaypointState {
                appended: Some(false),
                status: None,
                ..WaypointState::default()
            },
        );
        assert_eq!(view.waypoint_chip(2), WaypointChip::PostError);
        view.waypoints.insert(
            3,
            WaypointState {
                appended: Some(true),
                status: Some(503),
                ..WaypointState::default()
            },
        );
        assert_eq!(view.waypoint_chip(3), WaypointChip::PostError);
        view.waypoints.insert(
            8,
            WaypointState {
                appended: Some(false),
                status: Some(408),
                ..WaypointState::default()
            },
        );
        assert_eq!(view.waypoint_chip(8), WaypointChip::PostError);

        let chips = [
            WaypointChip::NotDueYet,
            WaypointChip::Posted,
            WaypointChip::WaitingForAnchor,
            WaypointChip::Anchored,
            WaypointChip::Skipped,
            WaypointChip::PostError,
        ];
        let classes: Vec<_> = chips.iter().map(|c| c.css_class()).collect();
        assert_eq!(
            classes,
            [
                "wp-not-due",
                "wp-posted",
                "wp-waiting",
                "wp-anchored",
                "wp-skipped",
                "wp-post-error",
            ]
        );
        assert_eq!(
            WaypointChip::WaitingForAnchor.label(),
            "posted, waiting for anchor path"
        );
        for chip in chips {
            let tip = chip.tooltip();
            assert!(!tip.is_empty(), "{}", chip.label());
            assert!(!tip.to_ascii_lowercase().contains("mempool"), "{tip}");
            assert!(
                !chip.label().to_ascii_lowercase().contains("mempool"),
                "{}",
                chip.label()
            );
        }
    }
}
