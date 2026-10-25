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
    /// Cumulative leg rate frozen when the latest waypoint fired.
    /// `None` until that waypoint, and unchanged by later legs.
    /// `(legs/s to three places, span seconds, legs in the numerator)`.
    /// Display only. Nothing in the gate, verifier, object, or policy reads it.
    pub rate_so_far: Option<(String, u64, u64)>,
    pub tip_height: Option<u64>,
    pub tip_natural: Option<[u8; 32]>,
    pub tip_seen_ms: Option<u64>,
    /// Handshake-complete peers. `None` is unknown, not zero.
    pub peers: Option<u32>,
    /// Sockets in the pool, including handshakes still in flight. `None` until counted.
    pub sockets: Option<u32>,
    /// Addresses of handshake-complete peers. Empty when the pipes are unnamed.
    pub peer_addrs: Vec<String>,
    /// Gossip thread ended without Stop.
    pub feed_stopped: bool,
    /// Short panel phrase for `header_peers_low` or `header_source_stalled`.
    pub gossip_alert: Option<String>,
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
            rate_so_far: None,
            tip_height: None,
            tip_natural: None,
            tip_seen_ms: None,
            peers: None,
            sockets: None,
            peer_addrs: Vec::new(),
            feed_stopped: false,
            gossip_alert: None,
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

/// Handshake count not published yet, and no tip has arrived.
pub const GOSSIP_CONNECTING: &str = "connecting…";
/// Tips are moving but this process has not published a handshake count.
pub const GOSSIP_PEERS_UNKNOWN: &str = "peer count unknown";
/// Fewer than two handshake-complete peers. SPEC §9 `HEADER_PEERS_MIN`.
pub const GOSSIP_NEED_TWO: &str = "Need 2 peers · waiting";
/// The header source reported a stall that is not a low peer count.
pub const GOSSIP_TIP_STALLED: &str = "Tip feed stalled";
/// The gossip thread ended. Sockets are gone. The window may still be open.
pub const GOSSIP_TIP_STOPPED: &str = "Tip feed stopped";
pub const GOSSIP_TIP_LIVE: &str = "Tip feed live";
pub const GOSSIP_READING: &str = "Connected · reading the tip";
pub const GOSSIP_NO_PEERS: &str = "No peers · reconnecting";
pub const GOSSIP_AGREE: &str = "Tip advances when 2 peers agree.";

/// Rows of the live Gossip panel. Tones are GTK css classes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GossipPanel {
    pub status: String,
    pub status_tone: &'static str,
    pub peers: String,
    pub from: String,
    pub addrs: String,
    pub header: String,
    pub seen: String,
    pub note: String,
}

