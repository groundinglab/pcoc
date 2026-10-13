//! Sign-in door for the locked §15.1 wire.
//!
//! The client sees delivery, `bad_signature`, or `malformed`. The POST is
//! never a thorough `accept`. Policy settles the website outcome after that
//! response. This process does not mint a session and does not call a website.
//!
//! A provisional line already in the log, with no thorough line, is settled
//! on restart. A new sign-in does not write that provisional line. There is
//! no friendly list. A pure bookend `not read` writes no thorough line.

use crate::canon::{self, hex32};
use crate::log::{self, DecisionLog, DecisionWrite, LogError};
use crate::policy::{self, Judgment};
use crate::signin_row::{Replay, SignInRows};
use crate::verifier::{self, Class, Report};
use crate::wire::{self, ClientClass, LockedBody};
use axum::http::StatusCode;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const VERIFIER_ERROR: &str = "verifier_error";

#[derive(Clone)]
pub struct Door {
    inner: Arc<Mutex<Inner>>,
    rows: Arc<SignInRows>,
    floor_p: u64,
    floor_q: u64,
    /// This build does not refuse at once for fewer than two of `W_1`–`W_16`,
    /// so the locked wire does not emit `too_few_waypoints_anchored`.
    enforces_waypoint_bar: bool,
}

struct Inner {
    log: DecisionLog,
    /// Unix second the locked wire uses. `None` reads the process clock.
    now: Option<u64>,
}

/// One locked-wire POST, after the transport edge.
pub struct LockedSignIn {
    pub status: StatusCode,
    pub body: Value,
    /// Set when step 9 consumed the nonce and policy still has to settle.
    pub thorough: Option<LockedThorough>,
}

#[derive(Clone)]
pub struct LockedThorough {
    pub nonce: [u8; 32],
    pub object: Vec<u8>,
    pub submission: [u8; 32],
    pub public_key: [u8; 32],
    pub object_sha: [u8; 32],
}

pub struct ThoroughJob {
    pub object: Vec<u8>,
    pub fast_seq: u64,
    pub submission: [u8; 32],
    pub public_key: [u8; 32],
    pub object_sha: [u8; 32],
    pub tip_height: u64,
}

