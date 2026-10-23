//! Header session with peer replacement.
//!
//! The tip moves only when at least two valid peers agree on a header that
//! extends the adopted chain. A peer that sends a bad proof of work, an
//! unexpected difficulty, or a header that does not link is disconnected and
//! replaced. So is a peer that contradicts a chain two other valid peers
//! agree on, a peer that drops, and a peer that stays silent for
//! [`HEADER_PEER_SILENCE`] after another peer announced a header. A one-to-one
//! split does not move the tip; the session asks another peer.
//!
//! Replacements come from the caller. The live clock uses the Bitcoin Core
//! mainnet DNS seeds and peers that have already served a valid header. Tests
//! pass a [`ScriptedSupply`]. A quiet gap, where nobody announces a header,
//! replaces no one. If two agreeing valid peers are still missing when
//! [`PEER_QUORUM_BOUND`] elapses, the read fails closed. The next read tries
//! again.

use std::collections::{HashSet, VecDeque};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::chain::{header_hash, header_prev, Chain, HEADER_PEERS_MIN};
use super::recorded::{ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME, MAINNET_DNS_SEEDS};
use super::wire::{
    decode_headers, decode_inv, encode_getheaders, encode_headers, encode_inv, encode_message,
    encode_pong, encode_version, is_block_inv, take_message, MAINNET_MAGIC, MAX_HEADERS, MSG_BLOCK,
};
use crate::TipTick;

const READ_WAIT: Duration = Duration::from_millis(400);
const DIAL_WAIT: Duration = Duration::from_secs(8);
/// Pause after one full pass of the seed list, so a miss does not resolve DNS in a loop.
const SEED_RETRY: Duration = Duration::from_secs(2);
/// Wall-clock pause between live polls. Tests jump [`MANUAL_STEP_MS`] instead.
const LIVE_IDLE: Duration = Duration::from_millis(2);
const MANUAL_STEP_MS: u64 = 1_000;

/// Producer silence window (SPEC §9 `HEADER_PEER_SILENCE_SECONDS`).
///
/// A peer is silent only after another connected peer has announced a header.
pub const HEADER_PEER_SILENCE: Duration = Duration::from_secs(120);

/// Time allowed for a replacement to finish a header sync after the silence window.
const HEADER_SYNC_BUDGET: Duration = Duration::from_secs(60);

/// How long the clock may go without two agreeing valid peers.
///
/// 180s is the 120s silence window plus one 60s header sync. A dial is at most
/// 8s, inside that minute. The bound is shorter than one Bitcoin block
/// interval, so the tip does not sit for a whole block on one peer. A quiet
/// gap does not start this timer.
pub const PEER_QUORUM_BOUND: Duration =
    Duration::from_secs(HEADER_PEER_SILENCE.as_secs() + HEADER_SYNC_BUDGET.as_secs());

trait Pipe: Send {
    fn send_all(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn recv_some(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn addr(&self) -> Option<SocketAddr> {
        None
    }
}

struct TcpPipe {
    stream: TcpStream,
    addr: SocketAddr,
}

impl Pipe for TcpPipe {
    fn send_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.stream.write_all(bytes)?;
        self.stream.flush()
    }

    fn recv_some(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.set_read_timeout(Some(READ_WAIT))?;
        match self.stream.read(buf) {
            Err(error)
                if error.kind() == ErrorKind::WouldBlock || error.kind() == ErrorKind::TimedOut =>
            {
                Err(io::Error::new(ErrorKind::WouldBlock, "idle"))
            }
            other => other,
        }
    }

    fn addr(&self) -> Option<SocketAddr> {
        Some(self.addr)
    }
}

struct ScriptInner {
    anchor: [u8; 32],
    headers: Vec<[u8; 80]>,
    /// When set, the first `getheaders` withholds the last header and announces it with `inv`.
    inv_last: bool,
    asks: u32,
    dropped: bool,
    silent: bool,
    inbox: Vec<u8>,
    outbox: VecDeque<u8>,
}

impl ScriptInner {
    fn enqueue(&mut self, cmd: &str, payload: &[u8]) {
        let bytes = encode_message(MAINNET_MAGIC, cmd, payload);
        self.outbox.extend(bytes);
    }

    fn pump(&mut self) {
        loop {
            let msg = match take_message(MAINNET_MAGIC, &mut self.inbox) {
                Ok(Some(msg)) => msg,
                Ok(None) => break,
                Err(()) => break,
            };
            match msg.cmd.as_str() {
                "version" => {
                    let now = 1_700_000_000i64;
                    self.enqueue("version", &encode_version(0, now));
                    self.enqueue("verack", &[]);
                }
                "verack" | "pong" => {}
                "ping" => {
                    if let Some(pong) = encode_pong(&msg.payload) {
                        self.enqueue("pong", &pong);
                    }
                }
                "getheaders" => {
                    let locator = super::wire::decode_getheaders(&msg.payload).unwrap_or_default();
                    let mut headers = self.headers_after(&locator);
                    let mut announced = None;
                    if self.inv_last && self.asks == 0 && !headers.is_empty() {
                        let last = headers.pop().unwrap();
                        announced = Some(header_hash(&last));
                    }
                    self.asks = self.asks.saturating_add(1);
                    self.enqueue("headers", &encode_headers(&headers));
                    if let Some(hash) = announced {
                        self.enqueue("inv", &encode_inv(&[(MSG_BLOCK, hash)]));
                    }
                }
                _ => {}
            }
        }
    }