/// Where this hike dials. Empty or absent means the DNS seed list.
pub fn gossip_from(named_peer: Option<&str>) -> String {
    match named_peer.map(str::trim).filter(|peer| !peer.is_empty()) {
        Some(peer) => format!("Named override {peer}, then DNS seeds"),
        None => "DNS seeds".to_owned(),
    }
}

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
                    rate_so_far: None,
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
                let advanced = self.tip_height.is_none_or(|h| height >= h);
                if advanced {
                    self.tip_height = Some(height);
                    self.tip_natural = Some(natural_hash);
                    self.tip_seen_ms = Some(header_seen_unix_ms);
                    if self.gossip_alert.as_deref() == Some(GOSSIP_TIP_STALLED) {
                        self.gossip_alert = None;
                    }
                }
                if peers.is_some() {
                    self.peers = peers;
                }
            }
            ProduceEvent::Gossip {
                peers,
                sockets,
                addrs,
            } => {
                self.peers = Some(peers);
                self.sockets = Some(sockets);
                self.peer_addrs = addrs;
                if peers >= 2 && self.gossip_alert.as_deref() == Some(GOSSIP_NEED_TWO) {
                    self.gossip_alert = None;
                }
            }
            ProduceEvent::TipFeedStopped => {
                self.feed_stopped = true;
                self.peers = Some(0);
                self.sockets = Some(0);
                self.peer_addrs.clear();
                self.gossip_alert = None;
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
                // The produce screen freezes its cumulative rate here.
                // Legs recorded between waypoints do not move the figure.
                self.rate_so_far = cumulative_leg_rate(
                    self.legs_total,
                    self.first_leg_ms,
                    self.last_leg_ms,
                );
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
                match line.as_str() {
                    "header_peers_low" if self.peers.is_none_or(|n| n < 2) => {
                        self.gossip_alert = Some(GOSSIP_NEED_TWO.to_owned());
                    }
                    "header_source_stalled" => {
                        self.gossip_alert = Some(GOSSIP_TIP_STALLED.to_owned());
                    }
                    _ => {}
                }
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

    /// Cumulative leg rate frozen at the latest waypoint, in legs/s.
    ///
    /// Total legs up to that waypoint, divided by (time of the latest of
    /// those legs − time of the first leg). Terrain fill is before the
    /// first leg and is not in it. Zero legs, one leg, and a zero span
    /// are `None` (the screen shows nothing, or an em dash). The figure
    /// does not change between waypoints. Display only.
    pub fn rate(&self) -> Option<(String, u64)> {
        if !matches!(
            self.phase,
            Phase::Recording | Phase::Settling | Phase::Written
        ) {
            return None;
        }
        self.rate_so_far
            .as_ref()
            .map(|(rate, span_s, _)| (rate.clone(), *span_s))
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
        match self.peers {
            Some(peers) => gossip.push_str(&format!(" · {peers} peers")),
            None if self.running() && self.tip_height.is_some() => {
                gossip.push_str(" · peers unknown");
            }
            None => {}
        }
        if self.feed_stopped {
            gossip.push_str(" · tip feed stopped");
        } else if let Some(alert) = &self.gossip_alert {
            gossip.push_str(" · ");
            gossip.push_str(alert);
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

    /// What the Network / Gossip panel draws for one live hike.
    ///
    /// `named_peer` is the override Start hike dialed. Empty means DNS seeds.
    /// A running hike keeps the value it started with.
    pub fn gossip_panel(&self, named_peer: Option<&str>) -> GossipPanel {
        let (status, status_tone) = self.gossip_status();
        let peers = self.peer_count_line();
        let from = gossip_from(named_peer);
        let addrs = self.peer_addrs.join("\n");
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
        let note = self.gossip_note();
        GossipPanel {
            status,
            status_tone,
            peers,
            from,
            addrs,
            header,
            seen,
            note,
        }
    }

    fn peer_count_line(&self) -> String {
        match self.peers {
            Some(0) if self.zero_is_real() => "0 connected".to_owned(),
            Some(0) if self.running() => GOSSIP_CONNECTING.to_owned(),
            Some(0) => "0 connected".to_owned(),
            Some(n) => format!("{n} connected"),
            None if self.feed_stopped || (self.running() && self.tip_height.is_some()) => {
                GOSSIP_PEERS_UNKNOWN.to_owned()
            }
            None if self.running() => GOSSIP_CONNECTING.to_owned(),
            None => "—".to_owned(),
        }
    }

    /// Zero handshakes is a settled count once a tip has been seen, the feed
    /// has stopped, an alert has fired, or the pool itself is empty.
    /// Sockets still in `version`/`verack` stay "connecting…".
    fn zero_is_real(&self) -> bool {
        if self.feed_stopped || self.tip_height.is_some() || self.gossip_alert.is_some() {
            return true;
        }
        self.sockets == Some(0)
    }

    fn gossip_status(&self) -> (String, &'static str) {
        if self.feed_stopped {
            return (GOSSIP_TIP_STOPPED.to_owned(), "magenta");
        }
        if let Some(alert) = &self.gossip_alert {
            let tone = if alert == GOSSIP_TIP_STALLED {
                "magenta"
            } else {
                "amber"
            };
            return (alert.clone(), tone);
        }
        if !self.running() {
            return ("—".to_owned(), "muted");
        }
        match self.peers {
            Some(n) if n >= 2 && self.tip_height.is_some() => (GOSSIP_TIP_LIVE.to_owned(), "teal"),
            Some(n) if n >= 2 => (GOSSIP_READING.to_owned(), "teal"),
            Some(n) if n > 0 => (GOSSIP_NEED_TWO.to_owned(), "amber"),
            Some(0) if self.zero_is_real() => (GOSSIP_NO_PEERS.to_owned(), "amber"),
            None if self.tip_height.is_some() => (GOSSIP_PEERS_UNKNOWN.to_owned(), "amber"),
            _ => ("Connecting…".to_owned(), "muted"),
        }
    }

    fn gossip_note(&self) -> String {
        if !self.running() || self.feed_stopped {
            return String::new();
        }
        if self.peers.is_some_and(|n| n < 2) && (self.tip_height.is_some() || self.zero_is_real()) {
            return GOSSIP_AGREE.to_owned();
        }
        if self.peers.is_none() && self.tip_height.is_some() {
            return "Peer count is not in yet. A moving tip is not a peer count.".to_owned();
        }
        String::new()
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

/// Total legs so far ÷ (latest leg time − first leg time), in whole seconds.
/// `None` for no legs, one leg, or a span that rounds to zero seconds.
/// The third value is the leg count in the numerator, for the label.
fn cumulative_leg_rate(
    legs: u64,
    first_leg_ms: Option<u64>,
    last_leg_ms: Option<u64>,
) -> Option<(String, u64, u64)> {
    let span_s = last_leg_ms?.saturating_sub(first_leg_ms?) / 1000;
    let rate = three_places(legs, span_s)?;
    Some((rate, span_s, legs))
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
        let panel = view.gossip_panel(None);
        assert_eq!(panel.peers, GOSSIP_CONNECTING);
        assert_eq!(panel.status, "Connecting…");
        assert_eq!(panel.header, READING_THE_TIP);
        assert_eq!(panel.seen, "—");
        assert_eq!(panel.from, "DNS seeds");
        assert_ne!(panel.peers, "0 connected");

        view.apply(ProduceEvent::Gossip {
            peers: 0,
            sockets: 2,
            addrs: Vec::new(),
        });
        let panel = view.gossip_panel(Some("203.0.113.10:8333"));
        assert_eq!(panel.peers, GOSSIP_CONNECTING);
        assert_eq!(
            panel.from,
            "Named override 203.0.113.10:8333, then DNS seeds"
        );

        view.apply(ProduceEvent::Gossip {
            peers: 1,
            sockets: 2,
            addrs: vec!["203.0.113.10:8333".into()],
        });
        view.apply(ProduceEvent::Tip {
            height: 971_174,
            natural_hash: [0x11; 32],
            peers: None,
            header_seen_unix_ms: 1_700_000_000_000,
        });
        let panel = view.gossip_panel(Some("203.0.113.10:8333"));
        assert_eq!(panel.peers, "1 connected");
        assert_eq!(panel.status, GOSSIP_NEED_TWO);
        assert_eq!(panel.addrs, "203.0.113.10:8333");
        assert_eq!(panel.note, GOSSIP_AGREE);
        assert!(panel.header.starts_with("971,174 · "), "{}", panel.header);
        assert!(panel.header.contains(&crate::hex::prefix(&[0x11; 32])));
        assert!(panel.seen.ends_with("UTC"), "{}", panel.seen);

        view.apply(ProduceEvent::Gossip {
            peers: 2,
            sockets: 2,
            addrs: vec!["203.0.113.10:8333".into(), "203.0.113.11:8333".into()],
        });
        let panel = view.gossip_panel(Some("203.0.113.10:8333"));
        assert_eq!(panel.peers, "2 connected");
        assert_eq!(panel.status, GOSSIP_TIP_LIVE);
        assert_eq!(panel.status_tone, "teal");
        assert!(panel.note.is_empty());
    }

    #[test]
    fn unknown_peer_count_is_not_zero_while_tips_move() {
        let mut view = ProduceView {
            phase: Phase::Recording,
            tip_height: Some(972_907),
            tip_natural: Some([0xab; 32]),
            tip_seen_ms: Some(1_700_000_000_000),
            peers: None,
            ..ProduceView::default()
        };
        let panel = view.gossip_panel(None);
        assert_eq!(panel.peers, GOSSIP_PEERS_UNKNOWN);
        assert_eq!(panel.status, GOSSIP_PEERS_UNKNOWN);
        assert_ne!(panel.peers, "0 connected");
        assert!(panel.note.contains("not a peer count"));
        let (gossip, _) = view.status_line(0, false);
        assert!(gossip.contains("peers unknown"), "{gossip}");
        assert!(!gossip.contains("0 peers"), "{gossip}");

        view.apply(ProduceEvent::Gossip {
            peers: 0,
            sockets: 0,
            addrs: Vec::new(),
        });
        let panel = view.gossip_panel(None);
        assert_eq!(panel.peers, "0 connected");
        assert_eq!(panel.status, GOSSIP_NO_PEERS);
    }

    #[test]
    fn stalled_feed_is_distinct_from_an_unknown_count() {
        let mut view = ProduceView {
            phase: Phase::Recording,
            tip_height: Some(972_906),
            tip_natural: Some([0x11; 32]),
            tip_seen_ms: Some(1),
            peers: Some(2),
            sockets: Some(2),
            ..ProduceView::default()
        };
        view.apply(ProduceEvent::Log("header_peers_low".into()));
        assert!(view.gossip_alert.is_none(), "two peers is not a low count");
        view.peers = Some(1);
        view.apply(ProduceEvent::Log("header_peers_low".into()));
        assert_eq!(view.gossip_panel(None).status, GOSSIP_NEED_TWO);
        view.apply(ProduceEvent::Log("header_source_stalled".into()));
        assert_eq!(view.gossip_panel(None).status, GOSSIP_TIP_STALLED);
        assert_eq!(view.gossip_panel(None).status_tone, "magenta");

        view.apply(ProduceEvent::Tip {
            height: 972_907,
            natural_hash: [0x22; 32],
            peers: Some(2),
            header_seen_unix_ms: 2,
        });
        assert_eq!(view.gossip_panel(None).status, GOSSIP_TIP_LIVE);

        view.apply(ProduceEvent::TipFeedStopped);
        let panel = view.gossip_panel(None);
        assert_eq!(panel.status, GOSSIP_TIP_STOPPED);
        assert_eq!(panel.peers, "0 connected");
        assert_eq!(panel.status_tone, "magenta");
        let (gossip, _) = view.status_line(0, false);
        assert!(gossip.contains("tip feed stopped"), "{gossip}");
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

    fn leg(stage: usize, j: u64, at_ms: u64) -> ProduceEvent {
        ProduceEvent::LegRecorded {
            stage,
            j,
            stored_in_stage: j + 1,
            at_ms,
        }
    }

    fn posted(g: u64) -> ProduceEvent {
        ProduceEvent::WaypointPosted {
            g,
            appended: true,
            status: Some(204),
        }
    }

    /// The rate so far is total legs ÷ (latest leg − first leg), frozen
    /// when a waypoint fires. Legs between waypoints do not move it.
    /// No legs and one leg show nothing.
    #[test]
    fn cumulative_rate_updates_only_when_a_waypoint_fires() {
        let mut view = ProduceView {
            phase: Phase::Recording,
            ..ProduceView::default()
        };
        assert_eq!(view.rate(), None);
        assert_eq!(cumulative_leg_rate(0, None, None), None);

        view.apply(leg(0, 0, 1_000));
        assert_eq!(view.rate(), None, "one leg has no span");
        view.apply(posted(1));
        assert_eq!(view.rate(), None, "a waypoint of one leg still has no span");

        view.apply(leg(0, 1, 5_000));
        assert_eq!(view.rate(), None, "a leg between waypoints does not show a rate");
        view.apply(posted(2));
        // 2 legs ÷ (5_000 − 1_000) ms = 2 / 4 s.
        assert_eq!(view.rate(), Some(("0.500".into(), 4)));

        view.apply(leg(0, 2, 9_000));
        view.apply(leg(0, 3, 13_000));
        assert_eq!(
            view.rate(),
            Some(("0.500".into(), 4)),
            "the figure stays at the previous waypoint"
        );
        view.apply(posted(3));
        // 4 legs ÷ (13_000 − 1_000) ms = 4 / 12 s.
        assert_eq!(view.rate(), Some(("0.333".into(), 12)));

        view.phase = Phase::Settling;
        assert_eq!(view.rate(), Some(("0.333".into(), 12)));
        view.phase = Phase::Written;
        assert_eq!(view.rate(), Some(("0.333".into(), 12)));
        view.phase = Phase::Stopped;
        assert_eq!(view.rate(), None);

        assert_eq!(cumulative_leg_rate(3, Some(1_000), Some(1_400)), None);
        assert_eq!(
            cumulative_leg_rate(3, Some(0), Some(2_000)),
            Some(("1.500".into(), 2, 3))
        );
    }
}
