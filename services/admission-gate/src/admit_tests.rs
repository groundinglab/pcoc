//! Gate tests for SPEC 2.2 §14–§15 on the 2.2 record and gl-admit-v10.
//!
//! A production terrain is filled only by `o3_prod_release_door_accepts_and_a_forged_checkpoint_does_not`,
//! and only in a release build. Every other recompute uses a conformance-small profile.

use crate::canon::{self, hex32};
use crate::door::{Door, FastPhase, LockedThorough, PendingAdmit};
use crate::fast::with_fast_elapsed;
use crate::policy::AdmissionPolicy;
use crate::thorough::{self, DrawFile};
use crate::wire::{self, LockedBody};
use pcoc::object::HikeObject;
use pcoc::path::{self, PathBody};
use pcoc::profile::Profile;
use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ORIGIN: &str = "https://groundinglab.io";
const O3_SHA: &str = "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7";

fn terrain_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

fn scratch(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pcoc-admit-{label}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn object_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../sdk/rust/objects/{name}"));
    std::fs::read(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn profile_of(obj: &HikeObject) -> Profile {
    let terrain = obj
        .n_slots
        .checked_mul(obj.slot_bytes)
        .expect("terrain bytes") as usize;
    Profile::conformance_small(
        obj.sample_bytes as usize,
        obj.slot_bytes as usize,
        terrain,
        obj.steps_per_leg,
    )
    .expect("conformance-small profile")
}

fn v10() -> AdmissionPolicy {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../policy/groundinglab-admission/gl-admit-v10.toml");
    let raw = std::fs::read_to_string(&path).unwrap();
    let mut policy = crate::levers::load(&raw, 2).expect("gl-admit-v10");
    // Fixtures such as O3 sit below the production born height. This value
    // belongs to the policy this call returns. It is not process state.
    policy.born_height = 0;
    policy
}

struct Pol {
    min_anchored: u64,
    p: u64,
    q: u64,
    k: u64,
    held_retry: u64,
    reverify: u64,
    max_blocks: u64,
    rounds: u64,
    provisional: &'static str,
    fast: &'static str,
    async_s: &'static str,
    lifetime: u64,
    skew: u64,
    retention: u64,
    session: Option<u64>,
    object_cap: Option<u64>,
    born_height: u64,
}

impl Default for Pol {
    fn default() -> Self {
        Self {
            min_anchored: 6,
            p: 1,
            q: 5,
            k: 3,
            held_retry: 600,
            reverify: 3,
            max_blocks: 6,
            rounds: 3,
            provisional: "on",
            fast: "2",
            async_s: "22.05",
            lifetime: 120,
            skew: 5,
            retention: 600,
            session: None,
            object_cap: None,
            born_height: 0,
        }
    }
}

impl Pol {
    fn load(&self) -> AdmissionPolicy {
        let mut extra = String::new();
        if let Some(cap) = self.session {
            extra.push_str(&format!("verify_cap_per_door_session = {cap}\n"));
        }
        if let Some(cap) = self.object_cap {
            extra.push_str(&format!("verify_cap_per_object = {cap}\n"));
        }
        let raw = format!(
            r#"
policy_id = "gl-admit-v10"
spec = "2.2"
[levers]
min_anchored_paths = {min}
leg_rate_floor = {{ p = {p}, q = {q} }}
submission_deadline_blocks = {k}
held_retry_seconds = {retry}
reverify_max_attempts = {reverify}
max_held_blocks = {blocks}
thorough_rounds = {rounds}
provisional = "{provisional}"
fast_accept_bound = {fast}
async_catch_budget = {async_s}
challenge_lifetime_seconds = {life}
challenge_clock_skew_seconds = {skew}
result_retention_seconds = {retention}
{extra}
[policy_rules]
expected_network = "main"
born_height = {born}
"#,
            min = self.min_anchored,
            p = self.p,
            q = self.q,
            k = self.k,
            retry = self.held_retry,
            reverify = self.reverify,
            blocks = self.max_blocks,
            rounds = self.rounds,
            provisional = self.provisional,
            fast = self.fast,
            async_s = self.async_s,
            life = self.lifetime,
            skew = self.skew,
            retention = self.retention,
            born = self.born_height,
        );
        crate::levers::load(&raw, 2).unwrap_or_else(|err| panic!("{err}\n{raw}"))
    }
}

fn open_door(policy: AdmissionPolicy, index: &str) -> Door {
    let dir = scratch("door");
    Door::open(
        &dir,
        policy,
        index,
        Duration::from_secs(2),
        vec![ORIGIN.to_owned()],
    )
    .expect("door")
}

fn job_for(object: &[u8], nonce: [u8; 32]) -> LockedThorough {
    let object_sha = canon::sha256(object);
    let public_key = pcoc::decode_object(object)
        .map(|obj| obj.public_key)
        .unwrap_or([0u8; 32]);
    let mut mixed = object_sha.to_vec();
    mixed.extend_from_slice(&nonce);
    LockedThorough {
        nonce,
        origin: ORIGIN.to_owned(),
        submission: canon::sha256(&mixed),
        public_key,
        object_sha,
        object: object.to_vec(),
        prior_accept: None,
    }
}

/// Pinned O3 stores an empty path on every `W_1` … `W_16` (a skip). Rule B4
/// then refuses at once under `min_anchored_paths = 6`, because a re-read
/// cannot anchor a skip. This copy keeps those legs and adds `count`
/// in-window paths so a fast phase that meets the lever can be provisional.
fn with_anchored_paths(bytes: &[u8], count: usize) -> Vec<u8> {
    let mut obj = pcoc::decode_object(bytes).expect("object");
    for g in 1..=count {
        let posted = obj.stages[g].beacon_height;
        let block_h = posted + 2;
        let hash = interrupt_hash(&obj, block_h).unwrap_or_else(|| {
            panic!("no stored interrupt at {block_h} for stage {g} posted at {posted}")
        });
        let waypoint = obj.stages[g].waypoint;
        obj.stages[g].path = Some(canonical_path(&waypoint, &hash, block_h as u32));
    }
    pcoc::encode_object(&obj)
}

fn interrupt_hash(obj: &HikeObject, height: u64) -> Option<[u8; 32]> {
    for stage in obj.stages.iter().skip(1) {
        if !stage.height_over && stage.beacon_height == height {
            return Some(stage.beacon);
        }
    }
    if obj.has_last
        && obj.has_last_height
        && !obj.last_height_over
        && obj.last_interrupt_height == height
    {
        return Some(obj.last_interrupt);
    }
    None
}

/// One-leaf canonical path. `root` is `SHA-256(0x1a ‖ waypoint)`.
fn canonical_path(waypoint: &[u8; 32], block_hash: &[u8; 32], block_height: u32) -> Vec<u8> {
    let root = pcoc::hash::sha256_parts(&[&[0x1a], waypoint]);
    let mut out = Vec::with_capacity(146);
    out.push(0x01);
    out.extend_from_slice(waypoint);
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&1u32.to_be_bytes());
    out.push(0);
    out.extend_from_slice(&root);
    out.extend_from_slice(&[0x11; 32]);
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(block_hash);
    out.extend_from_slice(&block_height.to_be_bytes());
    debug_assert_eq!(out.len(), 146);
    out
}

fn anchored_o3() -> Vec<u8> {
    with_anchored_paths(&object_bytes("O3-PROD.cbor"), 6)
}

fn chain(door: &Door) -> Vec<Value> {
    let raw = std::fs::read(door.log_dir().join("chain.jsonl")).unwrap_or_default();
    String::from_utf8(raw)
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).expect("chain line"))
        .collect()
}

fn client_code(body: &Value) -> String {
    body.get("error")
        .and_then(|item| item.get("code"))
        .and_then(|item| item.as_str())
        .unwrap_or("delivery")
        .to_owned()
}

fn cover(total: u64) -> Vec<Vec<u64>> {
    let mut runs = Vec::new();
    let mut next = 0u64;
    while next < total {
        let mut run = Vec::with_capacity(6);
        for _ in 0..6 {
            run.push(if next < total { next } else { 0 });
            if next < total {
                next += 1;
            }
        }
        runs.push(run);
    }
    runs
}

#[derive(Clone, Copy)]
enum Clock {
    Split { last_time: i128 },
    Identity,
}

fn sample(clock: Clock, height: u64, last: u64) -> i128 {
    match clock {
        Clock::Identity => i128::from(height),
        Clock::Split { last_time } => {
            let lo = last.saturating_sub(10);
            if height >= lo && height <= last {
                last_time
            } else {
                0
            }
        }
    }
}

fn realized(obj: &HikeObject, clock: Clock) -> i128 {
    let first = obj.stages[0].beacon_height;
    let last = obj.last_interrupt_height;
    pcoc::hike_span(first, last, |height| Some(sample(clock, height, last))).unwrap_or(0)
}

