//! SPEC §13.1 waypoint POST. Off the walk thread.
//! One resend, 30 seconds after a transport failure, the same bytes, no new solve.
//! Any other outcome is final. 402 is final.

use super::bari_client::{BariResponse, WaypointPost};
use super::events::{self, alert_line};
use super::state::{Run, PHASE_DROPPED, PHASE_PLANTED};
use super::Solver;
use crate::bari_rule::header_hash;
use crate::hashcash::{hashcash_challenge, nonce_decimal};
use crate::hexutil::hex_encode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Background posts. Four workers. The walk's send does not wait.
pub(crate) const POST_WORKERS: usize = 4;
pub(crate) const RESEND_AFTER_MS: u64 = 30_000;

pub(crate) struct PostJob {
    pub stage: usize,
    pub waypoint: [u8; 32],
}

struct Ctx {
    run: Arc<Run>,
    source: super::header::Source,
    client: super::bari_client::Client,
    clock: super::clock::SharedClock,
    origin: String,
    bari_url: String,
    solver: Solver,
    cancel: Arc<AtomicBool>,
}

pub(crate) fn spawn(ctx_run: Arc<Run>, source: super::header::Source, client: super::bari_client::Client, clock: super::clock::SharedClock, origin: String, bari_url: String, solver: Solver, cancel: Arc<AtomicBool>) -> (Sender<PostJob>, Vec<JoinHandle<()>>) {
    let (tx, rx) = mpsc::channel::<PostJob>();
    let rx = Arc::new(Mutex::new(rx));
    let ctx = Arc::new(Ctx {
        run: ctx_run,
        source,
        client,
        clock,
        origin,
        bari_url,
        solver,
        cancel,
    });
    let mut joins = Vec::with_capacity(POST_WORKERS);
    for i in 0..POST_WORKERS {
        let rx = Arc::clone(&rx);
        let ctx = Arc::clone(&ctx);
        let handle = thread::Builder::new()
            .name(format!("pcoc-post-{i}"))
            .spawn(move || worker(rx, ctx))
            .expect("post thread");
        joins.push(handle);
    }
    (tx, joins)
}

fn worker(rx: Arc<Mutex<Receiver<PostJob>>>, ctx: Arc<Ctx>) {
    loop {
        let job = {
            let rx = rx.lock().unwrap();
            match rx.recv() {
                Ok(job) => job,
                Err(_) => return,
            }
        };
        if ctx.cancel.load(Ordering::SeqCst) {
            return;
        }
        handle_job(&ctx, job);
    }
}

fn handle_job(ctx: &Ctx, job: PostJob) {
    let Some(fee) = ctx.source.fee_header() else {
        ctx.run.emit(alert_line("fee_header_missing", job.stage, None));
        set_phase(&ctx.run, job.stage, PHASE_DROPPED);
        return;
    };
    let challenge = hashcash_challenge(&ctx.origin, &header_hash(&fee));
    let Some((nonce, _)) = (ctx.solver)(&challenge, &job.waypoint) else {
        // The named alert examples do not include an exhausted nonce search.
        ctx.run.emit(alert_line("hashcash_exhausted", job.stage, None));
        set_phase(&ctx.run, job.stage, PHASE_DROPPED);
        return;
    };
    let req = WaypointPost {
        url: format!("{}/waypoint", ctx.bari_url),
        body: job.waypoint,
        content_type: "application/octet-stream",
        nonce: nonce_decimal(nonce),
        fee_header_hex: hex_encode(&fee),
    };
    let resp = ctx.client.post_waypoint(&req);
    if ctx.cancel.load(Ordering::SeqCst) {
        log_attempt(&ctx.run, &job, 1, &resp, true);
        set_phase(&ctx.run, job.stage, PHASE_DROPPED);
        return;
    }
    if resp.status() == Some(204) {
        log_attempt(&ctx.run, &job, 1, &resp, false);
        set_phase(&ctx.run, job.stage, PHASE_PLANTED);
        ctx.source.wake();
        return;
    }
    if resp.is_transport_failure() {
        log_attempt(&ctx.run, &job, 1, &resp, false);
        let deadline = ctx.clock.now_ms().saturating_add(RESEND_AFTER_MS);
        ctx.clock.wait_until(deadline, &ctx.cancel);
        if ctx.cancel.load(Ordering::SeqCst) {
            set_phase(&ctx.run, job.stage, PHASE_DROPPED);
            return;
        }
        let resp2 = ctx.client.post_waypoint(&req);
        if resp2.status() == Some(204) {
            log_attempt(&ctx.run, &job, 2, &resp2, false);
            set_phase(&ctx.run, job.stage, PHASE_PLANTED);
            ctx.source.wake();
        } else {
            log_attempt(&ctx.run, &job, 2, &resp2, true);
            set_phase(&ctx.run, job.stage, PHASE_DROPPED);
        }
        return;
    }
    log_attempt(&ctx.run, &job, 1, &resp, true);
    set_phase(&ctx.run, job.stage, PHASE_DROPPED);
}

fn set_phase(run: &Run, stage: usize, phase: u8) {
    run.plants.lock().unwrap()[stage].phase = phase;
}

/// `dropped` is the record's dropped flag. Transport failures omit `status`:
/// 502, 503, and 504 are not a Bari answer (section 13.1).
fn log_attempt(run: &Run, job: &PostJob, attempt: u8, resp: &BariResponse, dropped: bool) {
    if !run.live_log() {
        return;
    }
    let transport = resp.is_transport_failure();
    let status = if transport { None } else { resp.status() };
    let succeeded = resp.status() == Some(204);
    run.emit(events::posted_line(
        job.stage,
        &job.waypoint,
        attempt,
        !transport,
        succeeded,
        dropped,
        status,
    ));
}
