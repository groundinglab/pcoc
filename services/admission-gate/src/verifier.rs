//! Verifier seam.
//!
//! [`linked_measure`] is the only function that calls the Rust SDK
//! crate (`hike::verify`). Callers see [`Report`]. An earlier kind is the
//! SDK's Rule A5 (`earlier version`), which policy spells `earlier_version`.
#![forbid(unsafe_code)]

use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Locked version id (#206). Health reports this name.
pub const SPEC_NAME: &str = "Satoshi Hike v2.0";

/// Health field. The parenthetical in the ticket.
pub const SPEC_KIND_LABEL: &str = "v2.0";

/// Wire KIND exported by the Rust SDK.
pub const SPEC_KIND: &str = hike::KIND;

/// A one-shot index read did not return a document. Not a retry.
pub const INDEX_UNREAD_NOTE: &str = "the index did not answer for a stored beacon";

const INDEX_BODY_CAP: usize = 1024 * 1024;

/// Inputs for one thorough measurement. The gate does not read `PCOC_BIND_THREADS`.
#[derive(Clone, Debug)]
pub struct MeasureOpts {
    pub index_url: String,
    pub timeout: Duration,
    pub bind_threads: usize,
}

/// What the v2.0 verifier writes for an older object (Rule A5).
pub const EARLIER_VERSION_NOTE: &str = "earlier version";

/// Gate token for that note. `hike::failure_facts` uses this spelling.
pub const EARLIER_VERSION_FACT: &str = "earlier_version";

/// Measurements the SDK verifier returned for a v2.0 object.
///
/// `record` is the bytes of `hike::verify_record_json("production", …)`,
/// including the trailing newline. Policy reads `(N, S)` from that text.
/// It does not trim the newline, drop `profile`, or build a second object.
/// This struct does not say admit or refuse.
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    pub earlier_version: bool,
    pub kind: String,
    pub record: String,
    /// Verifier notes, in the verifier's words. `earlier version` is one.
    pub verifier_notes: Vec<String>,
    /// Numbers only. No threshold. A side the verifier did not obtain is `None`.
    pub measures: Measures,
}

/// Verifier numbers for one decision line.
///
/// Absent means the verifier did not obtain that side. A threshold never
/// appears here. `waypoints_total` is the spec stage count, 18.
#[derive(Clone, Debug, PartialEq)]
pub struct Measures {
    pub legs: Option<u64>,
    pub span_s: Option<u64>,
    pub leg_rate: Option<f64>,
    pub waypoints_fetched: Option<u64>,
    pub waypoints_total: u64,
    pub paths_included: Option<u64>,
    pub legs_recomputed_ok: Option<u64>,
}

impl Measures {
    /// JSON object. A missing side is null.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "legs": self.legs,
            "span_s": self.span_s,
            "leg_rate": self.leg_rate,
            "waypoints_fetched": self.waypoints_fetched,
            "waypoints_total": self.waypoints_total,
            "paths_included": self.paths_included,
            "legs_recomputed_ok": self.legs_recomputed_ok,
        })
    }
}

/// Keys that are policy, not measurements. A schema test fails if one appears.
pub const MEASURE_THRESHOLD_KEYS: &[&str] = &[
    "min_leg_rate",
    "min_waypoints",
    "min",
    "policy_id",
    "floor",
    "threshold",
];

/// Read measures off an SDK [`hike::Measurements`]. No judgment.
pub fn measures_of(measurements: &hike::Measurements) -> Measures {
    let (legs, span_s, leg_rate) = bookend_measures(&measurements.record_body);
    let waypoints_fetched = match &measurements.waypoints {
        hike::Recorded::Obtained(waypoints) => Some(
            waypoints
                .anchors
                .iter()
                .filter(|anchor| **anchor == hike::Anchor::Anchored)
                .count() as u64,
        ),
        hike::Recorded::NotObtained => None,
    };
    let paths_included = match measurements.paths_included {
        hike::Recorded::Obtained(count) => Some(count),
        hike::Recorded::NotObtained => None,
    };
    let legs_recomputed_ok = match &measurements.six_legs {
        hike::SixLegs::Rewalked { matches, .. } => {
            Some(matches.iter().filter(|flag| **flag).count() as u64)
        }
        hike::SixLegs::CouldNotDraw
        | hike::SixLegs::DrawNotObtained
        | hike::SixLegs::NotObtained => None,
    };
    Measures {
        legs,
        span_s,
        leg_rate,
        waypoints_fetched,
        waypoints_total: crate::policy::WAYPOINTS_TOTAL,
        paths_included,
        legs_recomputed_ok,
    }
}

