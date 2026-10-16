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
use crate::policy::{self, AdmissionPolicy, Judgment};
use crate::reasons;
use crate::signin_row::{Replay, SignInRows};
use crate::verifier::{self, Class, Measures, Report};
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
    policy: AdmissionPolicy,
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
    /// A prior thorough accept with `policy_id` covers this pair.
    /// Measurement is skipped. `None` means measure.
    pub prior_accept: Option<AccessRef>,
}

#[derive(Clone, Debug)]
pub struct AccessRef {
    pub day: Option<String>,
    pub seq: Option<u64>,
    pub policy_id: String,
}

pub struct ThoroughJob {
    pub object: Vec<u8>,
    pub fast_seq: u64,
    pub submission: [u8; 32],
    pub public_key: [u8; 32],
    pub object_sha: [u8; 32],
    pub tip_height: Option<u64>,
}

impl Door {
    pub fn open(dir: &Path, policy: AdmissionPolicy) -> Result<Self, String> {
        policy.bind(dir)?;
        let mut log = DecisionLog::open(dir).map_err(show_log)?;
        log.set_policy_hint(policy::public_document(&policy))
            .map_err(show_log)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner { log, now: None })),
            rows: Arc::new(SignInRows::new()),
            policy,
        })
    }

    pub fn policy(&self) -> &AdmissionPolicy {
        &self.policy
    }

    pub fn consumed_nonce(&self, nonce: &[u8; 32]) -> bool {
        self.rows.consumed(nonce)
    }

    /// Test seam. Consumes a nonce the way step 9 does, without a signature.
    pub fn consume_for_test(&self, nonce: &[u8; 32], object_sha: [u8; 32], now: u64) {
        let body = crate::wire::LockedBody {
            origin: crate::wire::ORIGIN.to_owned(),
            nonce: *nonce,
            exp: now.saturating_add(120),
            object_sha,
            object: Vec::new(),
            signature: [0u8; 64],
        };
        let _ = self
            .rows
            .consume(&body, ClientClass::DeliveryOk, wire::OUTCOME_PENDING, now);
        self.rows.note_holder(nonce, [0u8; 32]);
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
            let tip_height = line.value.get("bitcoin_tip_height").and_then(|item| item.as_u64());
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
        if !wire::origin_served(&[wire::ORIGIN], &body.origin)
            || !wire::window_accepts(now, body.exp)
        {
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
                let prior_accept = {
                    let inner = self.lock();
                    prior_accept(&inner.log, &public_key, &body.object_sha).map(|line| {
                        let (day, seq) = line_ref(&line.value);
                        AccessRef {
                            day,
                            seq,
                            policy_id: line
                                .value
                                .get("policy_id")
                                .and_then(|item| item.as_str())
                                .unwrap_or("")
                                .to_owned(),
                        }
                    })
                };
                LockedSignIn {
                    status: StatusCode::OK,
                    body: json!({ "delivery": "ok" }),
                    thorough: Some(LockedThorough {
                        nonce: body.nonce,
                        object: body.object,
                        submission,
                        public_key,
                        object_sha: body.object_sha,
                        prior_accept,
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

    /// Policy on a consumed v2.0 nonce, under the decision-log lock.
    ///
    /// A prior thorough accept of the same `(pubkey, object_sha256)` that
    /// carries `policy_id` writes one `return` line and skips measurement.
    /// A return already written for that pair inside 300 s is echoed: the
    /// website outcome is `accepted` and no new line is written. A pair whose
    /// only thorough accepts lack `policy_id` falls through and is measured.
    /// A new key has no return. A different object is measured.
    ///
    /// The website token stays `accepted`, `pending`, or `rejected`. The log
    /// stores one `reason_code` and a class. It does not store `rejected`.
    pub fn settle_locked(&self, job: LockedThorough, report: Result<Report, String>) {
        let now = self.now_unix_s();
        if let Some(prior) = &job.prior_accept {
            let mut inner = self.lock();
            if let Some(echo) = recent_return(&inner.log, &job.public_key, &job.object_sha, now) {
                let (day, seq) = line_ref(&echo.value);
                drop(inner);
                self.rows.note_bookends(&job.nonce, None, None);
                self.rows.note_access(&job.nonce, day, seq);
                self.rows.settle(&job.nonce, wire::OUTCOME_ACCEPTED, now);
                return;
            }
            let mut write = DecisionWrite::bare(now, None, "return", "return");
            write.submission_sha256 = job.submission;
            write.public_key = job.public_key;
            write.object_sha256 = job.object_sha;
            write.object = job.object;
            write.reasons = vec![wire::OUTCOME_ACCEPTED.to_owned()];
            write.summary = None;
            write.policy_id = Some(prior.policy_id.clone());
            write.policy_checks = Some(serde_json::json!([]));
            write.access_ref_day = prior.day.clone();
            write.access_ref_seq = prior.seq;
            match inner.log.append(&write) {
                Ok(new_seq) => {
                    let day = log::utc_day(now);
                    drop(inner);
                    self.rows.note_bookends(&job.nonce, None, None);
                    self.rows.note_access(&job.nonce, Some(day), Some(new_seq));
                    self.rows.settle(&job.nonce, wire::OUTCOME_ACCEPTED, now);
                }
                Err(error) => {
                    eprintln!(
                        "admission-service return was not written: {}",
                        show_log(error)
                    );
                }
            }
            return;
        }

        let measured = match &report {
            Ok(Report::Read(measured)) => Some(measured.measures.clone()),
            _ => None,
        };
        if let Some(measures) = &measured {
            self.rows
                .note_bookends(&job.nonce, measures.legs.map(|n| n as i64), measures.span_s.map(|n| n as i64));
        }
        let checks = measured
            .as_ref()
            .map(|measures| policy::policy_checks(measures, &self.policy))
            .unwrap_or_default();
        let checks_met = policy::policy_admits(&checks);
        let unmet = checks.iter().any(|check| !check.met);
        let held = match &report {
            Ok(report) => matches!(
                policy::judge(
                    report,
                    self.policy.leg_rate_floor.p,
                    self.policy.leg_rate_floor.q,
                ),
                Judgment::Held { .. }
            ),
            Err(_) => false,
        };
        // A measured miss is a refusal. An unread pair with no other note stays held.
        let held = held && !unmet;
        let outcome = match &report {
            Ok(report) => reasons::website_outcome(report, checks_met, held),
            Err(_) => wire::OUTCOME_REJECTED,
        };
        if outcome == wire::OUTCOME_PENDING {
            return;
        }
        let reason = match &report {
            Ok(report) => reasons::from_report(report, &checks, &self.policy),
            Err(_) => reasons::verifier_error(),
        };
        let accept = outcome == wire::OUTCOME_ACCEPTED;
        // Settle only after the log line is written, so an accepted result
        // already carries log_day and log_seq when the browser reads it.
        let mut write = DecisionWrite::bare(
            now,
            None,
            "thorough",
            if accept { "accept" } else { "refuse" },
        );
        write.submission_sha256 = job.submission;
        write.public_key = job.public_key;
        write.object_sha256 = job.object_sha;
        write.object = job.object;
        write.reasons = vec![outcome.to_owned()];
        write.summary = None;
        write.measures = Some(measured.unwrap_or_else(unknown_measures).to_json());
        write.policy_id = Some(self.policy.policy_id.clone());
        write.policy_checks = Some(checks_json(&checks));
        if !accept {
            write.reason = Some(reason.line.clone());
            write.reason_code = Some(reason.code.to_owned());
            write.class = Some(reason.class.to_owned());
        }
        let mut inner = self.lock();
        match inner.log.append(&write) {
            Ok(seq) => {
                let day = log::utc_day(now);
                drop(inner);
                if accept {
                    self.rows.note_access(&job.nonce, Some(day), Some(seq));
                }
                self.rows.settle(&job.nonce, outcome, now);
            }
            Err(error) => {
                eprintln!(
                    "admission-service locked thorough was not written: {}",
                    show_log(error)
                );
                self.rows.settle(&job.nonce, outcome, now);
            }
        }
    }

    pub async fn settle(&self, job: ThoroughJob, report: Result<Report, String>) {
        let judgment = match report {
            Ok(report) => Ok(policy::judge(
                &report,
                self.policy.leg_rate_floor.p,
                self.policy.leg_rate_floor.q,
            )),
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
        let mut write = DecisionWrite::bare(
            log::unix_now(),
            job.tip_height,
            "thorough",
            decision,
        );
        write.fast_seq = Some(job.fast_seq);
        write.submission_sha256 = job.submission;
        write.public_key = job.public_key;
        write.object_sha256 = job.object_sha;
        write.object = job.object;
        write.reason = line;
        write.summary = Some(summary);
        if decision == "refuse" {
            let code = reasons
                .first()
                .map(|note| reasons::code_of_note(note))
                .unwrap_or(reasons::CODE_VERIFIER_ERROR);
            write.reason_code = Some(code.to_owned());
            write.class = Some(reasons::class_of(code).to_owned());
        }
        write.reasons = reasons;
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

    /// Whether a thorough accept with `policy_id` already covers this pair.
    /// Tests and the sign-in path share this lookup.
    pub fn prior_accept_ref(&self, public_key: &[u8; 32], object_sha: &[u8; 32]) -> Option<AccessRef> {
        let inner = self.lock();
        prior_accept(&inner.log, public_key, object_sha).map(|line| {
            let (day, seq) = line_ref(&line.value);
            AccessRef {
                day,
                seq,
                policy_id: line
                    .value
                    .get("policy_id")
                    .and_then(|item| item.as_str())
                    .unwrap_or("")
                    .to_owned(),
            }
        })
    }

    /// One decision object. `None` when `seq` is missing or its time is another day.
    pub fn line_bytes(&self, day: &str, seq: u64) -> Option<Vec<u8>> {
        self.lock()
            .log
            .line_on_day(day, seq)
            .map(|line| line.raw.clone())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("admission log")
    }
}

fn checks_json(checks: &[policy::PolicyCheck]) -> Value {
    Value::Array(
        checks
            .iter()
            .map(|check| {
                json!({
                    "field": check.field,
                    "min": check.min,
                    "met": check.met,
                })
            })
            .collect(),
    )
}

fn unknown_measures() -> Measures {
    Measures {
        legs: None,
        span_s: None,
        leg_rate: None,
        waypoints_fetched: None,
        waypoints_total: policy::WAYPOINTS_TOTAL,
        paths_included: None,
        legs_recomputed_ok: None,
    }
}

/// A thorough accept of this pair that carries a non-empty `policy_id`.
fn prior_accept<'a>(
    log: &'a DecisionLog,
    public_key: &[u8; 32],
    object_sha: &[u8; 32],
) -> Option<&'a log::StoredLine> {
    let key = hex32(public_key);
    let object = hex32(object_sha);
    log.lines().iter().find(|line| {
        line.value.get("phase").and_then(|item| item.as_str()) == Some("thorough")
            && line.value.get("decision").and_then(|item| item.as_str()) == Some("accept")
            && line.value.get("public_key").and_then(|item| item.as_str()) == Some(key.as_str())
            && line.value.get("object_sha256").and_then(|item| item.as_str()) == Some(object.as_str())
            && line
                .value
                .get("policy_id")
                .and_then(|item| item.as_str())
                .is_some_and(|id| !id.is_empty())
    })
}

/// A return for this pair whose time is inside the last 300 s, inclusive.
fn recent_return<'a>(
    log: &'a DecisionLog,
    public_key: &[u8; 32],
    object_sha: &[u8; 32],
    now: u64,
) -> Option<&'a log::StoredLine> {
    let key = hex32(public_key);
    let object = hex32(object_sha);
    log.lines().iter().rev().find(|line| {
        if line.value.get("decision").and_then(|item| item.as_str()) != Some("return") {
            return false;
        }
        if line.value.get("public_key").and_then(|item| item.as_str()) != Some(key.as_str()) {
            return false;
        }
        if line.value.get("object_sha256").and_then(|item| item.as_str()) != Some(object.as_str()) {
            return false;
        }
        let Some(time) = line.value.get("time_unix_s").and_then(|item| item.as_u64()) else {
            return false;
        };
        now.saturating_sub(policy::RETURN_WINDOW_S) <= time && time <= now
    })
}

fn line_ref(value: &Value) -> (Option<String>, Option<u64>) {
    let seq = value.get("seq").and_then(|item| item.as_u64());
    let day = value
        .get("time_unix_s")
        .and_then(|item| item.as_u64())
        .map(log::utc_day);
    (day, seq)
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

fn replay_response(replay: Replay) -> LockedSignIn {
    match replay {
        Replay::Same { client, .. } => locked_client(client, None),
        Replay::Other => locked_client(ClientClass::Malformed, None),
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