impl Door {
    pub fn open(dir: &Path, floor_p: u64, floor_q: u64) -> Result<Self, String> {
        let log = DecisionLog::open(dir).map_err(show_log)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner { log, now: None })),
            rows: Arc::new(SignInRows::new()),
            floor_p,
            floor_q,
            enforces_waypoint_bar: false,
        })
    }

    pub fn consumed_nonce(&self, nonce: &[u8; 32]) -> bool {
        self.rows.consumed(nonce)
    }

    /// Pin the sign-in clock. `None` reads the process clock.
    pub fn set_now_unix_s(&self, now: Option<u64>) {
        self.lock().now = now;
    }

    fn now_unix_s(&self) -> u64 {
        if let Some(now) = self.lock().now {
            return now;
        }
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    }

    /// `GET /v1/sign-in/result/<nonce>`. An unknown nonce is `pending` and
    /// creates no row. A consumed pending row adds `submitted`. `accepted`
    /// adds measurement fields. Other tokens stay one field.
    pub fn sign_in_result(&self, nonce: &[u8; 32]) -> Value {
        self.rows.result_value(nonce, self.now_unix_s())
    }

    /// Step 2 for a field that breaks the Submit grammar. The nonce is not
    /// consumed. A hash match on a fresh nonce stores `malformed`.
    pub fn note_grammar_miss(&self, nonce: &[u8; 32], hash_matches: bool) {
        self.rows.note_unconsumed(
            nonce,
            hash_matches,
            wire::OUTCOME_MALFORMED,
            self.now_unix_s(),
        );
    }

    /// Transport edge for the locked body. Oversize and the rate write
    /// `rejected` and do not consume the nonce only when the object bytes
    /// hash to `object` and the nonce has no consumed row.
    pub fn note_transport_rejected(&self, body: &[u8]) {
        let Some(wire::RowParse::Body(locked)) = wire::parse_for_row(body) else {
            return;
        };
        let matches = canon::sha256(&locked.object) == locked.object_sha;
        self.rows.note_unconsumed(
            &locked.nonce,
            matches,
            wire::OUTCOME_REJECTED,
            self.now_unix_s(),
        );
    }

    /// Provisional lines that have no thorough line yet.
    pub fn unsettled(&self) -> Vec<ThoroughJob> {
        let inner = self.lock();
        let mut settled = Vec::new();
        for line in inner.log.lines() {
            if line.value.get("phase").and_then(|item| item.as_str()) != Some("thorough") {
                continue;
            }
            if let Some(seq) = line.value.get("fast_seq").and_then(|item| item.as_u64()) {
                settled.push(seq);
            }
        }
        let mut jobs = Vec::new();
        for line in inner.log.lines() {
            if line.value.get("decision").and_then(|item| item.as_str()) != Some("provisional") {
                continue;
            }
            let Some(seq) = line.value.get("seq").and_then(|item| item.as_u64()) else {
                continue;
            };
            if settled.contains(&seq) {
                continue;
            }
            let Some(object_sha) = line
                .value
                .get("object_sha256")
                .and_then(|item| item.as_str())
                .and_then(log::parse_hex32)
            else {
                continue;
            };
            let Some(public_key) = line
                .value
                .get("public_key")
                .and_then(|item| item.as_str())
                .and_then(log::parse_hex32)
            else {
                continue;
            };
            let Some(submission) = line
                .value
                .get("submission_sha256")
                .and_then(|item| item.as_str())
                .and_then(log::parse_hex32)
            else {
                continue;
            };
            let Some(object) = inner.log.object_bytes(&hex32(&object_sha)) else {
                eprintln!(
                    "admission-service thorough resume skipped: object {} is not on disk",
                    hex32(&object_sha)
                );
                continue;
            };
            let tip_height = line
                .value
                .get("bitcoin_tip_height")
                .and_then(|item| item.as_u64())
                .unwrap_or(0);
            jobs.push(ThoroughJob {
                object,
                fast_seq: seq,
                submission,
                public_key,
                object_sha,
                tip_height,
            });
        }
        jobs
    }

    /// Locked §15.1 check order. Returns on the first step that decides.
    ///
    /// The client body is `{"delivery":"ok"}`, `{"error":{"code":"bad_signature"}}`,
    /// or `{"error":{"code":"malformed"}}`. No `message`. No field that says
    /// which check missed.
    pub fn sign_in_locked(&self, body: LockedBody) -> LockedSignIn {
        let now = self.now_unix_s();
        let hash_matches = canon::sha256(&body.object) == body.object_sha;

        // 2. Origin, field grammar, or the window.
        if body.origin != wire::ORIGIN || !wire::window_accepts(now, body.exp) {
            self.rows
                .note_unconsumed(&body.nonce, hash_matches, wire::OUTCOME_MALFORMED, now);
            return locked_client(ClientClass::Malformed, None);
        }

        // 3. The object bytes do not hash to `object`. The stored outcome stays.
        if !hash_matches {
            return locked_client(ClientClass::BadSignature, None);
        }

        // 4. A consumed row. Same object and signature return the original
        // client class. Any other object or signature is client `malformed`
        // and leaves the stored outcome. Policy does not run again.
        if let Some(replay) = self.rows.replay(&body, now) {
            return replay_response(replay);
        }

        // 5. Shorter than 32 bytes. No key, so no signature check.
        if body.object.len() < 32 {
            return self.consume_terminal(
                &body,
                ClientClass::DeliveryOk,
                wire::OUTCOME_TOO_SHORT,
                now,
            );
        }

        // 6. No 32-byte key, or the signature does not verify. One client code.
        // The key is read from the bytes. Kind is steps 7 and 8, after the
        // signature, so an unreadable map that still yields a key is signed
        // before it is rejected.
        let key = hike_public_key(&body.object);
        let message = wire::bip340_message(&body.origin, &body.nonce, body.exp, &body.object_sha);
        let Some(public_key) = key else {
            return self.bad_signature_step(&body, now);
        };
        if !wire::verify_signature(&public_key, &message, &body.signature) {
            return self.bad_signature_step(&body, now);
        }

        // 7. Earlier kind, after a valid signature.
        if hike::earlier_object_kind(&body.object).is_some() {
            return self.consume_terminal(
                &body,
                ClientClass::DeliveryOk,
                wire::OUTCOME_EARLIER,
                now,
            );
        }

        // 8. Not a v2.0 hike object. A partial map is not that object.
        let v2 = matches!(verifier::classify(&body.object), Class::Ready { .. });
        if !v2 {
            return self.consume_terminal(
                &body,
                ClientClass::DeliveryOk,
                wire::OUTCOME_REJECTED,
                now,
            );
        }

        // 9. v2.0. Consume when this step starts. Policy settles later.
        let submission =
            wire::submission_sha256(&body.object_sha, &public_key, &message, &body.signature);
        match self
            .rows
            .consume(&body, ClientClass::DeliveryOk, wire::OUTCOME_PENDING, now)
        {
            Err(replay) => replay_response(replay),
            Ok(()) => {
                self.rows.note_holder(&body.nonce, public_key);
                LockedSignIn {
                    status: StatusCode::OK,
                    body: json!({ "delivery": "ok" }),
                    thorough: Some(LockedThorough {
                        nonce: body.nonce,
                        object: body.object,
                        submission,
                        public_key,
                        object_sha: body.object_sha,
                    }),
                }
            }
        }
    }

    fn bad_signature_step(&self, body: &LockedBody, now: u64) -> LockedSignIn {
        if let Some(replay) = self.rows.note_bad_signature(body, now) {
            return replay_response(replay);
        }
        locked_client(ClientClass::BadSignature, None)
    }

    fn consume_terminal(
        &self,
        body: &LockedBody,
        client: ClientClass,
        outcome: &str,
        now: u64,
    ) -> LockedSignIn {
        match self.rows.consume(body, client, outcome, now) {
            Ok(()) => locked_client(client, None),
            Err(replay) => replay_response(replay),
        }
    }

    /// Policy on a consumed v2.0 nonce.
    ///
    /// The website token is `accepted`, `pending` while unsettled, or
    /// `rejected`. This build does not emit `too_few_waypoints_anchored`.
    /// The public decision line uses that same token. It does not carry the
    /// leg-rate fraction, a threshold, a verifier note, or a retry sentence.
    pub fn settle_locked(&self, job: LockedThorough, report: Result<Report, String>) {
        let now = self.now_unix_s();
        if let Ok(Report::Read(measured)) = &report {
            let (legs, span_s) = bookends_from_record(&measured.record);
            self.rows.note_bookends(&job.nonce, legs, span_s);
        }
        let outcome = match &report {
            Ok(report) => website_outcome(
                report,
                self.floor_p,
                self.floor_q,
                self.enforces_waypoint_bar,
            ),
            Err(_) => wire::OUTCOME_REJECTED,
        };
        if outcome == wire::OUTCOME_PENDING {
            return;
        }
        self.rows.settle(&job.nonce, outcome, now);
        // The public line uses the website token. `verify_summary` stays `{}`
        // so a section 14 sentence cannot ride out on this wire.
        let (decision, reason, summary) = match outcome {
            wire::OUTCOME_ACCEPTED => ("accept", None, "{}".to_owned()),
            wire::OUTCOME_TOO_FEW => (
                "refuse",
                Some(wire::OUTCOME_TOO_FEW.to_owned()),
                "{}".to_owned(),
            ),
            _ => (
                "refuse",
                Some(wire::OUTCOME_REJECTED.to_owned()),
                "{}".to_owned(),
            ),
        };
        let mut inner = self.lock();
        let write = DecisionWrite {
            time_unix_s: now,
            bitcoin_tip_height: 0,
            phase: "thorough",
            decision,
            fast_seq: None,
            submission_sha256: job.submission,
            public_key: job.public_key,
            object_sha256: job.object_sha,
            object: job.object,
            reasons: vec![outcome.to_owned()],
            reason,
            summary,
        };
        if let Err(error) = inner.log.append(&write) {
            eprintln!(
                "admission-service locked thorough was not written: {}",
                show_log(error)
            );
        }
    }

    pub async fn settle(&self, job: ThoroughJob, report: Result<Report, String>) {
        let judgment = match report {
            Ok(report) => Ok(policy::judge(&report, self.floor_p, self.floor_q)),
            Err(error) => Err(error),
        };
        let (decision, reasons, line, summary) = match judgment {
            Ok(Judgment::Admit { record }) => ("accept", Vec::new(), None, record),
            Ok(Judgment::Refuse {
                reasons,
                line,
                record,
            }) => ("refuse", reasons, Some(line), record),
            // `held` is an HTTP outcome. It is not a decision-log event.
            Ok(Judgment::Held { .. }) => return,
            Err(_) => (
                "refuse",
                vec![VERIFIER_ERROR.to_owned()],
                Some("The verifier did not finish.".to_owned()),
                "{}".to_owned(),
            ),
        };
        let mut inner = self.lock();
        if already_settled(&inner.log, job.fast_seq) {
            return;
        }
        let write = DecisionWrite {
            time_unix_s: log::unix_now(),
            bitcoin_tip_height: job.tip_height,
            phase: "thorough",
            decision,
            fast_seq: Some(job.fast_seq),
            submission_sha256: job.submission,
            public_key: job.public_key,
            object_sha256: job.object_sha,
            object: job.object,
            reasons,
            reason: line,
            summary,
        };
        if let Err(error) = inner.log.append(&write) {
            eprintln!(
                "admission-service thorough fast_seq={} was not written: {}",
                job.fast_seq,
                show_log(error)
            );
        }
    }

    pub fn latest_bytes(&self) -> Option<Vec<u8>> {
        self.lock().log.latest_bytes()
    }

    pub fn day_bytes(&self, day: &str) -> Option<Vec<u8>> {
        self.lock().log.day_bytes(day)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("admission log")
    }
}

