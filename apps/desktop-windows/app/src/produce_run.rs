//! Start runs on a worker thread. Stop sets the cancel flag. A tip wait
//! returns within one wait slice and writes no object.

use crate::finish::{complete, FinishInput};
use crate::keys_ui::accept_ui_secret;
use pcoc::profile::Profile;
use pcoc_produce::bari::{HttpBari, LiveEvent, Progress, SilentBari, RESEND_AFTER};
use pcoc_produce::live::{produce_live, LiveRunOpts, LiveStop};
use pcoc_produce::source::{GossipSource, HeldTip, TipSource};
use pcoc_produce::{cases_rig, Case};
use pcoc_windows_core::endpoints::BARI;
use pcoc_windows_core::history;
use pcoc_windows_core::keys::PublicKey;
use pcoc_windows_core::produce::{produce_and_save, AcceptingWaypointPost};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartMode {
    /// Gossip headers and the production profile. The window's Start control.
    Live,
    /// No header arrives. Stop must return without a file.
    TipWait,
    /// Scripted rig. Finishes without gossip. Used by the harness.
    Scripted,
}

pub struct Job {
    pub nsec: Zeroizing<String>,
    pub history: PathBuf,
    pub sign_in_link: String,
    pub door: String,
    pub relays: Vec<String>,
    pub mode: StartMode,
    pub cancel: Arc<AtomicBool>,
    pub progress: Arc<Mutex<String>>,
}

pub enum WorkerDone {
    Report { status: String, report: String },
    Stopped,
    Failed(String),
}

enum Outcome {
    Object {
        path: PathBuf,
        bytes: Vec<u8>,
        profile: Profile,
    },
    Stopped,
    Failed(String),
}

pub fn run(job: Job) -> WorkerDone {
    if job.cancel.load(Ordering::SeqCst) && job.mode != StartMode::TipWait {
        return WorkerDone::Stopped;
    }
    let secret = match accept_ui_secret(&job.nsec) {
        Ok(secret) => secret,
        Err(err) => return WorkerDone::Failed(err.sentence().to_owned()),
    };
    let public = secret.public_key();
    let outcome = match job.mode {
        StartMode::Live => run_live(&job, &public),
        StartMode::TipWait => run_tip_wait(&job, &public),
        StartMode::Scripted => run_scripted(&job, &public),
    };
    match outcome {
        Outcome::Object {
            path,
            bytes,
            profile,
        } => {
            let relays: Vec<&str> = job.relays.iter().map(String::as_str).collect();
            let report = complete(&FinishInput {
                saved_path: &path,
                bytes: &bytes,
                profile: &profile,
                secret: &secret,
                sign_in_link: &job.sign_in_link,
                door: &job.door,
                relays: &relays,
            });
            WorkerDone::Report {
                status: "Finished.".to_owned(),
                report,
            }
        }
        Outcome::Stopped => WorkerDone::Stopped,
        Outcome::Failed(message) => WorkerDone::Failed(message),
    }
}

fn run_live(job: &Job, public: &PublicKey) -> Outcome {
    set_progress(&job.progress, "Waiting for the next block.");
    let profile = Profile::production();
    let hex = public.hex();
    let path = job.history.join(history::object_file_name(&hex));
    let source = GossipSource::spawn("", Arc::clone(&job.cancel));
    let progress = progress_cb(&job.progress);
    let bari = match HttpBari::open(BARI, RESEND_AFTER, source.tip_fn(), Arc::clone(&progress)) {
        Ok(bari) => bari,
        Err(message) => return Outcome::Failed(message),
    };
    let opts = LiveRunOpts {
        public_key: &public.0,
        profile: &profile,
        out: &path,
        cancel: &job.cancel,
    };
    let product = produce_live(&opts, &source, &bari, &progress);
    drop(source);
    match product {
        Ok(product) => match history::save_object(&job.history, &hex, &product.bytes) {
            Ok(saved) => Outcome::Object {
                path: saved,
                bytes: product.bytes,
                profile,
            },
            Err(err) => Outcome::Failed(format!("Could not write the object: {err}")),
        },
        Err(LiveStop::Cancelled) => Outcome::Stopped,
        Err(LiveStop::Failed(message)) => Outcome::Failed(message),
    }
}

