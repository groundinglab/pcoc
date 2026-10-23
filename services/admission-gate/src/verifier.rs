//! Verifier seam.
//!
//! The fast phase calls [`crate::fast`] (`pcoc::precheck`). Thorough draws
//! call [`crate::thorough`] (`pcoc::thorough_draw`). This module classifies
//! kind and key, and turns a pre-check into a [`Report`], without filling a
//! terrain. Leg recompute of a drawn index is the thorough phase.
#![forbid(unsafe_code)]

use crate::cbor_limit::{self, MAX_CBOR_NESTING};
use pcoc::cbor::{self, Cbor};
use pcoc::profile::Profile;
use pcoc::verify::{HeightIndex, Precheck, Verified};
use std::time::Duration;

/// Health name. The object kind is SPEC §6.
pub const SPEC_NAME: &str = "Satoshi Hike 2.2";

/// Health field.
pub const SPEC_KIND_LABEL: &str = "2.2";

/// Wire KIND exported by the Rust SDK. SPEC §6.
pub const SPEC_KIND: &str = pcoc::constants::KIND;

/// A one-shot index read did not return a document. Not a retry.
pub const INDEX_UNREAD_NOTE: &str = "the index did not answer for a stored beacon";

/// Kept so a note with this spelling still maps. The fast phase does not emit it.
pub const EARLIER_VERSION_NOTE: &str = "earlier version";

/// Kept so a note with this spelling still maps. The fast phase does not emit it.
pub const EARLIER_VERSION_FACT: &str = "earlier_version";

/// Inputs for one measurement. The gate does not read `PCOC_BIND_THREADS`.
#[derive(Clone, Debug)]
pub struct MeasureOpts {
    pub index_url: String,
    pub timeout: Duration,
    pub bind_threads: usize,
}

/// Measurements the fast pre-check produced. This struct does not admit.
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    /// Always false. Rule A5 is `unknown_kind`, not an earlier version.
    pub earlier_version: bool,
    pub kind: String,
    pub record: String,
    pub verifier_notes: Vec<String>,
    pub measures: Measures,
}

/// Verifier numbers for one decision line. Absent means not obtained.
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

    pub fn empty() -> Self {
        Self {
            legs: None,
            span_s: None,
            leg_rate: None,
            waypoints_fetched: None,
            waypoints_total: crate::policy::WAYPOINTS_TOTAL,
            paths_included: None,
            legs_recomputed_ok: None,
        }
    }
}

/// Keys that are policy, not measurements.
pub const MEASURE_THRESHOLD_KEYS: &[&str] = &[
    "min_leg_rate",
    "min_waypoints",
    "min",
    "policy_id",
    "floor",
    "threshold",
];

