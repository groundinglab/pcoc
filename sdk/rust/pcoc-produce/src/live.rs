//! SPEC §13 live hike.
//!
//! The walk calls [`TipSource::tip`] after each checkpoint. That call returns
//! the header already held. Posts, Hashcash, path asks, and the event log run
//! off the walk (Rule A6, Rule I3). A fail-closed source stops the run before
//! any object is written. Resume is not supported: the checkpoint file is a
//! crash report, and a later run starts over.

use crate::bari::{block_log_line, event_log_line, BariHub, LiveEvent, PostRequest, Progress};
use crate::source::TipSource;
use pcoc::checkpoint::checkpoint;
use pcoc::constants::{FILL, STAGES};
use pcoc::object::{make_object, Stage};
use pcoc::profile::Profile;
use pcoc::step::leg_result;
use pcoc::terrain::terrain_from_seed;
use pcoc::waypoint::{fold_checkpoints, waypoint};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

pub struct LiveRunOpts<'a> {
    pub public_key: &'a [u8; 32],
    pub profile: &'a Profile,
    pub out: &'a Path,
    pub cancel: &'a AtomicBool,
}

#[derive(Debug)]
pub enum LiveStop {
    Cancelled,
    Failed(String),
}

#[derive(Debug)]
pub struct LiveProduct {
    pub bytes: Vec<u8>,
    /// Rule A3.4 lines. The caller prints them after the object is in place.
    pub stderr: String,
}

struct Sink {
    tx: Option<Sender<String>>,
    thread: Option<JoinHandle<()>>,
}

impl Sink {
    fn send(&self, line: impl Into<String>) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(line.into());
        }
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn event_sink(path: PathBuf) -> Sink {
    let file = match std::fs::File::create(&path) {
        Ok(file) => file,
        Err(_) => {
            return Sink {
                tx: None,
                thread: None,
            }
        }
    };
    let (tx, rx) = mpsc::channel::<String>();
    let thread = std::thread::Builder::new()
        .name("pcoc-events".into())
        .spawn(move || {
            let mut file = file;
            while let Ok(line) = rx.recv() {
                let _ = writeln!(file, "{line}");
            }
        })
        .ok();
    Sink {
        tx: thread.is_some().then_some(tx),
        thread,
    }
}

fn checkpoint_sink(path: PathBuf) -> Sink {
    let (tx, rx) = mpsc::channel::<String>();
    let thread = std::thread::Builder::new()
        .name("pcoc-checkpoint".into())
        .spawn(move || {
            while let Ok(body) = rx.recv() {
                let _ = std::fs::write(&path, body);
            }
        })
        .ok();
    Sink {
        tx: thread.is_some().then_some(tx),
        thread,
    }
}

fn checkpoint_body(stage: usize, height: u64, legs: u64, closed: bool) -> String {
    format!(
        "{{\"stage\":{stage},\"height\":{height},\"legs\":{legs},\"closed\":{closed},\"resume\":false}}\n"
    )
}

fn emit(progress: &Progress, log: &Sink, event: LiveEvent) {
    if let Some(line) = event_log_line(&event) {
        log.send(line);
    }
    progress(event);
}

fn note_header(progress: &Progress, log: &Sink, bari: &dyn BariHub, held: &crate::source::HeldTip) {
    log.send(block_log_line(held.height, &held.hash));
    bari.enqueue_header(held.height);
    progress(LiveEvent::Header {
        height: held.height,
        hash: held.hash,
        n_time: held.n_time,
    });
}