fn honest_clock(obj: &HikeObject) -> Clock {
    let legs = pcoc::hike_legs(obj).unwrap_or(0);
    for clock in [Clock::Split { last_time: 1 }, Clock::Identity] {
        let span = realized(obj, clock);
        if span > 0 && legs > 0 && 5 * legs >= span {
            return clock;
        }
    }
    Clock::Split { last_time: 1 }
}

#[derive(Clone, Copy)]
enum Face {
    Honest,
    HeaderBad,
    HeaderOnce,
    HeaderFresh,
    PlantBad,
    PlantOnce,
    Synthetic,
    NoNetwork,
    Wide,
    TipGap(u64),
    TipBelow,
    Down,
    DownThenHonest,
}

struct Index {
    base: String,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Index {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(addr) = self.base.strip_prefix("http://") {
            let _ = TcpStream::connect_timeout(
                &addr.parse().expect("addr"),
                Duration::from_millis(200),
            );
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

enum Shot {
    Doc(String),
    Fail,
}

fn display_hex(natural: &[u8; 32]) -> String {
    hex::encode(pcoc::hexutil::reverse32(natural))
}

fn natural_at(obj: &HikeObject, height: u64) -> [u8; 32] {
    for stage in &obj.stages {
        if stage.beacon_height == height {
            return stage.beacon;
        }
    }
    if obj.has_last_height && obj.last_interrupt_height == height {
        return obj.last_interrupt;
    }
    let mut filler = [0u8; 32];
    filler[..8].copy_from_slice(&height.to_be_bytes());
    filler
}

fn plants_at(obj: &HikeObject, height: u64, flip: bool) -> Vec<PathBody> {
    let mut out = Vec::new();
    for stage in obj.stages.iter().skip(1).take(16) {
        let Some(bytes) = stage.path.as_ref() else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        let Some(mut body) = path::decode_path(bytes) else {
            continue;
        };
        if body.block_height as u64 != height {
            continue;
        }
        if flip {
            body.root[0] ^= 0xff;
        }
        out.push(body);
    }
    out
}

fn document(
    obj: &HikeObject,
    height: u64,
    network: Option<&str>,
    clock: Clock,
    flip_hash: bool,
    flip_plant: bool,
) -> String {
    let last = obj.last_interrupt_height;
    let mut natural = natural_at(obj, height);
    if flip_hash {
        natural[0] ^= 0xff;
    }
    let time = u64::try_from(sample(clock, height, last).max(0)).unwrap_or(0);
    let time = time.min(u32::MAX as u64);
    let plants = plants_at(obj, height, flip_plant);
    let mut plant_json = String::new();
    if !plants.is_empty() {
        let items: Vec<String> = plants
            .iter()
            .map(|body| {
                format!(
                    r#"{{"root":"{}","txid":"{}","vout":{},"block_hash":"{}","block_height":{}}}"#,
                    hex::encode(body.root),
                    display_hex(&body.txid),
                    body.vout,
                    display_hex(&body.block_hash),
                    body.block_height
                )
            })
            .collect();
        plant_json = format!(r#","plants":[{}]"#, items.join(","));
    }
    let network = match network {
        Some(text) => format!(r#""network":"{text}","#),
        None => String::new(),
    };
    format!(
        r#"{{{network}"header":{{"hash":"{}","height":{height},"time":{time}}}{plant_json}}}"#,
        display_hex(&natural)
    )
}

fn first_plant_height(obj: &HikeObject) -> Option<u64> {
    obj.stages.iter().skip(1).take(16).find_map(|stage| {
        let bytes = stage.path.as_ref()?;
        if bytes.is_empty() {
            return None;
        }
        path::decode_path(bytes).map(|body| body.block_height as u64)
    })
}

fn needed_heights(obj: &HikeObject) -> BTreeSet<u64> {
    let mut heights = BTreeSet::new();
    for stage in &obj.stages {
        heights.insert(stage.beacon_height);
    }
    if obj.has_last_height {
        heights.insert(obj.last_interrupt_height);
    }
    let mut windows = Vec::new();
    if let Some(first) = obj.stages.first() {
        windows.push(first.beacon_height);
    }
    if obj.has_last_height {
        windows.push(obj.last_interrupt_height);
    }
    for height in windows {
        let n = std::cmp::min(pcoc::constants::MTP_WINDOW, height.saturating_add(1));
        let start = height + 1 - n;
        for item in start..=height {
            heights.insert(item);
        }
    }
    for stage in obj.stages.iter().skip(1).take(16) {
        let Some(bytes) = stage.path.as_ref() else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        if let Some(body) = path::decode_path(bytes) {
            heights.insert(body.block_height as u64);
        }
    }
    heights
}

fn serve(obj: &HikeObject, face: Face) -> Index {
    let clock = match face {
        Face::Wide => {
            let legs = pcoc::hike_legs(obj).unwrap_or(1).max(1);
            Clock::Split {
                last_time: (5 * legs + 1).min(u32::MAX as i128),
            }
        }
        _ => honest_clock(obj),
    };
    let network = match face {
        Face::Synthetic => Some("synthetic"),
        Face::NoNetwork => None,
        _ => Some("main"),
    };
    let header_height = obj.stages.first().map(|stage| stage.beacon_height);
    let plant_height = first_plant_height(obj);
    let mut routes: HashMap<String, Vec<Shot>> = HashMap::new();
    let fail_all = matches!(face, Face::Down);
    let fail_first = matches!(face, Face::DownThenHonest);
    if !matches!(face, Face::Down | Face::TipBelow | Face::TipGap(_)) {
        for height in needed_heights(obj) {
            let path = format!("/index/{height}");
            let flip_hash = match face {
                Face::HeaderBad | Face::HeaderOnce | Face::HeaderFresh => {
                    header_height == Some(height)
                }
                _ => false,
            };
            let flip_plant = match face {
                Face::PlantBad | Face::PlantOnce => plant_height == Some(height),
                _ => false,
            };
            let bad = document(obj, height, network, clock, flip_hash, flip_plant);
            let good = document(obj, height, network, clock, false, false);
            let shots = match face {
                Face::HeaderOnce if flip_hash => vec![Shot::Doc(bad), Shot::Fail],
                Face::HeaderFresh if flip_hash => vec![Shot::Doc(bad), Shot::Doc(good)],
                Face::PlantOnce if flip_plant => vec![Shot::Doc(bad), Shot::Fail],
                Face::HeaderBad | Face::PlantBad if flip_hash || flip_plant => {
                    vec![Shot::Doc(bad)]
                }
                _ => vec![Shot::Doc(good)],
            };
            routes.insert(path, shots);
        }
    }
    let tip_height = match face {
        Face::TipGap(gap) => obj.last_interrupt_height.saturating_add(gap),
        Face::TipBelow => obj.last_interrupt_height.saturating_sub(1),
        _ => obj.last_interrupt_height,
    };
    if !matches!(face, Face::Down) {
        let tip = document(obj, tip_height, network, clock, false, false);
        // The tip body's height must be the deciding tip, not a beacon we reused.
        let tip = tip.replacen(
            &format!("\"height\":{}", natural_height_in_tip(&tip, tip_height)),
            &format!("\"height\":{tip_height}"),
            1,
        );
        routes.insert("/index".to_owned(), vec![Shot::Doc(tip)]);
    }
    bind_index(routes, fail_all, fail_first)
}

/// `document` already writes `height`. This keeps the call obvious at the tip.
fn natural_height_in_tip(tip: &str, fallback: u64) -> u64 {
    let _ = (tip, fallback);
    fallback
}

fn bind_index(routes: HashMap<String, Vec<Shot>>, fail_all: bool, fail_first: bool) -> Index {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let fail_budget = Arc::new(AtomicU64::new(u64::from(fail_first)));
    let join = std::thread::spawn(move || {
        let routes = Mutex::new(routes);
        let mut used: HashMap<String, usize> = HashMap::new();
        for conn in listener.incoming() {
            if flag.load(Ordering::SeqCst) {
                break;
            }
            let Ok(mut conn) = conn else { continue };
            if flag.load(Ordering::SeqCst) {
                break;
            }
            let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
            if fail_all || fail_budget.load(Ordering::SeqCst) > 0 {
                fail_budget.fetch_sub(1, Ordering::SeqCst);
                let _ = conn.shutdown(Shutdown::Both);
                continue;
            }
            let Some(path) = read_path(&mut conn) else {
                continue;
            };
            let mut routes = routes.lock().expect("routes");
            let shot = routes.get_mut(&path).map(|shots| {
                let n = used.entry(path.clone()).or_insert(0);
                let shot = if shots.is_empty() {
                    Shot::Fail
                } else if *n >= shots.len() {
                    let last = shots.len() - 1;
                    take_doc(shots, last)
                } else {
                    let shot = take_doc(shots, *n);
                    *n += 1;
                    shot
                };
                shot
            });
            drop(routes);
            match shot {
                Some(Shot::Doc(body)) => write_ok(&mut conn, &body),
                Some(Shot::Fail) | None => {
                    let _ = conn.shutdown(Shutdown::Both);
                }
            }
        }
    });
    Index {
        base: format!("http://{addr}"),
        stop,
        join: Some(join),
    }
}

fn take_doc(shots: &[Shot], index: usize) -> Shot {
    match &shots[index] {
        Shot::Doc(body) => Shot::Doc(body.clone()),
        Shot::Fail => Shot::Fail,
    }
}

fn read_path(conn: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 512];
    while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
        if buf.len() > 8192 {
            return None;
        }
        let n = conn.read(&mut tmp).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let text = String::from_utf8(buf).ok()?;
    let path = text.split_whitespace().nth(1)?;
    Some(path.split('?').next().unwrap_or(path).to_owned())
}

fn write_ok(conn: &mut TcpStream, body: &str) {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = conn.write_all(head.as_bytes());
    let _ = conn.write_all(body.as_bytes());
}

fn settle_face(
    bytes: &[u8],
    face: Face,
    policy: AdmissionPolicy,
) -> (Door, Index, FastPhase, Option<u64>) {
    let obj = pcoc::decode_object(bytes).expect("object");
    let index = serve(&obj, face);
    let door = open_door(policy, &index.base);
    let job = job_for(bytes, [9u8; 32]);
    let (phase, seq) = door.settle_fast_seq(&job);
    (door, index, phase, seq)
}

fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}

fn signer() -> (Secp256k1<secp256k1::All>, Keypair, [u8; 32]) {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_slice(&[2u8; 32]).expect("secret");
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let public = keypair.x_only_public_key().0.serialize();
    (secp, keypair, public)
}

fn signed_unknown(
    secp: &Secp256k1<secp256k1::All>,
    keypair: &Keypair,
    public: &[u8; 32],
    nonce: [u8; 32],
    exp: u64,
    mark: u8,
) -> LockedBody {
    let object = pcoc::cbor::encode_map(&[
        (
            "kind".into(),
            pcoc::cbor::encode_text(&format!("not-the-hike-kind-{mark}")),
        ),
        ("public_key".into(), pcoc::cbor::encode_bytes(public)),
    ]);
    let object_sha = canon::sha256(&object);
    let digest = wire::bip340_message(ORIGIN, &nonce, exp, &object_sha);
    let signature = secp
        .sign_schnorr_no_aux_rand(&Message::from_digest(digest), keypair)
        .serialize();
    LockedBody {
        origin: ORIGIN.to_owned(),
        nonce,
        exp,
        object_sha,
        object,
        signature,
    }
}

fn release_slot(door: &Door, signed: &crate::door::LockedSignIn) {
    if let Some(job) = &signed.thorough {
        door.end_pending(&job.pending_key());
    }
}

// --- AC-G8 named gate tests -------------------------------------------------

/// SPEC §15.3, §15.5. Honest mainnet reads are provisional. Thorough is the seq, not an accept.
#[test]
fn honest_index_reads_are_provisional_and_thorough_is_enqueued() {
    let bytes = anchored_o3();
    let (door, _index, phase, seq) = settle_face(&bytes, Face::Honest, v10());
    assert_eq!(phase, FastPhase::Provisional, "{:?}", chain(&door));
    assert!(seq.is_some());
    assert_eq!(door.test_decisions(), vec!["provisional".to_owned()]);
    assert!(!door.log_dir().join("friendly").exists());
    assert!(!door.log_dir().join("pending").exists());
}

/// SPEC §15.3. Index down is `not read`, so the fast phase holds.
#[test]
fn index_down_is_held() {
    let bytes = object_bytes("O3-PROD.cbor");
    let (door, _index, phase, seq) = settle_face(&bytes, Face::Down, v10());
    assert_eq!(phase, FastPhase::Held, "{:?}", chain(&door));
    assert!(seq.is_none());
    assert!(door.test_decisions().is_empty());
}

/// SPEC §14.3, §15.3. A fresh read that still disagrees refuses the plant.
#[test]
fn a_confirmed_plant_mismatch_refuses() {
    let bytes = anchored_o3();
    let obj = pcoc::decode_object(&bytes).unwrap();
    assert!(
        first_plant_height(&obj).is_some(),
        "O3 has no decodable W_1..W_16 path"
    );
    let (door, _index, phase, _) = settle_face(&bytes, Face::PlantBad, v10());
    assert_eq!(phase, FastPhase::Done, "{:?}", chain(&door));
    assert!(door
        .test_reasons()
        .iter()
        .any(|reason| reason.contains("plant")));
}

/// SPEC §15.3. A mirror mismatch whose fresh read fails stays held.
#[test]
fn an_unconfirmed_plant_mismatch_is_held() {
    let bytes = anchored_o3();
    let (door, _index, phase, _) = settle_face(&bytes, Face::PlantOnce, v10());
    assert_eq!(phase, FastPhase::Held, "{:?}", chain(&door));
    assert!(door.test_decisions().is_empty());
    assert!(door.mirror().bypass_count() >= 1);
}

/// SPEC §9, §15.3. A fresh read that still disagrees refuses the header.
#[test]
fn a_confirmed_header_mismatch_refuses() {
    let bytes = object_bytes("O3-PROD.cbor");
    let (door, _index, phase, _) = settle_face(&bytes, Face::HeaderBad, v10());
    assert_eq!(phase, FastPhase::Done, "{:?}", chain(&door));
    assert!(door
        .test_reasons()
        .iter()
        .any(|reason| reason.contains("checkpoint")));
}

/// SPEC §15.3. An unconfirmed header mismatch is held, not a refusal.
#[test]
fn an_unconfirmed_header_mismatch_is_held() {
    let bytes = object_bytes("O3-PROD.cbor");
    let (door, _index, phase, _) = settle_face(&bytes, Face::HeaderOnce, v10());
    assert_eq!(phase, FastPhase::Held, "{:?}", chain(&door));
    assert!(door.mirror().bypass_count() >= 1);
    assert!(door.test_decisions().is_empty());
}

/// SPEC §15.5 Rule B7. A fake leg revokes, bans the key, and a resubmit takes no slot.
#[test]
fn ac3_fake_leg_revokes_bans_the_key_and_ignores_a_resubmit() {
    let _guard = terrain_lock();
    let bytes = object_bytes("O1-S1.cbor");
    let mut obj = pcoc::decode_object(&bytes).unwrap();
    obj.stages[0].checkpoints[0][0] ^= 0xff;
    let forged = pcoc::encode_object(&obj);
    let profile = profile_of(&obj);
    let door = open_door(v10(), "http://127.0.0.1:9");
    let job = job_for(&forged, [3u8; 32]);
    let seq = door.test_grant_provisional(&job);
    let mut out = thorough::recompute_at(&forged, &[vec![0, 1, 2, 3, 4, 5]], &profile).unwrap();
    assert!(!out.matched);
    out.seq = seq;
    door.settle_thorough(&job, out);
    assert!(door.test_decisions().contains(&"revoke".to_owned()));
    assert!(door.is_banned(&job.public_key));
    assert_eq!(
        door.try_begin_pending(&job.public_key, &job.pending_key()),
        PendingAdmit::Banned
    );
    assert_eq!(door.pending_counts(&job.pending_key()), (0, 0));
    assert!(door.ban_ignored() >= 1);
}

/// The thorough ticket is the job's own provisional seq.
#[test]
fn interleaved_jobs_each_settle_against_their_own_provisional() {
    let _guard = terrain_lock();
    let bytes = object_bytes("O1-S1.cbor");
    let obj = pcoc::decode_object(&bytes).unwrap();
    let profile = profile_of(&obj);
    let door = open_door(v10(), "http://127.0.0.1:9");
    let job_a = job_for(&bytes, [1u8; 32]);
    let job_b = job_for(&bytes, [2u8; 32]);
    let seq_a = door.test_grant_provisional(&job_a);
    let seq_b = door.test_grant_provisional(&job_b);
    let matched = thorough::recompute_at(&bytes, &[vec![0, 1, 2, 3, 4, 5]], &profile).unwrap();
    let mut wrong = matched.clone();
    wrong.seq = seq_b;
    door.settle_thorough(&job_a, wrong);
    assert_eq!(
        door.test_decisions()
            .iter()
            .filter(|item| item.as_str() == "accept")
            .count(),
        0
    );
    let mut own = matched.clone();
    own.seq = seq_a;
    door.settle_thorough(&job_a, own);
    let mut other = matched;
    other.seq = seq_b;
    door.settle_thorough(&job_b, other);
    let accepts: Vec<u64> = chain(&door)
        .iter()
        .filter(|line| line["decision"] == "accept")
        .map(|line| line["fast_seq"].as_u64().unwrap())
        .collect();
    assert_eq!(accepts, vec![seq_a, seq_b]);
}

/// SPEC §15.1 / §15.5. Restart loads the stored draw and does not draw again.
#[test]
fn a_restart_resumes_with_the_stored_draw_and_never_draws_again() {
    let _guard = terrain_lock();
    let bytes = object_bytes("O1-S1.cbor");
    let obj = pcoc::decode_object(&bytes).unwrap();
    let sha = canon::sha256(&bytes);
    let door = open_door(v10(), "http://127.0.0.1:9");
    let job = job_for(&bytes, [4u8; 32]);
    let seq = door.test_grant_provisional(&job);
    let runs = vec![vec![0u64, 1, 2, 3, 4, 5]];
    let salt = [8u8; 32];
    let draw = DrawFile {
        seq,
        object_sha256: hex32(&sha),
        salt: hex::encode(salt),
        commitment: hex32(&thorough::commitment(seq, &sha, &salt, &runs)),
        runs: runs.clone(),
    };
    thorough::store_test_draw(&door.log_dir(), &draw).unwrap();
    thorough::reset_counts();
    let err = thorough::resume(&door.log_dir(), seq, &bytes, &sha, door.policy()).unwrap_err();
    assert_eq!(err, thorough::RELEASE_PRODUCTION_ONLY);
    assert_eq!(thorough::draws(), 0);
    assert_eq!(thorough::fills(), 0);
    let jobs = door.unsettled();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].fast_seq, seq);
    let out = thorough::recompute_at(&bytes, &runs, &profile_of(&obj)).unwrap();
    assert!(out.matched);
    assert_eq!(thorough::draws(), 0);
}

/// `pending/<seq>.draw` and `banned_keys.json` are private.
#[test]
fn the_draw_and_ban_files_are_private() {
    let bytes = object_bytes("O1-S1.cbor");
    let sha = canon::sha256(&bytes);
    let door = open_door(v10(), "http://127.0.0.1:9");
    let runs = vec![vec![0u64, 1, 2, 3, 4, 5]];
    let salt = [1u8; 32];
    let draw = DrawFile {
        seq: 4,
        object_sha256: hex32(&sha),
        salt: hex::encode(salt),
        commitment: hex32(&thorough::commitment(4, &sha, &salt, &runs)),
        runs,
    };
    thorough::store_test_draw(&door.log_dir(), &draw).unwrap();
    let pending = door.log_dir().join("pending");
    assert_eq!(mode_of(&pending), 0o700);
    assert_eq!(mode_of(&pending.join("4.draw")), 0o600);
    door.ban_key(&job_for(&bytes, [0u8; 32]).public_key, 4);
    assert_eq!(mode_of(&door.log_dir().join("banned_keys.json")), 0o600);
}

/// A thorough error writes no accept.
#[test]
fn a_thorough_error_is_held_and_never_accepts() {
    let bytes = object_bytes("O1-S1.cbor");
    let door = open_door(v10(), "http://127.0.0.1:9");
    let job = job_for(&bytes, [5u8; 32]);
    door.test_grant_provisional(&job);
    door.finish_thorough(&job, Err("leg worker".to_owned()));
    assert_eq!(door.test_decisions(), vec!["provisional".to_owned()]);
}

/// A key banned while the job is in flight is not accepted.
#[test]
fn a_key_banned_in_flight_is_never_accepted() {
    let _guard = terrain_lock();
    let bytes = object_bytes("O1-S1.cbor");
    let obj = pcoc::decode_object(&bytes).unwrap();
    let door = open_door(v10(), "http://127.0.0.1:9");
    let job = job_for(&bytes, [6u8; 32]);
    let seq = door.test_grant_provisional(&job);
    let mut out =
        thorough::recompute_at(&bytes, &[vec![0, 1, 2, 3, 4, 5]], &profile_of(&obj)).unwrap();
    assert!(out.matched);
    door.ban_key(&job.public_key, 1);
    out.seq = seq;
    door.settle_thorough(&job, out);
    assert!(!door.test_decisions().iter().any(|item| item == "accept"));
}

/// A banned key takes no slot. A panicking job releases the slot it took.
#[test]
fn a_banned_key_takes_no_slot_and_a_panicking_job_releases_its_slot() {
    let door = open_door(v10(), "http://127.0.0.1:9");
    let key = [9u8; 32];
    door.ban_key(&key, 1);
    let source = hex32(&key);
    assert_eq!(door.try_begin_pending(&key, &source), PendingAdmit::Banned);
    assert_eq!(door.pending_counts(&source), (0, 0));
    door.begin_pending("src");
    let door2 = door.clone();
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _slot = crate::door::PendingSlot::new(door2, "src".to_owned());
        panic!("job");
    }));
    assert!(panicked.is_err());
    assert_eq!(door.pending_counts("src"), (0, 0));
}