#[derive(Clone, Debug, PartialEq)]
pub enum Report {
    /// The bytes are not an object this gate can classify.
    Unreadable,
    Read(Measured),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facts {
    pub kind: Option<String>,
    pub public_key: Option<[u8; 32]>,
}

/// The 2.2 Rust SDK is linked.
pub fn reference_linked() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Class {
    /// Rule A5. Not the current kind.
    UnknownKind {
        kind: String,
    },
    Unreadable,
    NoPublicKey,
    /// Current kind and a 32-byte public key. Shape may still mismatch.
    Ready {
        public_key: [u8; 32],
    },
}

/// Kind and key only. Does not fill a terrain and does not draw.
pub fn classify(object: &[u8]) -> Class {
    if cbor_limit::nesting_exceeds(object, MAX_CBOR_NESTING) {
        return Class::Unreadable;
    }
    match pcoc::precheck(object, &Profile::production()) {
        Precheck::UnknownKind(record) => {
            let kind = serde_json::from_str::<serde_json::Value>(&record)
                .ok()
                .and_then(|value| {
                    value
                        .get("kind_text")
                        .and_then(|item| item.as_str())
                        .map(str::to_owned)
                })
                .unwrap_or_default();
            Class::UnknownKind { kind }
        }
        Precheck::Malformed | Precheck::DecodeFailure => Class::Unreadable,
        Precheck::Shape { .. } => match public_key(object) {
            Some(public_key) => Class::Ready { public_key },
            None => Class::NoPublicKey,
        },
    }
}

/// Section 14 for a current object. Unknown kind is a note `unknown_kind`
/// and does not read the index. A shape match calls `pcoc::verify` against
/// `{index-url}` so `hike_span` is `not read` only when that read fails.
pub fn measure(object: &[u8], opts: &MeasureOpts) -> Report {
    if cbor_limit::nesting_exceeds(object, MAX_CBOR_NESTING) {
        return Report::Unreadable;
    }
    match pcoc::precheck(object, &Profile::production()) {
        Precheck::UnknownKind(record) => Report::Read(Measured {
            earlier_version: false,
            kind: kind_text(&record),
            record,
            verifier_notes: vec!["unknown_kind".to_owned()],
            measures: Measures::empty(),
        }),
        Precheck::Malformed | Precheck::DecodeFailure => Report::Unreadable,
        Precheck::Shape { shape, shape_item } => {
            if shape != "match" {
                let record = serde_json::json!({
                    "profile": "production",
                    "shape": shape,
                    "shape_item": shape_item,
                })
                .to_string();
                return Report::Read(Measured {
                    earlier_version: false,
                    kind: SPEC_KIND.to_owned(),
                    record,
                    verifier_notes: vec!["shape_mismatch".to_owned()],
                    measures: Measures::empty(),
                });
            }
            let mut index = FetchIndex {
                client: IndexClient::open(&opts.index_url, opts.timeout),
            };
            match pcoc::verify(object, &Profile::production(), &mut index) {
                Verified::DecodeFailure => Report::Unreadable,
                Verified::UnknownKind(record) => Report::Read(Measured {
                    earlier_version: false,
                    kind: kind_text(&record),
                    record,
                    verifier_notes: vec!["unknown_kind".to_owned()],
                    measures: Measures::empty(),
                }),
                Verified::Record(record) => {
                    let notes = shape_notes(&record);
                    let mut record = record;
                    if !record.ends_with('\n') {
                        record.push('\n');
                    }
                    Report::Read(Measured {
                        earlier_version: false,
                        kind: SPEC_KIND.to_owned(),
                        record,
                        verifier_notes: notes,
                        measures: Measures::empty(),
                    })
                }
            }
        }
    }
}

/// `shape` other than `match` is a refusal note. A match leaves the notes
/// empty so Rule B2 can hold a real `hike_span` of `not read`, or admit.
fn shape_notes(record: &str) -> Vec<String> {
    let shape = serde_json::from_str::<serde_json::Value>(record)
        .ok()
        .and_then(|value| {
            value
                .get("shape")
                .and_then(|item| item.as_str())
                .map(str::to_owned)
        });
    if shape.as_deref() == Some("match") {
        Vec::new()
    } else {
        vec!["shape_mismatch".to_owned()]
    }
}

/// `{index-url}/index/{height}` for one section 14 verify. A non-2xx is `None`.
struct FetchIndex {
    client: IndexClient,
}

impl HeightIndex for FetchIndex {
    fn read_height(&mut self, height: u64) -> Option<Vec<u8>> {
        self.client
            .get_cached(&format!("/index/{height}"))
            .ok()
            .map(String::into_bytes)
    }
}

fn kind_text(record: &str) -> String {
    serde_json::from_str::<serde_json::Value>(record)
        .ok()
        .and_then(|value| {
            value
                .get("kind_text")
                .and_then(|item| item.as_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown kind".to_owned())
}

/// 32-byte `public_key`, after the nesting cap. `None` when the map has none.
pub fn public_key(object: &[u8]) -> Option<[u8; 32]> {
    if cbor_limit::nesting_exceeds(object, MAX_CBOR_NESTING) {
        return None;
    }
    let item = cbor::decode_one(object).ok()?;
    let Cbor::Map(map) = item else {
        return None;
    };
    let Cbor::Bytes(bytes) = cbor::map_get(&map, "public_key")? else {
        return None;
    };
    if bytes.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(bytes);
    Some(key)
}

/// Network string on a portable-index body. `None` when the body is not JSON
/// or the `network` string is absent. The string `"absent"` is not invented.
pub fn document_network(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let text = value.get("network").and_then(|item| item.as_str())?;
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

/// A document that parsed and carried no string `network`.
pub const NETWORK_ABSENT: &str = "absent";

/// Height documents the fast phase has read. A missing `network` string is a
/// gap, not the word `"absent"`.
pub trait IndexRead {
    fn observed_networks(&self) -> Vec<String>;
    /// A document was read and carried no `network` string.
    fn network_gap(&self) -> bool {
        false
    }
}

/// Cache of `{index-url}` response bodies. Not a terrain. One entry per path.
pub struct Mirror {
    bodies: std::sync::Mutex<std::collections::HashMap<String, String>>,
    bypasses: std::sync::atomic::AtomicU64,
    fresh: std::sync::atomic::AtomicU64,
}

impl Mirror {
    pub fn new() -> Self {
        Self {
            bodies: std::sync::Mutex::new(std::collections::HashMap::new()),
            bypasses: std::sync::atomic::AtomicU64::new(0),
            fresh: std::sync::atomic::AtomicU64::new(0),
        }
    }

    pub fn bypass_count(&self) -> u64 {
        self.bypasses.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn fresh_count(&self) -> u64 {
        self.fresh.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn cached(&self, path: &str) -> Option<String> {
        self.bodies.lock().expect("index mirror").get(path).cloned()
    }

    fn store(&self, path: &str, body: String) {
        self.bodies
            .lock()
            .expect("index mirror")
            .insert(path.to_owned(), body);
    }
}

impl Default for Mirror {
    fn default() -> Self {
        Self::new()
    }
}

/// One-shot index client. The mirror caches response bodies only.
pub struct IndexClient {
    url: String,
    timeout: Duration,
    mirror: std::sync::Arc<Mirror>,
    networks: std::sync::Mutex<Vec<String>>,
    gap: std::sync::atomic::AtomicBool,
}

impl IndexClient {
    pub fn open(url: &str, timeout: Duration) -> Self {
        Self::shared(url, timeout, std::sync::Arc::new(Mirror::new()))
    }

    pub fn shared(url: &str, timeout: Duration, mirror: std::sync::Arc<Mirror>) -> Self {
        Self {
            url: url.trim_end_matches('/').to_owned(),
            timeout,
            mirror,
            networks: std::sync::Mutex::new(Vec::new()),
            gap: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn mirror(&self) -> std::sync::Arc<Mirror> {
        self.mirror.clone()
    }

    /// Cached GET. A miss fetches once and stores the body.
    pub fn get_cached(&self, path: &str) -> Result<String, ()> {
        if let Some(hit) = self.mirror.cached(path) {
            self.note_body(&hit);
            return Ok(hit);
        }
        let body = self.fetch(path)?;
        self.mirror.store(path, body.clone());
        self.note_body(&body);
        Ok(body)
    }

    /// Fresh GET that bypasses the mirror, then replaces the cached body.
    pub fn confirm_fresh(&self, path: &str) -> Result<String, ()> {
        self.mirror
            .bypasses
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let body = self.fetch(path)?;
        self.mirror.store(path, body.clone());
        self.note_body(&body);
        Ok(body)
    }

    /// Tip read. Always fresh. Not stored as a height document.
    pub fn tip(&self) -> Result<String, ()> {
        self.mirror
            .fresh
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let body = self.fetch("/index")?;
        self.note_body(&body);
        Ok(body)
    }

    fn note_body(&self, body: &str) {
        match document_network(body) {
            Some(network) => {
                let mut networks = self.networks.lock().expect("networks");
                if !networks.iter().any(|item| item == &network) {
                    networks.push(network);
                }
            }
            None if serde_json::from_str::<serde_json::Value>(body).is_ok() => {
                self.gap.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            None => {}
        }
    }

    fn fetch(&self, path: &str) -> Result<String, ()> {
        let url = format!("{}{path}", self.url);
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(self.timeout)
            .timeout_read(self.timeout)
            .redirects(0)
            .build();
        let response = agent.get(&url).call().map_err(|_| ())?;
        if !(200..300).contains(&response.status()) {
            return Err(());
        }
        response.into_string().map_err(|_| ())
    }
}

impl IndexRead for IndexClient {
    fn observed_networks(&self) -> Vec<String> {
        self.networks.lock().expect("networks").clone()
    }

    fn network_gap(&self) -> bool {
        self.gap.load(std::sync::atomic::Ordering::SeqCst)
    }
}