/// Walk one live hike. Returns the object bytes. Does not write them.
/// Writes `{out}.events` and `{out}.checkpoint` when the run gets past the
/// fail-closed check. The checkpoint says `"resume": false`.
pub fn produce_live(
    opts: &LiveRunOpts<'_>,
    source: &dyn TipSource,
    bari: &dyn BariHub,
    progress: &Progress,
) -> Result<LiveProduct, LiveStop> {
    if opts.cancel.load(Ordering::SeqCst) {
        return Err(LiveStop::Cancelled);
    }
    if let Some(message) = source.failed() {
        return Err(LiveStop::Failed(message));
    }
    let seed = loop {
        if opts.cancel.load(Ordering::SeqCst) {
            return Err(LiveStop::Cancelled);
        }
        if let Some(message) = source.failed() {
            return Err(LiveStop::Failed(message));
        }
        if let Some(tip) = source.tip() {
            break tip;
        }
        source.wait_for_tip(std::time::Duration::from_millis(50));
    };

    let events_path = PathBuf::from(format!("{}.events", opts.out.display()));
    let checkpoint_path = PathBuf::from(format!("{}.checkpoint", opts.out.display()));
    let log = event_sink(events_path);
    let points = checkpoint_sink(checkpoint_path);
    note_header(progress, &log, bari, &seed);
    points.send(checkpoint_body(0, seed.height, 0, false));

    let (terrain, mut limb, _) = terrain_from_seed(opts.public_key, &seed.hash, opts.profile);
    emit(progress, &log, LiveEvent::Terrain);
    if opts.cancel.load(Ordering::SeqCst) {
        return Err(LiveStop::Cancelled);
    }

    let mut beacon = seed.hash;
    let mut beacon_height = seed.height;
    let mut stages: Vec<Stage> = Vec::with_capacity(STAGES);
    let mut last_interrupt = [0u8; 32];
    let mut last_interrupt_height = 0u64;

    for g in 0..STAGES {
        if opts.cancel.load(Ordering::SeqCst) {
            return Err(LiveStop::Cancelled);
        }
        let stage_limb = limb;
        let mut previous = stage_limb;
        let mut cps: Vec<[u8; 32]> = Vec::new();
        let mut lrs: Vec<u64> = Vec::new();
        let interrupt = loop {
            if opts.cancel.load(Ordering::SeqCst) {
                return Err(LiveStop::Cancelled);
            }
            if let Some(message) = source.failed() {
                // A terminal source error before any checkpoint of this stage,
                // and with no header yet, is the fail-closed arm. After the
                // seed, a later terminal error is an alert and the walk stops
                // only when the caller cancelled or the source says it failed
                // closed. The absent-source sentence is that failure.
                return Err(LiveStop::Failed(message));
            }
            for name in source.take_alerts() {
                emit(
                    progress,
                    &log,
                    LiveEvent::Alert {
                        name,
                        stage: g,
                        height: None,
                    },
                );
            }
            let lr = leg_result(&terrain, opts.profile, &previous);
            let cp = checkpoint(&previous, &stage_limb, &terrain, opts.profile, lr);
            cps.push(cp);
            lrs.push(lr);
            previous = cp;
            emit(
                progress,
                &log,
                LiveEvent::Leg {
                    stage: g,
                    leg: cps.len() - 1,
                    leg_result: lr,
                },
            );
            for held in source.drain_learned() {
                note_header(progress, &log, bari, &held);
            }
            let Some(held) = source.tip() else {
                emit(
                    progress,
                    &log,
                    LiveEvent::Alert {
                        name: "clock_read_failed".into(),
                        stage: g,
                        height: None,
                    },
                );
                continue;
            };
            if held.hash != beacon && held.height > beacon_height {
                break held;
            }
        };

        stages.push(Stage {
            beacon,
            beacon_height,
            height_over: false,
            leg_count: cps.len() as u64,
            waypoint: stage_limb,
            path: if g == 0 { None } else { Some(Vec::new()) },
            checkpoints: cps.iter().map(|c| c.to_vec()).collect(),
            leg_results: lrs.iter().map(|r| r.to_le_bytes().to_vec()).collect(),
            usable: true,
        });
        points.send(checkpoint_body(
            g,
            interrupt.height,
            cps.len() as u64,
            false,
        ));
        emit(
            progress,
            &log,
            LiveEvent::Interrupt {
                stage: g,
                height: interrupt.height,
            },
        );

        if g + 1 == STAGES {
            last_interrupt = interrupt.hash;
            last_interrupt_height = interrupt.height;
        } else {
            let fold = fold_checkpoints(&cps);
            limb = waypoint(&interrupt.hash, &terrain, opts.profile, &stage_limb, &fold);
            emit(
                progress,
                &log,
                LiveEvent::Waypoint {
                    stage: g + 1,
                    waypoint: limb,
                },
            );
            bari.enqueue_post(PostRequest {
                stage: g + 1,
                waypoint: limb,
                beacon_height: interrupt.height,
            });
            beacon = interrupt.hash;
            beacon_height = interrupt.height;
        }
    }

    for (stage, slot) in stages.iter_mut().enumerate().skip(1) {
        if let Some(path) = bari.path(stage) {
            slot.path = Some(path);
        }
    }
    let stderr = stderr_lines(&stages);
    let obj = make_object(
        FILL,
        opts.profile.slot_bytes as u64,
        opts.profile.n_slots,
        opts.profile.sample_bytes as u64,
        opts.profile.steps_per_leg,
        *opts.public_key,
        seed.hash,
        stages,
        last_interrupt,
        last_interrupt_height,
    );
    points.send(checkpoint_body(STAGES - 1, last_interrupt_height, 0, true));
    emit(progress, &log, LiveEvent::Closed);
    let bytes = pcoc::encode_object(&obj);
    drop(log);
    drop(points);
    Ok(LiveProduct { bytes, stderr })
}

fn stderr_lines(stages: &[Stage]) -> String {
    let mut out = String::new();
    for stage in 1..=16 {
        let empty = stages
            .get(stage)
            .and_then(|stage| stage.path.as_ref())
            .is_none_or(|path| path.is_empty());
        if empty {
            out.push_str("path_skip ");
            out.push_str(&stage.to_string());
            out.push('\n');
        }
    }
    out.push_str("not fetched by design 17\n");
    out
}

/// Temporary sibling, sync, then rename. A failure leaves no partial object.
pub fn write_atomically(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = match dest.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = dest
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"))?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, dest)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
