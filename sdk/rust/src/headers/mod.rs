//! SPEC §9 header source. P9.16, P9.17, Rule A6, F8.
//!
//! The walk calls [`HeaderSource::observe`] and [`HeaderSource::fee_header`].
//! Both return at once with the header already held. They do not dial, read a
//! socket, or take a lock that those operations hold. [`HeaderDriver::poll`]
//! does the peer work and publishes the tip afterward.
//!
//! Hand-off for the producer walk (P2): this module defines `HeaderSource`.
//! If P2 defined the same idea under different names, the CoS reconciles them.
//! Alerts are drained from [`HeaderSourceHandle::drain_alerts`], not from the
//! trait. There is no shared peer cache and no default `--gossip-peer`.

mod chain;
mod wire;

pub use chain::{
    header_hash, header_meets_own_target, header_nbits, header_prev, header_time, Appended,
    ConsensusParams, HeaderChain, HeaderFault, BITCOIN_DIFF1_NBITS, MAINNET_MAGIC, REGTEST_MAGIC,
    RETARGET_INTERVAL, SYNTH_NBITS, TARGET_TIMESPAN,
};
pub use wire::{
    decode_addr, decode_getheaders, decode_headers, decode_inv, decode_ping, drain_messages,
    encode_addr, encode_headers, encode_inv, encode_message, encode_version, MSG_BLOCK,
};

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chain::HeaderChain as Chain;
use chain::HeaderFault as Fault;

/// SPEC §9 P9.16. Not a lever and not an SDK option.
pub const HEADER_PEERS_MIN: usize = 2;
/// SPEC §9 P9.16. Not a lever and not an SDK option.
pub const HEADER_PEER_SILENCE_SECONDS: u64 = 120;

/// Simultaneous outbound dials. SDK constant, not a SPEC lever.
pub const HEADER_DIAL_CONCURRENCY: usize = 8;
/// First gap after a failed dial wave, in milliseconds. SDK constant, not a SPEC lever.
pub const HEADER_DIAL_BACKOFF_INITIAL_MS: u64 = 1_000;
/// Cap on the dial gap. SDK constant, not a SPEC lever.
pub const HEADER_DIAL_BACKOFF_MAX_MS: u64 = 60_000;
/// How often a live session is pinged. A pong is not a header announcement.
const HEADER_PING_INTERVAL_MS: u64 = 30_000;
const HEADER_ADDRBOOK_CAP: usize = 1_024;

/// SPEC §9. Bitcoin Core mainnet DNS seeds, in Core's order. Port 8333.
pub const MAINNET_DNS_SEEDS: &[&str] = &[
    "dnsseed.bluematt.me",
    "seed.bitcoin.jonasschnelli.ch",
    "seed.btc.petertodd.net",
    "seed.bitcoin.sprovoost.nl",
    "dnsseed.emzy.de",
    "seed.bitcoin.wiz.biz",
    "seed.mainnet.achownodes.xyz",
];

/// Which tip sequence the source follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    /// Bitcoin mainnet. Retarget at 2016. Most-work tip.
    Live,
    /// Synth headers, fixed `nBits` `0x207fffff`, regtest magic. No DNS seed list.
    Test,
}

impl Plane {
    pub fn params(self) -> ConsensusParams {
        match self {
            Plane::Live => ConsensusParams::mainnet(),
            Plane::Test => ConsensusParams::regtest(),
        }
    }
}

/// A gossip endpoint. No process shares another's book.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerAddr {
    pub host: String,
    pub port: u16,
}

impl PeerAddr {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }
}

/// Starting list. Named peers replace the list. An empty name list on the live
/// plane is the DNS seed copy. The test plane does not use that list. There is
/// no single default gossip peer.
pub fn initial_addresses(plane: Plane, named: &[PeerAddr]) -> Vec<PeerAddr> {
    if !named.is_empty() {
        return named.to_vec();
    }
    match plane {
        Plane::Live => mainnet_dns_seeds(),
        Plane::Test => Vec::new(),
    }
}

pub fn mainnet_dns_seeds() -> Vec<PeerAddr> {
    MAINNET_DNS_SEEDS
        .iter()
        .map(|host| PeerAddr::new(*host, 8333))
        .collect()
}

/// Resolve one DNS seed. Not called by tests and not called from [`HeaderSource::observe`].
/// The live producer invokes this off the walk before constructing a driver.
pub fn resolve_dns_seed(host: &str, port: u16) -> std::io::Result<Vec<PeerAddr>> {
    use std::net::ToSocketAddrs;
    let mut out = Vec::new();
    for addr in (host, port).to_socket_addrs()? {
        out.push(PeerAddr::new(addr.ip().to_string(), addr.port()));
    }
    Ok(out)
}