/// SPEC §15.3. No observed network is held. It is not a provisional grant.
#[test]
fn v7_holds_a_provisional_with_no_observed_network() {
    let bytes = object_bytes("O3-PROD.cbor");
    let (door, _index, phase, _) = settle_face(&bytes, Face::Down, v10());
    assert_eq!(phase, FastPhase::Held, "{:?}", chain(&door));
    assert!(door.test_decisions().is_empty());
}

/// SPEC §15.3 Rule B9. Mainnet documents are evidence. The fast phase grants provisional.
#[test]
fn v7_accepts_mainnet_index_evidence() {
    let bytes = anchored_o3();
    let (door, _index, phase, _) = settle_face(&bytes, Face::Honest, v10());
    assert_eq!(phase, FastPhase::Provisional, "{:?}", chain(&door));
    let lines = chain(&door);
    assert_eq!(lines[0]["index_network"][0], "main");
}

/// SPEC §15.3 L1385 / Rule B9. `network` other than `main` is not mainnet evidence.
#[test]
fn v7_refuses_synthetic_index_evidence() {
    let bytes = object_bytes("O3-PROD.cbor");
    let (door, _index, phase, _) = settle_face(&bytes, Face::Synthetic, v10());
    assert_eq!(phase, FastPhase::Done, "{:?}", chain(&door));
    assert!(door
        .test_reasons()
        .iter()
        .any(|reason| reason.contains("index_network")));
}