    fn headers_after(&self, locator: &[[u8; 32]]) -> Vec<[u8; 80]> {
        let mut start = 0usize;
        let mut found = false;
        for hash in locator {
            if hash == &self.anchor {
                found = true;
                start = 0;
                break;
            }
            if let Some(index) = self
                .headers
                .iter()
                .position(|header| header_hash(header) == *hash)
            {
                found = true;
                start = index + 1;
                break;
            }
        }
        if !found {
            return Vec::new();
        }
        self.headers[start..].to_vec()
    }
}

/// In-process peer. No socket. A clone shares the script, so a test can
/// announce a later header, drop the pipe, or stop answering.
#[derive(Clone)]
pub struct ScriptedPeer {
    inner: Arc<Mutex<ScriptInner>>,
}

impl ScriptedPeer {
    pub fn new(anchor: [u8; 32], headers: Vec<[u8; 80]>, inv_last: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ScriptInner {
                anchor,
                headers,
                inv_last,
                asks: 0,
                dropped: false,
                silent: false,
                inbox: Vec::new(),
                outbox: VecDeque::new(),
            })),
        }
    }

    /// Queue a block `inv` for a header this peer will serve on the next `getheaders`.
    #[cfg(test)]
    pub fn announce(&self, header: [u8; 80]) {
        let mut guard = self.inner.lock().expect("script");
        guard.headers.push(header);
        let hash = header_hash(&header);
        let payload = encode_inv(&[(MSG_BLOCK, hash)]);
        guard.enqueue("inv", &payload);
    }

    #[cfg(test)]
    fn drop_connection(&self) {
        self.inner.lock().expect("script").dropped = true;
    }

    #[cfg(test)]
    fn go_silent(&self) {
        self.inner.lock().expect("script").silent = true;
    }
}

impl Pipe for ScriptedPeer {
    fn send_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut guard = self.inner.lock().expect("script");
        if guard.dropped {
            return Err(io::Error::new(ErrorKind::BrokenPipe, "dropped"));
        }
        guard.inbox.extend_from_slice(bytes);
        if !guard.silent {
            guard.pump();
        }
        Ok(())
    }

    fn recv_some(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut guard = self.inner.lock().expect("script");
        if guard.dropped {
            return Ok(0);
        }
        if guard.silent {
            return Err(io::Error::new(ErrorKind::WouldBlock, "silent"));
        }
        guard.pump();
        if guard.outbox.is_empty() {
            return Err(io::Error::new(ErrorKind::WouldBlock, "idle"));
        }
        let n = buf.len().min(guard.outbox.len());
        for slot in buf.iter_mut().take(n) {
            *slot = guard.outbox.pop_front().expect("len");
        }
        Ok(n)
    }
}

enum Retire {
    Invalid,
    Silent,
    Dropped,
}

trait PeerSupply: Send {
    fn dial(&mut self) -> Option<Box<dyn Pipe>>;
    fn retire(&mut self, _addr: Option<SocketAddr>, _kind: Retire) {}
}

/// Test stand-in for the DNS seed list. Push peers, then the session dials them.
#[derive(Clone)]
struct ScriptedSupply {
    inner: Arc<Mutex<VecDeque<ScriptedPeer>>>,
}

impl ScriptedSupply {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    #[cfg(test)]
    fn push(&self, peer: ScriptedPeer) {
        self.inner.lock().expect("supply").push_back(peer);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().expect("supply").len()
    }
}

impl PeerSupply for ScriptedSupply {
    fn dial(&mut self) -> Option<Box<dyn Pipe>> {
        let peer = self.inner.lock().expect("supply").pop_front()?;
        Some(Box::new(peer))
    }
}

/// Live replacements. The named peer is first, then Bitcoin Core mainnet DNS
/// seeds, then addresses that completed a handshake and were not invalid.
struct SeedSupply {
    named: Option<SocketAddr>,
    named_tried: bool,
    resolved: VecDeque<SocketAddr>,
    seed_index: usize,
    known_good: VecDeque<SocketAddr>,
    skip: HashSet<SocketAddr>,
    connected: HashSet<SocketAddr>,
    /// Do not start another seed pass before this instant.
    retry_at: Option<Instant>,
}

impl SeedSupply {
    fn new(peer: &str) -> Self {
        Self {
            named: resolve_one(peer),
            named_tried: false,
            resolved: VecDeque::new(),
            seed_index: 0,
            known_good: VecDeque::new(),
            skip: HashSet::new(),
            connected: HashSet::new(),
            retry_at: None,
        }
    }

    fn next_addr(&mut self) -> Option<SocketAddr> {
        self.next_addr_limited(0)
    }

    fn next_addr_limited(&mut self, depth: u8) -> Option<SocketAddr> {
        if depth > 16 {
            return None;
        }
        if !self.named_tried {
            self.named_tried = true;
            if let Some(addr) = self.named {
                if self.usable(addr) {
                    return Some(addr);
                }
            }
        }
        while let Some(addr) = self.resolved.pop_front() {
            if self.usable(addr) {
                return Some(addr);
            }
        }
        if self.seed_index < MAINNET_DNS_SEEDS.len() {
            let host = MAINNET_DNS_SEEDS[self.seed_index];
            self.seed_index += 1;
            if let Ok(addrs) = (host, 8333u16).to_socket_addrs() {
                for addr in addrs.take(8) {
                    self.resolved.push_back(addr);
                }
            }
            return self.next_addr_limited(depth.saturating_add(1));
        }
        let known: Vec<SocketAddr> = self.known_good.iter().copied().collect();
        for addr in known {
            if self.usable(addr) {
                return Some(addr);
            }
        }
        None
    }

    fn usable(&self, addr: SocketAddr) -> bool {
        !self.skip.contains(&addr) && !self.connected.contains(&addr)
    }
}

