//! Two-peer header session.
//!
//! The clock advances only when every connected peer has the same tip.
//! Fewer than two peers, a bad header, a header that does not link, or two
//! different tips is fail-closed. A block `inv` is answered with `getheaders`.

use std::collections::VecDeque;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::chain::{header_hash, Chain, HEADER_PEERS_MIN};
use super::recorded::{ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME, MAINNET_DNS_SEEDS};
use super::wire::{
    decode_headers, decode_inv, encode_getheaders, encode_headers, encode_inv, encode_message,
    encode_pong, encode_version, is_block_inv, take_message, MAINNET_MAGIC, MAX_HEADERS, MSG_BLOCK,
};
use crate::header_source::TipTick;

const READ_WAIT: Duration = Duration::from_millis(400);
const ANNOUNCE_WAIT: Duration = Duration::from_secs(120);
const DIAL_WAIT: Duration = Duration::from_secs(8);

trait Pipe: Send {
    fn send_all(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn recv_some(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

struct TcpPipe(TcpStream);

impl Pipe for TcpPipe {
    fn send_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.0.write_all(bytes)?;
        self.0.flush()
    }

    fn recv_some(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.set_read_timeout(Some(READ_WAIT))?;
        match self.0.read(buf) {
            Err(error)
                if error.kind() == ErrorKind::WouldBlock || error.kind() == ErrorKind::TimedOut =>
            {
                Err(io::Error::new(ErrorKind::WouldBlock, "idle"))
            }
            other => other,
        }
    }
}

struct ScriptInner {
    anchor: [u8; 32],
    headers: Vec<[u8; 80]>,
    /// When set, the first `getheaders` withholds the last header and announces it with `inv`.
    inv_last: bool,
    asks: u32,
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
/// announce a later header after the opening sync.
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
}

impl Pipe for ScriptedPeer {
    fn send_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut guard = self.inner.lock().expect("script");
        guard.inbox.extend_from_slice(bytes);
        guard.pump();
        Ok(())
    }

    fn recv_some(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut guard = self.inner.lock().expect("script");
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

struct Peer {
    pipe: Box<dyn Pipe>,
    buf: Vec<u8>,
    saw_version: bool,
    saw_verack: bool,
    asked: bool,
    caught_up: bool,
    tip: [u8; 32],
}

pub struct Session {
    peers: Vec<Peer>,
    chain: Chain,
    opened: bool,
    laggard_retry: bool,
    sync_budget: Duration,
    /// `None` on the live clock: a quiet gap between blocks is not an error.
    /// Tests set a short budget so a stuck follow fails instead of hanging.
    follow_budget: Option<Duration>,
    diverged_at: Option<Instant>,
    last_ping: Instant,
}

impl Session {
    pub fn from_scripted(
        chain: Chain,
        peers: Vec<ScriptedPeer>,
        sync_budget: Duration,
    ) -> Result<Self, String> {
        if peers.len() < HEADER_PEERS_MIN {
            return Err("too few peers".to_owned());
        }
        let tip = chain.tip_hash();
        let peers = peers
            .into_iter()
            .map(|peer| Peer {
                pipe: Box::new(peer),
                buf: Vec::new(),
                saw_version: false,
                saw_verack: false,
                asked: false,
                caught_up: false,
                tip,
            })
            .collect();
        Ok(Self::from_peers(
            chain,
            peers,
            sync_budget,
            Some(sync_budget),
        ))
    }

    pub(super) fn is_open(&self) -> bool {
        self.opened
    }

    fn from_peers(
        chain: Chain,
        peers: Vec<Peer>,
        sync_budget: Duration,
        follow_budget: Option<Duration>,
    ) -> Self {
        Self {
            peers,
            chain,
            opened: false,
            laggard_retry: false,
            sync_budget,
            follow_budget,
            diverged_at: None,
            last_ping: Instant::now(),
        }
    }

    pub fn opening_tip(&mut self) -> Result<TipTick, String> {
        if self.peers.len() < HEADER_PEERS_MIN {
            return Err("too few peers".to_owned());
        }
        self.greet()?;
        let deadline = Instant::now() + self.sync_budget;
        let hash = self.drive_until(Some(deadline), None)?;
        self.opened = true;
        self.chain.tick(hash).map_err(|fault| fault.message())
    }

    /// Headers strictly after `after`, once every peer agrees on a newer tip.
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
        let deadline = self.follow_budget.map(|budget| Instant::now() + budget);
        let hash = self.drive_until(deadline, Some(after))?;
        self.chain
            .path_after(after, hash)
            .map_err(|fault| fault.message())
    }

    fn greet(&mut self) -> Result<(), String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let version = encode_version(self.chain.tip_height() as i32, now);
        for index in 0..self.peers.len() {
            self.send(index, "version", &version)?;
        }
        Ok(())
    }

    /// `stop_at` is the hash we already delivered. `None` accepts the first agreed tip.
    /// `deadline` is the opening sync budget. The live follow passes `None`: a
    /// gap with no announcement is a quiet chain, not a failed peer.
    fn drive_until(
        &mut self,
        deadline: Option<Instant>,
        stop_at: Option<[u8; 32]>,
    ) -> Result<[u8; 32], String> {
        loop {
            if deadline.is_some_and(|deadline| Instant::now() > deadline) {
                return Err(if self.peers.len() < HEADER_PEERS_MIN {
                    "too few peers".to_owned()
                } else {
                    "peer disagreement".to_owned()
                });
            }
            let mut progress = false;
            for index in 0..self.peers.len() {
                progress |= self.pump(index)?;
            }
            self.maybe_ping()?;
            if let Some(hash) = self.agreed_tip()? {
                let advanced = stop_at.is_none_or(|previous| previous != hash);
                if advanced
                    && self
                        .peers
                        .iter()
                        .all(|peer| peer.caught_up && peer.tip == hash)
                {
                    self.diverged_at = None;
                    return Ok(hash);
                }
            }
            if !progress {
                thread::sleep(Duration::from_millis(2));
            }
        }
    }

    fn maybe_ping(&mut self) -> Result<(), String> {
        if self.last_ping.elapsed() < Duration::from_secs(30) {
            return Ok(());
        }
        self.last_ping = Instant::now();
        let nonce = 1u64.to_le_bytes();
        for index in 0..self.peers.len() {
            self.send(index, "ping", &nonce)?;
        }
        Ok(())
    }

    fn pump(&mut self, index: usize) -> Result<bool, String> {
        let mut buf = [0u8; 8 * 1024];
        let read = match self.peers[index].pipe.recv_some(&mut buf) {
            Ok(0) => {
                return Err(self.lost_peer());
            }
            Ok(n) => n,
            Err(error)
                if error.kind() == ErrorKind::WouldBlock
                    || error.kind() == ErrorKind::Interrupted =>
            {
                0
            }
            Err(_) => return Err(self.lost_peer()),
        };
        if read > 0 {
            self.peers[index].buf.extend_from_slice(&buf[..read]);
        }
        let mut handled = false;
        loop {
            let msg = match take_message(MAINNET_MAGIC, &mut self.peers[index].buf) {
                Ok(Some(msg)) => msg,
                Ok(None) => break,
                Err(()) => return Err("bad peer message".to_owned()),
            };
            handled = true;
            self.handle(index, &msg.cmd, &msg.payload)?;
        }
        Ok(read > 0 || handled)
    }

    fn lost_peer(&self) -> String {
        if self.peers.len().saturating_sub(1) < HEADER_PEERS_MIN {
            "too few peers".to_owned()
        } else {
            "peer disagreement".to_owned()
        }
    }

    fn handle(&mut self, index: usize, cmd: &str, payload: &[u8]) -> Result<(), String> {
        match cmd {
            "version" => {
                self.peers[index].saw_version = true;
                if !self.peers[index].saw_verack {
                    self.send(index, "verack", &[])?;
                }
                self.ask_if_ready(index)?;
            }
            "verack" => {
                self.peers[index].saw_verack = true;
                self.ask_if_ready(index)?;
            }
            "ping" => {
                if let Some(pong) = encode_pong(payload) {
                    self.send(index, "pong", &pong)?;
                }
            }
            "headers" => self.on_headers(index, payload)?,
            "inv" => self.on_inv(index, payload)?,
            _ => {}
        }
        Ok(())
    }

    fn ask_if_ready(&mut self, index: usize) -> Result<(), String> {
        if self.peers[index].saw_version && self.peers[index].saw_verack && !self.peers[index].asked
        {
            self.send_getheaders(index)?;
        }
        Ok(())
    }

    fn send_getheaders(&mut self, index: usize) -> Result<(), String> {
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

    fn on_headers(&mut self, index: usize, payload: &[u8]) -> Result<(), String> {
        let headers = decode_headers(payload).map_err(|()| "bad peer message".to_owned())?;
        if headers.is_empty() {
            self.peers[index].caught_up = true;
            return Ok(());
        }
        for header in &headers {
            self.chain
                .append(*header)
                .map_err(|fault| fault.message())?;
            self.peers[index].tip = header_hash(header);
        }
        self.peers[index].caught_up = headers.len() < MAX_HEADERS;
        if headers.len() == MAX_HEADERS {
            self.send_getheaders(index)?;
        }
        Ok(())
    }

    fn on_inv(&mut self, index: usize, payload: &[u8]) -> Result<(), String> {
        let items = decode_inv(payload).map_err(|()| "bad peer message".to_owned())?;
        let mut want = false;
        for (kind, hash) in items {
            if is_block_inv(kind) && !self.chain.contains(&hash) {
                want = true;
                break;
            }
        }
        if !want {
            let _ = index;
            return Ok(());
        }
        self.diverged_at.get_or_insert_with(Instant::now);
        for peer in 0..self.peers.len() {
            self.peers[peer].caught_up = false;
            self.send_getheaders(peer)?;
        }
        Ok(())
    }

    fn agreed_tip(&mut self) -> Result<Option<[u8; 32]>, String> {
        if self.peers.iter().any(|peer| !peer.asked || !peer.caught_up) {
            return Ok(None);
        }
        let tip = self.peers[0].tip;
        if self.peers.iter().all(|peer| peer.tip == tip) {
            self.diverged_at = None;
            self.laggard_retry = false;
            return Ok(Some(tip));
        }
        if let Some(since) = self.diverged_at {
            if since.elapsed() >= ANNOUNCE_WAIT {
                return Err("peer disagreement".to_owned());
            }
        } else {
            self.diverged_at = Some(Instant::now());
        }
        if self.laggard_retry {
            return Err("peer disagreement".to_owned());
        }
        self.laggard_retry = true;
        let best = self
            .peers
            .iter()
            .max_by_key(|peer| self.chain.height_of(&peer.tip).unwrap_or(0))
            .map(|peer| peer.tip)
            .unwrap_or(tip);
        let behind: Vec<usize> = self
            .peers
            .iter()
            .enumerate()
            .filter(|(_, peer)| peer.tip != best)
            .map(|(index, _)| index)
            .collect();
        for index in behind {
            self.send_getheaders(index)?;
        }
        Ok(None)
    }

    fn send(&mut self, index: usize, cmd: &str, payload: &[u8]) -> Result<(), String> {
        let bytes = encode_message(MAINNET_MAGIC, cmd, payload);
        self.peers[index]
            .pipe
            .send_all(&bytes)
            .map_err(|_| "too few peers".to_owned())
    }
}

pub(super) fn connect_mainnet(peer: &str) -> Result<Session, String> {
    let streams = dial_two(peer)?;
    let chain = Chain::open_mainnet(ANCHOR_HEADER, ANCHOR_HEIGHT, ANCHOR_TIME)
        .map_err(|fault| fault.message())?;
    let tip = chain.tip_hash();
    let peers = streams
        .into_iter()
        .map(|stream| Peer {
            pipe: Box::new(TcpPipe(stream)),
            buf: Vec::new(),
            saw_version: false,
            saw_verack: false,
            asked: false,
            caught_up: false,
            tip,
        })
        .collect();
    Ok(Session::from_peers(
        chain,
        peers,
        Duration::from_secs(60),
        None,
    ))
}

fn dial_two(peer: &str) -> Result<Vec<TcpStream>, String> {
    let primary =
        resolve_one(peer).ok_or_else(|| "PCOC_LIVE_GOSSIP_PEER did not connect".to_owned())?;
    let mut streams = Vec::new();
    match TcpStream::connect_timeout(&primary, DIAL_WAIT) {
        Ok(stream) => streams.push(prepare(stream)?),
        Err(_) => return Err("PCOC_LIVE_GOSSIP_PEER did not connect".to_owned()),
    }
    for host in MAINNET_DNS_SEEDS {
        if streams.len() >= HEADER_PEERS_MIN {
            break;
        }
        let Ok(addrs) = (*host, 8333u16).to_socket_addrs() else {
            continue;
        };
        for addr in addrs.take(3) {
            if addr == primary {
                continue;
            }
            if let Ok(stream) = TcpStream::connect_timeout(&addr, DIAL_WAIT) {
                streams.push(prepare(stream)?);
                break;
            }
        }
    }
    if streams.len() < HEADER_PEERS_MIN {
        return Err("too few peers".to_owned());
    }
    Ok(streams)
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
    use crate::gossip::chain::{header_hash, mine_header, spoil_proof, synth_bits};
    use crate::gossip::recorded::{ANCHOR_HEIGHT, MAINNET_DNS_SEEDS};

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
    fn peers_that_disagree_fail_closed() {
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
    }

    #[test]
    fn one_peer_is_too_few_and_bad_proof_of_work_fails_closed() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let err = match Session::from_scripted(
            chain,
            vec![ScriptedPeer::new(anchor_hash, vec![], false)],
            Duration::from_secs(2),
        ) {
            Err(message) => message,
            Ok(_) => panic!("one peer was accepted"),
        };
        assert_eq!(err, "too few peers");

        let (chain, _anchor, anchor_hash) = fixed_chain();
        let mut bad = child(anchor_hash, 1_700_000_100);
        spoil_proof(&mut bad);
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![bad], false),
            ScriptedPeer::new(anchor_hash, vec![bad], false),
        ];
        let mut session = Session::from_scripted(chain, peers, Duration::from_secs(2)).unwrap();
        assert_eq!(session.opening_tip().unwrap_err(), "bad proof of work");
    }

    #[test]
    fn a_header_with_the_wrong_difficulty_fails_closed() {
        let (chain, _anchor, anchor_hash) = fixed_chain();
        let mut header = child(anchor_hash, 1_700_000_100);
        header[72..76].copy_from_slice(&0x1d00ffffu32.to_le_bytes());
        let peers = vec![
            ScriptedPeer::new(anchor_hash, vec![header], false),
            ScriptedPeer::new(anchor_hash, vec![header], false),
        ];
        let mut session = Session::from_scripted(chain, peers, Duration::from_secs(2)).unwrap();
        assert_eq!(session.opening_tip().unwrap_err(), "unexpected difficulty");
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