/// SPEC §15.3. A fresh read bypasses the mirror. A missing network is held.
#[test]
fn v7_checks_header_plant_and_fresh_reads_and_a_missing_network() {
    let bytes = anchored_o3();
    let (door, _index, phase, _) = settle_face(&bytes, Face::HeaderFresh, v10());
    assert_eq!(phase, FastPhase::Provisional, "{:?}", chain(&door));
    assert!(
        door.mirror().bypass_count() >= 1,
        "fresh read was not counted"
    );
    let lines = chain(&door);
    let record = &lines[0]["verify_summary"];
    assert!(record["anchored_total"].is_number(), "{record}");
    assert!(record["analysis"]["hike_legs"].is_number(), "{record}");
    let (door, _index, phase, _) = settle_face(&bytes, Face::NoNetwork, v10());
    assert_eq!(phase, FastPhase::Held, "{:?}", chain(&door));
    assert!(door.test_decisions().is_empty());
}

// --- recompute, levers, window, policy text --------------------------------

/// AC-G1. Honest legs match. A stored-checkpoint byte flip fails.
#[test]
fn honest_legs_recompute_and_a_checkpoint_byte_flip_fails() {
    let _guard = terrain_lock();
    let bytes = object_bytes("O1-S1.cbor");
    let obj = pcoc::decode_object(&bytes).unwrap();
    let profile = profile_of(&obj);
    let total: u64 = obj
        .stages
        .iter()
        .map(|stage| stage.checkpoints.len() as u64)
        .sum();
    thorough::reset_counts();
    let out = thorough::recompute_at(&bytes, &cover(total), &profile).unwrap();
    assert!(out.matched, "honest legs");
    assert!(out.legs_matched >= total);
    let mut forged = obj.clone();
    forged.stages[0].checkpoints[0][0] ^= 0xff;
    let encoded = pcoc::encode_object(&forged);
    let flipped = thorough::recompute_at(&encoded, &[vec![0, 1, 2, 3, 4, 5]], &profile).unwrap();
    assert!(!flipped.matched);
    assert!(flipped.digest_mismatch);
}

