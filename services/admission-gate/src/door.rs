//! Sign-in door for the locked §15.1 wire.
//!
//! The client sees delivery, `bad_signature`, or `malformed`. The POST is
//! never a thorough `accept`. Policy settles the website outcome after that
//! response. This process does not mint a session and does not call a website.
//!
//! A provisional line already in the log, with no thorough line, is settled
//! on restart through the thorough check with its stored draw. With no
//! stored draw it is held: nothing is drawn again and nothing is accepted. A new sign-in does not write that provisional line. There is
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

#[derive(Clone)]
pub struct Door {
    inner: Arc<Mutex<Inner>>,
    rows: Arc<SignInRows>,
    policy: AdmissionPolicy,
    index_url: String,
    index_timeout: std::time::Duration,
    /// Site origins this gate serves (config `served_origins`). Exact bytes.
    served_origins: Arc<Vec<String>>,
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
        served_origins: Vec<String>,
    ) -> Result<Self, String> {
        Self::open_as(
            dir,
            policy,
            index_url,
            index_timeout,
            served_origins,
            policy::gate_build(),
        )
    }

    /// [`Door::open`] for a named build. A policy that expects mainnet index
    /// evidence refuses to start in a testing build ([`policy::check_build`]).
    pub fn open_as(
        dir: &Path,
        policy: AdmissionPolicy,
        index_url: &str,
        index_timeout: std::time::Duration,
        served_origins: Vec<String>,
        build: policy::GateBuild,
    ) -> Result<Self, String> {
        policy::check_build(&policy, build)?;
        if served_origins.is_empty() {
            return Err("served_origins is empty; the gate serves no site origin".to_owned());
        }
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
            served_origins: Arc::new(served_origins),
        })
    }

    /// The site origins this gate serves for sign-in.
    pub fn served_origins(&self) -> &[String] {
        &self.served_origins
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

    /// Whether a public key is banned, without counting an ignore. The
    /// settle paths use it so a key banned while its job was in flight is
    /// never granted.
    pub fn is_banned(&self, public_key: &[u8; 32]) -> bool {
        self.lock().banned.contains_key(public_key)
    }

    pub fn last_seq(&self) -> u64 {
        self.lock()
            .log
            .lines()
            .last()
            .and_then(|line| line.value.get("seq").and_then(|item| item.as_u64()))
            .unwrap_or(0)
    }

    /// Gate 0.5.4 fields on every new line: the observed index networks, and
    /// the policy's `expected_network` when it sets one.
    fn stamp_network(&self, write: &mut DecisionWrite, networks: Value) {
        write.index_network = Some(networks);
        write.expected_network = self.policy.expected_network().map(str::to_owned);
    }

    /// `index_network` of line `seq`. `null` for a line written before 0.5.4.
    fn line_networks(&self, seq: u64) -> Value {
        let inner = self.lock();
        inner
            .log
            .lines()
            .get(seq as usize)
            .and_then(|line| line.value.get("index_network").cloned())
            .unwrap_or(Value::Null)
    }

    /// Whether line `seq` is the fast `provisional` this job wrote.
    fn provisional_is(&self, seq: u64, job: &LockedThorough) -> bool {
        let inner = self.lock();
        let Some(line) = inner.log.lines().get(seq as usize) else {
            return false;
        };
        let text = |key: &str| line.value.get(key).and_then(|item| item.as_str());
        line.value.get("seq").and_then(|item| item.as_u64()) == Some(seq)
            && text("decision") == Some("provisional")
            && text("submission_sha256") == Some(hex32(&job.submission).as_str())
            && text("public_key") == Some(hex32(&job.public_key).as_str())
            && text("object_sha256") == Some(hex32(&job.object_sha).as_str())
    }

    /// Thorough finished. A match is `accept`. A mismatch is `revoke` because
    /// the fast phase already issued `provisional`, and the public key is banned.
    ///
    /// `out.seq` is the job's own provisional seq, carried from the fast
    /// phase. It is never read back from the log tail, so two interleaved
    /// jobs cannot share a ticket. A seq that does not name this job's
    /// provisional writes no line: the provisional stays unsettled (held).
    pub fn settle_thorough(&self, job: &LockedThorough, out: crate::thorough::ThoroughOut) {
        if !self.provisional_is(out.seq, job) {
            eprintln!(
                "admission-service thorough fast_seq={} does not name this job's provisional; held",
                out.seq
            );
            return;
        }
        if out.matched && self.is_banned(&job.public_key) {
            // Banned while this job was in flight: never an accept. No line;
            // the provisional stays unsettled and the website stays pending.
            eprintln!(
                "admission-service thorough fast_seq={}: the key was banned in flight; held",
                out.seq
            );
            return;
        }
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
        let write = self.thorough_write(
            now,
            job.submission,
            job.public_key,
            job.object_sha,
            job.object.clone(),
            &out,
        );
        let seq = {
            let mut inner = self.lock();
            inner.log.append(&write).ok()
        };
        if out.matched {
            // The thorough accept line opens the claim window, like a return.
            if let Some(seq) = seq {
                self.rows
                    .note_access(&job.nonce, Some(log::utc_day(now)), Some(seq));
            }
            self.rows.settle(&job.nonce, wire::OUTCOME_ACCEPTED, now);
        } else {
            // The website token stays `rejected`. `revoked` is not a §15.1 token.
            self.rows.settle(&job.nonce, wire::OUTCOME_REJECTED, now);
            if out.fake_leg && self.policy.ban_key_on_fake_leg {
                self.ban_key(&job.public_key, seq.unwrap_or(0));
            }
        }
    }

    /// The thorough phase finished or failed. A result is settled by
    /// [`Door::settle_thorough`]. An error (a draw file that does not match,
    /// a leg that cannot be recomputed, a worker that died) is not a second
    /// admission path: no line is written, the provisional stays unsettled
    /// and the website token stays `pending` (held). There is no fallback to
    /// the six-leg verifier.
    pub fn finish_thorough(
        &self,
        job: &LockedThorough,
        outcome: Result<crate::thorough::ThoroughOut, String>,
    ) {
        match outcome {
            Ok(out) => self.settle_thorough(job, out),
            Err(error) => {
                eprintln!("admission-service thorough did not finish; held: {error}");
            }
        }
    }

    /// The thorough accept or revoke line for `out`. `fast_seq` is `out.seq`.
    fn thorough_write(
        &self,
        now: u64,
        submission: [u8; 32],
        public_key: [u8; 32],
        object_sha: [u8; 32],
        object: Vec<u8>,
        out: &crate::thorough::ThoroughOut,
    ) -> DecisionWrite {
        let mut write = if out.matched {
            DecisionWrite::bare(now, None, "thorough", "accept")
        } else {
            DecisionWrite::bare(now, None, "thorough", "revoke")
        };
        write.fast_seq = Some(out.seq);
        // The thorough check reads no index. It rests on the provisional's reads.
        let networks = self.line_networks(out.seq);
        self.stamp_network(&mut write, networks);
        write.submission_sha256 = submission;
        write.public_key = public_key;
        write.object_sha256 = object_sha;
        write.object = object;
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
        write
    }

    /// Restart resume of an unsettled provisional, after
    /// [`crate::thorough::resume`]. The stored draw was reused, so this is
    /// the same thorough check the live path runs, including the ban on a
    /// fake leg. An error (no stored draw, a draw that does not match) is
    /// held: no line, and never an accept.
    pub fn settle_resumed(
        &self,
        job: ThoroughJob,
        outcome: Result<crate::thorough::ThoroughOut, String>,
    ) {
        let out = match outcome {
            Ok(out) if out.seq == job.fast_seq => out,
            Ok(out) => {
                eprintln!(
                    "admission-service resume fast_seq={} got a result for seq {}; held",
                    job.fast_seq, out.seq
                );
                return;
            }
            Err(error) => {
                eprintln!(
                    "admission-service resume fast_seq={} held: {error}",
                    job.fast_seq
                );
                return;
            }
        };
        if out.matched && self.is_banned(&job.public_key) {
            eprintln!(
                "admission-service resume fast_seq={}: the key is banned; held",
                job.fast_seq
            );
            return;
        }
        let now = self.now_unix_s();
        let write = self.thorough_write(
            now,
            job.submission,
            job.public_key,
            job.object_sha,
            job.object,
            &out,
        );
        let seq = {
            let mut inner = self.lock();
            if already_settled(&inner.log, job.fast_seq) {
                return;
            }
            match inner.log.append(&write) {
                Ok(seq) => seq,
                Err(error) => {
                    eprintln!(
                        "admission-service resumed thorough fast_seq={} was not written: {}",
                        job.fast_seq,
                        show_log(error)
                    );
                    return;
                }
            }
        };
        if !out.matched && out.fake_leg && self.policy.ban_key_on_fake_leg {
            self.ban_key(&job.public_key, seq);
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

    /// The ban check, the pending caps and taking the slot, under one lock.
    /// A ban written by another task is either seen here or lands after the
    /// slot is taken; a job for the same key that holds a slot always bans
    /// before it releases that slot, so a banned key cannot slip between
    /// the check and the take.
    pub fn try_begin_pending(&self, public_key: &[u8; 32], source: &str) -> PendingAdmit {
        let mut inner = self.lock();
        if inner.banned.contains_key(public_key) {
            inner.ban_ignored = inner.ban_ignored.saturating_add(1);
            return PendingAdmit::Banned;
        }
        if inner.pending_global >= self.policy.pending_global
            || inner.pending_by_source.get(source).copied().unwrap_or(0)
                >= self.policy.pending_per_source
        {
            return PendingAdmit::OverCap;
        }
        inner.pending_global = inner.pending_global.saturating_add(1);
        *inner
            .pending_by_source
            .entry(source.to_owned())
            .or_insert(0) += 1;
        PendingAdmit::Begun
    }

    /// Number of pending slots held, globally and for `source`.
    pub fn pending_counts(&self, source: &str) -> (u64, u64) {
        let inner = self.lock();
        (
            inner.pending_global,
            inner.pending_by_source.get(source).copied().unwrap_or(0),
        )
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

    /// A later submission from a banned key. Same reason as the fake leg.
    /// Does not draw and does not walk. The ban file is not keyed by policy id,
    /// so a policy-version bump still refuses this key.
    pub fn record_banned_refusal(&self, public_key: &[u8; 32]) {
        let now = self.now_unix_s();
        let mut write = DecisionWrite::bare(now, None, "fast", "refuse");
        self.stamp_network(&mut write, Value::Null);
        write.public_key = *public_key;
        write.reasons = vec![wire::OUTCOME_REJECTED.to_owned()];
        write.reason = Some("a recomputed leg result differs".to_owned());
        let code = reasons::code_of_note("a recomputed leg result differs");
        write.reason_code = Some(code.to_owned());
        write.class = Some(reasons::class_of(code).to_owned());
        write.policy_id = Some(self.policy.policy_id.clone());
        let mut inner = self.lock();
        if inner.log.append(&write).is_err() {
            eprintln!("admission-service banned refusal was not written");
        }
    }

    /// Write a ban for the public key only. The seed is not stored.
    /// `banned_keys.json` is not keyed by `policy_id`. A restart and a new
    /// policy id both load the same file.
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
            origin: self.served_origins[0].clone(),
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
        if !wire::origin_served(&self.served_origins, &body.origin)
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

        // A banned key is ignored after the signature: the nonce is consumed
        // (the website token stays `pending`), no fast phase, no line, and no
        // pending slot is taken. Checked before the caps so a banned
        // resubmit cannot hold a slot.
        //
        // Pending caps, before the nonce is consumed. Over a cap the website
        // stays `pending` (the locked token; a `held` token is not in §15.1)
        // and this function returns no thorough job. The per-source cap is
        // keyed on the submitter's public key, not on the signed site origin,
        // so one submitter cannot hold every sign-in for a site. The ban
        // check and the slot are taken under one lock (try_begin_pending).
        let pending_key = pending_key(&public_key);
        match self.try_begin_pending(&public_key, &pending_key) {
            PendingAdmit::Banned => {
                // The same refusal as the fake leg that wrote the ban. No new
                // draw and no thorough walk. A cheap-check miss does not reach
                // this branch: it never called `ban_key`.
                self.record_banned_refusal(&public_key);
                return match self.rows.consume(
                    &body,
                    ClientClass::DeliveryOk,
                    wire::OUTCOME_REJECTED,
                    now,
                ) {
                    Ok(()) => locked_client(ClientClass::DeliveryOk, None),
                    Err(replay) => replay_response(replay),
                };
            }
            PendingAdmit::OverCap => {
                // No row. The nonce is not consumed. The result read of an
                // unknown nonce is `pending`, which is the locked token.
                return locked_client(ClientClass::DeliveryOk, None);
            }
            PendingAdmit::Begun => {}
        }

        // 9. v2.0. Consume when this step starts. Policy settles later.
        let submission =
            wire::submission_sha256(&body.object_sha, &public_key, &message, &body.signature);
        match self
            .rows
            .consume(&body, ClientClass::DeliveryOk, wire::OUTCOME_PENDING, now)
        {
            Err(replay) => {
                // No thorough job follows, so the slot is released here.
                self.end_pending(&pending_key);
                replay_response(replay)
            }
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
            // A return reads no index; it rests on the accept it points at.
            self.stamp_network(&mut write, Value::Null);
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
        self.stamp_network(&mut write, Value::Null);
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

    /// Fast phase for a consumed v2.0 object. A settled refuse writes the
    /// thorough-less line and does not start a walk. `provisional` is the
    /// grant. `held` writes no line.
    pub fn settle_fast(&self, job: &LockedThorough) -> FastPhase {
        self.settle_fast_seq(job).0
    }

    /// [`settle_fast`], plus the seq of the provisional line this job wrote.
    /// The thorough phase keys its draw and its line on that seq. A
    /// provisional that was not written returns `Held` and no seq.
    pub fn settle_fast_seq(&self, job: &LockedThorough) -> (FastPhase, Option<u64>) {
        if let Some(phase) = self.sticky_phase(job) {
            return (phase, None);
        }
        let index = self.index_client();
        let decision = fast::decide(&job.object, &self.policy, &index);
        self.finish_fast_read(job, decision, &index)
    }

    /// [`settle_fast`] at `conformance-small` sizes.
    ///
    /// The profile is not offered to production. [`settle_fast`] always uses
    /// section 6. A test builds the smallest conformance object and runs the
    /// same index client through this entry.
    #[cfg(test)]
    pub fn settle_fast_small(&self, job: &LockedThorough) -> FastPhase {
        self.settle_fast_small_seq(job).0
    }

    #[cfg(test)]
    pub fn settle_fast_small_seq(&self, job: &LockedThorough) -> (FastPhase, Option<u64>) {
        if let Some(phase) = self.sticky_phase(job) {
            return (phase, None);
        }
        let index = self.index_client();
        let decision = fast::decide_small(&job.object, &self.policy, &index);
        self.finish_fast_read(job, decision, &index)
    }

    /// Apply the policy's `[index] expected_network` to the documents the
    /// fast phase read, then write the fast line with the observed networks.
    /// Any document from another network, or with no `network`, refuses as
    /// `index_network_not_expected` (class `invalid`), whatever the fast
    /// checks found. A policy with no `[index]` table (gl-admit-v6) does not
    /// judge the network; the observation is still recorded.
    fn finish_fast_read(
        &self,
        job: &LockedThorough,
        decision: FastDecision,
        index: &impl verifier::IndexRead,
    ) -> (FastPhase, Option<u64>) {
        let networks = index.observed_networks();
        let decision = match self.policy.expected_network() {
            // Fail closed: a provisional that rests on no observed index
            // document is not mainnet evidence. Held, never a grant.
            Some(_)
                if networks.is_empty() && matches!(decision, FastDecision::Provisional { .. }) =>
            {
                FastDecision::Held {
                    record: "{}".to_owned(),
                }
            }
            Some(expected) if networks.iter().any(|network| network != expected) => {
                FastDecision::Refuse {
                    reasons: vec![reasons::CODE_INDEX_NETWORK_NOT_EXPECTED.to_owned()],
                    line: policy::network_line(expected),
                    record: "{}".to_owned(),
                    website: wire::OUTCOME_REJECTED,
                }
            }
            _ => decision,
        };
        self.finish_fast(job, decision, networks_json(&networks))
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

    fn finish_fast(
        &self,
        job: &LockedThorough,
        decision: FastDecision,
        networks: Value,
    ) -> (FastPhase, Option<u64>) {
        let phase = match decision {
            FastDecision::Refuse {
                website,
                reasons,
                line,
                record,
            } => {
                self.write_fast_refuse(job, &reasons, &line, &record, networks);
                self.rows.settle(&job.nonce, website, self.now_unix_s());
                FastPhase::Done
            }
            FastDecision::Held { .. } => {
                // Website token stays pending. The locked table has no `held`.
                FastPhase::Held
            }
            FastDecision::Provisional { record } => {
                return match self.write_provisional(job, &record, networks) {
                    Some(seq) => (FastPhase::Provisional, Some(seq)),
                    // No provisional line, so no thorough ticket. Held.
                    None => (FastPhase::Held, None),
                };
            }
        };
        (phase, None)
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
        networks: Value,
    ) {
        let now = self.now_unix_s();
        let mut write = DecisionWrite::bare(now, None, "fast", "refuse");
        self.stamp_network(&mut write, networks);
        write.submission_sha256 = job.submission;
        write.public_key = job.public_key;
        write.object_sha256 = job.object_sha;
        write.object = job.object.clone();
        write.reasons = reasons.to_vec();
        write.reason = Some(line.to_owned());
        write.summary = Some(record.to_owned());
        write.measures = crate::fast::fast_measures(record);
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

    fn write_provisional(
        &self,
        job: &LockedThorough,
        record: &str,
        networks: Value,
    ) -> Option<u64> {
        let now = self.now_unix_s();
        let mut write = DecisionWrite::bare(now, None, "fast", "provisional");
        self.stamp_network(&mut write, networks);
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
        match inner.log.append(&write) {
            Ok(seq) => Some(seq),
            Err(error) => {
                eprintln!(
                    "admission-service provisional was not written: {}",
                    show_log(error)
                );
                None
            }
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

fn networks_json(networks: &[String]) -> Value {
    Value::Array(networks.iter().cloned().map(Value::String).collect())
}

/// Outcome of [`Door::try_begin_pending`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAdmit {
    /// The key is banned. No slot was taken.
    Banned,
    /// A pending cap is full. No slot was taken.
    OverCap,
    /// A slot was taken. The caller releases it with `end_pending`
    /// (or a [`PendingSlot`]).
    Begun,
}

/// A taken pending slot, released exactly once when dropped. The thorough
/// task holds one, so a task that returns early, holds, errors or panics
/// still releases its slot.
pub struct PendingSlot {
    door: Door,
    source: String,
}

impl PendingSlot {
    pub fn new(door: Door, source: String) -> Self {
        Self { door, source }
    }
}

impl Drop for PendingSlot {
    fn drop(&mut self) {
        // A poisoned lock must not turn a panic unwind into an abort.
        if let Ok(mut inner) = self.door.inner.lock() {
            inner.pending_global = inner.pending_global.saturating_sub(1);
            if let Some(count) = inner.pending_by_source.get_mut(&self.source) {
                *count = count.saturating_sub(1);
            }
        }
    }
}

/// Key of the per-source pending cap: the submitter's public key, in hex.
pub fn pending_key(public_key: &[u8; 32]) -> String {
    hex32(public_key)
}

impl LockedThorough {
    /// The pending-cap key this job holds. `end_pending` takes this value.
    pub fn pending_key(&self) -> String {
        pending_key(&self.public_key)
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
    // Atomic and private: a 0600 tmp file, fsync, rename, fsync the directory.
    let tmp = dir.join("banned_keys.json.tmp");
    {
        use std::io::Write as _;
        let mut file = crate::thorough::private_file(&tmp)?;
        file.write_all(&bytes).map_err(|err| err.to_string())?;
        file.sync_all().map_err(|err| err.to_string())?;
    }
    std::fs::rename(&tmp, &path).map_err(|err| err.to_string())?;
    std::fs::File::open(dir)
        .and_then(|handle| handle.sync_all())
        .map_err(|err| err.to_string())
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
    use crate::policy::AdmissionPolicy;
    use crate::thorough;
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
        door.consume_for_test(&honest.nonce, honest.object_sha, door.now_unix_s());
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
        // The accepted result cites the thorough accept line for claims.
        let result = door.sign_in_result(&honest.nonce);
        assert_eq!(result["outcome"], "accepted");
        assert_eq!(result["log_seq"], serde_json::json!(door.last_seq()));
        assert_eq!(
            result["log_day"],
            serde_json::json!(crate::log::utc_day(door.now_unix_s()))
        );
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

        // A later submission is the same refusal: the fake-leg reason, no new draw.
        let seq = door.last_seq();
        assert!(door.banned(&honest.public_key));
        assert_eq!(door.ban_ignored(), 1);
        door.record_banned_refusal(&honest.public_key);
        let lines = chain(&dir);
        assert_eq!(lines.last().unwrap()["decision"], "refuse");
        assert_eq!(
            lines.last().unwrap()["reason"],
            "a recomputed leg result differs"
        );
        assert!(door.last_seq() > seq);
        let mut bumped = door.policy().clone();
        bumped.policy_id = "gl-admit-v9-resync".to_owned();
        let reopened = Door::open(
            &dir,
            bumped,
            "http://127.0.0.1:9",
            std::time::Duration::from_secs(1),
            vec!["https://groundinglab.io".to_owned()],
        )
        .unwrap();
        assert!(reopened.is_banned(&honest.public_key));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Defect 3 (gate review r2, finding 3). Two interleaved sign-ins each
    /// keep their own ticket: the draw and the thorough line are keyed on the
    /// job's own provisional seq, never on the log tail.
    #[test]
    fn interleaved_jobs_each_settle_against_their_own_provisional() {
        let built = small_honest();
        let (url, _) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_at(&url);
        let a = job_of(&built.bytes, 1);
        let b = job_of(&built.bytes, 2);
        assert_eq!(door.settle_fast_small(&a), FastPhase::Provisional);
        assert_eq!(door.settle_fast_small(&b), FastPhase::Provisional);
        let seq_a = provisional_seq(&dir, &a);
        let seq_b = provisional_seq(&dir, &b);
        assert_ne!(seq_a, seq_b);
        // B finishes first, then A: the log tail is no longer A's line.
        let out_b =
            thorough::run_for_test(&dir, seq_b, &built.hike, &b.object_sha, door.policy()).unwrap();
        door.settle_thorough(&b, out_b);
        let out_a =
            thorough::run_for_test(&dir, seq_a, &built.hike, &a.object_sha, door.policy()).unwrap();
        door.settle_thorough(&a, out_a);
        for (job, seq) in [(&a, seq_a), (&b, seq_b)] {
            let lines = chain(&dir);
            let line = lines
                .iter()
                .find(|line| {
                    line["phase"] == "thorough"
                        && line["submission_sha256"] == canon::hex32(&job.submission)
                })
                .unwrap_or_else(|| panic!("no thorough line for seq {seq}: {lines:?}"));
            assert_eq!(line["fast_seq"], serde_json::json!(seq), "{line}");
            assert_eq!(line["decision"], "accept", "{line}");
            assert_eq!(line["deep"]["indices"], draw_runs(&dir, seq), "{line}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Defect 1 (gate review r2, finding 1). A restart resumes an unsettled
    /// provisional with its stored draw: the indices on the thorough line are
    /// the stored draw, and a fake leg still revokes and bans. A provisional
    /// with no stored draw is held and never accepted.
    #[test]
    fn a_restart_resumes_with_the_stored_draw_and_never_draws_again() {
        let built = small_honest();
        let (url, _) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_at(&url);
        // Honest job: the draw is stored, then the process stops before the
        // thorough line is written.
        let honest = job_of(&built.bytes, 1);
        assert_eq!(door.settle_fast_small(&honest), FastPhase::Provisional);
        let honest_seq = provisional_seq(&dir, &honest);
        thorough::run_for_test(
            &dir,
            honest_seq,
            &built.hike,
            &honest.object_sha,
            door.policy(),
        )
        .unwrap();
        let stored = draw_runs(&dir, honest_seq);
        // A job whose process stopped before any draw was stored.
        let undrawn = job_of(&built.bytes, 2);
        assert_eq!(door.settle_fast_small(&undrawn), FastPhase::Provisional);
        let undrawn_seq = provisional_seq(&dir, &undrawn);
        // A fake leg whose draw was stored.
        let mut fake_hike = built.hike.clone();
        fake_hike.stages[0].leg_results[0][0] ^= 0xff;
        let fake_bytes = hike::encode_hike_sized(&fake_hike, hike::Sizes::conformance_small());
        let mut fake = job_of(&fake_bytes, 3);
        fake.public_key = [0x43; 32];
        assert_eq!(door.settle_fast_small(&fake), FastPhase::Provisional);
        let fake_seq = provisional_seq(&dir, &fake);
        thorough::run_for_test(&dir, fake_seq, &fake_hike, &fake.object_sha, door.policy())
            .unwrap();
        let fake_stored = draw_runs(&dir, fake_seq);
        drop(door);

        // Restart.
        let door = reopen(&dir, &url);
        let jobs = door.unsettled();
        assert_eq!(jobs.len(), 3);
        for job in jobs {
            let hike = if job.fast_seq == fake_seq {
                &fake_hike
            } else {
                &built.hike
            };
            let outcome =
                thorough::resume_for_test(&dir, job.fast_seq, hike, &job.object_sha, door.policy());
            door.settle_resumed(job, outcome);
        }
        assert!(!dir
            .join("pending")
            .join(format!("{undrawn_seq}.draw"))
            .exists());
        let lines = chain(&dir);
        let thorough_for = |seq: u64| {
            lines
                .iter()
                .find(|line| {
                    line["phase"] == "thorough" && line["fast_seq"] == serde_json::json!(seq)
                })
                .cloned()
        };
        let honest_line = thorough_for(honest_seq).expect("honest resume line");
        assert_eq!(honest_line["decision"], "accept");
        assert_eq!(honest_line["deep"]["indices"], stored);
        let fake_line = thorough_for(fake_seq).expect("fake resume line");
        assert_eq!(fake_line["decision"], "revoke");
        assert_eq!(fake_line["deep"]["indices"], fake_stored);
        assert!(door.banned(&fake.public_key), "the fake leg bans on resume");
        assert!(
            thorough_for(undrawn_seq).is_none(),
            "no draw: held, no line"
        );
        assert!(
            lines
                .iter()
                .filter(|line| line["decision"] == "accept")
                .all(|line| line["fast_seq"] == serde_json::json!(honest_seq)),
            "{lines:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The draw file and the ban file are 0600 and `pending/` is 0700,
    /// whatever the umask (the test process runs with the usual 022).
    #[test]
    fn the_draw_and_ban_files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let built = small_honest();
        let (url, _) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_at(&url);
        let job = job_of(&built.bytes, 1);
        assert_eq!(door.settle_fast_small(&job), FastPhase::Provisional);
        let seq = provisional_seq(&dir, &job);
        thorough::run_for_test(&dir, seq, &built.hike, &job.object_sha, door.policy()).unwrap();
        door.ban_key(&[0x44; 32], 0);
        door.ban_key(&[0x45; 32], 1);
        let mode = |path: PathBuf| std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(dir.join("pending").join(format!("{seq}.draw"))), 0o600);
        assert_eq!(mode(dir.join("pending")), 0o700);
        assert_eq!(mode(dir.join("banned_keys.json")), 0o600);
        assert!(!dir.join("banned_keys.json.tmp").exists());
        let bans: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("banned_keys.json")).unwrap()).unwrap();
        assert_eq!(bans.as_object().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn reopen(dir: &Path, index_url: &str) -> Door {
        let cfg = Config::for_tests(dir.to_path_buf(), index_url);
        Door::open(
            &cfg.log_dir,
            synthetic_policy(),
            &cfg.index_url,
            cfg.index_timeout,
            cfg.served_origins.clone(),
        )
        .unwrap()
    }

    /// Defect 2 (gate review r2, finding 2). A thorough error is held. It
    /// never falls through to another check that can write `accept`.
    #[test]
    fn a_thorough_error_is_held_and_never_accepts() {
        let built = small_honest();
        let (url, _) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_at(&url);
        let job = job_of(&built.bytes, 1);
        door.consume_for_test(&job.nonce, job.object_sha, door.now_unix_s());
        let (phase, seq) = door.settle_fast_small_seq(&job);
        assert_eq!(phase, FastPhase::Provisional);
        let seq = seq.unwrap();
        let pending = dir.join("pending");
        std::fs::create_dir_all(&pending).unwrap();
        let draw_path = pending.join(format!("{seq}.draw"));

        // Fault 1: a stored draw for another object.
        let other = [0x5au8; 32];
        let salt = [7u8; 32];
        let runs = vec![vec![0u64, 1, 2, 3, 4, 5]];
        let foreign = serde_json::json!({
            "seq": seq,
            "object_sha256": canon::hex32(&other),
            "salt": hex::encode(salt),
            "runs": runs,
            "commitment": canon::hex32(&thorough::commitment(seq, &other, &salt, &runs)),
        });
        std::fs::write(&draw_path, foreign.to_string()).unwrap();
        let outcome =
            thorough::run_for_test(&dir, seq, &built.hike, &job.object_sha, door.policy());
        assert!(outcome.is_err());
        door.finish_thorough(&job, outcome);

        // Fault 2: a committed draw naming a leg the object does not have,
        // so the leg recompute errors.
        let runs = vec![vec![0u64, 1, 2, 3, 4, 10_000]];
        let broken = serde_json::json!({
            "seq": seq,
            "object_sha256": canon::hex32(&job.object_sha),
            "salt": hex::encode(salt),
            "runs": runs,
            "commitment": canon::hex32(&thorough::commitment(seq, &job.object_sha, &salt, &runs)),
        });
        std::fs::write(&draw_path, broken.to_string()).unwrap();
        let outcome =
            thorough::run_for_test(&dir, seq, &built.hike, &job.object_sha, door.policy());
        assert!(outcome.is_err());
        door.finish_thorough(&job, outcome);

        let lines = chain(&dir);
        assert!(
            lines.iter().all(|line| line["decision"] != "accept"),
            "{lines:?}"
        );
        assert!(
            lines.iter().all(|line| line["phase"] != "thorough"),
            "{lines:?}"
        );
        assert_eq!(door.unsettled().len(), 1, "the provisional stays held");
        assert_eq!(door.sign_in_result(&job.nonce)["outcome"], "pending");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review fix: a key banned while its job was in flight is never
    /// granted. A matching thorough result writes no accept and the website
    /// stays pending.
    #[test]
    fn a_key_banned_in_flight_is_never_accepted() {
        let built = small_honest();
        let (url, _) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_at(&url);
        let job = job_of(&built.bytes, 1);
        door.consume_for_test(&job.nonce, job.object_sha, door.now_unix_s());
        let (phase, seq) = door.settle_fast_small_seq(&job);
        assert_eq!(phase, FastPhase::Provisional, "{}", chain_text(&dir));
        let seq = seq.unwrap();
        let out =
            thorough::run_for_test(&dir, seq, &built.hike, &job.object_sha, door.policy()).unwrap();
        assert!(out.matched);
        // Another task bans the key before this job settles.
        door.ban_key(&job.public_key, 0);
        door.finish_thorough(&job, Ok(out.clone()));
        assert!(
            chain(&dir).iter().all(|line| line["decision"] != "accept"),
            "{}",
            chain_text(&dir)
        );
        assert_eq!(door.sign_in_result(&job.nonce)["outcome"], "pending");
        // The restart resume path refuses to accept a banned key as well.
        let jobs = door.unsettled();
        assert_eq!(jobs.len(), 1);
        door.settle_resumed(jobs.into_iter().next().unwrap(), Ok(out));
        assert!(chain(&dir).iter().all(|line| line["decision"] != "accept"));
    }

    /// Review fix: the ban check and the pending slot are one step, and a
    /// taken slot is released by its guard even when the task panics.
    #[test]
    fn a_banned_key_takes_no_slot_and_a_panicking_job_releases_its_slot() {
        let (door, _dir) = open_with("http://127.0.0.1:9", AdmissionPolicy::default());
        let key = [0x42u8; 32];
        let source = super::pending_key(&key);
        assert_eq!(
            door.try_begin_pending(&key, &source),
            super::PendingAdmit::Begun
        );
        assert_eq!(door.pending_counts(&source), (1, 1));
        // Per-source cap (1) is full for this key.
        assert_eq!(
            door.try_begin_pending(&key, &source),
            super::PendingAdmit::OverCap
        );
        let slot = super::PendingSlot::new(door.clone(), source.clone());
        let panicked = std::thread::spawn(move || {
            let _slot = slot;
            panic!("thorough task died");
        })
        .join();
        assert!(panicked.is_err());
        assert_eq!(
            door.pending_counts(&source),
            (0, 0),
            "the guard released the slot"
        );
        door.ban_key(&key, 0);
        let before = door.ban_ignored();
        assert_eq!(
            door.try_begin_pending(&key, &source),
            super::PendingAdmit::Banned
        );
        assert_eq!(
            door.pending_counts(&source),
            (0, 0),
            "a banned key takes no slot"
        );
        assert_eq!(door.ban_ignored(), before + 1);
    }

    /// Review fix: under an expected network, a provisional that rests on no
    /// observed index document fails closed (held, no line).
    #[test]
    fn v7_holds_a_provisional_with_no_observed_network() {
        struct Silent;
        impl crate::verifier::IndexRead for Silent {
            fn beacon(&self, _: u64) -> Result<hike::IndexBeacon, ()> {
                Err(())
            }
            fn beacon_fresh(&self, _: u64) -> Result<hike::IndexBeacon, ()> {
                Err(())
            }
            fn plant(&self, _: &hike::PathLocators) -> hike::ChainRead {
                hike::ChainRead::NotRead
            }
            fn plant_fresh(&self, _: &hike::PathLocators) -> hike::ChainRead {
                hike::ChainRead::NotRead
            }
        }
        let (door, dir) = open_with("http://127.0.0.1:9", shipped_policy("gl-admit-v7.toml"));
        let built = small_honest();
        let job = job_of(&built.bytes, 1);
        let decision = crate::fast::FastDecision::Provisional {
            record: "{}".to_owned(),
        };
        let (phase, seq) = door.finish_fast_read(&job, decision, &Silent);
        assert_eq!(phase, FastPhase::Held);
        assert_eq!(seq, None);
        assert!(chain(&dir).is_empty(), "{}", chain_text(&dir));
    }

    fn provisional_seq(dir: &Path, job: &LockedThorough) -> u64 {
        chain(dir)
            .iter()
            .find(|line| {
                line["decision"] == "provisional"
                    && line["submission_sha256"] == canon::hex32(&job.submission)
            })
            .and_then(|line| line["seq"].as_u64())
            .expect("provisional line")
    }

    fn draw_runs(dir: &Path, seq: u64) -> serde_json::Value {
        let raw = std::fs::read(dir.join("pending").join(format!("{seq}.draw"))).expect("draw");
        let draw: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        draw["runs"].clone()
    }

    fn fast_line(dir: &Path) -> serde_json::Value {
        chain(dir)
            .into_iter()
            .find(|line| line["phase"] == "fast")
            .expect("fast line")
    }

    fn assert_network_refusal(dir: &Path, observed: &str) {
        let line = fast_line(dir);
        assert_eq!(line["decision"], "refuse", "{line}");
        assert_eq!(line["reason_code"], "index_network_not_expected", "{line}");
        assert_eq!(line["class"], "invalid", "{line}");
        assert_eq!(
            line["reason"],
            "The index evidence is not from the Bitcoin mainnet, so it cannot support admission here.",
            "{line}"
        );
        assert_eq!(line["policy_id"], "gl-admit-v7", "{line}");
        assert_eq!(line["expected_network"], "main", "{line}");
        let observed_list = line["index_network"].as_array().expect("index_network");
        assert!(observed_list.iter().any(|item| item == observed), "{line}");
    }

    /// gl-admit-v7: mainnet index documents are accepted, and both the fast
    /// line and the thorough line record the network.
    #[test]
    fn v7_accepts_mainnet_index_evidence() {
        let built = small_honest_on("main");
        let (url, _) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_with(&url, shipped_policy("gl-admit-v7.toml"));
        let job = job_of(&built.bytes, 1);
        let (phase, seq) = door.settle_fast_small_seq(&job);
        assert_eq!(phase, FastPhase::Provisional, "{}", chain_text(&dir));
        let seq = seq.unwrap();
        let out =
            thorough::run_for_test(&dir, seq, &built.hike, &job.object_sha, door.policy()).unwrap();
        door.settle_thorough(&job, out);
        let lines = chain(&dir);
        assert_eq!(lines.len(), 2, "{lines:?}");
        for line in &lines {
            assert_eq!(line["policy_id"], "gl-admit-v7", "{line}");
            assert_eq!(line["expected_network"], "main", "{line}");
            assert_eq!(line["index_network"], serde_json::json!(["main"]), "{line}");
        }
        assert_eq!(lines[1]["decision"], "accept");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// gl-admit-v7: synthetic index documents refuse as
    /// `index_network_not_expected`, class `invalid`, with no thorough run.
    #[test]
    fn v7_refuses_synthetic_index_evidence() {
        let built = small_honest_on("synthetic");
        let (url, _) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_with(&url, shipped_policy("gl-admit-v7.toml"));
        let job = job_of(&built.bytes, 1);
        door.consume_for_test(&job.nonce, job.object_sha, door.now_unix_s());
        assert_eq!(
            door.settle_fast_small(&job),
            FastPhase::Done,
            "{}",
            chain_text(&dir)
        );
        assert!(door.unsettled().is_empty());
        assert_eq!(chain(&dir).len(), 1);
        assert_network_refusal(&dir, "synthetic");
        assert_eq!(door.sign_in_result(&job.nonce)["outcome"], "rejected");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// gl-admit-v7 checks header reads and plant reads, including the fresh
    /// confirming read, and refuses a document with no `network`.
    #[test]
    fn v7_checks_header_plant_and_fresh_reads_and_a_missing_network() {
        let built = small_honest_on("main");
        // The seed beacon's height is read as a header only (no plant there).
        let header_only = built.header_height;
        assert!(built
            .docs
            .get(&header_only)
            .is_some_and(|doc| !doc.contains("\"plant\"")));
        for (mirror, observed) in [
            (Mirror::Network(header_only, "synthetic"), "synthetic"),
            (
                Mirror::Network(header_only, ""),
                crate::verifier::NETWORK_ABSENT,
            ),
            (
                Mirror::PlantUnconfirmedFreshNetwork(built.plant_height, "synthetic"),
                "synthetic",
            ),
        ] {
            let (url, _) = serve(&built.docs, mirror);
            let (door, dir) = open_with(&url, shipped_policy("gl-admit-v7.toml"));
            let phase = door.settle_fast_small(&job_of(&built.bytes, 1));
            assert_eq!(phase, FastPhase::Done, "{}", chain_text(&dir));
            assert_network_refusal(&dir, observed);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// gl-admit-v6 is unchanged: no network judgment (synthetic documents
    /// still reach `provisional`), no `expected_network` key, the same bind
    /// hash and the same published policy document as gate 0.5.3.
    #[test]
    fn v6_is_unchanged_by_the_network_rule() {
        let v6 = shipped_policy("gl-admit-v6.toml");
        assert_eq!(v6, AdmissionPolicy::default());
        assert_eq!(v6.expected_network(), None);
        assert_eq!(
            v6.bind_key(),
            "97d868a1d68f6e8b3a3fd4e3e656028fab04625b61ee3f9c030e70681ed88039"
        );
        assert_eq!(
            serde_json::to_string(&crate::policy::public_document(&v6)).unwrap(),
            r#"{"policy_id":"gl-admit-v6","leg_rate_floor":{"p":1,"q":5},"min_anchored_paths":6,"thorough_six_leg_runs":3,"thorough_early_stop":true,"provisional":"on","held_on_not_read":true,"ban_key_on_fake_leg":true,"draw_commitment":"sha256(draw || salt), revealed with the result","reverify_max_attempts":3,"max_held_blocks":6,"pending_per_source":1,"pending_global":4,"verify_cap_per_door_session":3,"source":"recommendation, not specification","spec":"Satoshi Hike v2.1","anchored_span":16}"#
        );
        let built = small_honest_on("synthetic");
        let (url, _) = serve(&built.docs, Mirror::Honest);
        let (door, dir) = open_with(&url, v6);
        assert_eq!(
            door.settle_fast_small(&job_of(&built.bytes, 1)),
            FastPhase::Provisional
        );
        let line = fast_line(&dir);
        assert_eq!(line["decision"], "provisional");
        assert_eq!(line["policy_id"], "gl-admit-v6");
        assert!(line.get("expected_network").is_none(), "{line}");
        assert_eq!(line["index_network"], serde_json::json!(["synthetic"]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// gl-admit-v7 is gl-admit-v6 plus `[index] expected_network = "main"`,
    /// and binds cleanly beside an existing gl-admit-v6 binding.
    #[test]
    fn v7_is_v6_plus_the_network_key_and_binds_beside_v6() {
        let v6 = shipped_policy("gl-admit-v6.toml");
        let v7 = shipped_policy("gl-admit-v7.toml");
        assert_eq!(v7.policy_id, "gl-admit-v7");
        assert_eq!(v7.expected_network(), Some("main"));
        let mut same = v7.clone();
        same.policy_id = v6.policy_id.clone();
        same.index = None;
        assert_eq!(same, v6);
        let doc = crate::policy::public_document(&v7);
        assert_eq!(doc["index"]["expected_network"], "main");
        let dir = scratch_dir();
        let cfg = Config::for_tests(dir.clone(), "http://127.0.0.1:9");
        let open = |policy: AdmissionPolicy| {
            Door::open(
                &dir,
                policy,
                &cfg.index_url,
                cfg.index_timeout,
                cfg.served_origins.clone(),
            )
        };
        drop(open(v6.clone()).unwrap());
        drop(open(v7.clone()).unwrap());
        drop(open(v6.clone()).unwrap());
        let bind: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("policy-bind.json")).unwrap()).unwrap();
        assert_eq!(
            bind["gl-admit-v6"],
            "97d868a1d68f6e8b3a3fd4e3e656028fab04625b61ee3f9c030e70681ed88039"
        );
        assert_eq!(bind["gl-admit-v7"], serde_json::json!(v7.bind_key()));
        let mut changed = v7.clone();
        changed.index = Some(crate::policy::IndexPolicy {
            expected_network: "synthetic".to_owned(),
        });
        assert!(
            open(changed).is_err(),
            "a bound id is never reused with other values"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A testing build does not start under a mainnet policy. It may start
    /// under a synthetic policy. A production build starts under either.
    #[test]
    fn a_testing_build_refuses_a_mainnet_policy() {
        use crate::policy::GateBuild;
        let cfg = Config::for_tests(scratch_dir(), "http://127.0.0.1:9");
        for (policy, build, starts) in [
            (
                shipped_policy("gl-admit-v7.toml"),
                GateBuild::Testing,
                false,
            ),
            (synthetic_policy(), GateBuild::Testing, true),
            (shipped_policy("gl-admit-v6.toml"), GateBuild::Testing, true),
            (
                shipped_policy("gl-admit-v7.toml"),
                GateBuild::Production,
                true,
            ),
        ] {
            let dir = scratch_dir();
            let opened = Door::open_as(
                &dir,
                policy.clone(),
                &cfg.index_url,
                cfg.index_timeout,
                cfg.served_origins.clone(),
                build,
            );
            assert_eq!(opened.is_ok(), starts, "{} {build:?}", policy.policy_id);
            if let Err(error) = opened {
                assert!(error.contains("testing build"), "{error}");
                assert!(
                    !dir.join("policy-bind.json").exists(),
                    "refused before binding"
                );
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        assert_eq!(crate::policy::gate_build(), GateBuild::Production);
    }

    struct Hits {
        normal: AtomicU64,
        fresh: AtomicU64,
    }

    #[derive(Clone, Copy)]
    enum Mirror {
        Honest,
        /// Both reads of this height carry this `network` ("" drops the key).
        Network(u64, &'static str),
        /// The cached plant read mismatches; the fresh confirming read is
        /// honest but carries this `network`.
        PlantUnconfirmedFreshNetwork(u64, &'static str),
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

    /// The honest-path fixtures serve `"network":"synthetic"` documents, so
    /// they run on a synthetic-policy gate: gl-admit-v7's table with
    /// `[index] expected_network = "synthetic"` under a test-only id.
    fn open_at(index_url: &str) -> (Door, PathBuf) {
        open_with(index_url, synthetic_policy())
    }

    fn scratch_dir() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("pcoc-gate-fast-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn open_with(index_url: &str, policy: AdmissionPolicy) -> (Door, PathBuf) {
        let dir = scratch_dir();
        let cfg = Config::for_tests(dir.clone(), index_url);
        let door = Door::open(
            &cfg.log_dir,
            policy,
            &cfg.index_url,
            cfg.index_timeout,
            cfg.served_origins.clone(),
        )
        .unwrap();
        (door, dir)
    }

    /// A shipped policy file, read the way a deploy reads it.
    fn shipped_policy(name: &str) -> AdmissionPolicy {
        let dir = scratch_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("policy")
            .join(name);
        let path = dir.join("gate.toml");
        std::fs::write(
            &path,
            format!(
                "served_origins = [\"https://groundinglab.io\"]\npolicy = {:?}\n",
                file.display().to_string()
            ),
        )
        .unwrap();
        let policy = Config::load(&path).unwrap().policy;
        let _ = std::fs::remove_dir_all(&dir);
        policy
    }

    fn synthetic_policy() -> AdmissionPolicy {
        let mut policy = shipped_policy("gl-admit-v7.toml");
        policy.policy_id = "gl-admit-v9007".to_owned();
        policy.index = Some(crate::policy::IndexPolicy {
            expected_network: "synthetic".to_owned(),
        });
        policy
    }

    fn job_of(bytes: &[u8], nonce_byte: u8) -> LockedThorough {
        let mut nonce = [0u8; 32];
        nonce[0] = nonce_byte;
        LockedThorough {
            nonce,
            origin: "https://groundinglab.io".to_owned(),
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
            Mirror::Network(target, network) if height == target => with_network(honest, network),
            Mirror::PlantUnconfirmedFreshNetwork(target, _) if height == target && !fresh => {
                bad_root(honest)
            }
            Mirror::PlantUnconfirmedFreshNetwork(target, network) if height == target => {
                with_network(honest, network)
            }
            Mirror::PlantConfirmed(target) if height == target => bad_root(honest),
            Mirror::PlantUnconfirmed(target) if height == target && !fresh => bad_root(honest),
            Mirror::HeaderConfirmed(target) if height == target => bad_hash(honest),
            Mirror::HeaderUnconfirmed(target) if height == target && !fresh => bad_hash(honest),
            _ => honest.to_owned(),
        }
    }

    /// Replace the document's `network`. An empty name drops the key.
    fn with_network(body: &str, network: &str) -> String {
        let mut value: serde_json::Value = serde_json::from_str(body).unwrap();
        let map = value.as_object_mut().unwrap();
        if network.is_empty() {
            map.remove("network");
        } else {
            map.insert("network".to_owned(), serde_json::json!(network));
        }
        value.to_string()
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
        small_honest_on("synthetic")
    }

    fn small_honest_on(network: &str) -> SmallHonest {
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
        let terrain = hike::fill_terrain_sized(&hike::seed_from_preimage(&seed), sizes);
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
                    network,
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
                network,
                &last,
                last_height,
                1_700_000_000 + last_height,
                plants.get(&last_height).map(Vec::as_slice).unwrap_or(&[]),
            ),
        );
        // BIP113 windows for the first and last beacons. Times stay linear in
        // height, so the median span equals the raw nTime span.
        let first = hike.stages[0].beacon_height;
        for height in first.saturating_sub(10)..first {
            if docs.contains_key(&height) {
                continue;
            }
            let mut hash = [0u8; 32];
            hash[0] = 0xEE;
            hash[1] = height as u8;
            docs.insert(
                height,
                index_document(network, &hash, height, 1_700_000_000 + height, &[]),
            );
        }
        for height in last_height.saturating_sub(10)..last_height {
            if docs.contains_key(&height) {
                continue;
            }
            let mut hash = [0u8; 32];
            hash[0] = 0xEF;
            hash[1] = height as u8;
            docs.insert(
                height,
                index_document(network, &hash, height, 1_700_000_000 + height, &[]),
            );
        }
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
        network: &str,
        natural: &[u8; 32],
        height: u64,
        time: u64,
        plants: &[hike::PathLocators],
    ) -> String {
        let hash = hike::pathchain::display_hex(natural);
        let mut body = format!(
            "{{\"network\":\"{network}\",\"header\":{{\"hash\":\"{hash}\",\"height\":{height},\"time\":{time}}}"
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