fn already_settled(log: &DecisionLog, fast_seq: u64) -> bool {
    log.lines().iter().any(|line| {
        line.value.get("phase").and_then(|item| item.as_str()) == Some("thorough")
            && line.value.get("fast_seq").and_then(|item| item.as_u64()) == Some(fast_seq)
    })
}

/// `N` and `S` from a verifier record. A word such as `not defined` or
/// `not read` is not a measurement, so that side stays absent.
pub fn bookends_from_record(record: &str) -> (Option<i64>, Option<i64>) {
    let Ok(value) = serde_json::from_str::<Value>(record) else {
        return (None, None);
    };
    fn one(item: Option<&Value>) -> Option<i64> {
        item.and_then(Value::as_i64)
    }
    (one(value.get("N")), one(value.get("S")))
}

fn hike_public_key(object: &[u8]) -> Option<[u8; 32]> {
    match hike::yielded_public_key(object) {
        hike::YieldedKey::PublicKey(key) => Some(key),
        hike::YieldedKey::NoPublicKey | hike::YieldedKey::Undecodable => None,
    }
}

fn website_outcome(
    report: &Report,
    floor_p: u64,
    floor_q: u64,
    waypoint_bar: bool,
) -> &'static str {
    match policy::judge(report, floor_p, floor_q) {
        Judgment::Admit { .. } => wire::OUTCOME_ACCEPTED,
        Judgment::Held { .. } => wire::OUTCOME_PENDING,
        Judgment::Refuse { reasons, .. } => {
            if waypoint_bar
                && reasons
                    .iter()
                    .any(|reason| wire::TOO_FEW_WAYPOINTS_REASONS.contains(&reason.as_str()))
            {
                wire::OUTCOME_TOO_FEW
            } else {
                wire::OUTCOME_REJECTED
            }
        }
    }
}