/// AC-G1. Each negative with a stored leg result fails as a checkpoint forgery.
#[test]
fn stored_checkpoint_forgeries_from_negatives_fail() {
    let _guard = terrain_lock();
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/candidates/negatives.txt");
    let text = std::fs::read_to_string(path).unwrap();
    let mut legs = HashMap::<String, String>::new();
    let mut bad = Vec::<String>::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('\t') else {
            continue;
        };
        let Some(rest) = key.strip_prefix("negatives.") else {
            continue;
        };
        if let Some(name) = rest.strip_suffix(".legs_le_hex") {
            legs.insert(name.to_owned(), value.to_owned());
        }
        if let Some(name) = rest.strip_suffix(".matches_honest") {
            if value == "false" {
                bad.push(name.to_owned());
            }
        }
    }
    let bytes = object_bytes("O1-S1.cbor");
    let obj = pcoc::decode_object(&bytes).unwrap();
    let profile = profile_of(&obj);
    let mut caught = 0u32;
    for name in bad {
        let Some(hex_le) = legs.get(&name) else {
            continue;
        };
        let raw = hex::decode(hex_le).unwrap();
        let mut leg = [0u8; 8];
        leg.copy_from_slice(&raw);
        let (terrain, seed_d, _) =
            pcoc::terrain_from_seed(&obj.public_key, &obj.seed_beacon, &profile);
        let lr = u64::from_le_bytes(leg);
        let cp = pcoc::checkpoint(&seed_d, &seed_d, &terrain, &profile, lr);
        drop(terrain);
        let mut forged = obj.clone();
        forged.stages[0].leg_results[0] = leg.to_vec();
        forged.stages[0].checkpoints[0] = cp.to_vec();
        let encoded = pcoc::encode_object(&forged);
        let out = thorough::recompute_at(&encoded, &[vec![0, 1, 2, 3, 4, 5]], &profile).unwrap();
        assert!(!out.matched, "{name} was not caught");
        caught += 1;
    }
    assert!(caught >= 10, "caught {caught}");
}

/// AC-G2. Six legs start before any result is recorded. One fill per check, again on retry.
#[test]
fn a_round_starts_six_legs_before_any_is_recorded_and_each_check_fills_once() {
    let _guard = terrain_lock();
    let o1 = object_bytes("O1-S1.cbor");
    let o2 = object_bytes("O2-S2.cbor");
    let p1 = profile_of(&pcoc::decode_object(&o1).unwrap());
    let p2 = profile_of(&pcoc::decode_object(&o2).unwrap());
    let run = vec![vec![0u64, 1, 2, 3, 4, 5]];
    thorough::reset_counts();
    let out = thorough::recompute_at(&o1, &run, &p1).unwrap();
    assert!(out.matched);
    assert_eq!(thorough::round_started(), 6);
    assert_eq!(thorough::recorded_at_barrier(), 0);
    assert_eq!(thorough::fills(), 1);
    thorough::recompute_at(&o2, &run, &p2).unwrap();
    assert_eq!(thorough::fills(), 2);
    thorough::recompute_at(&o1, &run, &p1).unwrap();
    assert_eq!(thorough::fills(), 3);
}

/// AC-G7. The release entry does not measure conformance-small.
#[test]
fn release_entry_refuses_a_conformance_small_object() {
    let _guard = terrain_lock();
    let bytes = object_bytes("O1-S1.cbor");
    let sha = canon::sha256(&bytes);
    let door = open_door(v10(), "http://127.0.0.1:9");
    thorough::reset_counts();
    let err = thorough::run(door.log_dir().as_path(), 1, &bytes, &sha, door.policy()).unwrap_err();
    assert_eq!(err, thorough::RELEASE_PRODUCTION_ONLY);
    assert_eq!(thorough::fills(), 0);
    assert_eq!(thorough::draws(), 0);
    let runs = vec![vec![0u64, 1, 2, 3, 4, 5]];
    let salt = [3u8; 32];
    let draw = DrawFile {
        seq: 1,
        object_sha256: hex32(&sha),
        salt: hex::encode(salt),
        commitment: hex32(&thorough::commitment(1, &sha, &salt, &runs)),
        runs,
    };
    thorough::store_test_draw(&door.log_dir(), &draw).unwrap();
    let err = thorough::run(door.log_dir().as_path(), 1, &bytes, &sha, door.policy()).unwrap_err();
    assert_eq!(err, thorough::RELEASE_PRODUCTION_ONLY);
    assert_eq!(thorough::draws(), 0);
    assert_eq!(thorough::fills(), 0);
}

/// AC-G7. Two OS draws differ.
#[test]
fn two_os_draws_differ() {
    let left = thorough::os_draw(18, 3).unwrap();
    let right = thorough::os_draw(18, 3).unwrap();
    assert_ne!(left, right);
    assert_eq!(left.len(), 3);
    assert!(left.iter().all(|run| run.len() == 6));
}

/// AC-G4. Provisional is not accept, and the friendly file appears only on thorough accept.
#[test]
fn provisional_is_not_accept_and_friendly_waits_for_thorough() {
    let _guard = terrain_lock();
    let bytes = object_bytes("O1-S1.cbor");
    let obj = pcoc::decode_object(&bytes).unwrap();
    let door = open_door(v10(), "http://127.0.0.1:9");
    let job = job_for(&bytes, [8u8; 32]);
    let seq = door.test_grant_provisional(&job);
    let friendly = door.log_dir().join("friendly").join(hex32(&job.public_key));
    assert!(!friendly.exists());
    assert!(!door.test_decisions().contains(&"accept".to_owned()));
    let mut out =
        thorough::recompute_at(&bytes, &[vec![0, 1, 2, 3, 4, 5]], &profile_of(&obj)).unwrap();
    assert!(out.matched);
    out.seq = seq;
    door.settle_thorough(&job, out);
    assert!(door.test_decisions().contains(&"accept".to_owned()));
    assert_eq!(mode_of(&friendly), 0o600);
}

/// AC-G4. No network holds. When the network returns, the same submission is provisional.
#[test]
fn no_network_holds_then_the_same_submission_is_provisional() {
    let bytes = anchored_o3();
    let obj = pcoc::decode_object(&bytes).unwrap();
    let index = serve(&obj, Face::DownThenHonest);
    let door = open_door(v10(), &index.base);
    door.set_now_unix_s(Some(1_700_000_000));
    let job = job_for(&bytes, [11u8; 32]);
    let (phase, _) = door.settle_fast_seq(&job);
    assert_eq!(phase, FastPhase::Held, "{:?}", chain(&door));
    door.set_now_unix_s(Some(1_700_000_000 + door.policy().held_retry_seconds));
    let ready = door.poll_held();
    assert_eq!(ready.len(), 1);
    assert!(ready[0].granted);
    assert_eq!(door.test_decisions(), vec!["provisional".to_owned()]);
    assert!(!door.log_dir().join("friendly").exists());
}

/// AC-G4. The hold budget refuses with no grant and revokes when a grant was issued.
#[test]
fn held_bound_refuses_without_a_grant_and_revokes_after_one() {
    let bytes = object_bytes("O3-PROD.cbor");
    let policy = Pol {
        held_retry: 30,
        reverify: 3,
        ..Pol::default()
    }
    .load();
    let obj = pcoc::decode_object(&bytes).unwrap();
    let index = serve(&obj, Face::Down);
    let door = open_door(policy, &index.base);
    let job = job_for(&bytes, [12u8; 32]);
    door.set_now_unix_s(Some(5_000));
    assert_eq!(door.settle_fast_seq(&job).0, FastPhase::Held);
    assert_eq!(door.test_held_attempts(&job.nonce), Some(1));
    door.set_now_unix_s(Some(5_029));
    assert!(door.poll_held().is_empty());
    assert_eq!(door.test_held_attempts(&job.nonce), Some(1));
    door.set_now_unix_s(Some(5_030));
    assert!(door.poll_held().is_empty());
    assert_eq!(door.test_held_attempts(&job.nonce), Some(2));
    door.set_now_unix_s(Some(5_060));
    assert!(door.poll_held().is_empty());
    assert!(door.test_decisions().contains(&"refuse".to_owned()));
    assert!(chain(&door)
        .iter()
        .any(|line| line["reason"] == "reverify_max_attempts"));

    let door = open_door(
        Pol {
            held_retry: 30,
            reverify: 3,
            ..Pol::default()
        }
        .load(),
        &index.base,
    );
    let job = job_for(&bytes, [13u8; 32]);
    door.test_grant_provisional(&job);
    door.set_now_unix_s(Some(8_000));
    assert_eq!(door.settle_fast_seq(&job).0, FastPhase::Held);
    door.set_now_unix_s(Some(8_030));
    door.poll_held();
    door.set_now_unix_s(Some(8_060));
    door.poll_held();
    assert!(door.test_decisions().contains(&"revoke".to_owned()));
}

