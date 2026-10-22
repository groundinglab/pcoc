//! Verifier seam.
//!
//! The fast phase calls [`crate::fast`] (`pcoc::precheck`). Thorough draws
//! call [`crate::thorough`] (`pcoc::thorough_draw`). This module classifies
//! kind and key, and turns a pre-check into a [`Report`], without filling a
//! terrain. Leg recompute of a drawn index is not in this build.
#![forbid(unsafe_code)]

use crate::cbor_limit::{self, MAX_CBOR_NESTING};
use pcoc::cbor::{self, Cbor};
use pcoc::profile::Profile;
use pcoc::verify::Precheck;
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

    fn empty() -> Self {
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
    UnknownKind { kind: String },
    Unreadable,
    NoPublicKey,
    /// Current kind and a 32-byte public key. Shape may still mismatch.
    Ready { public_key: [u8; 32] },
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

/// Pre-check only. A full section 14 record would fill the production terrain.
/// That walk is not this call. Unknown kind is a note `unknown_kind`.
pub fn measure(object: &[u8], _opts: &MeasureOpts) -> Report {
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
            let record = serde_json::json!({
                "profile": "production",
                "shape": shape,
                "shape_item": shape_item,
            })
            .to_string();
            let notes = if shape == "match" {
                // Header, plant, and leg recompute are not measured here.
                vec!["thorough recompute is not in this build".to_owned()]
            } else {
                vec!["shape_mismatch".to_owned()]
            };
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

/// Network string on a portable-index body. `None` when the body is not JSON.
pub fn document_network(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    Some(
        value
            .get("network")
            .and_then(|item| item.as_str())
            .unwrap_or(NETWORK_ABSENT)
            .to_owned(),
    )
}

/// A document that parsed and carried no string `network`.
pub const NETWORK_ABSENT: &str = "absent";

/// Height documents the fast phase may have read. This build's fast phase
/// does not read them yet (SPEC §15.3 header and plant reads are a follow-up),
/// so [`IndexRead::observed_networks`] is empty and a provisional that the
/// policy ties to a network is held.
pub trait IndexRead {
    fn observed_networks(&self) -> Vec<String>;
}

/// One-shot index client. Reads are not issued by the fast pre-check.
pub struct IndexClient {
    url: String,
    timeout: Duration,
}

impl IndexClient {
    pub fn open(url: &str, timeout: Duration) -> Self {
        Self {
            url: url.to_owned(),
            timeout,
        }
    }
}

impl IndexRead for IndexClient {
    fn observed_networks(&self) -> Vec<String> {
        let _ = (&self.url, self.timeout);
        Vec::new()
    }
}
