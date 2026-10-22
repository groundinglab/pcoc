//! Header-source tests for SPEC §9 P9.16, P9.17 and Rule I4 (the mock-clock
//! silent-peer test). No live network. Clocks are [`MockClock`].

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex};

use pcoc_sdk::headers::{
    decode_getheaders, decode_ping, drain_messages, encode_addr, encode_headers, encode_inv,
    encode_message, encode_version, header_hash, header_meets_own_target, header_nbits,
    header_prev, header_time, initial_addresses, ConsensusParams, HeaderChain, HeaderDriver,
    HeaderFault, HeaderSource, HeaderSourceConfig, MockClock, ObservedTip, PeerAddr, Plane,
    ReadOut, BITCOIN_DIFF1_NBITS, HEADER_DIAL_BACKOFF_INITIAL_MS, HEADER_DIAL_BACKOFF_MAX_MS,
    HEADER_DIAL_CONCURRENCY, HEADER_PEERS_MIN, HEADER_PEER_SILENCE_SECONDS, MAINNET_DNS_SEEDS,
    MAINNET_MAGIC, MSG_BLOCK, SYNTH_NBITS,
};
use pcoc_sdk::headers::{Clock, Conn, Net, PollDial};

fn mine(prev: [u8; 32], time: u32, nbits: u32) -> [u8; 80] {
    let mut header = [0u8; 80];
    header[0] = 1;
    header[4..36].copy_from_slice(&prev);
    header[68..72].copy_from_slice(&time.to_le_bytes());
    header[72..76].copy_from_slice(&nbits.to_le_bytes());
    let mut extra = 0u32;
    loop {
        header[36..40].copy_from_slice(&extra.to_le_bytes());
        for nonce in 0u32..200_000 {
            header[76..80].copy_from_slice(&nonce.to_le_bytes());
            if header_meets_own_target(&header) {
                return header;
            }
        }
        extra = extra.wrapping_add(1);
        if extra > 64 {
            panic!("miner exhausted at nbits {nbits:#x}");
        }
    }
}