/// AC-G4. `provisional = off` writes `received` and no provisional line.
#[test]
fn provisional_off_writes_received_and_no_provisional() {
    let bytes = anchored_o3();
    let policy = Pol {
        provisional: "off",
        ..Pol::default()
    }
    .load();
    let (door, _index, phase, seq) = settle_face(&bytes, Face::Honest, policy);
    assert_eq!(phase, FastPhase::Direct, "{:?}", chain(&door));
    assert!(seq.is_some());
    assert_eq!(door.test_decisions(), vec!["received".to_owned()]);
    assert!(!door.test_decisions().contains(&"provisional".to_owned()));
}

/// AC-G6 / AC-G13. Each lever in an in-test policy file changes what the gate does.
#[test]
fn each_loaded_lever_changes_what_the_gate_does() {
    let one = Pol {
        rounds: 1,
        ..Pol::default()
    }
    .load();
    let three = Pol::default().load();
    assert_eq!(
        thorough::os_draw(18, one.thorough_six_leg_runs as u32)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        thorough::os_draw(18, three.thorough_six_leg_runs as u32)
            .unwrap()
            .len(),
        3
    );

    let bytes = anchored_o3();
    let obj = pcoc::decode_object(&bytes).unwrap();
    let wide = serve(&obj, Face::Wide);
    let span_policy_fails = Pol {
        p: 1,
        q: 5,
        ..Pol::default()
    }
    .load();
    let (door, _index, phase, _) = {
        let index = serve(&obj, Face::Wide);
        let door = open_door(span_policy_fails, &index.base);
        let job = job_for(&bytes, [21u8; 32]);
        let (phase, seq) = door.settle_fast_seq(&job);
        (door, index, phase, seq)
    };
    let _ = wide;
    assert_eq!(phase, FastPhase::Done, "{:?}", chain(&door));
    assert!(door
        .test_reasons()
        .iter()
        .any(|reason| reason.contains("leg_rate")));
    let legs = pcoc::hike_legs(&obj).unwrap();
    let span = realized(
        &obj,
        Clock::Split {
            last_time: (5 * legs + 1).min(u32::MAX as i128),
        },
    );
    let q = u64::try_from(span.max(1)).unwrap_or(u64::MAX);
    let meets = Pol {
        p: 1,
        q,
        ..Pol::default()
    }
    .load();
    let (door, _index, phase, _) = settle_face(&bytes, Face::Wide, meets);
    assert_eq!(
        phase,
        FastPhase::Provisional,
        "span {span} legs {legs} {:?}",
        chain(&door)
    );

    let anchored = chain(&door)
        .iter()
        .find_map(|line| line["verify_summary"]["anchored_total"].as_u64())
        .expect("anchored");
    assert!(anchored >= 2 && anchored < 16, "anchored {anchored}");
    let tight = Pol {
        min_anchored: anchored + 1,
        p: 1,
        q,
        ..Pol::default()
    }
    .load();
    let (door, _index, phase, _) = settle_face(&bytes, Face::Wide, tight);
    assert_eq!(phase, FastPhase::Done, "{:?}", chain(&door));
    assert!(door
        .test_reasons()
        .iter()
        .any(|reason| reason.contains("too_few")));

    let last = obj.last_interrupt_height;
    let k = 3u64;
    let (door, _index, phase, _) = settle_face(
        &bytes,
        Face::TipGap(k),
        Pol {
            k,
            ..Pol::default()
        }
        .load(),
    );
    assert_eq!(
        phase,
        FastPhase::Held,
        "gap K should pass B10 {:?}",
        chain(&door)
    );
    let (door, _index, phase, _) = settle_face(
        &bytes,
        Face::TipGap(k + 1),
        Pol {
            k,
            ..Pol::default()
        }
        .load(),
    );
    assert_eq!(phase, FastPhase::Done, "{:?}", chain(&door));
    assert!(door
        .test_reasons()
        .iter()
        .any(|reason| reason == "submission_deadline"));
    assert!(!door.log_dir().join("pending").exists());
    let _ = last;

    let slow = Pol {
        held_retry: 30,
        ..Pol::default()
    }
    .load();
    let later = Pol {
        held_retry: 600,
        ..Pol::default()
    }
    .load();
    let index = serve(&obj, Face::Down);
    let early = open_door(slow, &index.base);
    let late = open_door(later, &index.base);
    let job = job_for(&bytes, [22u8; 32]);
    early.set_now_unix_s(Some(10_000));
    late.set_now_unix_s(Some(10_000));
    early.settle_fast_seq(&job);
    late.settle_fast_seq(&job_for(&bytes, [23u8; 32]));
    early.set_now_unix_s(Some(10_030));
    late.set_now_unix_s(Some(10_030));
    assert!(early.poll_held().is_empty());
    assert_eq!(early.test_held_attempts(&job.nonce), Some(2));
    assert_eq!(
        late.test_held_attempts(&job_for(&bytes, [23u8; 32]).nonce),
        Some(1)
    );

    let elapsed_hold = Pol {
        fast: "0.001",
        async_s: "100",
        p: 1,
        q,
        ..Pol::default()
    }
    .load();
    let elapsed_ok = Pol {
        fast: "100",
        async_s: "100",
        p: 1,
        q,
        ..Pol::default()
    }
    .load();
    let index = serve(&obj, Face::Wide);
    let held = open_door(elapsed_hold, &index.base);
    let passed = open_door(elapsed_ok, &index.base);
    let job = job_for(&bytes, [24u8; 32]);
    let phase = with_fast_elapsed(10, || held.settle_fast_seq(&job).0);
    assert_eq!(phase, FastPhase::Held);
    held.set_now_unix_s(Some(50_000));
    // The budget hold must not become provisional when the clock is retried.
    let _ = with_fast_elapsed(10, || held.poll_held());
    assert!(!held.test_decisions().contains(&"provisional".to_owned()));
    let phase = with_fast_elapsed(10, || {
        passed.settle_fast_seq(&job_for(&bytes, [25u8; 32])).0
    });
    assert_eq!(phase, FastPhase::Provisional, "{:?}", chain(&passed));

    let async_hold = Pol {
        fast: "100",
        async_s: "0.001",
        p: 1,
        q,
        ..Pol::default()
    }
    .load();
    let async_ok = Pol {
        fast: "100",
        async_s: "10",
        p: 1,
        q,
        ..Pol::default()
    }
    .load();
    let held = open_door(async_hold, &index.base);
    let passed = open_door(async_ok, &index.base);
    assert_eq!(
        with_fast_elapsed(5_000, || held
            .settle_fast_seq(&job_for(&bytes, [26u8; 32]))
            .0),
        FastPhase::Held
    );
    assert_eq!(
        with_fast_elapsed(5_000, || passed
            .settle_fast_seq(&job_for(&bytes, [27u8; 32]))
            .0),
        FastPhase::Provisional
    );

    let short = Pol {
        lifetime: 60,
        ..Pol::default()
    }
    .load();
    let long = Pol {
        lifetime: 120,
        ..Pol::default()
    }
    .load();
    let a = open_door(short, "http://127.0.0.1:9");
    let b = open_door(long, "http://127.0.0.1:9");
    a.set_now_unix_s(Some(1_000));
    b.set_now_unix_s(Some(1_000));
    a.test_set_store_start(0);
    b.test_set_store_start(0);
    let body = LockedBody {
        origin: ORIGIN.to_owned(),
        nonce: [30u8; 32],
        exp: 1_090,
        object_sha: canon::sha256(&[1u8; 40]),
        object: vec![1u8; 40],
        signature: [0u8; 64],
    };
    assert_eq!(
        client_code(&a.sign_in_locked(body.clone()).body),
        "malformed"
    );
    assert_ne!(client_code(&b.sign_in_locked(body).body), "malformed");

    let tight = Pol {
        skew: 5,
        ..Pol::default()
    }
    .load();
    let loose = Pol {
        skew: 0,
        ..Pol::default()
    }
    .load();
    let a = open_door(tight, "http://127.0.0.1:9");
    let b = open_door(loose, "http://127.0.0.1:9");
    for door in [&a, &b] {
        door.set_now_unix_s(Some(1_010));
        door.test_set_store_start(1_000);
    }
    let body = LockedBody {
        origin: ORIGIN.to_owned(),
        nonce: [31u8; 32],
        exp: 1_124,
        object_sha: canon::sha256(&[2u8; 40]),
        object: vec![2u8; 40],
        signature: [0u8; 64],
    };
    assert_eq!(
        client_code(&a.sign_in_locked(body.clone()).body),
        "malformed"
    );
    assert_ne!(client_code(&b.sign_in_locked(body).body), "malformed");

    let brief = Pol {
        retention: 10,
        ..Pol::default()
    }
    .load();
    let kept = Pol {
        retention: 1_000,
        ..Pol::default()
    }
    .load();
    let brief_door = open_door(brief, "http://127.0.0.1:9");
    let kept_door = open_door(kept, "http://127.0.0.1:9");
    assert_eq!(brief_door.nonce_rows().retention_seconds(), 10);
    assert_eq!(kept_door.nonce_rows().retention_seconds(), 1_000);
    assert_eq!(
        brief_door.nonce_rows().skew_seconds(),
        brief_door.policy().challenge_clock_skew_seconds
    );

    let cap1 = Pol {
        session: Some(1),
        ..Pol::default()
    }
    .load();
    let door = open_door(cap1, "http://127.0.0.1:9");
    door.set_now_unix_s(Some(2_000));
    door.test_set_store_start(0);
    let (secp, keypair, public) = signer();
    let first = signed_unknown(&secp, &keypair, &public, [40u8; 32], 2_100, 1);
    let signed = door.sign_in_locked(first);
    assert!(signed.thorough.is_some(), "{:?}", signed.body);
    release_slot(&door, &signed);
    let second = signed_unknown(&secp, &keypair, &public, [41u8; 32], 2_100, 1);
    let signed = door.sign_in_locked(second);
    assert!(signed.thorough.is_none());
    assert!(door
        .test_reasons()
        .iter()
        .any(|reason| reason == "verify_cap_per_door_session"));

    let per_object = Pol {
        session: Some(8),
        object_cap: Some(1),
        ..Pol::default()
    }
    .load();
    let door = open_door(per_object, "http://127.0.0.1:9");
    door.set_now_unix_s(Some(3_000));
    door.test_set_store_start(0);
    let first = signed_unknown(&secp, &keypair, &public, [42u8; 32], 3_100, 7);
    let signed = door.sign_in_locked(first);
    assert!(signed.thorough.is_some());
    release_slot(&door, &signed);
    let again = signed_unknown(&secp, &keypair, &public, [43u8; 32], 3_100, 7);
    assert!(door.sign_in_locked(again).thorough.is_none());
    assert!(door
        .test_reasons()
        .iter()
        .any(|reason| reason == "verify_cap_per_object"));
    let other = signed_unknown(&secp, &keypair, &public, [44u8; 32], 3_100, 8);
    let signed = door.sign_in_locked(other);
    assert!(signed.thorough.is_some(), "{:?}", chain(&door));
    release_slot(&door, &signed);

    let loaded = v10();
    assert_eq!(loaded.verify_cap_per_door_session, Some(3));
    let n = loaded
        .verify_cap_per_door_session
        .expect("v10 sets the door cap");
    let door = open_door(loaded, "http://127.0.0.1:9");
    door.set_now_unix_s(Some(4_000));
    door.test_set_store_start(0);
    for i in 0..n {
        let mut nonce = [50u8; 32];
        nonce[0] = i as u8;
        let body = signed_unknown(&secp, &keypair, &public, nonce, 4_100, 9);
        let signed = door.sign_in_locked(body);
        assert!(signed.thorough.is_some(), "sign-in {i}");
        release_slot(&door, &signed);
    }
    let mut nonce = [50u8; 32];
    nonce[0] = n as u8;
    let body = signed_unknown(&secp, &keypair, &public, nonce, 4_100, 9);
    assert!(door.sign_in_locked(body).thorough.is_none());
}