/// `N`, `S`, and `N/S` from a section 14 record body. A word is not a number.
///
/// The body starts with a comma. Wrapping it makes one JSON object.
fn bookend_measures(record_body: &str) -> (Option<u64>, Option<u64>, Option<f64>) {
    let wrapped;
    let text = if let Some(rest) = record_body.strip_prefix(',') {
        wrapped = format!("{{{rest}");
        wrapped.as_str()
    } else {
        record_body
    };
    let rate = hike::beacon_rate(text);
    let legs = rate.legs;
    let span_s = rate.span_s.and_then(|span| u64::try_from(span).ok());
    let leg_rate = match (legs, span_s) {
        (Some(legs), Some(span)) if span > 0 => Some(legs as f64 / span as f64),
        _ => None,
    };
    (legs, span_s, leg_rate)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Report {
    /// The object is an older Satoshi Hike object.
    Earlier {
        kind: String,
    },
    /// The bytes are not a Satoshi Hike object this gate can classify.
    Unreadable,
    Read(Measured),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facts {
    pub kind: Option<String>,
    pub public_key: Option<[u8; 32]>,
}

/// The Rust SDK crate (`hike`) is linked. Thorough verify can run.
pub fn reference_linked() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Class {
    Earlier {
        kind: String,
    },
    Unreadable,
    NoPublicKey,
    /// v2.0 kind and a public key. Thorough verify may run.
    Ready {
        public_key: [u8; 32],
    },
}

/// Kind and key only. This does not run section 14, so the fast sign-in
/// response stays ahead of thorough verify. An earlier kind is Rule A5 in
/// the SDK (`hike::earlier_object_kind`), the same check `hike::verify` runs first.
pub fn classify(object: &[u8]) -> Class {
    if let Some(kind) = hike::earlier_object_kind(object) {
        return Class::Earlier { kind };
    }
    if !kind_is_current(object) {
        return Class::Unreadable;
    }
    // The SDK parser decides the key. A local scan can see a key the
    // SDK will not yield (a duplicate key is one case) and must not
    // open a provisional line for those bytes.
    match hike::yielded_public_key(object) {
        hike::YieldedKey::PublicKey(public_key) => Class::Ready { public_key },
        hike::YieldedKey::NoPublicKey => Class::NoPublicKey,
        hike::YieldedKey::Undecodable => Class::Unreadable,
    }
}

fn kind_is_current(object: &[u8]) -> bool {
    matches!(
        facts(object).ok().and_then(|item| item.kind),
        Some(kind) if kind == hike::KIND
    )
}

pub fn measure(object: &[u8], opts: &MeasureOpts) -> Report {
    if hike::earlier_object_kind(object).is_some() {
        return linked_measure(object, opts);
    }
    if !kind_is_current(object) {
        return Report::Unreadable;
    }
    if matches!(
        hike::yielded_public_key(object),
        hike::YieldedKey::Undecodable
    ) {
        return Report::Unreadable;
    }
    linked_measure(object, opts)
}

/// The only call into the Rust SDK verifier.
///
/// Index reads are one shot. `hike::read_index_beacon` is not called here.
/// A failed read is `Err` to the SDK, which records that beacon as
/// `not read`. This function adds [`INDEX_UNREAD_NOTE`] only when the
/// SDK also recorded a fact. The slot returns either way.
fn linked_measure(object: &[u8], opts: &MeasureOpts) -> Report {
    let agent = index_agent(opts.timeout);
    let beacon_agent = agent.clone();
    let path_agent = agent;
    let beacon_url = opts.index_url.clone();
    let path_url = opts.index_url.clone();
    let beacon_failures = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&beacon_failures);
    let threads = opts.bind_threads.max(1);
    let measurements = hike::verify_with_bind_threads(
        object,
        hike::AuditDraw::OsCsprng,
        move |height| match fetch_beacon(&beacon_agent, &beacon_url, height) {
            Ok(beacon) => Ok(beacon),
            Err(()) => {
                counted.fetch_add(1, Ordering::Relaxed);
                Err(())
            }
        },
        move |loc| fetch_path(&path_agent, &path_url, loc),
        threads,
    );
    if let hike::ObjectLineage::Earlier(kind) = measurements.version.clone() {
        return Report::Earlier { kind };
    }
    let mut verifier_notes = hike::failure_facts(&measurements)
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    // A failed read is already "not read" in the section 14 record. It becomes
    // a public note only when the SDK also recorded a fact, so an unread
    // closing header does not refuse an object the SDK admitted.
    if beacon_failures.load(Ordering::Relaxed) > 0 && !verifier_notes.is_empty() {
        verifier_notes.push(INDEX_UNREAD_NOTE.to_owned());
    }
    let measures = measures_of(&measurements);
    let record = hike::verify_record_json("production", &measurements);
    Report::Read(Measured {
        earlier_version: verifier_notes
            .iter()
            .any(|note| note == EARLIER_VERSION_FACT),
        kind: hike::KIND.to_owned(),
        record,
        verifier_notes,
        measures,
    })
}

fn index_agent(timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(timeout)
        .timeout_connect(timeout)
        .timeout_read(timeout)
        .user_agent("pcoc-admission")
        .redirects(0)
        .build()
}

/// Header and plant reads for one fast phase.
///
/// Each measurement is one shot, bounded by the timeout passed to [`IndexClient::open`].
/// A failed read is [`hike::ChainRead::NotRead`] (a header failure is `Err`).
/// [`IndexRead::beacon_fresh`] and [`IndexRead::plant_fresh`] skip the mirror
/// cache and send `Cache-Control: no-cache`. A plant or header `mismatch` is
/// refused only when that second read confirms it (§15.3, Rule B9).
pub trait IndexRead {
    fn beacon(&self, height: u64) -> Result<hike::IndexBeacon, ()>;
    fn beacon_fresh(&self, height: u64) -> Result<hike::IndexBeacon, ()>;
    fn plant(&self, loc: &hike::PathLocators) -> hike::ChainRead;
    fn plant_fresh(&self, loc: &hike::PathLocators) -> hike::ChainRead;
    /// The `network` of every index document this reader returned, header
    /// and plant reads alike, including fresh (confirming) reads. Distinct
    /// and sorted. A document with no string `network` is [`NETWORK_ABSENT`].
    fn observed_networks(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Recorded for an index document that carries no string `network` field.
pub const NETWORK_ABSENT: &str = "absent";

/// The `network` field of one index document. `None` when the body is not a
/// JSON object (not a document).
pub fn document_network(body: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(body).ok()?;
    let object = value.as_object()?;
    Some(
        object
            .get("network")
            .and_then(|item| item.as_str())
            .unwrap_or(NETWORK_ABSENT)
            .to_owned(),
    )
}

#[derive(Clone)]
enum Cached {
    Body(String),
    /// The shot failed. A later read of the same measurement does not retry it.
    Unread,
}

/// One-shot client for `GET {index_url}/index/{height}`.
///
/// The cache is this phase's mirror of `{index_url}`. Header reads and plant
/// reads are separate shots, so they do not share a cache entry. A fresh read
/// does not consult the cache and does not fill it.
#[derive(Clone)]
pub struct IndexClient {
    agent: ureq::Agent,
    url: String,
    cache: Arc<std::sync::Mutex<std::collections::HashMap<String, Cached>>>,
    networks: Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>,
}

impl IndexClient {
    pub fn open(url: &str, timeout: Duration) -> Self {
        Self {
            agent: index_agent(timeout),
            url: url.to_owned(),
            cache: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            networks: Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new())),
        }
    }

    fn note_network(&self, body: &str) {
        if let Some(network) = document_network(body) {
            self.networks
                .lock()
                .expect("index networks")
                .insert(network);
        }
    }

    fn url_at(&self, height: u64) -> String {
        format!("{}/index/{height}", self.url)
    }

    fn read_body(&self, url: &str, kind: &str, fresh: bool) -> Result<String, ()> {
        let key = format!("{kind} {url}");
        if !fresh {
            let cache = self.cache.lock().expect("index cache");
            if let Some(hit) = cache.get(&key) {
                return match hit {
                    Cached::Body(body) => {
                        let body = body.clone();
                        drop(cache);
                        self.note_network(&body);
                        Ok(body)
                    }
                    Cached::Unread => Err(()),
                };
            }
        }
        let fetched = fetch_body(&self.agent, url, fresh);
        if let Ok(body) = &fetched {
            self.note_network(body);
        }
        if !fresh {
            let slot = match &fetched {
                Ok(body) => Cached::Body(body.clone()),
                Err(()) => Cached::Unread,
            };
            self.cache.lock().expect("index cache").insert(key, slot);
        }
        fetched
    }

    fn plant_at(&self, loc: &hike::PathLocators, fresh: bool) -> hike::ChainRead {
        let url = self.url_at(u64::from(loc.block_height));
        let body = match self.read_body(&url, "plant", fresh) {
            Ok(body) => body,
            Err(()) => return hike::ChainRead::NotRead,
        };
        // A returned document with no matching plant is `mismatch`. A body
        // that is not a JSON object was not a document: `not read`.
        if !returned_document(&body) {
            return hike::ChainRead::NotRead;
        }
        match hike::proxy_matches(&body, loc) {
            Ok(()) => hike::ChainRead::Match,
            Err(()) => hike::ChainRead::Mismatch,
        }
    }
}

impl IndexRead for IndexClient {
    fn beacon(&self, height: u64) -> Result<hike::IndexBeacon, ()> {
        let body = self.read_body(&self.url_at(height), "header", false)?;
        hike::parse_index_beacon(&body).map_err(|_| ())
    }

    fn beacon_fresh(&self, height: u64) -> Result<hike::IndexBeacon, ()> {
        let body = self.read_body(&self.url_at(height), "header", true)?;
        hike::parse_index_beacon(&body).map_err(|_| ())
    }

    fn plant(&self, loc: &hike::PathLocators) -> hike::ChainRead {
        self.plant_at(loc, false)
    }

    fn plant_fresh(&self, loc: &hike::PathLocators) -> hike::ChainRead {
        self.plant_at(loc, true)
    }

    fn observed_networks(&self) -> Vec<String> {
        self.networks
            .lock()
            .expect("index networks")
            .iter()
            .cloned()
            .collect()
    }
}

/// A JSON object is a returned document. Anything else, including bytes that
/// are not JSON, is not a document.
fn returned_document(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body).is_ok_and(|value| value.is_object())
}