fn extend(chain: &mut HeaderChain, step: u32, until: u64) -> Vec<[u8; 80]> {
    let mut out = Vec::new();
    while chain.tip_height() < until {
        let parent = chain.tip_hash();
        let nbits = chain.expected_next_nbits(&parent).expect("parent");
        let time = header_time(&chain.tip_header()).saturating_add(step);
        let header = mine(parent, time, nbits);
        chain.append(header).expect("mined header");
        out.push(header);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Honest,
    Silent,
    Refuse,
}

struct PeerSlot {
    kind: Kind,
    delay_ms: u64,
    chain: Vec<[u8; 80]>,
    /// Headers of `chain` served before an inv (inv peers) or in general.
    visible: usize,
    inv_mode: bool,
    inv_sent: bool,
    to_engine: Vec<u8>,
    from_engine: Vec<u8>,
    closed: bool,
    pongs_sent: u32,
    commands: Vec<String>,
    last_locator: Vec<[u8; 32]>,
    getheaders: u32,
}

struct Inner {
    clock: Arc<MockClock>,
    magic: [u8; 4],
    genesis: [u8; 32],
    peers: HashMap<PeerAddr, PeerSlot>,
    pending: HashMap<u64, PeerAddr>,
    next_dial: u64,
    inflight: usize,
    max_inflight: usize,
    dial_log: Vec<(u64, PeerAddr)>,
    killed: HashSet<PeerAddr>,
}

struct ScriptedNet {
    inner: Arc<Mutex<Inner>>,
}

struct MockConn {
    inner: Arc<Mutex<Inner>>,
    addr: PeerAddr,
}

impl Inner {
    fn push_msg(&mut self, addr: &PeerAddr, cmd: &str, payload: &[u8]) {
        let bytes = encode_message(self.magic, cmd, payload);
        if let Some(peer) = self.peers.get_mut(addr) {
            peer.to_engine.extend_from_slice(&bytes);
        }
    }

    fn open(&mut self, addr: &PeerAddr) {
        let peer = self.peers.get_mut(addr).expect("peer");
        peer.closed = false;
        peer.to_engine.clear();
        peer.from_engine.clear();
        peer.commands.clear();
        peer.pongs_sent = 0;
        peer.getheaders = 0;
        peer.inv_sent = false;
    }
}

// Version bytes are built without exporting encode_version twice. The helper
// below is the library's encode_version, re-exported for the mock.
impl Net for ScriptedNet {
    type Conn = MockConn;

    fn start_dial(&mut self, addr: &PeerAddr) -> u64 {
        let mut g = self.inner.lock().expect("net");
        g.inflight += 1;
        g.max_inflight = g.max_inflight.max(g.inflight);
        let now = g.clock.now_ms();
        g.dial_log.push((now, addr.clone()));
        let id = g.next_dial;
        g.next_dial += 1;
        g.pending.insert(id, addr.clone());
        id
    }

    fn poll_dial(&mut self, id: u64) -> PollDial<MockConn> {
        let mut g = self.inner.lock().expect("net");
        g.inflight = g.inflight.saturating_sub(1);
        let Some(addr) = g.pending.remove(&id) else {
            return PollDial::Failed;
        };
        if g.killed.contains(&addr) {
            return PollDial::Failed;
        }
        let kind = g.peers.get(&addr).map(|p| p.kind).unwrap_or(Kind::Refuse);
        if kind == Kind::Refuse {
            return PollDial::Failed;
        }
        let delay = g.peers.get(&addr).map(|p| p.delay_ms).unwrap_or(0);
        let ready = g
            .dial_log
            .iter()
            .rev()
            .find(|(_, a)| a == &addr)
            .map(|(t, _)| t + delay)
            .unwrap_or(0);
        if g.clock.now_ms() < ready {
            g.pending.insert(id, addr);
            g.inflight += 1;
            return PollDial::Pending;
        }
        g.open(&addr);
        // Queue version + verack + a ping (so an ESTAB peer has a ping in flight).
        let version = version_payload();
        let ping = 7u64.to_le_bytes().to_vec();
        let magic = g.magic;
        let peer = g.peers.get_mut(&addr).expect("slot");
        peer.to_engine
            .extend(encode_message(magic, "version", &version));
        peer.to_engine.extend(encode_message(magic, "verack", &[]));
        peer.to_engine.extend(encode_message(magic, "ping", &ping));
        PollDial::Connected(MockConn {
            inner: Arc::clone(&self.inner),
            addr,
        })
    }
}

fn version_payload() -> Vec<u8> {
    encode_version(0, 1, 0)
}

impl Conn for MockConn {
    fn try_read(&mut self, buf: &mut [u8]) -> ReadOut {
        let mut g = self.inner.lock().expect("net");
        let magic = g.magic;
        let genesis = g.genesis;
        let closed = g.peers.get(&self.addr).map(|p| p.closed).unwrap_or(true);
        if closed {
            return ReadOut::Closed;
        }
        service_peer(&mut g, &self.addr, magic, genesis);
        let peer = g.peers.get_mut(&self.addr).expect("slot");
        if peer.to_engine.is_empty() {
            return ReadOut::Empty;
        }
        let n = buf.len().min(peer.to_engine.len());
        buf[..n].copy_from_slice(&peer.to_engine[..n]);
        peer.to_engine.drain(..n);
        ReadOut::N(n)
    }

    fn write_all(&mut self, bytes: &[u8]) {
        let mut g = self.inner.lock().expect("net");
        if let Some(peer) = g.peers.get_mut(&self.addr) {
            peer.from_engine.extend_from_slice(bytes);
        }
    }
}

fn service_peer(g: &mut Inner, addr: &PeerAddr, magic: [u8; 4], genesis: [u8; 32]) {
    let buf = match g.peers.get_mut(addr) {
        Some(peer) => std::mem::take(&mut peer.from_engine),
        None => return,
    };
    let parsed = drain_messages(magic, buf);
    let Ok((msgs, rest)) = parsed else {
        return;
    };
    if let Some(peer) = g.peers.get_mut(addr) {
        peer.from_engine = rest;
    }
    let mut replies: Vec<Vec<u8>> = Vec::new();
    for msg in msgs {
        if let Some(peer) = g.peers.get_mut(addr) {
            peer.commands.push(msg.cmd.clone());
        }
        match msg.cmd.as_str() {
            "ping" => {
                if let Some(nonce) = decode_ping(&msg.payload) {
                    replies.push(encode_message(magic, "pong", &nonce.to_le_bytes()));
                    if let Some(peer) = g.peers.get_mut(addr) {
                        peer.pongs_sent = peer.pongs_sent.saturating_add(1);
                    }
                }
            }
            "getheaders" => {
                let peer = g.peers.get_mut(addr).expect("slot");
                peer.getheaders = peer.getheaders.saturating_add(1);
                if peer.kind == Kind::Silent {
                    continue;
                }
                if let Some(gh) = decode_getheaders(&msg.payload) {
                    peer.last_locator = gh.locator.clone();
                    let limit = if peer.inv_mode && !peer.inv_sent {
                        peer.visible.min(peer.chain.len())
                    } else if peer.inv_mode && peer.inv_sent {
                        peer.chain.len()
                    } else {
                        peer.visible.min(peer.chain.len())
                    };
                    let headers = serve(&peer.chain[..limit], &gh.locator, gh.stop, genesis);
                    if !headers.is_empty() {
                        // Chunk at 2000, which is also the wire maximum.
                        for chunk in headers.chunks(2000) {
                            replies.push(encode_message(magic, "headers", &encode_headers(chunk)));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(peer) = g.peers.get_mut(addr) {
        for reply in replies {
            peer.to_engine.extend(reply);
        }
    }
}

fn serve(
    chain: &[[u8; 80]],
    locator: &[[u8; 32]],
    stop: [u8; 32],
    genesis: [u8; 32],
) -> Vec<[u8; 80]> {
    let mut start = 0usize;
    let mut found = false;
    for hash in locator {
        if *hash == genesis {
            start = 0;
            found = true;
            break;
        }
        if let Some(pos) = chain.iter().position(|h| header_hash(h) == *hash) {
            start = pos + 1;
            found = true;
            break;
        }
    }
    if !found {
        start = 0;
    }
    let mut out = Vec::new();
    for header in chain.iter().skip(start) {
        out.push(*header);
        if stop != [0u8; 32] && header_hash(header) == stop {
            break;
        }
        if out.len() == 2000 {
            break;
        }
    }
    out
}

struct World {
    clock: Arc<MockClock>,
    inner: Arc<Mutex<Inner>>,
    driver: HeaderDriver<ScriptedNet>,
}

fn world(
    params: ConsensusParams,
    addrs: Vec<(PeerAddr, Kind, u64)>,
    shuffle: bool,
    seed: u64,
) -> World {
    let clock = Arc::new(MockClock::new(0));
    let genesis = header_hash(&params.genesis);
    let mut peers = HashMap::new();
    for (addr, kind, delay) in &addrs {
        peers.insert(
            addr.clone(),
            PeerSlot {
                kind: *kind,
                delay_ms: *delay,
                chain: Vec::new(),
                visible: 0,
                inv_mode: false,
                inv_sent: false,
                to_engine: Vec::new(),
                from_engine: Vec::new(),
                closed: false,
                pongs_sent: 0,
                commands: Vec::new(),
                last_locator: Vec::new(),
                getheaders: 0,
            },
        );
    }
    let inner = Arc::new(Mutex::new(Inner {
        clock: Arc::clone(&clock),
        magic: params.magic,
        genesis,
        peers,
        pending: HashMap::new(),
        next_dial: 1,
        inflight: 0,
        max_inflight: 0,
        dial_log: Vec::new(),
        killed: HashSet::new(),
    }));
    let mut cfg =
        HeaderSourceConfig::new(params, addrs.into_iter().map(|(a, _, _)| a).collect(), seed);
    cfg.shuffle = shuffle;
    let driver = HeaderDriver::new(
        cfg,
        Arc::clone(&clock) as Arc<dyn pcoc_sdk::headers::Clock>,
        ScriptedNet {
            inner: Arc::clone(&inner),
        },
    )
    .expect("driver");
    World {
        clock,
        inner,
        driver,
    }
}

impl World {
    fn set_chain(&self, addr: &PeerAddr, chain: Vec<[u8; 80]>, visible: usize, inv_mode: bool) {
        let mut g = self.inner.lock().expect("net");
        let peer = g.peers.get_mut(addr).expect("addr");
        peer.chain = chain;
        peer.visible = visible;
        peer.inv_mode = inv_mode;
    }

    fn push_headers(&self, addr: &PeerAddr, headers: &[[u8; 80]]) {
        let mut g = self.inner.lock().expect("net");
        for chunk in headers.chunks(2000) {
            g.push_msg(addr, "headers", &encode_headers(chunk));
        }
    }

    fn push_inv(&self, addr: &PeerAddr, hash: [u8; 32]) {
        let mut g = self.inner.lock().expect("net");
        if let Some(peer) = g.peers.get_mut(addr) {
            peer.inv_sent = true;
        }
        let payload = encode_inv(&[(MSG_BLOCK, hash)]);
        g.push_msg(addr, "inv", &payload);
    }

    fn kill(&self, addr: &PeerAddr) {
        let mut g = self.inner.lock().expect("net");
        g.killed.insert(addr.clone());
        if let Some(peer) = g.peers.get_mut(addr) {
            peer.closed = true;
        }
    }

    fn pongs_sent(&self, addr: &PeerAddr) -> u32 {
        self.inner
            .lock()
            .expect("net")
            .peers
            .get(addr)
            .map(|p| p.pongs_sent)
            .unwrap_or(0)
    }

    fn commands(&self, addr: &PeerAddr) -> Vec<String> {
        self.inner
            .lock()
            .expect("net")
            .peers
            .get(addr)
            .map(|p| p.commands.clone())
            .unwrap_or_default()
    }

    fn last_locator(&self, addr: &PeerAddr) -> Vec<[u8; 32]> {
        self.inner
            .lock()
            .expect("net")
            .peers
            .get(addr)
            .map(|p| p.last_locator.clone())
            .unwrap_or_default()
    }

    fn getheaders_count(&self, addr: &PeerAddr) -> u32 {
        self.inner
            .lock()
            .expect("net")
            .peers
            .get(addr)
            .map(|p| p.getheaders)
            .unwrap_or(0)
    }

    fn dial_log(&self) -> Vec<(u64, PeerAddr)> {
        self.inner.lock().expect("net").dial_log.clone()
    }

    fn max_inflight(&self) -> usize {
        self.inner.lock().expect("net").max_inflight
    }
}

fn addr(name: &str) -> PeerAddr {
    PeerAddr::new(name, 8333)
}

fn settle(world: &mut World, polls: usize) {
    for _ in 0..polls {
        world.driver.poll();
    }
}

#[derive(Default)]
struct Probe {
    seeded: bool,
    start_h: u64,
    start_hash: [u8; 32],
    stage: u32,
    interrupts: u32,
    legs: u64,
    closed: bool,
}

impl Probe {
    /// One leg, then the post-checkpoint tip read. The read is the value passed
    /// in; this does not call the network.
    fn on_tip(&mut self, tip: Option<ObservedTip>) {
        let Some(tip) = tip else {
            return;
        };
        if !self.seeded {
            self.seeded = true;
            self.start_h = tip.height;
            self.start_hash = tip.natural;
            return;
        }
        if self.closed {
            return;
        }
        self.legs += 1;
        if tip.height > self.start_h && tip.natural != self.start_hash {
            self.interrupts += 1;
            if self.stage == 17 {
                self.closed = true;
                return;
            }
            self.stage += 1;
            self.start_h = tip.height;
            self.start_hash = tip.natural;
        }
    }
}

fn regtest_chain(n: u64) -> (ConsensusParams, Vec<[u8; 80]>) {
    let params = Plane::Test.params();
    let mut chain = HeaderChain::open(params.clone()).expect("genesis");
    let headers = extend(&mut chain, 1, n);
    (params, headers)
}

/// AC3.1. After sync, two sessions stay up. Killing one connects a replacement
/// without pausing legs, and the stand-in walk still closes at I_17.
#[test]
fn ac31_two_peers_replacement_does_not_pause_legs() {
    let (params, headers) = regtest_chain(18);
    assert_eq!(headers.len(), 18);
    let a = addr("A");
    let b = addr("B");
    let c = addr("C");
    let mut w = world(
        params,
        vec![
            (a.clone(), Kind::Honest, 0),
            (b.clone(), Kind::Honest, 0),
            (c.clone(), Kind::Honest, 0),
        ],
        false,
        1,
    );
    w.set_chain(&a, headers.clone(), 0, false);
    w.set_chain(&b, headers.clone(), 0, false);
    w.set_chain(&c, headers.clone(), 0, false);
    settle(&mut w, 4);
    let up = w.driver.connected();
    assert_eq!(
        up.len(),
        HEADER_PEERS_MIN,
        "after initial sync, exactly the minimum stay up"
    );
    assert!(
        up.contains(&a) && up.contains(&b),
        "dial order kept A and B, got {up:?}"
    );

    let mut probe = Probe::default();
    probe.on_tip(w.driver.observe());
    assert!(probe.seeded);
    assert_eq!(probe.legs, 0, "no leg before the seed");

    for h in 1..=18 {
        if h == 6 {
            let legs_before = probe.legs;
            let interrupts_before = probe.interrupts;
            let clock_before = w.clock.now_ms();
            let survivor = w.driver.connected()[0].clone();
            w.kill(&b);
            for _ in 0..6 {
                w.driver.poll();
                probe.on_tip(w.driver.observe());
            }
            let up = w.driver.connected();
            assert_eq!(up.len(), 2, "replacement restores two sessions, got {up:?}");
            assert!(up.contains(&survivor));
            assert!(!up.contains(&b), "killed peer is gone");
            assert!(up.contains(&c), "spare peer replaced the killed one");
            assert!(
                probe.legs > legs_before,
                "legs advanced while the peer was replaced"
            );
            assert_eq!(probe.interrupts, interrupts_before);
            assert_eq!(
                w.clock.now_ms(),
                clock_before,
                "replacement did not wait on the clock"
            );
            for _ in 0..1000 {
                let _ = w.driver.observe();
            }
            assert_eq!(
                w.clock.now_ms(),
                clock_before,
                "observe returns without time passing"
            );
        }
        w.push_headers(&a, &headers[h as usize - 1..h as usize]);
        w.push_headers(&c, &headers[h as usize - 1..h as usize]);
        for _ in 0..4 {
            w.driver.poll();
            if w.driver.tip_height() >= h {
                break;
            }
        }
        assert_eq!(w.driver.tip_height(), h, "header {h} did not arrive");
        let before = probe.interrupts;
        probe.on_tip(w.driver.observe());
        assert_eq!(probe.interrupts, before + 1);
    }
    assert!(probe.closed, "object closes at I_17");
    assert_eq!(probe.interrupts, 18);
    assert!(probe.legs >= 18);
    let fee = w.driver.fee_header().expect("fee header");
    let tip = w.driver.observe().expect("tip");
    assert_eq!(
        header_hash(&fee),
        tip.natural,
        "fee header hash is natural order"
    );
    let mut reversed = tip.natural;
    reversed.reverse();
    assert_ne!(header_hash(&fee), reversed);
    assert_eq!(
        header_prev(&fee),
        header_hash(&headers[16]),
        "prev field is the parent's natural hash"
    );
    assert_eq!(w.driver.connected().len(), 2);
}

/// AC3.2 and the RC2 case. Silence is 120s from the other peer's announcement,
/// not a closed socket and not an unanswered ping. A gap with no announcement
/// replaces nobody.
#[test]
fn ac32_silent_peer_at_120s_not_before_and_gap_replaces_nobody() {
    let (params, headers) = regtest_chain(1);
    let a = addr("announcer");
    let b = addr("silent");
    let c = addr("spare");
    let mut w = world(
        params.clone(),
        vec![
            (a.clone(), Kind::Honest, 0),
            (b.clone(), Kind::Silent, 0),
            (c.clone(), Kind::Honest, 0),
        ],
        false,
        2,
    );
    w.set_chain(&a, headers.clone(), 1, false);
    w.set_chain(&c, headers.clone(), 1, false);

    let mut guard = 0;
    while w.driver.tip_height() < 1 && guard < 12 {
        w.driver.poll();
        guard += 1;
    }
    assert_eq!(w.clock.now_ms(), 0);
    assert_eq!(
        w.driver.tip_height(),
        1,
        "announcer delivered the header at t=0"
    );
    let ids_before = w.driver.session_ids();
    assert_eq!(ids_before.len(), 2);
    assert!(ids_before.iter().any(|(_, p)| p == &b));
    let announcer_id = ids_before.iter().find(|(_, p)| p == &a).unwrap().0;

    // Just before 120s: ESTAB, pings answered, still the same silent session.
    w.clock.set(HEADER_PEER_SILENCE_SECONDS * 1000 - 1);
    w.driver.poll();
    w.driver.poll();
    assert!(
        w.driver.connected().contains(&b),
        "not replaced before 120s"
    );
    assert!(w.pongs_sent(&b) >= 1, "silent peer answered a ping");
    assert!(
        w.driver.pongs_from(&b) >= 1,
        "engine saw the pong and did not treat it as an announcement"
    );
    assert!(
        w.commands(&b).iter().any(|c| c == "pong") || w.commands(&b).iter().any(|c| c == "ping")
    );

    let mut probe = Probe::default();
    probe.on_tip(w.driver.observe());
    let legs_at_119 = {
        probe.on_tip(w.driver.observe());
        probe.legs
    };

    w.clock.set(HEADER_PEER_SILENCE_SECONDS * 1000);
    w.driver.poll();
    w.driver.poll();
    w.driver.poll();
    probe.on_tip(w.driver.observe());
    let up = w.driver.connected();
    assert!(
        !up.contains(&b),
        "silent peer replaced at 120s, still up: {up:?}"
    );
    assert!(
        up.contains(&a) && up.contains(&c),
        "announcer stays and the spare connects: {up:?}"
    );
    assert_eq!(up.len(), 2);
    let ids = w.driver.session_ids();
    assert!(ids.iter().any(|(id, p)| p == &a && *id == announcer_id));
    assert!(
        probe.legs > legs_at_119,
        "legs kept running across the replacement"
    );

    // Gap: nobody announces anything. Ten minutes later the same two sessions remain.
    let ga = addr("gap-a");
    let gb = addr("gap-b");
    let mut gap = world(
        params,
        vec![(ga.clone(), Kind::Silent, 0), (gb.clone(), Kind::Silent, 0)],
        false,
        3,
    );
    settle(&mut gap, 6);
    let before = gap.driver.session_ids();
    assert_eq!(before.len(), 2);
    gap.clock.set(600_000);
    gap.driver.poll();
    gap.driver.poll();
    assert_eq!(
        gap.driver.session_ids(),
        before,
        "a block gap replaces no one"
    );
    assert_eq!(gap.driver.connected().len(), 2);
}

/// AC3.3. Every peer refuses for 30 minutes of mock time. Dial waves stay at
/// the SDK concurrency cap and the gaps double until the SDK cap. One alert.
#[test]
fn ac33_refusal_backoff_is_bounded_and_one_alert() {
    let params = Plane::Test.params();
    let addrs: Vec<(PeerAddr, Kind, u64)> = (0..20)
        .map(|i| (addr(&format!("refuse-{i}")), Kind::Refuse, 0))
        .collect();
    let mut w = world(params, addrs, false, 4);
    let mut probe = Probe::default();
    probe.on_tip(w.driver.observe());
    assert_eq!(probe.legs, 0);

    let end = 30 * 60 * 1000;
    for sec in 0..=(end / 1000) {
        w.clock.set(sec * 1000);
        w.driver.poll();
        w.driver.poll();
        probe.on_tip(w.driver.observe());
    }
    assert_eq!(
        probe.legs,
        end / 1000 + 1,
        "a leg ran on every mock second while every dial failed"
    );
    assert!(
        !probe.closed,
        "no new header, so the stage does not invent I_17"
    );
    assert!(w.driver.connected().is_empty());
    assert!(w.max_inflight() <= HEADER_DIAL_CONCURRENCY);
    assert_eq!(w.max_inflight(), HEADER_DIAL_CONCURRENCY);

    let log = w.dial_log();
    let mut waves: Vec<u64> = Vec::new();
    let mut counts: HashMap<u64, usize> = HashMap::new();
    for (t, _) in &log {
        *counts.entry(*t).or_insert(0) += 1;
        if waves.last() != Some(t) {
            waves.push(*t);
        }
    }
    for (t, n) in &counts {
        assert_eq!(
            *n, HEADER_DIAL_CONCURRENCY,
            "wave at {t} dialed {n}, cap is {HEADER_DIAL_CONCURRENCY}"
        );
    }
    let mut expected = vec![0u64];
    let mut t = 0u64;
    let mut backoff = HEADER_DIAL_BACKOFF_INITIAL_MS;
    loop {
        let next = t + backoff;
        if next > end {
            break;
        }
        expected.push(next);
        t = next;
        backoff = (backoff * 2).min(HEADER_DIAL_BACKOFF_MAX_MS);
    }
    assert_eq!(
        waves, expected,
        "dial waves are the exponential schedule, not a storm"
    );
    let gaps: Vec<u64> = waves.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert_eq!(gaps[0], HEADER_DIAL_BACKOFF_INITIAL_MS);
    assert_eq!(gaps[1], HEADER_DIAL_BACKOFF_INITIAL_MS * 2);
    assert_eq!(gaps[2], HEADER_DIAL_BACKOFF_INITIAL_MS * 4);
    assert!(gaps.iter().all(|g| *g <= HEADER_DIAL_BACKOFF_MAX_MS));
    assert!(
        log.len() < 1_000,
        "30 min of refusals dialed {} times",
        log.len()
    );
    assert!(log.len() > HEADER_DIAL_CONCURRENCY);

    let alerts = w.driver.drain_alerts();
    assert_eq!(
        alerts.len(),
        1,
        "one header_peers_low, not one per dial: {alerts:?}"
    );
    assert_eq!(alerts[0].alert, "header_peers_low");
    assert_eq!(
        alerts[0].event_json(4),
        "{\"event\":\"hike.produce.alert\",\"alert\":\"header_peers_low\",\"stage\":4}"
    );
    assert!(w.driver.drain_alerts().is_empty());
}

/// AC3.4. Six producers, one host, 20 addresses. Each keeps its own shuffle's
/// first two, not the two that answer first. Books are not shared. No default
/// gossip peer and no cache file.
#[test]
fn ac34_six_producers_do_not_share_the_fastest_peers() {
    let params = Plane::Test.params();
    let mut spec = Vec::new();
    spec.push((addr("fast-0"), Kind::Honest, 0u64));
    spec.push((addr("fast-1"), Kind::Honest, 0));
    for i in 2..20 {
        spec.push((addr(&format!("peer-{i}")), Kind::Honest, 10_000));
    }
    let fast_pair: BTreeSet<PeerAddr> = [addr("fast-0"), addr("fast-1")].into_iter().collect();

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("ac34-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut worlds = Vec::new();
    let mut sets = Vec::new();
    for seed in 1..=6 {
        let mut w = world(params.clone(), spec.clone(), true, seed);
        // Two polls at t=0 start the wave and take anyone already ready.
        w.driver.poll();
        w.driver.poll();
        w.clock.set(10_000);
        w.driver.poll();
        w.driver.poll();
        let up = w.driver.connected();
        let order = w.driver.dial_order();
        assert_eq!(
            up,
            order[..2].to_vec(),
            "seed {seed} kept {:?} but its dial order starts {:?}",
            up,
            &order[..2]
        );
        sets.push(BTreeSet::from_iter(up.iter().cloned()));
        worlds.push(w);
    }
    assert!(
        sets.iter().any(|s| s != &fast_pair),
        "all six latched onto the fastest two: {sets:?}"
    );
    let union: BTreeSet<_> = sets.iter().flatten().cloned().collect();
    assert!(union.len() > 2, "peer sets collapsed: {sets:?}");

    // An address one producer learns does not appear in another's book.
    let learned = {
        let mut ip = [0u8; 16];
        ip[10] = 0xff;
        ip[11] = 0xff;
        ip[12] = 9;
        ip[13] = 9;
        ip[14] = 9;
        ip[15] = 9;
        let payload = encode_addr(&[(ip, 9333)]);
        let who = worlds[0].driver.connected()[0].clone();
        {
            let mut g = worlds[0].inner.lock().expect("net");
            g.push_msg(&who, "addr", &payload);
        }
        worlds[0].driver.poll();
        PeerAddr::new("9.9.9.9", 9333)
    };
    assert!(worlds[0].driver.address_book().contains(&learned));
    assert!(!worlds[1].driver.address_book().contains(&learned));

    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "no shared cache file"
    );
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        initial_addresses(Plane::Test, &[]).is_empty(),
        "test plane has no DNS seed list"
    );
    let seeds = initial_addresses(Plane::Live, &[]);
    let hosts: Vec<&str> = seeds.iter().map(|a| a.host.as_str()).collect();
    assert_eq!(hosts, MAINNET_DNS_SEEDS);
    assert!(seeds.iter().all(|a| a.port == 8333));
    let named = addr("operator.example");
    assert_eq!(
        initial_addresses(Plane::Live, &[named.clone()]),
        vec![named]
    );
    let cfg = HeaderSourceConfig::new(Plane::Live.params(), vec![], 0);
    assert!(cfg.shuffle, "production dial order is per-process");
    assert!(cfg.addresses.is_empty(), "no default gossip peer");
}

/// AC3.5. A peer that announces only by inv is answered with getheaders and a
/// locator that still finds a common block more than 8 back. One round trip.
#[test]
fn ac35_inv_becomes_getheaders_with_locator_over_8_gap() {
    let (params, headers) = regtest_chain(20);
    assert!(headers.len() > 8);
    let a = addr("sync");
    let b = addr("inv");
    let mut w = world(
        params,
        vec![(a.clone(), Kind::Honest, 0), (b.clone(), Kind::Honest, 0)],
        false,
        5,
    );
    w.set_chain(&a, headers.clone(), 10, false);
    w.set_chain(&b, headers.clone(), 10, true);

    let mut guard = 0;
    while w.driver.tip_height() < 10 && guard < 12 {
        w.driver.poll();
        guard += 1;
    }
    assert_eq!(
        w.driver.tip_height(),
        10,
        "synced through the common prefix"
    );
    let before = w.getheaders_count(&b);
    let hash20 = header_hash(&headers[19]);
    let hash10 = header_hash(&headers[9]);
    let hash1 = header_hash(&headers[0]);
    w.push_inv(&b, hash20);
    w.driver.poll();
    w.driver.poll();
    assert_eq!(
        w.driver.tip_height(),
        20,
        "inv header arrived in one getheaders round trip"
    );
    assert_eq!(
        w.getheaders_count(&b),
        before + 1,
        "one getheaders after the inv, not a second fetch"
    );
    let cmds = w.commands(&b);
    assert!(
        !cmds.iter().any(|c| c == "getdata"),
        "inv is not answered with getdata: {cmds:?}"
    );
    assert!(w.driver.contains(&hash20));
    assert!(
        w.driver.contains(&header_hash(&headers[10])),
        "intermediate header 11 stored"
    );
    assert!(w.driver.contains(&header_hash(&headers[17])));

    let locator = w.last_locator(&b);
    assert!(locator.contains(&hash10), "locator has the tip we had");
    assert!(
        locator.contains(&hash1),
        "locator reaches more than 8 blocks back"
    );
    assert_eq!(locator[0], hash10, "locator hash is natural order");
    let mut reversed = hash10;
    reversed.reverse();
    assert_ne!(
        locator[0], reversed,
        "a display-order locator is not this locator"
    );
    let gap = 20 - 10;
    assert!(gap > 8);
}

/// AC3.6. A 2,000-header diff-1 batch under mainnet magic is rejected and the
/// peer is bad. A higher fork with less work loses to a lower fork with more.
#[test]
fn ac36_diff1_batch_rejected_and_more_work_beats_more_height() {
    let params = mineable_mainnet();
    assert_eq!(params.magic, MAINNET_MAGIC);

    // --- diff-1 batch ---
    let peer = addr("diff1");
    let mut w = world(
        params.clone(),
        vec![(peer.clone(), Kind::Honest, 0)],
        false,
        6,
    );
    settle(&mut w, 4);
    assert_eq!(w.driver.tip_height(), 0);
    let mut batch = Vec::with_capacity(2000);
    let mut prev = header_hash(&params.genesis);
    let mut time = header_time(&params.genesis).saturating_add(1);
    for _ in 0..2000 {
        let mut header = [0u8; 80];
        header[0] = 1;
        header[4..36].copy_from_slice(&prev);
        header[68..72].copy_from_slice(&time.to_le_bytes());
        header[72..76].copy_from_slice(&BITCOIN_DIFF1_NBITS.to_le_bytes());
        prev = header_hash(&header);
        time = time.saturating_add(1);
        batch.push(header);
    }
    let raw = encode_message(MAINNET_MAGIC, "headers", &encode_headers(&batch));
    assert_eq!(&raw[..4], &MAINNET_MAGIC);
    {
        let mut g = w.inner.lock().expect("net");
        g.peers.get_mut(&peer).unwrap().to_engine.extend(raw);
    }
    w.driver.poll();
    assert!(w.driver.is_bad(&peer), "diff-1 peer is scored bad");
    match w.driver.ban_fault(&peer) {
        Some(HeaderFault::UnexpectedNBits {
            height: 1,
            expected,
            got,
        }) => {
            assert_eq!(*expected, SYNTH_NBITS);
            assert_eq!(*got, BITCOIN_DIFF1_NBITS);
        }
        other => panic!("expected UnexpectedNBits, got {other:?}"),
    }
    assert_eq!(
        w.driver.tip_height(),
        0,
        "none of the diff-1 batch was stored"
    );
    assert!(!w.driver.contains(&header_hash(&batch[0])));
    assert!(!w.driver.contains(&header_hash(&batch[1999])));

    // --- chainwork: slow tall fork vs fast short fork ---
    let mut fast_chain = HeaderChain::open(params.clone()).expect("fast");
    let mut slow_chain = HeaderChain::open(params.clone()).expect("slow");
    let fast_headers = extend(&mut fast_chain, 1, 2016 + 50);
    let slow_headers = extend(&mut slow_chain, 3_000, fast_chain.tip_height() + 30);
    assert!(slow_chain.tip_height() > fast_chain.tip_height());
    let n_fast = header_nbits(&fast_headers[2015]);
    let n_slow = header_nbits(&slow_headers[2015]);
    assert_eq!(n_slow, SYNTH_NBITS, "slow retarget stays at the pow limit");
    assert_ne!(n_fast, n_slow, "fast interval retargets at height 2016");

    let feeder = addr("forks");
    let mut eng = world(
        params.clone(),
        vec![(feeder.clone(), Kind::Honest, 0)],
        false,
        7,
    );
    settle(&mut eng, 4);
    eng.push_headers(&feeder, &slow_headers);
    for _ in 0..8 {
        eng.driver.poll();
    }
    assert_eq!(eng.driver.tip_height(), slow_chain.tip_height());
    assert_eq!(eng.driver.tip_hash(), slow_chain.tip_hash());
    eng.push_headers(&feeder, &fast_headers);
    for _ in 0..8 {
        eng.driver.poll();
    }
    assert_eq!(
        eng.driver.tip_hash(),
        fast_chain.tip_hash(),
        "lower heavier fork becomes the tip"
    );
    assert_eq!(eng.driver.tip_height(), fast_chain.tip_height());
    assert!(eng.driver.tip_height() < slow_chain.tip_height());
    assert!(
        eng.driver.contains(&slow_chain.tip_hash()),
        "the taller fork is kept, it just loses"
    );
    let tip = eng.driver.observe().unwrap();
    assert_eq!(tip.natural, fast_chain.tip_hash());
    assert_eq!(tip.height, fast_chain.tip_height());

    // A header whose own nBits proof-of-work passes, but whose nBits is not
    // the expected retarget, is rejected. That is the overnight diff-1 hole.
    let parent = header_hash(&fast_headers[2014]);
    let expected = fast_chain.expected_next_nbits(&parent).unwrap();
    assert_ne!(expected, SYNTH_NBITS);
    let bad = mine(
        parent,
        header_time(&fast_headers[2014]).saturating_add(1),
        SYNTH_NBITS,
    );
    assert!(
        header_meets_own_target(&bad),
        "naive own-nBits check would accept this header"
    );
    assert_ne!(header_nbits(&bad), expected);
    let sneaky = addr("own-nbits");
    // Deliver on a fresh engine that already holds the fast parent, from a second peer.
    let mut parent_only = HeaderChain::open(params.clone()).unwrap();
    let prefix: Vec<[u8; 80]> = fast_headers.iter().take(2015).copied().collect();
    for header in &prefix {
        parent_only.append(*header).unwrap();
    }
    let mut trap = world(
        params,
        vec![
            (addr("prefix"), Kind::Honest, 0),
            (sneaky.clone(), Kind::Honest, 0),
        ],
        false,
        8,
    );
    settle(&mut trap, 4);
    let prefix_peer = addr("prefix");
    trap.push_headers(&prefix_peer, &prefix);
    for _ in 0..6 {
        trap.driver.poll();
    }
    assert_eq!(trap.driver.tip_height(), 2015);
    trap.push_headers(&sneaky, &[bad]);
    trap.driver.poll();
    assert!(trap.driver.is_bad(&sneaky));
    assert!(
        matches!(
            trap.driver.ban_fault(&sneaky),
            Some(HeaderFault::UnexpectedNBits { .. })
        ),
        "fault was {:?}",
        trap.driver.ban_fault(&sneaky)
    );
    assert!(!trap.driver.contains(&header_hash(&bad)));
    assert_eq!(trap.driver.tip_height(), 2015);
}

fn mineable_mainnet() -> ConsensusParams {
    let genesis = mine([0u8; 32], 1_700_000_000, SYNTH_NBITS);
    ConsensusParams {
        magic: MAINNET_MAGIC,
        pow_limit_bits: SYNTH_NBITS,
        fixed_nbits: None,
        genesis,
        port: 8333,
    }
}