/// The shipped policy file sets `verify_cap_per_door_session`, and load reads it.
/// An absent key is unset. It is not filled with 3.
#[test]
fn shipped_policy_reads_verify_cap_per_door_session() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../policy/groundinglab-admission/gl-admit-v10.toml");
    let raw = std::fs::read_to_string(&path).unwrap();
    let set = raw.lines().any(|line| {
        let line = line.trim();
        !line.starts_with('#') && line.starts_with("verify_cap_per_door_session")
    });
    assert!(
        set,
        "gl-admit-v10.toml does not set verify_cap_per_door_session"
    );
    let loaded = crate::levers::load(&raw, 2).expect("shipped policy");
    assert_eq!(loaded.verify_cap_per_door_session, Some(3));
    let four = raw.replace(
        "verify_cap_per_door_session  = 3",
        "verify_cap_per_door_session  = 4",
    );
    assert_ne!(four, raw);
    let loaded = crate::levers::load(&four, 2).expect("cap 4");
    assert_eq!(loaded.verify_cap_per_door_session, Some(4));
    let absent = raw
        .lines()
        .filter(|line| !line.trim_start().starts_with("verify_cap_per_door_session"))
        .collect::<Vec<_>>()
        .join("\n");
    let loaded = crate::levers::load(&absent, 2).expect("absent key");
    assert_eq!(loaded.verify_cap_per_door_session, None);
}

/// AC-G12. Rule I4 on a mock clock: restart, flush, two workers.
#[test]
fn rule_i4_restart_flush_and_two_workers() {
    let policy = Pol::default().load();
    let door = open_door(policy.clone(), "http://127.0.0.1:9");
    door.set_now_unix_s(Some(1_005));
    door.test_set_store_start(1_000);
    let early = LockedBody {
        origin: ORIGIN.to_owned(),
        nonce: [60u8; 32],
        exp: 1_124,
        object_sha: canon::sha256(&[4u8; 40]),
        object: vec![4u8; 40],
        signature: [0u8; 64],
    };
    let late = LockedBody {
        origin: ORIGIN.to_owned(),
        nonce: [61u8; 32],
        exp: 1_125,
        object_sha: canon::sha256(&[5u8; 40]),
        object: vec![5u8; 40],
        signature: [0u8; 64],
    };
    assert_eq!(
        client_code(&door.sign_in_locked(early.clone()).body),
        "malformed"
    );
    assert!(!door.consumed_nonce(&early.nonce));
    assert!(door.test_decisions().is_empty());
    assert_ne!(
        client_code(&door.sign_in_locked(late.clone()).body),
        "malformed"
    );

    door.test_set_store_start(2_000);
    let replay = door.sign_in_locked(late.clone());
    assert_eq!(client_code(&replay.body), "malformed");
    assert!(!door.consumed_nonce(&late.nonce));
    assert!(door.test_decisions().is_empty());

    let door = open_door(policy.clone(), "http://127.0.0.1:9");
    door.set_now_unix_s(Some(1_005));
    door.test_set_store_start(1_000);
    assert_ne!(
        client_code(&door.sign_in_locked(late.clone()).body),
        "malformed"
    );
    door.test_flush_nonce_store(3_000);
    door.set_now_unix_s(Some(3_005));
    assert_eq!(door.store_start(), 3_000);
    assert_eq!(
        client_code(&door.sign_in_locked(late.clone()).body),
        "malformed"
    );
    assert!(!door.consumed_nonce(&late.nonce));
    assert!(door.test_decisions().is_empty());

    let rows = Arc::new(crate::signin_row::SignInRows::open(1_000, 600, 5));
    let left = Door::open_with_rows(
        &scratch("left"),
        policy.clone(),
        "http://127.0.0.1:9",
        Duration::from_secs(1),
        vec![ORIGIN.to_owned()],
        crate::policy::gate_build(),
        rows.clone(),
    )
    .unwrap();
    let right = Door::open_with_rows(
        &scratch("right"),
        policy,
        "http://127.0.0.1:9",
        Duration::from_secs(1),
        vec![ORIGIN.to_owned()],
        crate::policy::gate_build(),
        rows,
    )
    .unwrap();
    for door in [&left, &right] {
        door.set_now_unix_s(Some(1_005));
    }
    assert_eq!(left.store_start(), right.store_start());
    assert_eq!(
        client_code(&left.sign_in_locked(early.clone()).body),
        "malformed"
    );
    assert_eq!(client_code(&right.sign_in_locked(early).body), "malformed");
    assert_ne!(
        client_code(&left.sign_in_locked(late.clone()).body),
        "malformed"
    );
    assert_ne!(client_code(&right.sign_in_locked(late).body), "malformed");
}

