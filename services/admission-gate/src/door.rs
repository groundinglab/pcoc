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
    /// Cache of index response bodies for this door. Not a terrain.
    mirror: Arc<verifier::Mirror>,
}

#[derive(Clone)]
struct HeldEntry {
    job: LockedThorough,
    attempts: u64,
    last_unix: u64,
    start_tip: Option<u64>,
    deciding_tip: Option<u64>,
    grant: bool,
    budget: bool,
    record: String,
    networks: Value,
}

struct Inner {
    log: DecisionLog,
    /// Unix second the locked wire uses. `None` reads the process clock.
    now: Option<u64>,
    /// Public keys this gate ignores. A fake leg writes one. The value is
    /// the seq of the revoke that banned it.
    banned: std::collections::HashMap<[u8; 32], u64>,
    /// Unix second the ban was recorded. A loaded ban uses process start.
    banned_at: std::collections::HashMap<[u8; 32], u64>,
    /// Open fast or thorough work, counted for the pending caps.
    pending_global: u64,
    pending_by_source: std::collections::HashMap<String, u64>,
    /// How many times a banned key was ignored. Not a decision line.
    ban_ignored: u64,
    /// Consuming sign-ins that started a verify in this process.
    verify_session: u64,
    verify_object: std::collections::HashMap<[u8; 32], u64>,
    held: Vec<HeldEntry>,
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
        let rows = Arc::new(SignInRows::open(
            unix_now(),
            policy.result_retention_seconds,
            policy.challenge_clock_skew_seconds,
        ));
        Self::open_with_rows(
            dir,
            policy,
            index_url,
            index_timeout,
            served_origins,
            build,
            rows,
        )
    }

    /// Two workers share one nonce store. The store start is the shared one.
    pub fn open_with_rows(
        dir: &Path,
        policy: AdmissionPolicy,
        index_url: &str,
        index_timeout: std::time::Duration,
        served_origins: Vec<String>,
        build: policy::GateBuild,
        rows: Arc<SignInRows>,
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
        let opened = unix_now();
        let banned_at = banned.keys().map(|key| (*key, opened)).collect();
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                log,
                now: None,
                banned,
                banned_at,
                pending_global: 0,
                pending_by_source: std::collections::HashMap::new(),
                ban_ignored: 0,
                verify_session: 0,
                verify_object: std::collections::HashMap::new(),
                held: Vec::new(),
            })),
            rows,
            policy,
            index_url: index_url.to_owned(),
            index_timeout,
            served_origins: Arc::new(served_origins),
            mirror: Arc::new(verifier::Mirror::new()),
        })
    }

    pub fn nonce_rows(&self) -> Arc<SignInRows> {
        self.rows.clone()
    }

    pub fn store_start(&self) -> u64 {
        self.rows.store_start()
    }

    /// The site origins this gate serves for sign-in.
    pub fn served_origins(&self) -> &[String] {
        &self.served_origins
    }

    /// A banned public key is ignored. No verify, no decision line.
    /// The counter is the only record of the ignore.
    pub fn banned(&self, public_key: &[u8; 32]) -> bool {
        let mut inner = self.lock();
        if self.ban_live(&inner, public_key) {
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
        let inner = self.lock();
        self.ban_live(&inner, public_key)
    }

    fn ban_live(&self, inner: &Inner, public_key: &[u8; 32]) -> bool {
        if !inner.banned.contains_key(public_key) {
            return false;
        }
        let Some(seconds) = self.policy.fake_leg_ignore_seconds else {
            return true;
        };
        let at = inner.banned_at.get(public_key).copied().unwrap_or(0);
        clock(inner.now) < at.saturating_add(seconds)
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
        self.settle_with(job, out, true);
    }

    /// Thorough after `provisional = off`. No grant was issued, so a mismatch
    /// is `refuse`, and the accept line omits `fast_seq`.
    pub fn settle_direct(&self, job: &LockedThorough, out: crate::thorough::ThoroughOut) {
        self.settle_with(job, out, false);
    }

    fn settle_with(&self, job: &LockedThorough, out: crate::thorough::ThoroughOut, granted: bool) {
        self.drop_held(&job.nonce);
        if granted && !self.provisional_is(out.seq, job) {
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
            granted,
        );
        let seq = {
            let mut inner = self.lock();
            inner.log.append(&write).ok()
        };
        if out.matched {
            self.write_friendly(&job.public_key, &job.object_sha);
            // The thorough accept line opens the claim window, like a return.
            if let Some(seq) = seq {
                self.rows
                    .note_access(&job.nonce, Some(log::utc_day(now)), Some(seq));
            }
            self.rows.settle(&job.nonce, wire::OUTCOME_ACCEPTED, now);
        } else {
            // The website token stays `rejected`. `revoked` is not a §15.1 token.
            self.rows.settle(&job.nonce, wire::OUTCOME_REJECTED, now);
            if self.policy.ban_key_on_fake_leg {
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
        self.finish_with(job, outcome, true);
    }

    pub fn finish_direct(
        &self,
        job: &LockedThorough,
        outcome: Result<crate::thorough::ThoroughOut, String>,
    ) {
        self.finish_with(job, outcome, false);
    }

    fn finish_with(
        &self,
        job: &LockedThorough,
        outcome: Result<crate::thorough::ThoroughOut, String>,
        granted: bool,
    ) {
        match outcome {
            Ok(out) => self.settle_with(job, out, granted),
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
        granted: bool,
    ) -> DecisionWrite {
        let decision = if out.matched {
            "accept"
        } else if granted {
            "revoke"
        } else {
            "refuse"
        };
        let mut write = DecisionWrite::bare(now, None, "thorough", decision);
        if granted {
            write.fast_seq = Some(out.seq);
        }
        // The thorough check reads no index. It rests on the provisional's reads.
        let networks = self.line_networks(out.seq);
        self.stamp_network(&mut write, networks);
        write.submission_sha256 = submission;
        write.public_key = public_key;
        write.object_sha256 = object_sha;
        write.object = object;
        write.reasons = vec![if out.matched {
            wire::OUTCOME_ACCEPTED.to_owned()
        } else if granted {
            "revoke".to_owned()
        } else {
            "refuse".to_owned()
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
            true,
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
        if out.matched {
            self.write_friendly(&job.public_key, &job.object_sha);
        } else if self.policy.ban_key_on_fake_leg {
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
        if self.ban_live(&inner, public_key) {
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

    /// Write a ban for the public key only. The seed is not stored.
    pub fn ban_key(&self, public_key: &[u8; 32], seq: u64) {
        let mut inner = self.lock();
        let at = clock(inner.now);
        inner.banned.insert(*public_key, seq);
        inner.banned_at.insert(*public_key, at);
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
            exp: now.saturating_add(self.policy.challenge_lifetime_seconds),
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
        clock(self.lock().now)
    }

    /// Count a verify that is about to start. `None` means it may start.
    fn over_verify_cap(&self, object_sha: &[u8; 32]) -> Option<&'static str> {
        let mut inner = self.lock();
        if inner.verify_session >= self.policy.verify_cap_per_door_session {
            return Some(reasons::CODE_VERIFY_CAP_SESSION);
        }
        if let Some(cap) = self.policy.verify_cap_per_object {
            if inner.verify_object.get(object_sha).copied().unwrap_or(0) >= cap {
                return Some(reasons::CODE_VERIFY_CAP_OBJECT);
            }
        }
        inner.verify_session = inner.verify_session.saturating_add(1);
        *inner.verify_object.entry(*object_sha).or_insert(0) += 1;
        None
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
            let decision = line.value.get("decision").and_then(|item| item.as_str());
            if decision != Some("provisional") && decision != Some("received") {
                continue;
            }
            if decision == Some("received") {
                let submission = line
                    .value
                    .get("submission_sha256")
                    .and_then(|item| item.as_str());
                let settled_direct = inner.log.lines().iter().any(|later| {
                    later.value.get("phase").and_then(|item| item.as_str()) == Some("thorough")
                        && later
                            .value
                            .get("submission_sha256")
                            .and_then(|item| item.as_str())
                            == submission
                });
                if settled_direct {
                    continue;
                }
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
            || !wire::window_accepts(
                now,
                body.exp,
                self.policy.challenge_lifetime_seconds,
                self.policy.challenge_clock_skew_seconds,
                self.rows.store_start(),
            )
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
        let key = verifier::public_key(&body.object);
        let message = wire::bip340_message(&body.origin, &body.nonce, body.exp, &body.object_sha);
        let Some(public_key) = key else {
            return self.bad_signature_step(&body, now);
        };
        if !wire::verify_signature(&public_key, &message, &body.signature) {
            return self.bad_signature_step(&body, now);
        }

        // 7–8. Kind is the fast phase (SPEC §14.7, Rule A5). Any other kind
        // reaches that phase and is `unknown_kind`. Bytes that do not decode
        // are refused here, with no fast line.
        let class = verifier::classify(&body.object);
        if matches!(class, Class::Unreadable | Class::NoPublicKey) {
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
                return match self.rows.consume(
                    &body,
                    ClientClass::DeliveryOk,
                    wire::OUTCOME_PENDING,
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

        // 9. Consume when this step starts. A verify cap refuses here, before
        // thorough, and does not increment the counter.
        if let Some(code) = self.over_verify_cap(&body.object_sha) {
            self.end_pending(&pending_key);
            let signed =
                self.rows
                    .consume(&body, ClientClass::DeliveryOk, wire::OUTCOME_PENDING, now);
            if signed.is_ok() {
                self.rows.note_holder(&body.nonce, public_key);
                let submission = wire::submission_sha256(
                    &body.object_sha,
                    &public_key,
                    &message,
                    &body.signature,
                );
                let job = LockedThorough {
                    nonce: body.nonce,
                    origin: body.origin.clone(),
                    object: body.object,
                    submission,
                    public_key,
                    object_sha: body.object_sha,
                    prior_accept: None,
                };
                self.write_fast_refuse(
                    &job,
                    &[code.to_owned()],
                    &reasons::public_sentence(code),
                    "{}",
                    Value::Null,
                );
                self.rows.settle(&body.nonce, wire::OUTCOME_REJECTED, now);
            }
            return match signed {
                Ok(()) => locked_client(ClientClass::DeliveryOk, None),
                Err(replay) => replay_response(replay),
            };
        }

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
        // Website `legs` / `span_s` are bookend N/S (Rule B6). They are not
        // the Rule B2 pair that `policy_checks` and `judge` compare.
        if let Ok(Report::Read(measured)) = &report {
            let (legs, span_s) = bookends_from_record(&measured.record);
            self.rows.note_bookends(&job.nonce, legs, span_s);
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
                    self.policy.born_height,
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
                if (networks.is_empty() || index.network_gap())
                    && matches!(
                        decision,
                        FastDecision::Provisional { .. } | FastDecision::Direct { .. }
                    ) =>
            {
                let tip = match &decision {
                    FastDecision::Provisional { tip, .. } | FastDecision::Direct { tip, .. } => {
                        *tip
                    }
                    _ => None,
                };
                FastDecision::Held {
                    record: "{}".to_owned(),
                    budget: false,
                    tip,
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
        verifier::IndexClient::shared(&self.index_url, self.index_timeout, self.mirror.clone())
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
                self.drop_held(&job.nonce);
                self.write_fast_refuse(job, &reasons, &line, &record, networks);
                self.rows.settle(&job.nonce, website, self.now_unix_s());
                FastPhase::Done
            }
            FastDecision::Held {
                record,
                budget,
                tip,
            } => {
                // Website token stays pending. The locked table has no `held`.
                self.note_held(job, &record, budget, networks.clone(), tip, None);
                FastPhase::Held
            }
            FastDecision::Provisional { record, .. } => {
                return match self.write_provisional(job, &record, networks) {
                    Some(seq) => (FastPhase::Provisional, Some(seq)),
                    // No provisional line, so no thorough ticket. Held.
                    None => (FastPhase::Held, None),
                };
            }
            FastDecision::Direct { record, .. } => {
                return match self.write_received(job, &record, networks) {
                    Some(seq) => (FastPhase::Direct, Some(seq)),
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

    fn write_received(&self, job: &LockedThorough, record: &str, networks: Value) -> Option<u64> {
        let now = self.now_unix_s();
        let mut write = DecisionWrite::bare(now, None, "fast", "received");
        self.stamp_network(&mut write, networks);
        write.submission_sha256 = job.submission;
        write.public_key = job.public_key;
        write.object_sha256 = job.object_sha;
        write.object = job.object.clone();
        write.reasons = vec!["received".to_owned()];
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
                    "admission-service received was not written: {}",
                    show_log(error)
                );
                None
            }
        }
    }

    fn drop_held(&self, nonce: &[u8; 32]) {
        self.lock().held.retain(|entry| entry.job.nonce != *nonce);
    }

    fn note_held(
        &self,
        job: &LockedThorough,
        record: &str,
        budget: bool,
        networks: Value,
        tip: Option<u64>,
        previous_attempts: Option<u64>,
    ) {
        let now = self.now_unix_s();
        let grant = self.has_provisional(job);
        let mut inner = self.lock();
        let attempts = if let Some(existing) = inner
            .held
            .iter_mut()
            .find(|entry| entry.job.nonce == job.nonce)
        {
            existing.attempts = existing.attempts.saturating_add(1);
            existing.last_unix = now;
            existing.budget = budget;
            existing.record = record.to_owned();
            existing.networks = networks;
            existing.grant = existing.grant || grant;
            if existing.start_tip.is_none() {
                existing.start_tip = tip;
            }
            if tip.is_some() {
                existing.deciding_tip = tip;
            }
            existing.attempts
        } else {
            let attempts = previous_attempts.unwrap_or(1);
            inner.held.push(HeldEntry {
                job: job.clone(),
                attempts,
                last_unix: now,
                start_tip: tip,
                deciding_tip: tip,
                grant,
                budget,
                record: record.to_owned(),
                networks,
            });
            attempts
        };
        let start_tip = inner
            .held
            .iter()
            .find(|entry| entry.job.nonce == job.nonce)
            .and_then(|entry| entry.start_tip);
        let deciding = inner
            .held
            .iter()
            .find(|entry| entry.job.nonce == job.nonce)
            .and_then(|entry| entry.deciding_tip);
        let grant = inner
            .held
            .iter()
            .find(|entry| entry.job.nonce == job.nonce)
            .map(|entry| entry.grant)
            .unwrap_or(grant);
        let detail = held_bound_detail(
            attempts,
            start_tip,
            deciding,
            self.policy.reverify_max_attempts,
            self.policy.max_held_blocks,
        );
        if detail.is_some() {
            inner.held.retain(|entry| entry.job.nonce != job.nonce);
        }
        drop(inner);
        if let Some(detail) = detail {
            self.write_held_bound(job, grant, detail);
        }
    }

    fn write_held_bound(&self, job: &LockedThorough, grant: bool, detail: &str) {
        let now = self.now_unix_s();
        let decision = if grant { "revoke" } else { "refuse" };
        let mut write = DecisionWrite::bare(now, None, "fast", decision);
        if grant {
            if let Some(seq) = self.provisional_seq(job) {
                write.fast_seq = Some(seq);
            }
        }
        self.stamp_network(&mut write, Value::Null);
        write.submission_sha256 = job.submission;
        write.public_key = job.public_key;
        write.object_sha256 = job.object_sha;
        write.object = job.object.clone();
        write.reasons = vec![reasons::CODE_HELD_BOUND.to_owned()];
        write.reason = Some(detail.to_owned());
        write.reason_code = Some(reasons::CODE_HELD_BOUND.to_owned());
        write.class = Some(reasons::class_of(reasons::CODE_HELD_BOUND).to_owned());
        write.summary = Some("{}".to_owned());
        write.policy_id = Some(self.policy.policy_id.clone());
        let mut inner = self.lock();
        if inner.log.append(&write).is_ok() {
            drop(inner);
            self.rows.settle(&job.nonce, wire::OUTCOME_REJECTED, now);
        }
    }

    fn has_provisional(&self, job: &LockedThorough) -> bool {
        self.provisional_seq(job).is_some()
    }

    fn provisional_seq(&self, job: &LockedThorough) -> Option<u64> {
        let inner = self.lock();
        let submission = hex32(&job.submission);
        inner.log.lines().iter().find_map(|line| {
            let decision = line.value.get("decision").and_then(|item| item.as_str());
            let same = line
                .value
                .get("submission_sha256")
                .and_then(|item| item.as_str())
                == Some(submission.as_str());
            if decision == Some("provisional") && same {
                line.value.get("seq").and_then(|item| item.as_u64())
            } else {
                None
            }
        })
    }

    /// Retry held jobs whose `held_retry_seconds` has elapsed. Returns the
    /// jobs that left `held` for provisional or direct thorough.
    pub fn poll_held(&self) -> Vec<HeldReady> {
        let now = self.now_unix_s();
        let retry = self.policy.held_retry_seconds;
        let due: Vec<HeldEntry> = {
            let inner = self.lock();
            inner
                .held
                .iter()
                .filter(|entry| now >= entry.last_unix.saturating_add(retry))
                .cloned()
                .collect()
        };
        let mut ready = Vec::new();
        for entry in due {
            if entry.budget {
                self.note_held(
                    &entry.job,
                    &entry.record,
                    true,
                    entry.networks.clone(),
                    entry.deciding_tip,
                    None,
                );
                continue;
            }
            let index = self.index_client();
            let decision = fast::decide(&entry.job.object, &self.policy, &index);
            let (phase, seq) = self.finish_fast_read(&entry.job, decision, &index);
            if phase != FastPhase::Held {
                self.drop_held(&entry.job.nonce);
            }
            if let (Some(seq), true) = (
                seq,
                matches!(phase, FastPhase::Provisional | FastPhase::Direct),
            ) {
                ready.push(HeldReady {
                    job: entry.job,
                    seq,
                    granted: phase == FastPhase::Provisional,
                });
            }
        }
        ready
    }

    fn write_friendly(&self, public_key: &[u8; 32], object_sha: &[u8; 32]) {
        use std::io::Write;
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let dir = self.lock().log.dir().join("friendly");
        let _ = std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir);
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        let path = dir.join(hex32(public_key));
        if let Ok(mut file) = crate::thorough::private_file(&path) {
            let _ = file.write_all(hex32(object_sha).as_bytes());
            let _ = file.sync_all();
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

    /// The door's index-body cache. Tests count fresh reads that bypass it.
    pub fn mirror(&self) -> Arc<verifier::Mirror> {
        self.mirror.clone()
    }

    /// Pin the nonce-store start. Production [`Door::open`] records the
    /// process clock and does not call this. A test whose clock is a fixture
    /// epoch calls it so a challenge issued in that epoch is not a restart
    /// replay against the wall clock.
    pub fn test_set_store_start(&self, start: u64) {
        self.rows.set_start(start);
    }

    #[cfg(test)]
    pub fn test_flush_nonce_store(&self, now: u64) {
        self.rows.flush(now);
    }

    /// A provisional line with no index read. The seq is the thorough ticket.
    #[cfg(test)]
    pub fn test_grant_provisional(&self, job: &LockedThorough) -> u64 {
        self.write_provisional(job, "{}", serde_json::json!(["main"]))
            .expect("provisional")
    }

    #[cfg(test)]
    pub fn test_held_attempts(&self, nonce: &[u8; 32]) -> Option<u64> {
        self.lock()
            .held
            .iter()
            .find(|entry| entry.job.nonce == *nonce)
            .map(|entry| entry.attempts)
    }

    #[cfg(test)]
    pub fn test_decisions(&self) -> Vec<String> {
        self.lock()
            .log
            .lines()
            .iter()
            .filter_map(|line| {
                line.value
                    .get("decision")
                    .and_then(|item| item.as_str())
                    .map(str::to_owned)
            })
            .collect()
    }

    #[cfg(test)]
    pub fn test_reasons(&self) -> Vec<String> {
        self.lock()
            .log
            .lines()
            .iter()
            .filter_map(|line| {
                line.value
                    .get("reasons")
                    .and_then(|item| item.as_array())
                    .and_then(|items| items.first())
                    .and_then(|item| item.as_str())
                    .map(str::to_owned)
            })
            .collect()
    }

    #[cfg(test)]
    pub fn log_dir(&self) -> std::path::PathBuf {
        self.lock().log.dir().to_path_buf()
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

/// A held job that may now start thorough.
pub struct HeldReady {
    pub job: LockedThorough,
    pub seq: u64,
    pub granted: bool,
}

/// What the fast phase did with a consumed nonce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FastPhase {
    /// A grant was issued. Thorough runs.
    Provisional,
    /// Measurements were read and `provisional` is off. Thorough runs. No grant.
    Direct,
    /// `not read`. No provisional line. The website stays `pending`.
    Held,
    /// A settled refuse, or a sticky stored outcome. Thorough does not run.
    Done,
    Sticky,
}

fn clock(now: Option<u64>) -> u64 {
    now.unwrap_or_else(unix_now)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn held_bound_detail(
    attempts: u64,
    start_tip: Option<u64>,
    deciding_tip: Option<u64>,
    reverify_max_attempts: u64,
    max_held_blocks: u64,
) -> Option<&'static str> {
    let attempts_hit = attempts >= reverify_max_attempts;
    let blocks_hit = match (start_tip, deciding_tip) {
        (Some(start), Some(tip)) => tip >= start.saturating_add(max_held_blocks),
        _ => false,
    };
    if attempts_hit {
        Some("reverify_max_attempts")
    } else if blocks_hit {
        Some("max_held_blocks")
    } else {
        None
    }
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