fn fetch_beacon(
    agent: &ureq::Agent,
    index_url: &str,
    height: u64,
) -> Result<hike::IndexBeacon, ()> {
    let body = fetch_body(agent, &format!("{index_url}/index/{height}"), false)?;
    hike::parse_index_beacon(&body).map_err(|_| ())
}

fn fetch_path(agent: &ureq::Agent, index_url: &str, loc: &hike::PathLocators) -> Result<(), ()> {
    let body = fetch_body(
        agent,
        &format!("{index_url}/index/{}", loc.block_height),
        false,
    )?;
    hike::proxy_matches(&body, loc)
}

fn fetch_body(agent: &ureq::Agent, url: &str, fresh: bool) -> Result<String, ()> {
    let request = agent.get(url);
    // A fresh read bypasses a cache in front of `{index_url}`. The mirror
    // cache inside [`IndexClient`] is skipped by the caller.
    let request = if fresh {
        request
            .set("Cache-Control", "no-cache")
            .set("Pragma", "no-cache")
    } else {
        request
    };
    let response = request.call().map_err(|_| ())?;
    if response.status() != 200 {
        return Err(());
    }
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut reader = response.into_reader();
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len().saturating_add(n) > INDEX_BODY_CAP {
                    return Err(());
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(_) => return Err(()),
        }
    }
    String::from_utf8(buf).map_err(|_| ())
}