/// AC-G13. Tip below the last beacon, and a tip that is not read, are held and counted.
#[test]
fn tip_not_read_or_below_the_last_beacon_is_held_and_counted() {
    let bytes = object_bytes("O3-PROD.cbor");
    let obj = pcoc::decode_object(&bytes).unwrap();
    assert!(obj.last_interrupt_height > 0);
    let policy = Pol {
        held_retry: 10,
        reverify: 2,
        ..Pol::default()
    }
    .load();
    let index = serve(&obj, Face::TipBelow);
    let door = open_door(policy.clone(), &index.base);
    door.set_now_unix_s(Some(1_000_000));
    let job = job_for(&bytes, [80u8; 32]);
    assert_eq!(door.settle_fast_seq(&job).0, FastPhase::Held);
    assert_eq!(door.test_held_attempts(&job.nonce), Some(1));
    door.set_now_unix_s(Some(1_000_009));
    assert!(door.poll_held().is_empty());
    assert_eq!(door.test_held_attempts(&job.nonce), Some(1));
    door.set_now_unix_s(Some(1_000_010));
    assert!(door.poll_held().is_empty());
    assert!(door.test_decisions().iter().any(|item| item == "refuse"));
    assert!(chain(&door)
        .iter()
        .any(|line| line["reason"] == "reverify_max_attempts"));

    let index = serve(&obj, Face::Down);
    let door = open_door(policy, &index.base);
    door.set_now_unix_s(Some(2_000_000));
    let job = job_for(&bytes, [81u8; 32]);
    assert_eq!(door.settle_fast_seq(&job).0, FastPhase::Held);
    door.set_now_unix_s(Some(2_000_010));
    door.poll_held();
    assert!(chain(&door)
        .iter()
        .any(|line| line["reason"] == "reverify_max_attempts"));
}

/// AC-G14. Encoding faults refuse the whole file. A missing path is a startup error.
#[test]
fn policy_encoding_faults_refuse_the_whole_file() {
    let good = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../policy/groundinglab-admission/gl-admit-v10.toml"),
    )
    .unwrap();
    assert!(crate::levers::load(&good, 2).is_ok());
    let cases = [
        (
            good.replace("min_anchored_paths", "surprise"),
            "unknown lever",
        ),
        (
            good.replace("submission_deadline_blocks", "K"),
            "unknown lever",
        ),
        (
            good.replace(
                "min_anchored_paths           = 6",
                "min_anchored_paths = \"6\"",
            ),
            "wrong type",
        ),
        (
            good.replace(
                "submission_deadline_blocks   = 3",
                "submission_deadline_blocks = 7",
            ),
            "out of range",
        ),
        (good.replace("p = 1", "p = 0"), "out of range"),
        (
            good.replace(
                "async_catch_budget           = 22.05",
                "async_catch_budget = 2.0001",
            ),
            "fractional",
        ),
        (
            good.replace("reverify_max_attempts        = 3\n", ""),
            "absent",
        ),
        (
            good.replace("spec      = \"2.2\"", "spec      = \"2.0\""),
            "2.2",
        ),
    ];
    for (raw, needle) in cases {
        let err = crate::levers::load(&raw, 2).expect_err(&raw);
        assert!(err.contains(needle), "{err}");
    }
    let dir = scratch("cfg");
    let path = dir.join("gate.toml");
    std::fs::write(&path, "served_origins = [\"https://groundinglab.io\"]\n").unwrap();
    let err = crate::config::Config::load(&path).unwrap_err();
    assert!(err.contains("policy path is required"), "{err}");
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config.rs"))
        .unwrap();
    let prod = src.split("#[cfg(test)]").next().unwrap();
    assert!(!prod.contains("CARGO_MANIFEST_DIR"));
    assert!(!prod.contains("policy_table"));
    assert!(!prod.contains("policy_from_file"));
}

/// AC-G15. Public lines do not name v2.0.
#[test]
fn public_lines_do_not_name_v2_0() {
    assert!(crate::policy::EARLIER_LINE.contains("pinned Satoshi Hike kind"));
    assert!(crate::policy::UNREADABLE_LINE.contains("pinned Satoshi Hike kind"));
    assert!(!crate::policy::EARLIER_LINE.contains("2.2 only"));
    assert!(
        crate::reasons::public_sentence(crate::reasons::CODE_SHAPE_MISMATCH).contains("section 6")
    );
    for line in crate::reasons::public_reason_lines() {
        assert!(!line.contains("v2.0"), "{line}");
        assert!(!line.contains("2.0"), "{line}");
    }
}

/// SPEC Rule B4. The pinned O3 file's empty paths are skips, so the count is
/// settled below `min_anchored_paths` and the fast phase refuses at once.
#[test]
fn pinned_o3_empty_paths_refuse_under_rule_b4() {
    let bytes = object_bytes("O3-PROD.cbor");
    assert_eq!(hex::encode(canon::sha256(&bytes)), O3_SHA);
    let (door, _index, phase, seq) = settle_face(&bytes, Face::Honest, v10());
    assert_eq!(phase, FastPhase::Done, "{:?}", chain(&door));
    assert!(seq.is_none());
    assert!(door
        .test_reasons()
        .iter()
        .any(|reason| reason.contains("too_few")));
    assert!(!door.log_dir().join("pending").exists());
    assert!(!door.log_dir().join("friendly").exists());
}

/// AC-G9. One production admission through the release door, then one forged leg.
#[test]
fn o3_prod_release_door_accepts_and_a_forged_checkpoint_does_not() {
    if cfg!(debug_assertions) {
        return;
    }
    let _guard = terrain_lock();
    let text = std::fs::read_to_string("/proc/meminfo").expect("meminfo");
    let kb = text
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix("MemAvailable:")?;
            rest.split_whitespace().next()?.parse::<u64>().ok()
        })
        .expect("MemAvailable");
    assert!(
        kb >= 800 * 1024,
        "MemAvailable is {kb} KiB; a production terrain needs 800 MiB free"
    );
    let pinned = object_bytes("O3-PROD.cbor");
    let sha_pinned = canon::sha256(&pinned);
    assert_eq!(hex::encode(sha_pinned), O3_SHA);
    // Empty paths are skips. Rule B4 refuses the pinned file at once.
    // The same legs, with six in-window paths, are what policy can admit.
    thorough::reset_counts();
    {
        let obj = pcoc::decode_object(&pinned).unwrap();
        let index = serve(&obj, Face::Honest);
        let door = open_door(v10(), &index.base);
        let (phase, seq) = door.settle_fast_seq(&job_for(&pinned, [69u8; 32]));
        assert_eq!(phase, FastPhase::Done, "{:?}", chain(&door));
        assert!(seq.is_none());
        assert!(door
            .test_reasons()
            .iter()
            .any(|reason| reason.contains("too_few")));
        assert_eq!(thorough::fills(), 0);
        assert_eq!(thorough::draws(), 0);
    }
    let bytes = with_anchored_paths(&pinned, 6);
    let original = pcoc::decode_object(&pinned).unwrap();
    let round = pcoc::decode_object(&bytes).unwrap();
    for (before, after) in original.stages.iter().zip(round.stages.iter()) {
        assert_eq!(before.checkpoints, after.checkpoints);
        assert_eq!(before.leg_results, after.leg_results);
    }
    let sha = canon::sha256(&bytes);
    let obj = pcoc::decode_object(&bytes).unwrap();
    let index = serve(&obj, Face::Honest);
    let policy = v10();
    let door = open_door(policy.clone(), &index.base);
    let job = job_for(&bytes, [70u8; 32]);
    let (phase, seq) = door.settle_fast_seq(&job);
    assert_eq!(phase, FastPhase::Provisional, "{:?}", chain(&door));
    let seq = seq.expect("provisional seq");
    let fills = thorough::fills();
    let started = std::time::Instant::now();
    let out =
        thorough::run(door.log_dir().as_path(), seq, &bytes, &sha, &policy).expect("thorough");
    eprintln!(
        "production thorough wall seconds (fact, not a measurement): {}",
        started.elapsed().as_secs()
    );
    assert!(out.matched, "drawn legs did not match");
    assert_eq!(thorough::fills(), fills + 1);
    door.settle_thorough(&job, out);
    assert!(door.test_decisions().contains(&"accept".to_owned()));
    assert!(door
        .log_dir()
        .join("friendly")
        .join(hex32(&job.public_key))
        .is_file());

    let kb = std::fs::read_to_string("/proc/meminfo")
        .unwrap()
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix("MemAvailable:")?;
            rest.split_whitespace().next()?.parse::<u64>().ok()
        })
        .unwrap();
    assert!(
        kb >= 800 * 1024,
        "MemAvailable is {kb} KiB before the forged leg"
    );
    let mut forged_obj = pcoc::decode_object(&bytes).unwrap();
    forged_obj.stages[0].checkpoints[0][0] ^= 0xff;
    let forged = pcoc::encode_object(&forged_obj);
    let one = thorough::recompute_one(&forged, 0, &Profile::production()).expect("one leg");
    assert!(!one.matched);
    let forged_job = job_for(&forged, [71u8; 32]);
    let seq = door.test_grant_provisional(&forged_job);
    let mut one = one;
    one.seq = seq;
    door.settle_thorough(&forged_job, one);
    assert!(door.test_decisions().contains(&"revoke".to_owned()));
}