fn replay_response(replay: Replay) -> LockedSignIn {
    match replay {
        Replay::Same { client, .. } => locked_client(client, None),
        Replay::Other => locked_client(ClientClass::Malformed, None),
    }
}

#[cfg(test)]
mod tests {
    use super::bookends_from_record;

    #[test]
    fn numeric_bookends_parse_and_words_do_not() {
        assert_eq!(
            bookends_from_record(r#"{"N":1348,"S":5294}"#),
            (Some(1348), Some(5294))
        );
        assert_eq!(
            bookends_from_record("{\"N\":\"not defined\",\"S\":\"not defined\"}\n"),
            (None, None)
        );
        assert_eq!(
            bookends_from_record(r#"{"N":"not read","S":"not read"}"#),
            (None, None)
        );
        assert_eq!(bookends_from_record("not json"), (None, None));
        assert_eq!(bookends_from_record(r#"{"N":7}"#), (Some(7), None));
    }
}

fn locked_client(client: ClientClass, _outcome: Option<&str>) -> LockedSignIn {
    let (status, body) = match client {
        ClientClass::Malformed => (
            StatusCode::BAD_REQUEST,
            json!({ "error": { "code": "malformed" } }),
        ),
        ClientClass::BadSignature => (
            StatusCode::BAD_REQUEST,
            json!({ "error": { "code": "bad_signature" } }),
        ),
        ClientClass::DeliveryOk => (StatusCode::OK, json!({ "delivery": "ok" })),
    };
    LockedSignIn {
        status,
        body,
        thorough: None,
    }
}

fn show_log(error: LogError) -> String {
    match error {
        LogError::Io(error) => format!("the decision log could not be written: {error}"),
        LogError::NotThisLineage(message) | LogError::Unusable(message) => message,
    }
}