pub fn facts(bytes: &[u8]) -> Result<Facts, ()> {
    let mut index = 0;
    let (major, count) = read_header(bytes, &mut index)?;
    if major != 5 {
        return Err(());
    }
    let mut kind = None;
    let mut public_key = None;
    for _ in 0..count {
        let key = read_text(bytes, &mut index)?;
        if key == "kind" && kind.is_none() {
            kind = Some(read_text(bytes, &mut index)?);
        } else if key == "public_key" && public_key.is_none() {
            public_key = read_key(bytes, &mut index)?;
        } else {
            skip(bytes, &mut index)?;
        }
    }
    Ok(Facts { kind, public_key })
}

fn read_key(bytes: &[u8], index: &mut usize) -> Result<Option<[u8; 32]>, ()> {
    let start = *index;
    let (major, len) = read_header(bytes, index)?;
    if major != 2 {
        *index = start;
        skip(bytes, index)?;
        return Ok(None);
    }
    let n = usize::try_from(len).map_err(|_| ())?;
    let end = index.checked_add(n).ok_or(())?;
    if end > bytes.len() {
        return Err(());
    }
    let slice = &bytes[*index..end];
    *index = end;
    if slice.len() != 32 {
        return Ok(None);
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(slice);
    Ok(Some(key))
}

fn read_text(bytes: &[u8], index: &mut usize) -> Result<String, ()> {
    let (major, len) = read_header(bytes, index)?;
    if major != 3 {
        return Err(());
    }
    let n = usize::try_from(len).map_err(|_| ())?;
    let end = index.checked_add(n).ok_or(())?;
    if end > bytes.len() {
        return Err(());
    }
    let text = std::str::from_utf8(&bytes[*index..end]).map_err(|_| ())?;
    *index = end;
    Ok(text.to_owned())
}

fn skip(bytes: &[u8], index: &mut usize) -> Result<(), ()> {
    let (major, arg) = read_header(bytes, index)?;
    match major {
        0 | 1 | 7 => Ok(()),
        2 | 3 => {
            let n = usize::try_from(arg).map_err(|_| ())?;
            let end = index.checked_add(n).ok_or(())?;
            if end > bytes.len() {
                return Err(());
            }
            *index = end;
            Ok(())
        }
        4 => {
            for _ in 0..arg {
                skip(bytes, index)?;
            }
            Ok(())
        }
        5 => {
            for _ in 0..arg {
                skip(bytes, index)?;
                skip(bytes, index)?;
            }
            Ok(())
        }
        6 => skip(bytes, index),
        _ => Err(()),
    }
}

fn read_header(bytes: &[u8], index: &mut usize) -> Result<(u8, u64), ()> {
    if *index >= bytes.len() {
        return Err(());
    }
    let byte = bytes[*index];
    *index += 1;
    let major = byte >> 5;
    let ai = byte & 0x1f;
    let arg = match ai {
        0..=23 => u64::from(ai),
        24 => read_uint(bytes, index, 1)?,
        25 => read_uint(bytes, index, 2)?,
        26 => read_uint(bytes, index, 4)?,
        27 => read_uint(bytes, index, 8)?,
        _ => return Err(()),
    };
    Ok((major, arg))
}

fn read_uint(bytes: &[u8], index: &mut usize, width: usize) -> Result<u64, ()> {
    let end = index.checked_add(width).ok_or(())?;
    if end > bytes.len() {
        return Err(());
    }
    let mut raw = [0u8; 8];
    raw[8 - width..].copy_from_slice(&bytes[*index..end]);
    *index = end;
    Ok(u64::from_be_bytes(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(url: &str) -> MeasureOpts {
        MeasureOpts {
            index_url: url.to_owned(),
            timeout: Duration::from_secs(1),
            bind_threads: 1,
        }
    }

    #[test]
    fn a_real_v135_object_is_an_earlier_kind() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/vectors/earlier-kind.cbor");
        let bytes = std::fs::read(&path).expect("earlier kind object");
        let facts = facts(&bytes).expect("cbor map");
        assert_eq!(facts.kind.as_deref(), Some("satoshi-hike-v1.35-object"));
        match measure(&bytes, &opts("http://127.0.0.1:9")) {
            Report::Earlier { kind } => assert_eq!(kind, "satoshi-hike-v1.35-object"),
            other => panic!("expected the reference earlier lineage, got {other:?}"),
        }
        assert!(reference_linked());
    }

    #[test]
    fn run_b_is_earlier_version_through_the_v2_reference() {
        let pack: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/vectors/earlier-run-b.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let bytes = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/vectors")
                .join(
                    std::path::Path::new(pack["object"].as_str().unwrap())
                        .file_name()
                        .unwrap(),
                ),
        )
        .unwrap();
        let measurements = hike::verify(
            &bytes,
            hike::AuditDraw::PinnedSeed([0u8; 32]),
            |_| -> Result<hike::IndexBeacon, ()> { panic!("an earlier object is not measured") },
            |_| -> Result<(), ()> { panic!("an earlier object is not measured") },
        );
        let record = hike::verify_record_json("production", &measurements);
        assert_eq!(record, pack["record"].as_str().unwrap());
        assert!(record.ends_with('\n'));
        match measure(&bytes, &opts("http://127.0.0.1:9")) {
            Report::Earlier { kind } => assert_eq!(kind, "satoshi-hike-v1.35-object"),
            other => panic!("expected earlier_version from the reference, got {other:?}"),
        }
    }

    #[test]
    fn the_v2_kind_is_not_classified_as_earlier() {
        let bytes = object_with_kind(SPEC_KIND, &[7u8; 32]);
        assert!(matches!(
            measure(&bytes, &opts("http://127.0.0.1:9")),
            Report::Read(_)
        ));
        assert!(matches!(classify(&bytes), Class::Ready { .. }));
        let facts = facts(&bytes).unwrap();
        assert_eq!(facts.public_key, Some([7u8; 32]));
    }

    #[test]
    fn garbage_is_unreadable() {
        assert!(matches!(
            measure(b"not-cbor", &opts("")),
            Report::Unreadable
        ));
        assert!(matches!(measure(&[0x80], &opts("")), Report::Unreadable));
    }

    #[test]
    fn a_duplicate_key_is_not_ready() {
        let mut bytes = object_with_kind(SPEC_KIND, &[7u8; 32]);
        bytes[0] = 0xa3;
        bytes.extend(cbor_text("public_key"));
        bytes.push(0x58);
        bytes.push(32);
        bytes.extend_from_slice(&[8u8; 32]);
        let parsed = facts(&bytes).expect("the local scan still sees the first key");
        assert_eq!(parsed.public_key, Some([7u8; 32]));
        assert!(matches!(
            hike::yielded_public_key(&bytes),
            hike::YieldedKey::Undecodable
        ));
        assert!(matches!(classify(&bytes), Class::Unreadable));
        assert!(matches!(
            measure(&bytes, &opts("http://127.0.0.1:9")),
            Report::Unreadable
        ));
    }

    /// B2. The Gate library, through the linked SDK, reproduces the TEST vector.
    /// This does not open a socket and does not call the live service.
    #[test]
    fn b2_test_3leg_record_matches_the_committed_vector() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/test-3leg.json");
        let text = std::fs::read_to_string(&path).expect("test-3leg vector");
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        let object = hex::decode(doc["object_hex"].as_str().unwrap()).unwrap();
        let sizes = hike::Sizes::conformance_small();
        let measurements = hike::verify_sized(
            &object,
            hike::AuditDraw::PinnedSeed([0xA5; 32]),
            |_| Err(()),
            |_| Ok(()),
            sizes,
        );
        let record = hike::verify_record_json("conformance-small", &measurements);
        // The committed vector is a v2.0 object. NL4 and C1 do not reproduce its record.
        assert_ne!(record, doc["record"].as_str().unwrap());
        assert!(record.contains("\"shape\":\"mismatch\""));
        let facts = hike::failure_facts(&measurements);
        assert!(facts.iter().any(|fact| *fact == "shape is false"));
        assert!(facts.iter().all(|fact| !fact.contains("fewer than 6")));
    }

    fn small_hike(stages: Vec<hike::Stage>, last_height: u64) -> Vec<u8> {
        let object = hike::Hike {
            public_key: [0x42; 32],
            seed_beacon: stages[0].beacon,
            last_interrupt: {
                let mut hash = [0u8; 32];
                hash[0] = 0x63;
                hash
            },
            last_interrupt_height: last_height,
            last_interrupt_absent: false,
            last_interrupt_hash_absent: false,
            last_interrupt_height_absent: false,
            stages,
        };
        hike::encode_hike_sized(&object, hike::Sizes::conformance_small())
    }

    fn stage(
        tag: u8,
        height: u64,
        waypoint: [u8; 32],
        checkpoints: Vec<[u8; 32]>,
        legs: Vec<[u8; 8]>,
    ) -> hike::Stage {
        let mut beacon = [0u8; 32];
        beacon[0] = tag;
        hike::Stage {
            beacon,
            beacon_height: height,
            leg_count: checkpoints.len() as u64,
            waypoint,
            path: Vec::new(),
            checkpoints,
            leg_results: legs,
            checkpoint_rows_defined: true,
            leg_result_missing: Vec::new(),
        }
    }

    /// Rule B9 through [`crate::policy::judge`]. The object is the SPEC §16.1
    /// size set, so this does not fill the production terrain.
    fn b9_reasons(bytes: &[u8]) -> Vec<String> {
        b9_reasons_indexed(bytes, |_| Err(()))
    }

    fn b9_reasons_indexed(
        bytes: &[u8],
        beacon_at: impl FnMut(u64) -> Result<hike::IndexBeacon, ()>,
    ) -> Vec<String> {
        let sizes = hike::Sizes::conformance_small();
        let measurements = hike::verify_sized(
            bytes,
            hike::AuditDraw::PinnedSeed([0xA5; 32]),
            beacon_at,
            |_| Ok(()),
            sizes,
        );
        let notes: Vec<String> = hike::failure_facts(&measurements)
            .into_iter()
            .map(str::to_owned)
            .collect();
        let record = hike::verify_record_json("conformance-small", &measurements);
        let report = Report::Read(Measured {
            earlier_version: notes.iter().any(|note| note == EARLIER_VERSION_FACT),
            kind: hike::KIND.to_owned(),
            record,
            verifier_notes: notes,
            measures: measures_of(&measurements),
        });
        match crate::policy::judge(&report, 1, 4) {
            crate::policy::Judgment::Refuse { reasons, .. } => reasons,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// Rule B9. Malformed bytes are a decode refusal. The Gate does not measure them.
    #[test]
    fn b9_decode_is_unreadable() {
        assert!(matches!(
            hike::yielded_public_key(&[0xff]),
            hike::YieldedKey::Undecodable
        ));
        assert!(matches!(classify(&[0xff]), Class::Unreadable));
        let measurements = hike::verify(
            &[0xff],
            hike::AuditDraw::PinnedSeed([0; 32]),
            |_| Err(()),
            |_| Ok(()),
        );
        let record = hike::verify_record_json("production", &measurements);
        assert!(record.contains("\"shape_item\":[\"malformed cbor\"]"));
        assert_eq!(hike::failure_facts(&measurements), vec!["shape is false"]);
        match crate::policy::judge(&Report::Unreadable, 1, 4) {
            crate::policy::Judgment::Refuse {
                reasons, record, ..
            } => {
                assert_eq!(reasons, vec!["unreadable".to_owned()]);
                assert_eq!(record, "{}");
            }
            other => panic!("expected a decode refusal, got {other:?}"),
        }
    }

    /// Section 14.1. Absent stages include `leg_count` in table order.
    /// The Gate records that string through the linked SDK.
    #[test]
    fn absent_stages_include_leg_count_in_shape_item() {
        let bytes = small_hike(
            vec![stage(
                1,
                10,
                [1; 32],
                vec![[1; 32]],
                vec![1u64.to_le_bytes()],
            )],
            20,
        );
        let sizes = hike::Sizes::conformance_small();
        let measurements = hike::verify_sized(
            &bytes,
            hike::AuditDraw::PinnedSeed([0xA5; 32]),
            |_| Err(()),
            |_| Ok(()),
            sizes,
        );
        let record = hike::verify_record_json("conformance-small", &measurements);
        let value: serde_json::Value = serde_json::from_str(record.trim_end()).unwrap();
        assert_eq!(
            value["shape_item"],
            serde_json::json!([
                "W_17 path",
                "stages length",
                "leg_count",
                "last_interrupt",
                "header height order",
                "header hash differ"
            ])
        );
    }

    /// Rule B9. Shape `mismatch` on a short object is a refusal.
    #[test]
    fn b9_shape_mismatch_refuses() {
        let bytes = small_hike(
            vec![stage(
                1,
                10,
                [1; 32],
                vec![[1; 32]],
                vec![1u64.to_le_bytes()],
            )],
            20,
        );
        let reasons = b9_reasons(&bytes);
        assert!(reasons.iter().any(|reason| reason == "shape is false"));
    }

    /// Rule B9. `height_range` is a shape refusal. The stored height is `not defined`.
    #[test]
    fn b9_height_range_refuses() {
        let bytes = small_hike(
            vec![stage(
                1,
                1 << 53,
                [1; 32],
                vec![[1; 32]],
                vec![1u64.to_le_bytes()],
            )],
            20,
        );
        let sizes = hike::Sizes::conformance_small();
        let measurements = hike::verify_sized(
            &bytes,
            hike::AuditDraw::PinnedSeed([0xA5; 32]),
            |_| Err(()),
            |_| Ok(()),
            sizes,
        );
        let record = hike::verify_record_json("conformance-small", &measurements);
        assert!(record.contains("\"height_range\""));
        assert!(record.contains("\"stored_height\":\"not defined\""));
        let reasons = b9_reasons(&bytes);
        assert!(reasons.iter().any(|reason| reason == "shape is false"));
    }

    /// Rule B9. A stored stage-0 waypoint that is not `SHA-256(seed)` is refused.
    #[test]
    fn b9_seed_value_mismatch_refuses() {
        let bytes = small_hike(
            vec![stage(
                1,
                10,
                [0xEE; 32],
                vec![[1; 32]],
                vec![1u64.to_le_bytes()],
            )],
            20,
        );
        let reasons = b9_reasons(&bytes);
        assert!(reasons
            .iter()
            .any(|reason| reason == "seed value mismatches"));
    }

    /// Rule B9. A waypoint that is not the section 12 rehash is refused.
    #[test]
    fn b9_waypoint_rehash_mismatch_refuses() {
        let bytes = small_hike(
            vec![
                stage(1, 10, [1; 32], vec![[2; 32]], vec![1u64.to_le_bytes()]),
                stage(2, 20, [0x77; 32], vec![[9; 32]], vec![3u64.to_le_bytes()]),
            ],
            30,
        );
        let reasons = b9_reasons(&bytes);
        assert!(reasons
            .iter()
            .any(|reason| reason == "a successor waypoint differs"));
    }

    /// Rule B9. A checkpoint digest that is not the recomputed preimage is refused.
    ///
    /// The index returns the stored headers, so a beacon miss cannot supply
    /// the `checkpoint mismatch` fact.
    #[test]
    fn b9_checkpoint_digest_mismatch_refuses() {
        use sha2::{Digest, Sha256};
        let sizes = hike::Sizes::conformance_small();
        let mut beacon = [0u8; 32];
        beacon[0] = 1;
        let seed = hike::production_seed(&[0x42; 32], &beacon);
        let seed_value = {
            let digest = Sha256::digest(seed);
            let mut out = [0u8; 32];
            out.copy_from_slice(&digest);
            out
        };
        let terrain = hike::fill_terrain_sized(&hike::seed_from_preimage(&seed), sizes);
        let leg = hike::leg_result_sized(&terrain, &seed_value, sizes);
        let sample = hike::terrain_sample_sized(&terrain, &seed_value, sizes);
        let digest = hike::checkpoint_sized(&seed_value, &seed_value, &sample, leg, sizes);
        let good = small_hike(
            vec![stage(
                1,
                10,
                seed_value,
                vec![digest],
                vec![leg.to_le_bytes()],
            )],
            20,
        );
        let mut flipped = digest;
        flipped[0] ^= 0xff;
        let bad = small_hike(
            vec![stage(
                1,
                10,
                seed_value,
                vec![flipped],
                vec![leg.to_le_bytes()],
            )],
            20,
        );
        let index = |height: u64| {
            let mut natural = [0u8; 32];
            natural[0] = if height == 10 { 1 } else { 0x63 };
            Ok(hike::IndexBeacon {
                natural,
                height,
                header_time_s: Some(1_700_000_000),
                timestamp_unix_ms: None,
            })
        };
        let good_reasons = b9_reasons_indexed(&good, index);
        assert!(good_reasons
            .iter()
            .all(|reason| reason != "checkpoint mismatch"));
        let bad_reasons = b9_reasons_indexed(&bad, index);
        assert!(bad_reasons
            .iter()
            .any(|reason| reason == "checkpoint mismatch"));
    }

    /// Rule B9. A six-leg recompute that disagrees with the stored leg is refused.
    #[test]
    fn b9_six_leg_mismatch_refuses() {
        let bytes = small_hike(
            vec![stage(1, 10, [4; 32], vec![[0xAB; 32]; 6], vec![[0; 8]; 6])],
            20,
        );
        let reasons = b9_reasons(&bytes);
        assert!(reasons
            .iter()
            .any(|reason| reason == "a recomputed leg result differs"));
    }

    /// Rule B9. An earlier kind is `earlier_version`, not a decode failure.
    #[test]
    fn b9_earlier_version_refuses() {
        match crate::policy::judge(
            &Report::Earlier {
                kind: "satoshi-hike-v1.35-object".to_owned(),
            },
            1,
            4,
        ) {
            crate::policy::Judgment::Refuse { reasons, line, .. } => {
                assert_eq!(reasons, vec!["earlier_version".to_owned()]);
                assert_eq!(line, crate::policy::EARLIER_LINE);
            }
            other => panic!("expected earlier_version, got {other:?}"),
        }
    }

    #[test]
    fn the_stored_record_is_verify_record_json() {
        let bytes = object_with_kind(SPEC_KIND, &[7u8; 32]);
        let direct = hike::verify(
            &bytes,
            hike::AuditDraw::PinnedSeed([0u8; 32]),
            |_| -> Result<hike::IndexBeacon, ()> { Err(()) },
            |_| -> Result<(), ()> { Err(()) },
        );
        let expected = hike::verify_record_json("production", &direct);
        match measure(&bytes, &opts("http://127.0.0.1:9")) {
            Report::Read(measured) => {
                assert_eq!(measured.record, expected);
                assert!(measured.record.starts_with("{\"profile\":\"production\","));
                assert!(measured.record.ends_with('\n'));
            }
            other => panic!("expected a read, got {other:?}"),
        }
    }

    #[test]
    fn verify_record_sha256_excludes_the_trailing_newline() {
        let file = "{\"b\":1,\"a\":2}\n";
        let hash = hike::verify_record_sha256(file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(file.trim_end_matches('\n')).unwrap();
        assert_eq!(
            hash,
            crate::canon::sha256(&crate::canon::jcs(&parsed).unwrap())
        );
        assert_ne!(hash, crate::canon::sha256(file.as_bytes()));
        assert_eq!(
            hex::encode(hash),
            "d3626ac30a87e6f7a6428233b3c68299976865fa5508e4267c5415c76af7a772"
        );
    }

    pub fn object_with_kind(kind: &str, key: &[u8; 32]) -> Vec<u8> {
        let mut out = vec![0xa2];
        out.extend(cbor_text("kind"));
        out.extend(cbor_text(kind));
        out.extend(cbor_text("public_key"));
        out.push(0x58);
        out.push(32);
        out.extend_from_slice(key);
        out
    }

    fn cbor_text(text: &str) -> Vec<u8> {
        let bytes = text.as_bytes();
        let mut out = Vec::new();
        if bytes.len() < 24 {
            out.push(0x60 | bytes.len() as u8);
        } else {
            out.push(0x78);
            out.push(bytes.len() as u8);
        }
        out.extend_from_slice(bytes);
        out
    }
}