fn run_tip_wait(job: &Job, public: &PublicKey) -> Outcome {
    set_progress(&job.progress, "Waiting for the next block.");
    let profile = tiny_profile();
    let path = job.history.join(history::object_file_name(&public.hex()));
    let source = NeverTip;
    let bari = SilentBari;
    let progress: Progress = Arc::new(|_| {});
    let opts = LiveRunOpts {
        public_key: &public.0,
        profile: &profile,
        out: &path,
        cancel: &job.cancel,
    };
    match produce_live(&opts, &source, &bari, &progress) {
        Err(LiveStop::Cancelled) if !path.exists() => Outcome::Stopped,
        Err(LiveStop::Cancelled) => Outcome::Failed("Stop wrote a file.".to_owned()),
        Ok(_) => Outcome::Failed("A tip wait produced an object.".to_owned()),
        Err(LiveStop::Failed(message)) => Outcome::Failed(message),
    }
}

fn run_scripted(job: &Job, public: &PublicKey) -> Outcome {
    if job.cancel.load(Ordering::SeqCst) {
        return Outcome::Stopped;
    }
    set_progress(&job.progress, "Recording a short hike.");
    let profile = tiny_profile();
    let rig = cases_rig(Case::S1);
    match produce_and_save(
        &public.0,
        &rig,
        &profile,
        &AcceptingWaypointPost,
        &job.history,
    ) {
        Ok(result) => Outcome::Object {
            path: result.saved,
            bytes: result.bytes,
            profile,
        },
        Err(message) if message.contains("cancelled") => Outcome::Stopped,
        Err(message) => Outcome::Failed(message),
    }
}

fn tiny_profile() -> Profile {
    Profile::conformance_small(32, 64, 65_536, 1).expect("profile")
}

fn set_progress(slot: &Mutex<String>, line: &str) {
    *slot.lock().unwrap_or_else(|poison| poison.into_inner()) = line.to_owned();
}

fn progress_cb(slot: &Arc<Mutex<String>>) -> Progress {
    let slot = Arc::clone(slot);
    Arc::new(move |event: LiveEvent| {
        let line = match event {
            LiveEvent::Terrain => "Terrain is filled. Waiting for the next block.".to_owned(),
            LiveEvent::Leg { stage, leg, .. } => format!(
                "Stage {} of 18, leg {leg}. Waiting for the next block.",
                stage + 1
            ),
            LiveEvent::Interrupt { stage, height } => {
                format!("Stage {} closed at height {height}.", stage + 1)
            }
            LiveEvent::Header { height, .. } => format!("Saw a block at height {height}."),
            LiveEvent::Waypoint { stage, .. } => format!("Waypoint for stage {stage} is queued."),
            LiveEvent::Posted {
                stage, succeeded, ..
            } => {
                if succeeded {
                    format!("Waypoint post for stage {stage} was accepted. The hike keeps going.")
                } else {
                    format!(
                        "Waypoint post for stage {stage} was not accepted. The hike keeps going."
                    )
                }
            }
            LiveEvent::Alert { name, .. } => format!("Notice: {name}."),
            LiveEvent::Closed => "The hike closed.".to_owned(),
            LiveEvent::Fetch { .. } | LiveEvent::Checked { .. } => return,
        };
        set_progress(&slot, &line);
    })
}

struct NeverTip;

impl TipSource for NeverTip {
    fn tip(&self) -> Option<HeldTip> {
        None
    }
    fn failed(&self) -> Option<String> {
        None
    }
    fn drain_learned(&self) -> Vec<HeldTip> {
        Vec::new()
    }
    fn wait_for_tip(&self, timeout: Duration) -> bool {
        std::thread::sleep(timeout.min(Duration::from_millis(50)));
        false
    }
}