impl PeerSupply for SeedSupply {
    fn dial(&mut self) -> Option<Box<dyn Pipe>> {
        if let Some(at) = self.retry_at {
            if Instant::now() < at {
                return None;
            }
            self.retry_at = None;
            self.skip.clear();
            self.seed_index = 0;
            self.named_tried = false;
            self.resolved.clear();
        }
        let Some(addr) = self.next_addr() else {
            self.retry_at = Some(Instant::now() + SEED_RETRY);
            return None;
        };
        match TcpStream::connect_timeout(&addr, DIAL_WAIT) {
            Ok(stream) => {
                let stream = prepare(stream).ok()?;
                self.connected.insert(addr);
                if !self.known_good.contains(&addr) {
                    self.known_good.push_back(addr);
                }
                Some(Box::new(TcpPipe { stream, addr }))
            }
            Err(_) => {
                self.skip.insert(addr);
                None
            }
        }
    }

    fn retire(&mut self, addr: Option<SocketAddr>, kind: Retire) {
        let Some(addr) = addr else {
            return;
        };
        self.connected.remove(&addr);
        self.skip.insert(addr);
        if matches!(kind, Retire::Invalid) {
            self.known_good.retain(|have| *have != addr);
        }
    }
}

enum Time {
    System {
        start: Instant,
    },
    /// Test clock. `park` jumps it forward. No thread sleep.
    Manual {
        now_ms: u64,
    },
}

impl Time {
    fn system() -> Self {
        Self::System {
            start: Instant::now(),
        }
    }

    fn now_ms(&self) -> u64 {
        match self {
            Self::System { start } => start.elapsed().as_millis() as u64,
            Self::Manual { now_ms } => *now_ms,
        }
    }