/// Monotonic milliseconds. The header source does not sleep on this clock.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

pub struct SystemClock {
    start: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

/// Test clock. Advancing it does not block the walk.
#[derive(Clone, Debug)]
pub struct MockClock {
    now_ms: Arc<std::sync::atomic::AtomicU64>,
}

impl MockClock {
    pub fn new(start_ms: u64) -> Self {
        Self {
            now_ms: Arc::new(std::sync::atomic::AtomicU64::new(start_ms)),
        }
    }

    pub fn set(&self, ms: u64) {
        self.now_ms.store(ms, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn advance(&self, ms: u64) {
        self.now_ms
            .fetch_add(ms, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Clock for MockClock {
    fn now_ms(&self) -> u64 {
        self.now_ms.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// What [`HeaderSource::observe`] returns. `natural` is byte 0 of the header hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedTip {
    pub natural: [u8; 32],
    pub height: u64,
}

/// SPEC §13.5 alert the header source asks the producer to append.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceAlert {
    pub alert: String,
}

impl SourceAlert {
    /// Compact JSON, keys `event`, `alert`, `stage`. Height is omitted.
    pub fn event_json(&self, stage: u64) -> String {
        format!(
            "{{\"event\":\"hike.produce.alert\",\"alert\":\"{}\",\"stage\":{}}}",
            self.alert, stage
        )
    }
}

/// SPEC §9 P9.11 and the fee-header read. Both return at once.
pub trait HeaderSource {
    fn observe(&self) -> Option<ObservedTip>;
    fn fee_header(&self) -> Option<[u8; 80]>;
}

/// Cloneable view of the tip and alerts. Safe to call on the walk thread.
#[derive(Clone)]
pub struct HeaderSourceHandle {
    shared: Arc<Shared>,
}

impl HeaderSource for HeaderSourceHandle {
    fn observe(&self) -> Option<ObservedTip> {
        let tip = self.shared.tip.lock().unwrap_or_else(|e| e.into_inner());
        tip.as_ref().map(|t| ObservedTip {
            natural: t.natural,
            height: t.height,
        })
    }

    fn fee_header(&self) -> Option<[u8; 80]> {
        let tip = self.shared.tip.lock().unwrap_or_else(|e| e.into_inner());
        tip.as_ref().map(|t| t.header)
    }
}

impl HeaderSourceHandle {
    pub fn drain_alerts(&self) -> Vec<SourceAlert> {
        let mut alerts = self.shared.alerts.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *alerts)
    }
}

struct StoredTip {
    natural: [u8; 32],
    height: u64,
    header: [u8; 80],
}

struct Shared {
    tip: Mutex<Option<StoredTip>>,
    alerts: Mutex<Vec<SourceAlert>>,
}

/// Per-process address book and dial seed. `shuffle` is true in production so
/// co-located producers do not dial in the same order. No cache path.
#[derive(Clone, Debug)]
pub struct HeaderSourceConfig {
    pub params: ConsensusParams,
    pub addresses: Vec<PeerAddr>,
    pub dial_seed: u64,
    pub shuffle: bool,
}

impl HeaderSourceConfig {
    pub fn new(params: ConsensusParams, addresses: Vec<PeerAddr>, dial_seed: u64) -> Self {
        Self {
            params,
            addresses,
            dial_seed,
            shuffle: true,
        }
    }
}

pub trait Conn {
    fn try_read(&mut self, buf: &mut [u8]) -> ReadOut;
    fn write_all(&mut self, bytes: &[u8]);
}

pub enum ReadOut {
    Empty,
    Closed,
    N(usize),
}

pub trait Net {
    type Conn: Conn;
    fn start_dial(&mut self, addr: &PeerAddr) -> u64;
    fn poll_dial(&mut self, id: u64) -> PollDial<Self::Conn>;
}

pub enum PollDial<C> {
    Pending,
    Failed,
    Connected(C),
}

struct Session<C> {
    id: u64,
    addr: PeerAddr,
    conn: C,
    buf: Vec<u8>,
    seen_version: bool,
    seen_verack: bool,
    sent_verack: bool,
    sent_getheaders: bool,
    announced: HashSet<[u8; 32]>,
    connected_at: u64,
    last_ping_at: u64,
    pongs: u32,
}

struct Inflight {
    id: u64,
    addr: PeerAddr,
}

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

enum SessOp {
    Keep,
    Dead,
    Ban(Fault),
}

/// One producer's header source. The address book, dial order, and sessions
/// belong to this value alone.
pub struct HeaderDriver<N: Net> {
    net: N,
    clock: Arc<dyn Clock>,
    chain: Chain,
    shared: Arc<Shared>,
    rng: Rng,
    order: Vec<PeerAddr>,
    cursor: usize,
    sessions: Vec<Session<N::Conn>>,
    in_flight: Vec<Inflight>,
    next_session: u64,
    next_not_before: u64,
    backoff_ms: u64,
    low_alerted: bool,
    banned: HashMap<PeerAddr, Fault>,
    avoid: HashSet<PeerAddr>,
    first_announce: HashMap<[u8; 32], u64>,
}

impl<N: Net> HeaderDriver<N> {
    pub fn new(config: HeaderSourceConfig, clock: Arc<dyn Clock>, net: N) -> Result<Self, Fault> {
        let chain = Chain::open(config.params)?;
        let mut order = config.addresses;
        if config.shuffle {
            shuffle(&mut Rng(config.dial_seed), &mut order);
        }
        let shared = Arc::new(Shared {
            tip: Mutex::new(None),
            alerts: Mutex::new(Vec::new()),
        });
        let driver = Self {
            net,
            clock,
            chain,
            shared,
            rng: Rng(config.dial_seed ^ 0xA5A5_A5A5_A5A5_A5A5),
            order,
            cursor: 0,
            sessions: Vec::new(),
            in_flight: Vec::new(),
            next_session: 1,
            next_not_before: 0,
            backoff_ms: HEADER_DIAL_BACKOFF_INITIAL_MS,
            low_alerted: false,
            banned: HashMap::new(),
            avoid: HashSet::new(),
            first_announce: HashMap::new(),
        };
        driver.publish_tip();
        Ok(driver)
    }

    pub fn handle(&self) -> HeaderSourceHandle {
        HeaderSourceHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    pub fn dial_order(&self) -> &[PeerAddr] {
        &self.order
    }

    pub fn address_book(&self) -> Vec<PeerAddr> {
        self.order.clone()
    }

    pub fn connected(&self) -> Vec<PeerAddr> {
        let mut v: Vec<PeerAddr> = self.sessions.iter().map(|s| s.addr.clone()).collect();
        v.sort_by_key(|a| self.rank(a));
        v
    }

    pub fn session_ids(&self) -> Vec<(u64, PeerAddr)> {
        let mut v: Vec<(u64, PeerAddr)> = self
            .sessions
            .iter()
            .map(|s| (s.id, s.addr.clone()))
            .collect();
        v.sort_by_key(|(_, a)| self.rank(a));
        v
    }

    pub fn pongs_from(&self, addr: &PeerAddr) -> u32 {
        self.sessions
            .iter()
            .find(|s| &s.addr == addr)
            .map(|s| s.pongs)
            .unwrap_or(0)
    }

    pub fn tip_height(&self) -> u64 {
        self.chain.tip_height()
    }

    pub fn tip_hash(&self) -> [u8; 32] {
        self.chain.tip_hash()
    }

    pub fn contains(&self, hash: &[u8; 32]) -> bool {
        self.chain.contains(hash)
    }

    pub fn is_bad(&self, addr: &PeerAddr) -> bool {
        self.banned.contains_key(addr)
    }

    pub fn ban_fault(&self, addr: &PeerAddr) -> Option<&Fault> {
        self.banned.get(addr)
    }

    /// Peer maintenance. Does not run inside [`HeaderSource::observe`].
    pub fn poll(&mut self) {
        self.note_peer_count();
        self.harvest_dials();
        self.pump_reads();
        self.maybe_ping();
        self.check_silence();
        self.start_dials_if_needed();
        self.note_peer_count();
        self.publish_tip();
    }

    fn now(&self) -> u64 {
        self.clock.now_ms()
    }

    fn rank(&self, addr: &PeerAddr) -> usize {
        self.order
            .iter()
            .position(|a| a == addr)
            .unwrap_or(usize::MAX)
    }

    fn publish_tip(&self) {
        let stored = StoredTip {
            natural: self.chain.tip_hash(),
            height: self.chain.tip_height(),
            header: self.chain.tip_header(),
        };
        let mut tip = self.shared.tip.lock().unwrap_or_else(|e| e.into_inner());
        *tip = Some(stored);
    }

    fn push_alert(&self, name: &str) {
        let mut alerts = self.shared.alerts.lock().unwrap_or_else(|e| e.into_inner());
        alerts.push(SourceAlert {
            alert: name.to_string(),
        });
    }

    fn note_peer_count(&mut self) {
        if self.sessions.len() < HEADER_PEERS_MIN {
            if !self.low_alerted {
                self.push_alert("header_peers_low");
                self.low_alerted = true;
            }
        } else {
            self.low_alerted = false;
        }
    }

    fn harvest_dials(&mut self) {
        let now = self.now();
        let mut failed = 0u32;
        let mut still = Vec::new();
        for dial in std::mem::take(&mut self.in_flight) {
            match self.net.poll_dial(dial.id) {
                PollDial::Pending => still.push(dial),
                PollDial::Failed => failed += 1,
                PollDial::Connected(conn) => self.attach(dial.addr, conn),
            }
        }
        self.in_flight = still;
        let below = self.sessions.len() < HEADER_PEERS_MIN;
        if failed > 0 && self.in_flight.is_empty() && below {
            self.next_not_before = now.saturating_add(self.backoff_ms);
            self.backoff_ms = (self.backoff_ms.saturating_mul(2)).min(HEADER_DIAL_BACKOFF_MAX_MS);
        }
        if !below && self.in_flight.is_empty() {
            self.drop_extra();
            self.backoff_ms = HEADER_DIAL_BACKOFF_INITIAL_MS;
            self.next_not_before = now;
            self.avoid.clear();
        }
    }

    fn start_dials_if_needed(&mut self) {
        if self.sessions.len() >= HEADER_PEERS_MIN {
            return;
        }
        if !self.in_flight.is_empty() {
            return;
        }
        if self.now() < self.next_not_before {
            return;
        }
        let mut spins = 0;
        while self.in_flight.len() < HEADER_DIAL_CONCURRENCY && spins < 64 {
            spins += 1;
            let Some(addr) = self.next_addr() else {
                break;
            };
            let id = self.net.start_dial(&addr);
            self.in_flight.push(Inflight { id, addr });
        }
    }

    fn next_addr(&mut self) -> Option<PeerAddr> {
        let n = self.order.len();
        if n == 0 {
            return None;
        }
        let mut skipped = None;
        for _ in 0..n {
            let addr = self.order[self.cursor % n].clone();
            self.cursor += 1;
            if self.banned.contains_key(&addr) {
                continue;
            }
            if self.sessions.iter().any(|s| s.addr == addr) {
                continue;
            }
            if self.in_flight.iter().any(|d| d.addr == addr) {
                continue;
            }
            if self.avoid.contains(&addr) {
                skipped = Some(addr);
                continue;
            }
            return Some(addr);
        }
        // Every other address is connected, in flight, or banned. An avoided
        // peer is a last resort, and not while a fresh dial is already out.
        if self.in_flight.is_empty() {
            if let Some(addr) = skipped {
                self.avoid.remove(&addr);
                return Some(addr);
            }
        }
        None
    }

    fn attach(&mut self, addr: PeerAddr, conn: N::Conn) {
        let id = self.next_session;
        self.next_session += 1;
        let now = self.now();
        let nonce = self.rng.next_u64();
        self.sessions.push(Session {
            id,
            addr,
            conn,
            buf: Vec::new(),
            seen_version: false,
            seen_verack: false,
            sent_verack: false,
            sent_getheaders: false,
            announced: HashSet::new(),
            connected_at: now,
            last_ping_at: now,
            pongs: 0,
        });
        let idx = self.sessions.len() - 1;
        let height = i32::try_from(self.chain.tip_height()).unwrap_or(i32::MAX);
        let payload = wire::encode_version(height, nonce, (now / 1000) as i64);
        self.send(idx, "version", &payload);
    }

    fn drop_extra(&mut self) {
        if self.sessions.len() <= HEADER_PEERS_MIN {
            return;
        }
        let mut ranked: Vec<(u8, usize, u64)> = self
            .sessions
            .iter()
            .map(|s| {
                let avoided = u8::from(self.avoid.contains(&s.addr));
                (avoided, self.rank(&s.addr), s.id)
            })
            .collect();
        ranked.sort_by_key(|(avoided, rank, id)| (*avoided, *rank, *id));
        let keep: HashSet<u64> = ranked
            .iter()
            .take(HEADER_PEERS_MIN)
            .map(|(_, _, id)| *id)
            .collect();
        let drop: Vec<u64> = self
            .sessions
            .iter()
            .filter(|s| !keep.contains(&s.id))
            .map(|s| s.id)
            .collect();
        for id in drop {
            self.remove_session(id, None, false);
        }
    }

    fn pump_reads(&mut self) {
        let mut doomed = Vec::new();
        for idx in 0..self.sessions.len() {
            let id = self.sessions[idx].id;
            match self.read_one(idx) {
                SessOp::Keep => {}
                SessOp::Dead => doomed.push((id, None)),
                SessOp::Ban(fault) => doomed.push((id, Some(fault))),
            }
        }
        for (id, fault) in doomed {
            let avoid = fault.is_none();
            self.remove_session(id, fault, avoid);
        }
    }

    fn read_one(&mut self, idx: usize) -> SessOp {
        let mut tmp = [0u8; 8192];
        let mut closed = false;
        loop {
            match self.sessions[idx].conn.try_read(&mut tmp) {
                ReadOut::Empty => break,
                ReadOut::Closed => {
                    closed = true;
                    break;
                }
                ReadOut::N(n) => self.sessions[idx].buf.extend_from_slice(&tmp[..n]),
            }
        }
        let buf = std::mem::take(&mut self.sessions[idx].buf);
        let magic = self.chain.params().magic;
        let (msgs, rest) = match wire::drain_messages(magic, buf) {
            Ok(pair) => pair,
            Err(()) => return SessOp::Ban(Fault::Wire),
        };
        self.sessions[idx].buf = rest;
        for msg in msgs {
            let op = self.handle_msg(idx, &msg.cmd, &msg.payload);
            if !matches!(op, SessOp::Keep) {
                return op;
            }
        }
        if closed {
            SessOp::Dead
        } else {
            SessOp::Keep
        }
    }

    fn handle_msg(&mut self, idx: usize, cmd: &str, payload: &[u8]) -> SessOp {
        match cmd {
            "version" => {
                self.sessions[idx].seen_version = true;
                if !self.sessions[idx].sent_verack {
                    self.sessions[idx].sent_verack = true;
                    self.send(idx, "verack", &[]);
                }
                SessOp::Keep
            }
            "verack" => {
                self.sessions[idx].seen_verack = true;
                if !self.sessions[idx].sent_getheaders {
                    self.sessions[idx].sent_getheaders = true;
                    self.send_getheaders(idx, [0u8; 32]);
                    self.send(idx, "sendheaders", &[]);
                    self.send(idx, "getaddr", &[]);
                }
                SessOp::Keep
            }
            "ping" => {
                if let Some(nonce) = wire::decode_ping(payload) {
                    let pong = wire::encode_ping(nonce);
                    self.send(idx, "pong", &pong);
                }
                SessOp::Keep
            }
            "pong" => {
                self.sessions[idx].pongs = self.sessions[idx].pongs.saturating_add(1);
                SessOp::Keep
            }
            "headers" => self.handle_headers(idx, payload),
            "inv" => self.handle_inv(idx, payload),
            "addr" => {
                if self.handle_addr(payload).is_err() {
                    return SessOp::Ban(Fault::Wire);
                }
                SessOp::Keep
            }
            _ => SessOp::Keep,
        }
    }

    fn handle_headers(&mut self, idx: usize, payload: &[u8]) -> SessOp {
        let parsed = match wire::decode_headers(payload) {
            Ok(headers) => headers,
            Err(()) => return SessOp::Ban(Fault::Wire),
        };
        for header in parsed {
            let hash = header_hash(&header);
            match self.chain.append(header) {
                Ok(_) => self.note_announce(idx, hash),
                Err(Fault::UnknownParent) => break,
                Err(fault) => return SessOp::Ban(fault),
            }
        }
        SessOp::Keep
    }

    fn handle_inv(&mut self, idx: usize, payload: &[u8]) -> SessOp {
        let items = match wire::decode_inv(payload) {
            Ok(items) => items,
            Err(()) => return SessOp::Ban(Fault::Wire),
        };
        let mut stop = None;
        for (kind, hash) in items {
            if wire::is_block_inv(kind) && !self.chain.contains(&hash) {
                stop = Some(hash);
            }
        }
        if let Some(stop) = stop {
            self.send_getheaders(idx, stop);
        }
        SessOp::Keep
    }

    fn handle_addr(&mut self, payload: &[u8]) -> Result<(), ()> {
        let entries = wire::decode_addr(payload)?;
        for (ip, port) in entries {
            if self.order.len() >= HEADER_ADDRBOOK_CAP {
                break;
            }
            let host = host_of_ip(&ip);
            let addr = PeerAddr::new(host, port);
            if self.banned.contains_key(&addr) || self.order.iter().any(|a| a == &addr) {
                continue;
            }
            self.order.push(addr);
        }
        Ok(())
    }

    fn note_announce(&mut self, idx: usize, mut hash: [u8; 32]) {
        let now = self.now();
        loop {
            if !self.sessions[idx].announced.insert(hash) {
                break;
            }
            self.first_announce.entry(hash).or_insert(now);
            let Some(header) = self.chain.header(&hash) else {
                break;
            };
            let height = self.chain.height_of(&hash).unwrap_or(0);
            if height == 0 {
                break;
            }
            hash = header_prev(&header);
        }
    }

    fn send_getheaders(&mut self, idx: usize, stop: [u8; 32]) {
        let locator = self.chain.locator();
        let payload = wire::encode_getheaders(wire::PROTOCOL_VERSION as u32, &locator, &stop);
        self.send(idx, "getheaders", &payload);
    }

    fn maybe_ping(&mut self) {
        let now = self.now();
        let idxs: Vec<usize> = (0..self.sessions.len()).collect();
        for idx in idxs {
            if !self.sessions[idx].seen_version {
                continue;
            }
            if now.saturating_sub(self.sessions[idx].last_ping_at) < HEADER_PING_INTERVAL_MS {
                continue;
            }
            let nonce = self.rng.next_u64();
            self.sessions[idx].last_ping_at = now;
            let payload = wire::encode_ping(nonce);
            self.send(idx, "ping", &payload);
        }
    }

    fn check_silence(&mut self) {
        let ids = self.silence_ids();
        for id in ids {
            self.remove_session(id, None, true);
        }
    }

    fn silence_ids(&self) -> Vec<u64> {
        let now = self.now();
        let window = HEADER_PEER_SILENCE_SECONDS.saturating_mul(1000);
        let mut ids = Vec::new();
        for session in &self.sessions {
            if !session.seen_version {
                continue;
            }
            let mut due = false;
            for other in &self.sessions {
                if other.id == session.id || !other.seen_version {
                    continue;
                }
                for hash in &other.announced {
                    if session.announced.contains(hash) {
                        continue;
                    }
                    let t0 = self.first_announce.get(hash).copied().unwrap_or(now);
                    let start = t0.max(session.connected_at);
                    if now >= start.saturating_add(window) {
                        due = true;
                        break;
                    }
                }
                if due {
                    break;
                }
            }
            if due {
                ids.push(session.id);
            }
        }
        ids
    }

    fn remove_session(&mut self, id: u64, fault: Option<Fault>, avoid: bool) {
        let Some(pos) = self.sessions.iter().position(|s| s.id == id) else {
            return;
        };
        let session = self.sessions.remove(pos);
        if let Some(fault) = fault {
            self.banned.insert(session.addr.clone(), fault);
        }
        if avoid {
            self.avoid.insert(session.addr);
        }
        self.note_peer_count();
    }

    fn send(&mut self, idx: usize, cmd: &str, payload: &[u8]) {
        let magic = self.chain.params().magic;
        let bytes = wire::encode_message(magic, cmd, payload);
        self.sessions[idx].conn.write_all(&bytes);
    }
}

impl<N: Net> HeaderSource for HeaderDriver<N> {
    fn observe(&self) -> Option<ObservedTip> {
        self.handle().observe()
    }

    fn fee_header(&self) -> Option<[u8; 80]> {
        self.handle().fee_header()
    }
}

impl<N: Net> HeaderDriver<N> {
    pub fn drain_alerts(&self) -> Vec<SourceAlert> {
        self.handle().drain_alerts()
    }
}

fn shuffle(rng: &mut Rng, items: &mut [PeerAddr]) {
    if items.len() < 2 {
        return;
    }
    for i in (1..items.len()).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        items.swap(i, j);
    }
}

fn host_of_ip(ip: &[u8; 16]) -> String {
    if ip[..10].iter().all(|&b| b == 0) && ip[10] == 0xff && ip[11] == 0xff {
        format!("{}.{}.{}.{}", ip[12], ip[13], ip[14], ip[15])
    } else {
        let mut s = String::new();
        for (i, byte) in ip.iter().enumerate() {
            if i > 0 {
                s.push(':');
            }
            s.push_str(&format!("{byte:02x}"));
        }
        s
    }
}
