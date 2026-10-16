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
use crate::fast::{self, FastDecision};
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
    index_url: String,
    index_timeout: std::time::Duration,
}

struct Inner {
    log: DecisionLog,
    /// Unix second the locked wire uses. `None` reads the process clock.
    now: Option<u64>,
    /// Public keys this gate ignores. A fake leg writes one. The value is
    /// the seq of the revoke that banned it.
    banned: std::collections::HashMap<[u8; 32], u64>,
    /// Open fast or thorough work, counted for the pending caps.
    pending_global: u64,
    pending_by_source: std::collections::HashMap<String, u64>,
    /// How many times a banned key was ignored. Not a decision line.
    ban_ignored: u64,
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
    pub origin: String,
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
    pub fn open(
        dir: &Path,
        policy: AdmissionPolicy,
        index_url: &str,
        index_timeout: std::time::Duration,
    ) -> Result<Self, String> {
        policy.bind(dir)?;
        let mut log = DecisionLog::open(dir).map_err(show_log)?;
        log.set_policy_hint(policy::public_document(&policy))
            .map_err(show_log)?;
        let banned = load_bans(log.dir());
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                log,
                now: None,
                banned,
                pending_global: 0,
                pending_by_source: std::collections::HashMap::new(),
                ban_ignored: 0,
            })),
            rows: Arc::new(SignInRows::new()),
            policy,
            index_url: index_url.to_owned(),
            index_timeout,
        })
    }

    /// A banned public key is ignored. No verify, no decision line.
    /// The counter is the only record of the ignore.
    pub fn banned(&self, public_key: &[u8; 32]) -> bool {
        let mut inner = self.lock();
        if inner.banned.contains_key(public_key) {
            inner.ban_ignored = inner.ban_ignored.saturating_add(1);
            true
        } else {
            false
        }
    }

    pub fn last_seq(&self) -> u64 {
        self.lock()
            .log
            .lines()
            .last()
            .and_then(|line| line.value.get("seq").and_then(|item| item.as_u64()))
            .unwrap_or(0)
    }

    /// Thorough finished. A match is `accept`. A mismatch is `revoke` because
    /// the fast phase already issued `provisional`, and the public key is banned.
    pub fn settle_thorough(&self, job: &LockedThorough, out: crate::thorough::ThoroughOut) {
        self.rows.note_deep(
            &job.nonce,
            crate::signin_row::Deep {
                runs_done: out.runs_done,
                legs_matched: out.legs_matched,
                legs_total: out.legs_total,
                salt: out.salt.clone(),
                indices: out.indices.clone(),
                commitment: out.commitment.clone(),
            },
        );
        let now = self.now_unix_s();
        let mut write = if out.matched {
            DecisionWrite::bare(now, None, "thorough", "accept")
        } else {
            DecisionWrite::bare(now, None, "thorough", "revoke")
        };
        write.fast_seq = Some(self.last_seq());
        write.submission_sha256 = job.submission;
        write.public_key = job.public_key;
        write.object_sha256 = job.object_sha;
        write.object = job.object.clone();
        write.reasons = vec![if out.matched {
            wire::OUTCOME_ACCEPTED.to_owned()
        } else {
            "revoke".to_owned()
        }];
        write.summary = Some("{}".to_owned());
        write.policy_id = Some(self.policy.policy_id.clone());
        write.deep = Some(serde_json::json!({
            "runs_done": out.runs_done,
            "legs_matched": out.legs_matched,
            "legs_total": out.legs_total,
            "salt": out.salt,
            "indices": out.indices,
            "commitment": out.commitment,
        }));
        if !out.matched {
            let note = if out.digest_mismatch {
                "checkpoint mismatch"
            } else {
                "a recomputed leg result differs"
            };
            write.reason = Some(note.to_owned());
            write.reason_code = Some(reasons::code_of_note(note).to_owned());
            write.class = Some(reasons::class_of(reasons::code_of_note(note)).to_owned());
        }
        let seq = {
            let mut inner = self.lock();
            inner.log.append(&write).ok()
        };
        if out.matched {
            self.rows.settle(&job.nonce, wire::OUTCOME_ACCEPTED, now);
        } else {
            // The website token stays `rejected`. `revoked` is not a §15.1 token.
            self.rows.settle(&job.nonce, wire::OUTCOME_REJECTED, now);
            if self.policy.ban_key_on_fake_leg {
                self.ban_key(&job.public_key, seq.unwrap_or(0));
            }
        }
    }

    pub fn note_deep(&self, nonce: &[u8; 32], out: crate::thorough::ThoroughOut) {
        self.rows.note_deep(
            nonce,
            crate::signin_row::Deep {
                runs_done: out.runs_done,
                legs_matched: out.legs_matched,
                legs_total: out.legs_total,
                salt: out.salt,
                indices: out.indices,
                commitment: out.commitment,
            },
        );
    }

    pub fn ban_ignored(&self) -> u64 {
        self.lock().ban_ignored
    }

    /// Pending caps. Over a cap the nonce is not consumed and nothing is logged.
    pub fn over_pending_cap(&self, source: &str) -> bool {
        let inner = self.lock();
        if inner.pending_global >= self.policy.pending_global {
            return true;
        }
        inner.pending_by_source.get(source).copied().unwrap_or(0) >= self.policy.pending_per_source
    }

    pub fn begin_pending(&self, source: &str) {
        let mut inner = self.lock();
        inner.pending_global = inner.pending_global.saturating_add(1);
        *inner
            .pending_by_source
            .entry(source.to_owned())
            .or_insert(0) += 1;
    }

    pub fn end_pending(&self, source: &str) {
        let mut inner = self.lock();
        inner.pending_global = inner.pending_global.saturating_sub(1);
        if let Some(count) = inner.pending_by_source.get_mut(source) {
            *count = count.saturating_sub(1);
        }
    }

    /// Write a ban for the public key only. The seed is not stored.
    pub fn ban_key(&self, public_key: &[u8; 32], seq: u64) {
        let mut inner = self.lock();
        inner.banned.insert(*public_key, seq);
        let dir = inner.log.dir().to_path_buf();
        drop(inner);
        if let Err(err) = store_ban(&dir, public_key, seq) {
            eprintln!("admission-service ban was not stored: {err}");
        }
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
            let tip_height = line
                .value
                .get("bitcoin_tip_height")
                .and_then(|item| item.as_u64());
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

        // Pending caps, before the nonce is consumed. Over a cap the website
        // stays `pending` (the locked token; a `held` token is not in §15.1)
        // and this function returns no thorough job.
        if self.over_pending_cap(&body.origin) {
            // No row. The nonce is not consumed. The result read of an
            // unknown nonce is `pending`, which is the locked token.
            return locked_client(ClientClass::DeliveryOk, None);
        }
        self.begin_pending(&body.origin);

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
                        origin: body.origin.clone(),
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
            self.rows.note_bookends(
                &job.nonce,
                measures.legs.map(|n| n as i64),
                measures.span_s.map(|n| n as i64),
            );
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
        let mut write = DecisionWrite::bare(log::unix_now(), job.tip_height, "thorough", decision);
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

    /// Fast phase for a consumed v2.0 object. A settled refuse writes the
    /// thorough-less line and does not start a walk. `provisional` is the
    /// grant. `held` writes no line.
    pub fn settle_fast(&self, job: &LockedThorough) -> FastPhase {
        if let Some(phase) = self.sticky_phase(job) {
            return phase;
        }
        self.finish_fast(
            job,
            fast::decide(&job.object, &self.policy, &self.index_client()),
        )
    }

    /// [`settle_fast`] at `conformance-small` sizes.
    ///
    /// The profile is not offered to production. [`settle_fast`] always uses
    /// section 6. A test builds the smallest conformance object and runs the
    /// same index client through this entry.
    #[cfg(test)]
    pub fn settle_fast_small(&self, job: &LockedThorough) -> FastPhase {
        if let Some(phase) = self.sticky_phase(job) {
            return phase;
        }
        self.finish_fast(
            job,
            fast::decide_small(&job.object, &self.policy, &self.index_client()),
        )
    }

    /// A stored refuse or revoke for this pair. No index read.
    fn sticky_phase(&self, job: &LockedThorough) -> Option<FastPhase> {
        let stored = self.sticky(&job.public_key, &job.object_sha)?;
        self.rows.settle(&job.nonce, &stored, self.now_unix_s());
        Some(FastPhase::Sticky)
    }

    fn index_client(&self) -> verifier::IndexClient {
        verifier::IndexClient::open(&self.index_url, self.index_timeout)
    }

    fn finish_fast(&self, job: &LockedThorough, decision: FastDecision) -> FastPhase {
        match decision {
            FastDecision::Refuse {
                website,
                reasons,
                line,
                record,
            } => {
                self.write_fast_refuse(job, &reasons, &line, &record);
                self.rows.settle(&job.nonce, website, self.now_unix_s());
                FastPhase::Done
            }
            FastDecision::Held { .. } => {
                // Website token stays pending. The locked table has no `held`.
                FastPhase::Held
            }
            FastDecision::Provisional { record } => {
                self.write_provisional(job, &record);
                FastPhase::Provisional
            }
        }
    }

    fn sticky(&self, public_key: &[u8; 32], object_sha: &[u8; 32]) -> Option<String> {
        let inner = self.lock();
        let key = hex32(public_key);
        let object = hex32(object_sha);
        let mut found = None;
        for line in inner.log.lines() {
            let same = line.value.get("public_key").and_then(|item| item.as_str())
                == Some(key.as_str())
                && line
                    .value
                    .get("object_sha256")
                    .and_then(|item| item.as_str())
                    == Some(object.as_str())
                && line.value.get("policy_id").and_then(|item| item.as_str())
                    == Some(self.policy.policy_id.as_str());
            if !same {
                continue;
            }
            let decision = line.value.get("decision").and_then(|item| item.as_str());
            if decision == Some("refuse") || decision == Some("revoke") {
                found = line
                    .value
                    .get("reasons")
                    .and_then(|item| item.as_array())
                    .and_then(|items| items.first())
                    .and_then(|item| item.as_str())
                    .map(str::to_owned);
            }
        }
        found
    }

    fn write_fast_refuse(
        &self,
        job: &LockedThorough,
        reasons: &[String],
        line: &str,
        record: &str,
    ) {
        let now = self.now_unix_s();
        let mut write = DecisionWrite::bare(now, None, "fast", "refuse");
        write.submission_sha256 = job.submission;
        write.public_key = job.public_key;
        write.object_sha256 = job.object_sha;
        write.object = job.object.clone();
        write.reasons = reasons.to_vec();
        write.reason = Some(line.to_owned());
        write.summary = Some(record.to_owned());
        write.policy_id = Some(self.policy.policy_id.clone());
        let code = reasons
            .first()
            .map(|note| reasons::code_of_note(note))
            .unwrap_or(reasons::CODE_SHAPE_MISMATCH);
        write.reason_code = Some(code.to_owned());
        write.class = Some(reasons::class_of(code).to_owned());
        let mut inner = self.lock();
        if let Err(error) = inner.log.append(&write) {
            eprintln!(
                "admission-service fast refuse was not written: {}",
                show_log(error)
            );
        }
    }

    fn write_provisional(&self, job: &LockedThorough, record: &str) {
        let now = self.now_unix_s();
        let mut write = DecisionWrite::bare(now, None, "fast", "provisional");
        write.submission_sha256 = job.submission;
        write.public_key = job.public_key;
        write.object_sha256 = job.object_sha;
        write.object = job.object.clone();
        write.reasons = vec!["provisional".to_owned()];
        write.summary = Some(if record.is_empty() {
            "{}".to_owned()
        } else {
            record.to_owned()
        });
        write.policy_id = Some(self.policy.policy_id.clone());
        let mut inner = self.lock();
        if let Err(error) = inner.log.append(&write) {
            eprintln!(
                "admission-service provisional was not written: {}",
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
    pub fn prior_accept_ref(
        &self,
        public_key: &[u8; 32],
        object_sha: &[u8; 32],
    ) -> Option<AccessRef> {
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
            && line
                .value
                .get("object_sha256")
                .and_then(|item| item.as_str())
                == Some(object.as_str())
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
        if line
            .value
            .get("object_sha256")
            .and_then(|item| item.as_str())
            != Some(object.as_str())
        {
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

/// What the fast phase did with a consumed nonce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FastPhase {
    /// A grant was issued. Thorough runs.
    Provisional,
    /// `not read`. No line. The website stays `pending`.
    Held,
    /// A settled refuse, or a sticky stored outcome. Thorough does not run.
    Done,
    Sticky,
}

fn load_bans(dir: &Path) -> std::collections::HashMap<[u8; 32], u64> {
    let mut out = std::collections::HashMap::new();
    let path = dir.join("banned_keys.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return out;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return out;
    };
    let Some(map) = value.as_object() else {
        return out;
    };
    for (key, seq) in map {
        if let (Some(bytes), Some(seq)) = (log::parse_hex32(key), seq.as_u64()) {
            out.insert(bytes, seq);
        }
    }
    out
}

fn store_ban(dir: &Path, public_key: &[u8; 32], seq: u64) -> Result<(), String> {
    let path = dir.join("banned_keys.json");
    let mut map = if path.exists() {
        let raw = std::fs::read_to_string(&path).map_err(|err| err.to_string())?;
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&raw).unwrap_or_default()
    } else {
        serde_json::Map::new()
    };
    map.insert(hex32(public_key), serde_json::json!(seq));
    let bytes = serde_json::to_vec(&map).map_err(|err| err.to_string())?;
    std::fs::write(&path, bytes).map_err(|err| err.to_string())
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
    use super::{Door, FastPhase, LockedThorough};
    use crate::canon;
    use crate::config::Config;
    use crate::thorough;
    use crate::wire;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

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

    /// (a) Every index read matches. The fast phase grants `provisional`,
    /// and that line is the thorough job a restart would start.
    #[test]
    fn honest_index_reads_are_provisional_and_thorough_is_enqueued() {
        let built = small_honest();
        let (url, hits) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_at(&url);
        let job = job_of(&built.bytes, 1);
        let phase = door.settle_fast_small(&job);
        assert_eq!(phase, FastPhase::Provisional, "{}", chain_text(&dir));
        let pending = door.unsettled();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].object_sha, job.object_sha);
        assert_eq!(pending[0].public_key, job.public_key);
        let lines = chain(&dir);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["decision"], "provisional");
        assert_eq!(lines[0]["phase"], "fast");
        assert_eq!(hits.fresh.load(Ordering::Relaxed), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (b) A refused connection is `not read`, so the state is held.
    #[test]
    fn index_down_is_held() {
        let built = small_honest();
        let (door, dir) = open_at("http://127.0.0.1:9");
        let phase = door.settle_fast_small(&job_of(&built.bytes, 1));
        assert_eq!(phase, FastPhase::Held);
        assert!(door.unsettled().is_empty());
        assert_eq!(door.last_seq(), 0);
        assert!(!dir.join("chain.jsonl").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (c) Both plant reads disagree. That confirmed mismatch refuses.
    #[test]
    fn a_confirmed_plant_mismatch_refuses() {
        let built = small_honest();
        let (url, hits) = serve(&built.docs, Mirror::PlantConfirmed(built.plant_height));
        let (door, dir) = open_at(&url);
        let phase = door.settle_fast_small(&job_of(&built.bytes, 1));
        assert_eq!(phase, FastPhase::Done, "{}", chain_text(&dir));
        assert!(door.unsettled().is_empty());
        let lines = chain(&dir);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["decision"], "refuse");
        assert_eq!(lines[0]["phase"], "fast");
        let reasons = lines[0]["reasons"].to_string();
        assert!(
            reasons.contains("present path does not match the plant"),
            "{reasons}"
        );
        assert!(hits.fresh.load(Ordering::Relaxed) >= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (d) The mirror mismatches and the fresh read does not. Held, no line.
    #[test]
    fn an_unconfirmed_plant_mismatch_is_held() {
        let built = small_honest();
        let (url, hits) = serve(&built.docs, Mirror::PlantUnconfirmed(built.plant_height));
        let (door, dir) = open_at(&url);
        let phase = door.settle_fast_small(&job_of(&built.bytes, 1));
        assert_eq!(phase, FastPhase::Held, "{}", chain_text(&dir));
        assert!(door.unsettled().is_empty());
        assert_eq!(door.last_seq(), 0);
        assert!(hits.fresh.load(Ordering::Relaxed) >= 1);
        assert!(hits.normal.load(Ordering::Relaxed) >= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A stored header that still disagrees on the fresh read is a refuse.
    #[test]
    fn a_confirmed_header_mismatch_refuses() {
        let built = small_honest();
        let (url, hits) = serve(&built.docs, Mirror::HeaderConfirmed(built.header_height));
        let (door, dir) = open_at(&url);
        let phase = door.settle_fast_small(&job_of(&built.bytes, 1));
        assert_eq!(phase, FastPhase::Done, "{}", chain_text(&dir));
        let lines = chain(&dir);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["decision"], "refuse");
        let reasons = lines[0]["reasons"].to_string();
        assert!(reasons.contains("checkpoint mismatch"), "{reasons}");
        assert!(hits.fresh.load(Ordering::Relaxed) >= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A header mismatch the fresh read does not repeat is held.
    #[test]
    fn an_unconfirmed_header_mismatch_is_held() {
        let built = small_honest();
        let (url, hits) = serve(&built.docs, Mirror::HeaderUnconfirmed(built.header_height));
        let (door, dir) = open_at(&url);
        let phase = door.settle_fast_small(&job_of(&built.bytes, 1));
        assert_eq!(phase, FastPhase::Held, "{}", chain_text(&dir));
        assert_eq!(door.last_seq(), 0);
        assert!(hits.fresh.load(Ordering::Relaxed) >= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC3. A small honest object is accepted. A faked leg revokes and bans
    /// the public key. A later submit from that key is ignored: no fast phase
    /// and no decision line. `conformance-small` is not a production profile;
    /// the walk uses the test-only entry.
    #[test]
    fn ac3_fake_leg_revokes_bans_the_key_and_ignores_a_resubmit() {
        let built = small_honest();
        let (url, _) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_at(&url);
        let honest = job_of(&built.bytes, 1);
        assert_eq!(
            door.settle_fast_small(&honest),
            FastPhase::Provisional,
            "{}",
            chain_text(&dir)
        );
        let out = thorough::run_for_test(
            &dir,
            door.last_seq(),
            &built.hike,
            &honest.object_sha,
            door.policy(),
        )
        .unwrap();
        assert!(
            out.matched,
            "runs {} matched {}",
            out.runs_done, out.legs_matched
        );
        door.settle_thorough(&honest, out);
        let lines = chain(&dir);
        assert_eq!(lines.last().unwrap()["decision"], "accept");
        assert!(!dir.join("banned_keys.json").exists());

        let mut fake_hike = built.hike.clone();
        fake_hike.stages[0].leg_results[0][0] ^= 0xff;
        let fake_bytes = hike::encode_hike_sized(&fake_hike, hike::Sizes::conformance_small());
        let fake = job_of(&fake_bytes, 2);
        assert_eq!(door.settle_fast_small(&fake), FastPhase::Provisional);
        let bad = thorough::run_for_test(
            &dir,
            door.last_seq(),
            &fake_hike,
            &fake.object_sha,
            door.policy(),
        )
        .unwrap();
        assert!(!bad.matched);
        door.settle_thorough(&fake, bad);
        let lines = chain(&dir);
        assert_eq!(lines.last().unwrap()["decision"], "revoke");
        assert_eq!(
            lines.last().unwrap()["public_key"],
            canon::hex32(&honest.public_key)
        );
        let ban = std::fs::read_to_string(dir.join("banned_keys.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&ban).unwrap();
        let keys: Vec<_> = parsed.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, vec![canon::hex32(&honest.public_key)]);
        assert!(!ban.contains(&canon::hex32(&built.hike.seed_beacon)));

        // The sign-in task calls `banned` before `settle_fast`. That call is
        // the ignore: the counter moves, and no decision line is written.
        let seq = door.last_seq();
        assert!(door.banned(&honest.public_key));
        assert_eq!(door.last_seq(), seq);
        assert_eq!(door.ban_ignored(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    struct Hits {
        normal: AtomicU64,
        fresh: AtomicU64,
    }

    #[derive(Clone, Copy)]
    enum Mirror {
        Honest,
        PlantConfirmed(u64),
        PlantUnconfirmed(u64),
        HeaderConfirmed(u64),
        HeaderUnconfirmed(u64),
    }

    struct SmallHonest {
        hike: hike::Hike,
        bytes: Vec<u8>,
        docs: HashMap<u64, String>,
        plant_height: u64,
        header_height: u64,
    }

    fn open_at(index_url: &str) -> (Door, PathBuf) {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("pcoc-gate-fast-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = Config::for_tests(dir.clone(), index_url);
        let door = Door::open(
            &cfg.log_dir,
            cfg.policy(),
            &cfg.index_url,
            cfg.index_timeout,
        )
        .unwrap();
        (door, dir)
    }

    fn job_of(bytes: &[u8], nonce_byte: u8) -> LockedThorough {
        let mut nonce = [0u8; 32];
        nonce[0] = nonce_byte;
        LockedThorough {
            nonce,
            origin: wire::ORIGIN.to_owned(),
            object: bytes.to_vec(),
            submission: [nonce_byte; 32],
            public_key: [0x42; 32],
            object_sha: canon::sha256(bytes),
            prior_accept: None,
        }
    }

    fn chain(dir: &Path) -> Vec<serde_json::Value> {
        // A section 14 record keeps its newline inside the decision object,
        // so a split on `\n` is not one decision.
        let Ok(log) = crate::log::DecisionLog::open(dir) else {
            return Vec::new();
        };
        log.lines().iter().map(|line| line.value.clone()).collect()
    }

    fn chain_text(dir: &Path) -> String {
        std::fs::read_to_string(dir.join("chain.jsonl")).unwrap_or_default()
    }

    fn serve(docs: &HashMap<u64, String>, mode: Mirror) -> (String, Arc<Hits>) {
        let docs = Arc::new(docs.clone());
        let hits = Arc::new(Hits {
            normal: AtomicU64::new(0),
            fresh: AtomicU64::new(0),
        });
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let hits_task = Arc::clone(&hits);
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(mut stream) = conn else {
                    continue;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = answer(&mut stream, &docs, mode, &hits_task);
            }
        });
        (format!("http://127.0.0.1:{port}"), hits)
    }

    fn answer(
        stream: &mut std::net::TcpStream,
        docs: &HashMap<u64, String>,
        mode: Mirror,
        hits: &Hits,
    ) -> std::io::Result<()> {
        let mut got = Vec::new();
        let mut buf = [0u8; 2048];
        loop {
            let n = stream.read(&mut buf)?;
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
            if got.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
            if got.len() > 65536 {
                break;
            }
        }
        let text = String::from_utf8_lossy(&got);
        let fresh = text
            .to_ascii_lowercase()
            .contains("cache-control: no-cache");
        if fresh {
            hits.fresh.fetch_add(1, Ordering::Relaxed);
        } else {
            hits.normal.fetch_add(1, Ordering::Relaxed);
        }
        let path = text.lines().next().unwrap_or("");
        let height = path
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .trim_start_matches("/index/")
            .parse::<u64>()
            .unwrap_or(0);
        let body = docs
            .get(&height)
            .map(|honest| mirror_body(honest, height, fresh, mode));
        let raw = match body {
            Some(body) => format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
            None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_owned(),
        };
        stream.write_all(raw.as_bytes())?;
        stream.flush()
    }

    fn mirror_body(honest: &str, height: u64, fresh: bool, mode: Mirror) -> String {
        match mode {
            Mirror::Honest => honest.to_owned(),
            Mirror::PlantConfirmed(target) if height == target => bad_root(honest),
            Mirror::PlantUnconfirmed(target) if height == target && !fresh => bad_root(honest),
            Mirror::HeaderConfirmed(target) if height == target => bad_hash(honest),
            Mirror::HeaderUnconfirmed(target) if height == target && !fresh => bad_hash(honest),
            _ => honest.to_owned(),
        }
    }

    fn bad_root(body: &str) -> String {
        replace_hex_field(body, "\"root\":\"", &"00".repeat(32))
    }

    fn bad_hash(body: &str) -> String {
        replace_hex_field(body, "\"hash\":\"", &"aa".repeat(32))
    }

    fn replace_hex_field(body: &str, key: &str, next: &str) -> String {
        let start = body.find(key).expect(key) + key.len();
        let mut out = body.to_owned();
        out.replace_range(start..start + 64, next);
        out
    }

    /// Eighteen stages, one real leg each, six anchored paths (`W_1`…`W_5`
    /// and `W_16`). Section 6 sizes are not filled.
    fn small_honest() -> SmallHonest {
        use sha2::{Digest, Sha256};
        let sizes = hike::Sizes::conformance_small();
        let public_key = [0x42u8; 32];
        let mut beacons = [[0u8; 32]; 18];
        let mut heights = [0u64; 18];
        for g in 0..18 {
            beacons[g][0] = g as u8 + 1;
            beacons[g][1] = 0xA0;
            heights[g] = 1000 + g as u64;
        }
        let seed_beacon = beacons[0];
        let seed = hike::production_seed(&public_key, &seed_beacon);
        let mut seed_value = [0u8; 32];
        seed_value.copy_from_slice(&Sha256::digest(seed));
        let terrain = hike::fill_terrain_sized(hike::fill_pin(&seed), sizes);
        let mut limb = seed_value;
        let mut stages = Vec::with_capacity(18);
        for g in 0..18 {
            let previous = limb;
            let leg = hike::leg_result_sized(&terrain, &previous, sizes);
            let sample = hike::terrain_sample_sized(&terrain, &previous, sizes);
            let digest = hike::checkpoint_sized(&previous, &limb, &sample, leg, sizes);
            let path = anchor_path(g, &limb, &beacons, &heights);
            stages.push(hike::Stage {
                beacon: beacons[g],
                beacon_height: heights[g],
                leg_count: 1,
                waypoint: limb,
                path,
                checkpoints: vec![digest],
                leg_results: vec![leg.to_le_bytes()],
                checkpoint_rows_defined: true,
                leg_result_missing: Vec::new(),
            });
            if g < 17 {
                limb = hike::successor_waypoint_sized(
                    &terrain,
                    &beacons[g + 1],
                    &previous,
                    &[digest],
                    sizes,
                );
            }
        }
        let mut last = [0u8; 32];
        last[0] = 0x63;
        last[1] = 0xA0;
        let last_height = 1018u64;
        let hike = hike::Hike {
            public_key,
            seed_beacon,
            stages,
            last_interrupt: last,
            last_interrupt_height: last_height,
            last_interrupt_absent: false,
            last_interrupt_hash_absent: false,
            last_interrupt_height_absent: false,
        };
        let bytes = hike::encode_hike_sized(&hike, sizes);
        let mut docs = HashMap::new();
        let mut plants: HashMap<u64, Vec<hike::PathLocators>> = HashMap::new();
        for stage in hike.stages.iter().skip(1) {
            if stage.path.is_empty() {
                continue;
            }
            let loc = hike::path_locators(&stage.path).expect("path");
            plants
                .entry(u64::from(loc.block_height))
                .or_default()
                .push(loc);
        }
        for stage in &hike.stages {
            let time = 1_700_000_000 + stage.beacon_height;
            docs.insert(
                stage.beacon_height,
                index_document(
                    &stage.beacon,
                    stage.beacon_height,
                    time,
                    plants
                        .get(&stage.beacon_height)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                ),
            );
        }
        docs.insert(
            last_height,
            index_document(
                &last,
                last_height,
                1_700_000_000 + last_height,
                plants.get(&last_height).map(Vec::as_slice).unwrap_or(&[]),
            ),
        );
        let plant_height = u64::from(
            hike::path_locators(&hike.stages[1].path)
                .expect("W_1 path")
                .block_height,
        );
        SmallHonest {
            plant_height,
            header_height: hike.stages[0].beacon_height,
            hike,
            bytes,
            docs,
        }
    }

    fn anchor_path(
        g: usize,
        waypoint: &[u8; 32],
        beacons: &[[u8; 32]; 18],
        heights: &[u64; 18],
    ) -> Vec<u8> {
        let anchored = matches!(g, 1 | 2 | 3 | 4 | 5 | 16);
        if !anchored {
            return Vec::new();
        }
        if g == 16 {
            let mut last = [0u8; 32];
            last[0] = 0x63;
            last[1] = 0xA0;
            return hike::encode_path_for_block(waypoint, &last, 1018);
        }
        let target = g + 2;
        hike::encode_path_for_block(waypoint, &beacons[target], heights[target] as u32)
    }

    fn index_document(
        natural: &[u8; 32],
        height: u64,
        time: u64,
        plants: &[hike::PathLocators],
    ) -> String {
        let hash = hike::pathchain::display_hex(natural);
        let mut body = format!(
            "{{\"network\":\"synthetic\",\"header\":{{\"hash\":\"{hash}\",\"height\":{height},\"time\":{time}}}"
        );
        if let Some(loc) = plants.first() {
            body.push_str(",\"plant\":");
            body.push_str(&plant_json(loc));
        }
        body.push('}');
        body
    }

    fn plant_json(loc: &hike::PathLocators) -> String {
        format!(
            "{{\"root\":\"{}\",\"txid\":\"{}\",\"vout\":{},\"block_hash\":\"{}\",\"block_height\":{}}}",
            hike::encode_hex32(&loc.root),
            hike::pathchain::display_hex(&loc.txid),
            loc.vout,
            hike::pathchain::display_hex(&loc.block_hash),
            loc.block_height
        )
    }
}