    fn park(&mut self) {
        match self {
            Self::System { .. } => thread::sleep(LIVE_IDLE),
            Self::Manual { now_ms } => *now_ms = now_ms.saturating_add(MANUAL_STEP_MS),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pump {
    Idle,
    Progress,
    Removed,
}

struct Peer {
    pipe: Box<dyn Pipe>,
    buf: Vec<u8>,
    version_sent: bool,
    saw_version: bool,
    saw_verack: bool,
    asked: bool,
    caught_up: bool,
    tip: [u8; 32],
    extra: Vec<[u8; 80]>,
    /// First header this peer announced that is not on the adopted chain.
    announced_at: Option<u64>,
    connected_at: u64,
}

impl Peer {
    fn new(pipe: Box<dyn Pipe>, tip: [u8; 32], connected_at: u64) -> Self {
        Self {
            pipe,
            buf: Vec::new(),
            version_sent: false,
            saw_version: false,
            saw_verack: false,
            asked: false,
            caught_up: false,
            tip,
            extra: Vec::new(),
            announced_at: None,
            connected_at,
        }
    }

    fn voting(&self) -> bool {
        self.saw_version && self.saw_verack && self.asked && self.caught_up
    }
}

pub struct Session {
    peers: Vec<Peer>,
    chain: Chain,
    supply: Box<dyn PeerSupply>,
    time: Time,
    opened: bool,
    greeted: bool,
    quorum_ms: u64,
    silence_ms: u64,
    /// `None` on the live clock: a quiet gap waits. Tests set a budget so
    /// `wait_after` returns instead of parking forever.
    follow_ms: Option<u64>,
    below_since: Option<u64>,
    last_ping: u64,
    /// Set by a producer Stop. Bari leaves this empty. Checked between reads,
    /// never inside a header check, so a leg is not paused by the wait itself.
    cancel: Option<Arc<AtomicBool>>,
}

impl Session {
    pub fn from_scripted(
        chain: Chain,
        peers: Vec<ScriptedPeer>,
        sync_budget: Duration,
    ) -> Result<Self, String> {
        Ok(Self::scripted(
            chain,
            peers,
            ScriptedSupply::new(),
            sync_budget,
        ))
    }

    fn scripted(
        chain: Chain,
        peers: Vec<ScriptedPeer>,
        supply: ScriptedSupply,
        quorum: Duration,
    ) -> Self {
        let tip = chain.tip_hash();
        Self::assemble(
            chain,
            peers
                .into_iter()
                .map(|peer| Peer::new(Box::new(peer), tip, 0))
                .collect(),
            Box::new(supply),
            Time::Manual { now_ms: 0 },
            quorum.as_millis() as u64,
            HEADER_PEER_SILENCE.as_millis() as u64,
            Some(quorum.as_millis() as u64),
            None,
        )
    }

    fn assemble(
        chain: Chain,
        peers: Vec<Peer>,
        supply: Box<dyn PeerSupply>,
        time: Time,
        quorum_ms: u64,
        silence_ms: u64,
        follow_ms: Option<u64>,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Self {
        Self {
            peers,
            chain,
            supply,
            time,
            opened: false,
            greeted: false,
            quorum_ms,
            silence_ms,
            follow_ms,
            below_since: None,
            last_ping: 0,
            cancel,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.opened
    }

    pub fn opening_tip(&mut self) -> Result<TipTick, String> {
        if !self.greeted {
            self.greet()?;
            self.greeted = true;
        }
        let hash = self.drive_until(None)?;
        self.opened = true;
        self.chain.tick(hash).map_err(|fault| fault.message())
    }

    /// Headers strictly after `after`, once at least two peers agree on a newer tip.
    pub fn wait_after(&mut self, after: [u8; 32]) -> Result<Vec<TipTick>, String> {
        if !self.opened {
            let tip = self.opening_tip()?;
            if tip.hash != after {
                return self
                    .chain
                    .path_after(after, tip.hash)
                    .map_err(|f| f.message());
            }
        }
        let hash = self.drive_until(Some(after))?;
        self.chain
            .path_after(after, hash)
            .map_err(|fault| fault.message())
    }

    fn now_ms(&self) -> u64 {
        self.time.now_ms()
    }

    #[cfg(test)]
    fn adopted_height(&self) -> u32 {
        self.chain.tip_height()
    }

    #[cfg(test)]
    fn adopted_hash(&self) -> [u8; 32] {
        self.chain.tip_hash()
    }

    #[cfg(test)]
    fn peer_count(&self) -> usize {
        self.peers.len()
    }

    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }

    fn drive_until(&mut self, stop_at: Option<[u8; 32]>) -> Result<[u8; 32], String> {
        let started = self.now_ms();
        self.below_since = None;
        let follow_deadline = self.follow_ms.map(|budget| started.saturating_add(budget));
        loop {
            if self.cancelled() {
                return Err("cancelled".to_owned());
            }
            self.sweep_silence()?;
            let mut progress = self.pump_all()?;
            self.maybe_ping()?;
            if let Some(hash) = self.agreed_extension() {
                self.commit(hash)?;
                self.eject_contradictions(hash)?;
                self.clear_adopted_announcements();
                let advanced = stop_at.is_none_or(|previous| previous != hash);
                if advanced {
                    self.below_since = None;
                    return Ok(hash);
                }
            }
            if self.need_peer() && self.dial_one()? {
                progress = true;
            }
            if let Some(deadline) = follow_deadline {
                if stop_at.is_some() && self.now_ms() >= deadline && self.quiet_agreement() {
                    return Ok(self.chain.tip_hash());
                }
            }
            self.check_quorum()?;
            if !progress {
                self.time.park();
            }
        }
    }

    fn greet(&mut self) -> Result<(), String> {
        let mut index = 0;
        while index < self.peers.len() {
            match self.greet_one(index)? {
                Pump::Removed => {}
                Pump::Idle | Pump::Progress => index += 1,
            }
        }
        Ok(())
    }

    fn greet_one(&mut self, index: usize) -> Result<Pump, String> {
        if index >= self.peers.len() || self.peers[index].version_sent {
            return Ok(Pump::Progress);
        }
        self.peers[index].version_sent = true;
        let version = self.version_payload();
        self.send(index, "version", &version)
    }

    fn version_payload(&self) -> Vec<u8> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or(0);
        encode_version(self.chain.tip_height() as i32, now)
    }

    fn pump_all(&mut self) -> Result<bool, String> {
        let mut progress = false;
        let mut index = 0;
        while index < self.peers.len() {
            match self.pump(index)? {
                Pump::Idle => index += 1,
                Pump::Progress => {
                    progress = true;
                    index += 1;
                }
                Pump::Removed => progress = true,
            }
        }
        Ok(progress)
    }

    fn pump(&mut self, index: usize) -> Result<Pump, String> {
        let mut buf = [0u8; 8 * 1024];
        let read = match self.peers[index].pipe.recv_some(&mut buf) {
            Ok(0) => {
                self.swap_out(index, Retire::Dropped)?;
                return Ok(Pump::Removed);
            }
            Ok(n) => n,
            Err(error)
                if error.kind() == ErrorKind::WouldBlock
                    || error.kind() == ErrorKind::Interrupted =>
            {
                0
            }
            Err(_) => {
                self.swap_out(index, Retire::Dropped)?;
                return Ok(Pump::Removed);
            }
        };
        if read > 0 {
            self.peers[index].buf.extend_from_slice(&buf[..read]);
        }
        let mut handled = false;
        loop {
            let msg = match take_message(MAINNET_MAGIC, &mut self.peers[index].buf) {
                Ok(Some(msg)) => msg,
                Ok(None) => break,
                Err(()) => {
                    self.swap_out(index, Retire::Invalid)?;
                    return Ok(Pump::Removed);
                }
            };
            handled = true;
            if self.handle(index, &msg.cmd, &msg.payload)? == Pump::Removed {
                return Ok(Pump::Removed);
            }
        }
        if read > 0 || handled {
            Ok(Pump::Progress)
        } else {
            Ok(Pump::Idle)
        }
    }

    fn handle(&mut self, index: usize, cmd: &str, payload: &[u8]) -> Result<Pump, String> {
        match cmd {
            "version" => {
                self.peers[index].saw_version = true;
                if !self.peers[index].saw_verack {
                    if self.send(index, "verack", &[])? == Pump::Removed {
                        return Ok(Pump::Removed);
                    }
                }
                self.ask_if_ready(index)
            }
            "verack" => {
                self.peers[index].saw_verack = true;
                self.ask_if_ready(index)
            }
            "ping" => {
                if let Some(pong) = encode_pong(payload) {
                    return self.send(index, "pong", &pong);
                }
                Ok(Pump::Progress)
            }
            "headers" => self.on_headers(index, payload),
            "inv" => self.on_inv(index, payload),
            _ => Ok(Pump::Progress),
        }
    }

    fn ask_if_ready(&mut self, index: usize) -> Result<Pump, String> {
        if self.peers[index].saw_version && self.peers[index].saw_verack && !self.peers[index].asked
        {
            return self.send_getheaders(index);
        }
        Ok(Pump::Progress)
    }

    fn send_getheaders(&mut self, index: usize) -> Result<Pump, String> {
        let mut locator = self.chain.locator();
        let peer_tip = self.peers[index].tip;
        if !locator.contains(&peer_tip) {
            locator.push(peer_tip);
        }
        let payload = encode_getheaders(&locator, &[0u8; 32]);
        self.peers[index].asked = true;
        self.peers[index].caught_up = false;
        self.send(index, "getheaders", &payload)
    }

    fn on_headers(&mut self, index: usize, payload: &[u8]) -> Result<Pump, String> {
        let headers = match decode_headers(payload) {
            Ok(headers) => headers,
            Err(()) => {
                self.swap_out(index, Retire::Invalid)?;
                return Ok(Pump::Removed);
            }
        };
        if headers.is_empty() {
            self.peers[index].caught_up = true;
            return Ok(Pump::Progress);
        }
        for header in headers.iter().copied() {
            if self.consider(index, header).is_err() {
                self.swap_out(index, Retire::Invalid)?;
                return Ok(Pump::Removed);
            }
        }
        let full = headers.len() == MAX_HEADERS;
        self.peers[index].caught_up = !full;
        if full {
            return self.send_getheaders(index);
        }
        Ok(Pump::Progress)
    }

    fn on_inv(&mut self, index: usize, payload: &[u8]) -> Result<Pump, String> {
        let items = match decode_inv(payload) {
            Ok(items) => items,
            Err(()) => {
                self.swap_out(index, Retire::Invalid)?;
                return Ok(Pump::Removed);
            }
        };
        let mut want = false;
        let now = self.now_ms();
        for (kind, hash) in items {
            if is_block_inv(kind) && !self.chain.contains(&hash) {
                want = true;
                if self.peers[index].announced_at.is_none() {
                    self.peers[index].announced_at = Some(now);
                }
            }
        }
        if !want {
            return Ok(Pump::Progress);
        }
        let mut peer = 0;
        while peer < self.peers.len() {
            self.peers[peer].caught_up = false;
            match self.send_getheaders(peer)? {
                Pump::Removed => {}
                Pump::Idle | Pump::Progress => peer += 1,
            }
        }
        Ok(Pump::Progress)
    }

    /// Validate `header` against this peer's view. A failure leaves the peer unchanged.
    fn consider(&mut self, index: usize, header: [u8; 80]) -> Result<(), String> {
        let mut scratch = self.chain.clone();
        for prior in &self.peers[index].extra {
            scratch.stage(*prior).map_err(|fault| fault.message())?;
        }
        scratch.stage(header).map_err(|fault| fault.message())?;
        let hash = header_hash(&header);
        let now = self.now_ms();
        let on_chain = self.chain.contains(&hash);
        let peer = &mut self.peers[index];
        if !peer.extra.iter().any(|have| header_hash(have) == hash) {
            peer.extra.push(header);
        }
        peer.tip = hash;
        if !on_chain && peer.announced_at.is_none() {
            peer.announced_at = Some(now);
        }
        Ok(())
    }

    fn sweep_silence(&mut self) -> Result<(), String> {
        let adopted = self.chain.tip_hash();
        let now = self.now_ms();
        let Some(first) = self.peers.iter().filter_map(|peer| peer.announced_at).min() else {
            return Ok(());
        };
        let mut silent = Vec::new();
        for (index, peer) in self.peers.iter().enumerate() {
            if peer.announced_at.is_some() {
                continue;
            }
            if peer.tip != adopted && !self.chain.contains(&peer.tip) {
                continue;
            }
            let start = first.max(peer.connected_at);
            if now.saturating_sub(start) >= self.silence_ms {
                silent.push(index);
            }
        }
        for index in silent.into_iter().rev() {
            self.swap_out(index, Retire::Silent)?;
        }
        Ok(())
    }

    fn need_peer(&self) -> bool {
        if self.peers.len() >= 4 {
            return false;
        }
        if self.peers.len() < HEADER_PEERS_MIN {
            return true;
        }
        if self.peers.iter().any(|peer| !peer.voting()) {
            return false;
        }
        self.agreed_extension().is_none()
    }

    fn dial_one(&mut self) -> Result<bool, String> {
        let Some(pipe) = self.supply.dial() else {
            return Ok(false);
        };
        self.add_peer(pipe)?;
        Ok(true)
    }

    fn add_peer(&mut self, pipe: Box<dyn Pipe>) -> Result<(), String> {
        let tip = self.chain.tip_hash();
        let connected_at = self.now_ms();
        self.peers.push(Peer::new(pipe, tip, connected_at));
        let index = self.peers.len() - 1;
        self.greet_one(index)?;
        Ok(())
    }

    fn swap_out(&mut self, index: usize, kind: Retire) -> Result<(), String> {
        if index >= self.peers.len() {
            return Ok(());
        }
        let peer = self.peers.remove(index);
        self.supply.retire(peer.pipe.addr(), kind);
        if self.peers.len() >= 4 {
            return Ok(());
        }
        if let Some(pipe) = self.supply.dial() {
            self.add_peer(pipe)?;
        }
        Ok(())
    }

    /// A tip at least two voting peers share, and that extends the adopted chain.
    /// A tie, including a one-to-one split, is not an agreement.
    fn agreed_extension(&self) -> Option<[u8; 32]> {
        let mut counts: Vec<([u8; 32], usize)> = Vec::new();
        for peer in &self.peers {
            if !peer.voting() {
                continue;
            }
            if let Some(slot) = counts.iter_mut().find(|(hash, _)| *hash == peer.tip) {
                slot.1 += 1;
            } else {
                counts.push((peer.tip, 1));
            }
        }
        let mut best: Option<([u8; 32], usize)> = None;
        let mut tied = false;
        for (hash, count) in counts {
            match best {
                None => best = Some((hash, count)),
                Some((_, best_count)) if count > best_count => {
                    best = Some((hash, count));
                    tied = false;
                }
                Some((_, best_count)) if count == best_count => tied = true,
                _ => {}
            }
        }
        let (hash, count) = best?;
        if tied || count < HEADER_PEERS_MIN {
            return None;
        }
        if hash == self.chain.tip_hash() || self.reaches(hash, self.chain.tip_hash()) {
            Some(hash)
        } else {
            None
        }
    }

    fn quiet_agreement(&self) -> bool {
        if self.peers.iter().any(|peer| peer.announced_at.is_some()) {
            return false;
        }
        self.agreed_extension() == Some(self.chain.tip_hash())
    }

    fn check_quorum(&mut self) -> Result<(), String> {
        if self.agreed_extension().is_some() {
            self.below_since = None;
            return Ok(());
        }
        let now = self.now_ms();
        let since = *self.below_since.get_or_insert(now);
        if now.saturating_sub(since) >= self.quorum_ms {
            return Err(self.quorum_error());
        }
        Ok(())
    }

    fn quorum_error(&self) -> String {
        if self.peers.len() < HEADER_PEERS_MIN {
            "too few peers".to_owned()
        } else {
            "peer disagreement".to_owned()
        }
    }

    fn commit(&mut self, hash: [u8; 32]) -> Result<(), String> {
        if hash == self.chain.tip_hash() || self.chain.contains(&hash) {
            return Ok(());
        }
        let mut path = Vec::new();
        let mut cursor = hash;
        for _ in 0..10_000 {
            if self.chain.contains(&cursor) {
                break;
            }
            let header = self
                .find_extra(cursor)
                .ok_or_else(|| "inconsistent chain".to_owned())?;
            cursor = header_prev(&header);
            path.push(header);
        }
        if !self.chain.contains(&cursor) {
            return Err("inconsistent chain".to_owned());
        }
        path.reverse();
        for header in path {
            self.chain.append(header).map_err(|fault| fault.message())?;
        }
        Ok(())
    }

    fn eject_contradictions(&mut self, agreed: [u8; 32]) -> Result<(), String> {
        let mut bad = Vec::new();
        for (index, peer) in self.peers.iter().enumerate() {
            if peer.voting() && self.contradicts(peer, agreed) {
                bad.push(index);
            }
        }
        for index in bad.into_iter().rev() {
            self.swap_out(index, Retire::Invalid)?;
        }
        Ok(())
    }

    fn contradicts(&self, peer: &Peer, agreed: [u8; 32]) -> bool {
        if peer.tip == agreed {
            return false;
        }
        if self.reaches(agreed, peer.tip) || self.reaches(peer.tip, agreed) {
            return false;
        }
        true
    }

    /// `true` when walking previous-hash links from `from` reaches `target`.
    fn reaches(&self, from: [u8; 32], target: [u8; 32]) -> bool {
        let mut hash = from;
        for _ in 0..10_000 {
            if hash == target {
                return true;
            }
            if let Some(prev) = self.chain.prev_of(&hash) {
                hash = prev;
                continue;
            }
            if let Some(header) = self.find_extra(hash) {
                hash = header_prev(&header);
                continue;
            }
            return false;
        }
        false
    }

    fn find_extra(&self, hash: [u8; 32]) -> Option<[u8; 80]> {
        for peer in &self.peers {
            for header in &peer.extra {
                if header_hash(header) == hash {
                    return Some(*header);
                }
            }
        }
        None
    }

    fn clear_adopted_announcements(&mut self) {
        let tip = self.chain.tip_hash();
        for peer in &mut self.peers {
            if peer.tip == tip {
                peer.announced_at = None;
                peer.extra.clear();
            }
        }
    }

    fn maybe_ping(&mut self) -> Result<(), String> {
        let now = self.now_ms();
        if now.saturating_sub(self.last_ping) < 30_000 {
            return Ok(());
        }
        self.last_ping = now;
        let nonce = 1u64.to_le_bytes();
        let mut index = 0;
        while index < self.peers.len() {
            match self.send(index, "ping", &nonce)? {
                Pump::Removed => {}
                Pump::Idle | Pump::Progress => index += 1,
            }
        }
        Ok(())
    }

    fn send(&mut self, index: usize, cmd: &str, payload: &[u8]) -> Result<Pump, String> {
        if index >= self.peers.len() {
            return Ok(Pump::Removed);
        }
        let bytes = encode_message(MAINNET_MAGIC, cmd, payload);
        match self.peers[index].pipe.send_all(&bytes) {
            Ok(()) => Ok(Pump::Progress),
            Err(_) => {
                self.swap_out(index, Retire::Dropped)?;
                Ok(Pump::Removed)
            }
        }
    }
}

pub(crate) fn connect_mainnet(peer: &str) -> Result<Session, String> {
    let chain = Chain::open_mainnet(ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME)
        .map_err(|fault| fault.message())?;
    let mut supply = SeedSupply::new(peer);
    let tip = chain.tip_hash();
    let mut peers = Vec::new();
    for _ in 0..HEADER_PEERS_MIN {
        if let Some(pipe) = supply.dial() {
            peers.push(Peer::new(pipe, tip, 0));
        }
    }
    Ok(Session::assemble(
        chain,
        peers,
        Box::new(supply),
        Time::system(),
        PEER_QUORUM_BOUND.as_millis() as u64,
        HEADER_PEER_SILENCE.as_millis() as u64,
        None,
        None,
    ))
}

/// Same dial as [`connect_mainnet`]. The read returns `cancelled` once `cancel`
/// is set, so a producer Stop does not wait out a block.
pub(crate) fn connect_mainnet_cancellable(
    peer: &str,
    cancel: Arc<AtomicBool>,
) -> Result<Session, String> {
    let mut session = connect_mainnet(peer)?;
    session.cancel = Some(cancel);
    Ok(session)
}

fn resolve_one(peer: &str) -> Option<SocketAddr> {
    peer.to_socket_addrs().ok()?.next()
}

fn prepare(stream: TcpStream) -> Result<TcpStream, String> {
    stream
        .set_nodelay(true)
        .map_err(|_| "too few peers".to_owned())?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{header_hash, mine_header, spoil_proof, synth_bits};

    fn fixed_chain() -> (Chain, [u8; 80], [u8; 32]) {
        let bits = synth_bits();
        let anchor = mine_header([0x10; 32], 1_700_000_000, bits).unwrap();
        let chain = Chain::open_fixed(anchor, 80, bits).unwrap();
        let hash = header_hash(&anchor);
        (chain, anchor, hash)
    }

    fn child(anchor_hash: [u8; 32], time: u32) -> [u8; 80] {
        mine_header(anchor_hash, time, synth_bits()).unwrap()
    }

    fn scripted(
        chain: Chain,
        peers: Vec<ScriptedPeer>,
        supply: ScriptedSupply,
        quorum: Duration,
    ) -> Session {
        Session::scripted(chain, peers, supply, quorum)
    }

    #[test]
    fn quorum_bound_is_silence_plus_one_sync() {
        assert_eq!(HEADER_PEER_SILENCE, Duration::from_secs(120));
        assert_eq!(HEADER_SYNC_BUDGET, Duration::from_secs(60));
        assert_eq!(PEER_QUORUM_BOUND, HEADER_PEER_SILENCE + HEADER_SYNC_BUDGET);
        assert_eq!(PEER_QUORUM_BOUND, Duration::from_secs(180));
    }

    #[test]
    fn two_peers_agreeing_on_a_header_advance_the_tip() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let header = child(anchor_hash, 1_700_000_100);
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![header], false),
            ScriptedPeer::new(anchor_hash, vec![header], false),
        ];
        let mut session = Session::from_scripted(chain, peers, Duration::from_secs(2)).unwrap();
        let tip = session.opening_tip().unwrap();
        assert_eq!(tip.hash, header_hash(&header));
        assert_eq!(tip.height, 81);
        assert_eq!(tip.previous, Some(anchor_hash));
    }

    #[test]
    fn an_inv_is_answered_with_getheaders_and_both_peers_must_serve_it() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let header = child(anchor_hash, 1_700_000_150);
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![header], true),
            ScriptedPeer::new(anchor_hash, vec![header], true),
        ];
        let mut session = Session::from_scripted(chain, peers, Duration::from_secs(2)).unwrap();
        let tip = session.opening_tip().unwrap();
        assert_eq!(tip.hash, header_hash(&header));
    }

    #[test]
    fn a_later_inv_from_both_peers_is_the_next_tip() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let first = child(anchor_hash, 1_700_000_100);
        let second = child(header_hash(&first), 1_700_000_200);
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![first], false),
            ScriptedPeer::new(anchor_hash, vec![first], false),
        ];
        let handles = peers.clone();
        let mut session = Session::from_scripted(chain, peers, Duration::from_secs(2)).unwrap();
        let opened = session.opening_tip().unwrap();
        assert_eq!(opened.hash, header_hash(&first));
        for peer in &handles {
            peer.announce(second);
        }
        let next = session.wait_after(opened.hash).unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].hash, header_hash(&second));
        assert_eq!(next[0].previous, Some(header_hash(&first)));
    }

    #[test]
    fn a_one_to_one_split_does_not_advance_and_fails_closed_at_the_bound() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let a = child(anchor_hash, 1_700_000_100);
        let b = child(anchor_hash, 1_700_000_300);
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![a], false),
            ScriptedPeer::new(anchor_hash, vec![b], false),
        ];
        let mut session = Session::from_scripted(chain, peers, Duration::from_secs(2)).unwrap();
        let err = session.opening_tip().unwrap_err();
        assert_eq!(err, "peer disagreement");
        assert_eq!(session.adopted_height(), 80);
        assert_eq!(session.adopted_hash(), anchor_hash);
        assert!(session.now_ms() >= 2_000);
    }

    #[test]
    fn a_bad_proof_of_work_peer_is_swapped_out() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let good = child(anchor_hash, 1_700_000_100);
        let mut bad = good;
        spoil_proof(&mut bad);
        let supply = ScriptedSupply::new();
        supply.push(ScriptedPeer::new(anchor_hash, vec![good], false));
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![good], false),
            ScriptedPeer::new(anchor_hash, vec![bad], false),
        ];
        let mut session = scripted(chain, peers, supply.clone(), Duration::from_secs(2));
        let tip = session.opening_tip().unwrap();
        assert_eq!(tip.hash, header_hash(&good));
        assert_eq!(tip.height, 81);
        assert_eq!(supply.len(), 0);
        assert_eq!(session.peer_count(), 2);
    }

    #[test]
    fn a_peer_with_unexpected_difficulty_is_swapped_out() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let good = child(anchor_hash, 1_700_000_100);
        let mut bad = good;
        bad[72..76].copy_from_slice(&0x1d00ffffu32.to_le_bytes());
        let supply = ScriptedSupply::new();
        supply.push(ScriptedPeer::new(anchor_hash, vec![good], false));
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![good], false),
            ScriptedPeer::new(anchor_hash, vec![bad], false),
        ];
        let mut session = scripted(chain, peers, supply.clone(), Duration::from_secs(2));
        let tip = session.opening_tip().unwrap();
        assert_eq!(tip.hash, header_hash(&good));
        assert_eq!(supply.len(), 0);
    }

    #[test]
    fn a_header_that_does_not_link_is_swapped_out() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let good = child(anchor_hash, 1_700_000_100);
        let orphan = mine_header([0x99; 32], 1_700_000_050, synth_bits()).unwrap();
        let supply = ScriptedSupply::new();
        supply.push(ScriptedPeer::new(anchor_hash, vec![good], false));
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![good], false),
            ScriptedPeer::new(anchor_hash, vec![orphan], false),
        ];
        let mut session = scripted(chain, peers, supply.clone(), Duration::from_secs(2));
        let tip = session.opening_tip().unwrap();
        assert_eq!(tip.hash, header_hash(&good));
        assert_eq!(supply.len(), 0);
    }

    #[test]
    fn a_disagreeing_peer_is_swapped_out() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let agreed = child(anchor_hash, 1_700_000_100);
        let other = child(anchor_hash, 1_700_000_300);
        let supply = ScriptedSupply::new();
        supply.push(ScriptedPeer::new(anchor_hash, vec![agreed], false));
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![agreed], false),
            ScriptedPeer::new(anchor_hash, vec![other], false),
        ];
        let mut session = scripted(chain, peers, supply.clone(), Duration::from_secs(2));
        let tip = session.opening_tip().unwrap();
        assert_eq!(tip.hash, header_hash(&agreed));
        assert_eq!(tip.height, 81);
        assert_ne!(tip.hash, header_hash(&other));
        assert_eq!(supply.len(), 0);
        assert_eq!(session.peer_count(), 2);
    }

    #[test]
    fn a_dropped_peer_is_replaced() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let good = child(anchor_hash, 1_700_000_100);
        let dropped = ScriptedPeer::new(anchor_hash, vec![good], false);
        dropped.drop_connection();
        let supply = ScriptedSupply::new();
        supply.push(ScriptedPeer::new(anchor_hash, vec![good], false));
        let peers = vec![ScriptedPeer::new(anchor_hash, vec![good], false), dropped];
        let mut session = scripted(chain, peers, supply.clone(), Duration::from_secs(2));
        let tip = session.opening_tip().unwrap();
        assert_eq!(tip.hash, header_hash(&good));
        assert_eq!(tip.height, 81);
        assert_eq!(supply.len(), 0);
    }

    #[test]
    fn a_silent_peer_is_replaced_after_the_silence_window() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let good = child(anchor_hash, 1_700_000_100);
        let silent = ScriptedPeer::new(anchor_hash, vec![good], false);
        silent.go_silent();
        let supply = ScriptedSupply::new();
        supply.push(ScriptedPeer::new(anchor_hash, vec![good], false));
        let peers = vec![ScriptedPeer::new(anchor_hash, vec![good], false), silent];
        let mut session = scripted(chain, peers, supply.clone(), PEER_QUORUM_BOUND);
        let tip = session.opening_tip().unwrap();
        assert_eq!(tip.hash, header_hash(&good));
        assert!(session.now_ms() >= HEADER_PEER_SILENCE.as_millis() as u64);
        assert!(session.now_ms() < PEER_QUORUM_BOUND.as_millis() as u64);
        assert_eq!(supply.len(), 0);
    }

    #[test]
    fn a_quiet_gap_does_not_replace_a_peer() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let supply = ScriptedSupply::new();
        supply.push(ScriptedPeer::new(
            anchor_hash,
            vec![child(anchor_hash, 1_700_000_400)],
            false,
        ));
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![], false),
            ScriptedPeer::new(anchor_hash, vec![], false),
        ];
        let mut session = scripted(chain, peers, supply.clone(), Duration::from_secs(2));
        let opened = session.opening_tip().unwrap();
        assert_eq!(opened.hash, anchor_hash);
        let next = session.wait_after(opened.hash).unwrap();
        assert!(next.is_empty());
        assert_eq!(supply.len(), 1);
        assert_eq!(session.peer_count(), 2);
        assert!(session.now_ms() < HEADER_PEER_SILENCE.as_millis() as u64);
        assert_eq!(session.adopted_hash(), anchor_hash);
    }

    #[test]
    fn fewer_than_two_peers_fail_closed_then_recover() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let good = child(anchor_hash, 1_700_000_100);
        let supply = ScriptedSupply::new();
        let peers = vec![ScriptedPeer::new(anchor_hash, vec![good], false)];
        let mut session = scripted(chain, peers, supply.clone(), Duration::from_secs(2));
        assert_eq!(session.opening_tip().unwrap_err(), "too few peers");
        assert_eq!(session.adopted_height(), 80);
        assert_eq!(session.adopted_hash(), anchor_hash);
        assert!(session.now_ms() >= 2_000);
        supply.push(ScriptedPeer::new(anchor_hash, vec![good], false));
        let tip = session.opening_tip().unwrap();
        assert_eq!(tip.hash, header_hash(&good));
        assert_eq!(tip.height, 81);
        assert_eq!(tip.previous, Some(anchor_hash));
    }

    #[test]
    fn the_tip_never_advances_on_one_peer() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let good = child(anchor_hash, 1_700_000_100);
        let peers = vec![ScriptedPeer::new(anchor_hash, vec![good], false)];
        let mut session = Session::from_scripted(chain, peers, Duration::from_secs(3)).unwrap();
        assert_eq!(session.opening_tip().unwrap_err(), "too few peers");
        assert_eq!(session.adopted_height(), 80);
        assert_eq!(session.adopted_hash(), anchor_hash);
        assert_ne!(session.adopted_hash(), header_hash(&good));
        assert!(session.now_ms() >= 3_000);
    }

    /// Manual read-only check against mainnet peers. `cargo test` does not run it.
    #[test]
    #[ignore = "dials mainnet peers; not for CI"]
    fn live_smoke_agreed_tip_is_at_least_the_anchor() {
        let peer = format!("{}:8333", MAINNET_DNS_SEEDS[0]);
        let mut session = connect_mainnet(&peer).expect("dial");
        let tip = session.opening_tip().expect("headers");
        assert!(tip.height >= u64::from(ANCHOR_HEIGHT));
        assert!(tip.height < u64::from(ANCHOR_HEIGHT) + 50_000);
        eprintln!("live-smoke height={} n_time={}", tip.height, tip.n_time);
    }
}
