//! In-memory verify jobs.
//!
//! Each job is one call through the verifier seam. The seam does not judge.
//! A v1.35 object finishes as `earlier_version` and is not measured as a
//! current object. Sign-in thorough uses the same slot limit.

use crate::config::Config;
use crate::ids::IdGen;
use crate::policy::{self, EARLIER_LINE, REFUSE_EARLIER, UNREADABLE_LINE, UNREADABLE_REASON};
use crate::verifier::{self, Report};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JobState {
    Queued,
    Running,
    Done,
    Held,
    Error,
}

impl JobState {
    fn as_str(self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Running => "running",
            JobState::Done => "done",
            JobState::Held => "held",
            JobState::Error => "error",
        }
    }

    fn is_terminal(self) -> bool {
        matches!(self, JobState::Done | JobState::Held | JobState::Error)
    }
}

struct Job {
    id: String,
    state: JobState,
    created_at_ms: u64,
    updated_at_ms: u64,
    terminal_at: Option<Instant>,
    /// Section 14 record from `verify_record_json`, including the trailing
    /// newline. Absent until the job has measured a v2.0 object.
    record: Option<String>,
    reasons: Option<Vec<String>>,
    reason: Option<String>,
    error: Option<(&'static str, String)>,
}

impl Job {
    fn touch(&mut self) {
        self.updated_at_ms = now_ms();
        if self.state.is_terminal() && self.terminal_at.is_none() {
            self.terminal_at = Some(Instant::now());
        }
    }

    fn to_json_string(&self) -> String {
        // Splice the record bytes in. Re-encoding would drop the trailing
        // newline `verify_record_json` writes, and `Map::remove` would move
        // `analysis`. That newline is part of the record.
        let head = JobHead {
            job_id: &self.id,
            status: self.state.as_str(),
            terminal: self.state.is_terminal(),
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
        };
        let mut text = serde_json::to_string(&head).expect("job head");
        text.pop();
        if let Some(record) = &self.record {
            text.push_str(",\"record\":");
            text.push_str(record);
        }
        if let Some(reasons) = &self.reasons {
            text.push_str(",\"reasons\":");
            text.push_str(&serde_json::to_string(reasons).expect("reasons"));
        }
        if let Some(reason) = &self.reason {
            text.push_str(",\"reason\":");
            text.push_str(&serde_json::to_string(reason).expect("reason"));
        }
        if let Some((code, message)) = &self.error {
            let err = serde_json::to_string(&JobError {
                code,
                message: message.as_str(),
            })
            .expect("job error");
            text.push_str(",\"error\":");
            text.push_str(&err);
        }
        text.push('}');
        text
    }
}

#[derive(Serialize)]
struct JobHead<'a> {
    job_id: &'a str,
    status: &'static str,
    terminal: bool,
    created_at_ms: u64,
    updated_at_ms: u64,
}

#[derive(Serialize)]
struct JobError<'a> {
    code: &'a str,
    message: &'a str,
}

#[derive(Debug)]
pub enum EnqueueError {
    Busy,
}

#[derive(Clone)]
pub struct JobStore {
    cfg: Arc<Config>,
    jobs: Arc<Mutex<HashMap<String, Job>>>,
    ids: Arc<IdGen>,
    slots: Arc<Semaphore>,
}

impl JobStore {
    pub fn new(cfg: Arc<Config>) -> Self {
        let slots = Arc::new(Semaphore::new(cfg.max_concurrent_verify));
        Self {
            cfg,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            ids: Arc::new(IdGen::new()),
            slots,
        }
    }

    pub fn enqueue(&self, object: Vec<u8>) -> Result<String, EnqueueError> {
        let id = self.ids.next_id();
        {
            let mut jobs = self.jobs.lock().expect("job map");
            self.evict_locked(&mut jobs);
            if jobs.len() >= self.cfg.max_jobs {
                return Err(EnqueueError::Busy);
            }
            let now = now_ms();
            jobs.insert(
                id.clone(),
                Job {
                    id: id.clone(),
                    state: JobState::Queued,
                    created_at_ms: now,
                    updated_at_ms: now,
                    terminal_at: None,
                    record: None,
                    reasons: None,
                    reason: None,
                    error: None,
                },
            );
        }

        let store = self.clone();
        let job_id = id.clone();
        tokio::spawn(async move {
            store.mark(&job_id, |job| job.state = JobState::Running);
            let report = store.measure(object).await;
            match report {
                Ok(report) => store.finish(&job_id, report),
                Err(message) => store.finish_error(&job_id, "worker_panic", message),
            }
        });

        Ok(id)
    }

    /// One verifier call on the shared slot. Sign-in thorough uses this too.
    pub async fn measure(&self, object: Vec<u8>) -> Result<Report, String> {
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "verify slot semaphore closed".to_owned())?;
        let opts = verifier::MeasureOpts {
            index_url: self.cfg.index_url.clone(),
            timeout: self.cfg.index_timeout,
            bind_threads: self.cfg.bind_threads,
        };
        let joined = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            verifier::measure(&object, &opts)
        })
        .await;
        joined.map_err(|err| format!("verify worker failed: {err}"))
    }

    pub fn snapshot(&self, job_id: &str) -> Option<String> {
        let mut jobs = self.jobs.lock().expect("job map");
        self.evict_locked(&mut jobs);
        jobs.get(job_id).map(Job::to_json_string)
    }

    pub fn log_dir(&self) -> std::path::PathBuf {
        self.cfg.log_dir.clone()
    }

    pub fn counts(&self) -> (usize, usize) {
        let jobs = self.jobs.lock().expect("job map");
        let live = jobs.values().filter(|job| !job.state.is_terminal()).count();
        (jobs.len(), live)
    }

    fn finish(&self, job_id: &str, report: Report) {
        match report {
            Report::Earlier { .. } => {
                self.finish_error(job_id, REFUSE_EARLIER, EARLIER_LINE.to_owned())
            }
            Report::Unreadable => {
                self.finish_error(job_id, UNREADABLE_REASON, UNREADABLE_LINE.to_owned())
            }
            Report::Read(measured) => {
                let judgment = policy::judge(
                    &Report::Read(measured.clone()),
                    self.cfg.policy.leg_rate_floor.p,
                    self.cfg.policy.leg_rate_floor.q,
                );
                let record = measured.record;
                match judgment {
                    policy::Judgment::Admit { .. } => {
                        self.mark(job_id, |job| {
                            job.state = JobState::Done;
                            job.record = Some(record);
                        });
                    }
                    policy::Judgment::Held { .. } => {
                        self.mark(job_id, |job| {
                            job.state = JobState::Held;
                            job.record = Some(record);
                        });
                    }
                    policy::Judgment::Refuse { reasons, line, .. } => {
                        // The verify page shows `error.message`.
                        let code = static_code(&reasons);
                        self.mark(job_id, |job| {
                            job.state = JobState::Error;
                            job.record = Some(record);
                            job.reasons = Some(reasons);
                            job.reason = Some(line.clone());
                            job.error = Some((code, line));
                        });
                    }
                }
            }
        }
    }

    fn finish_error(&self, job_id: &str, code: &'static str, message: String) {
        let line = message.clone();
        eprintln!("admission-service verify job={job_id} code={code}");
        self.mark(job_id, |job| {
            job.state = JobState::Error;
            job.reasons = Some(vec![code.to_owned()]);
            job.reason = Some(line);
            job.error = Some((code, message));
        });
    }

    fn mark(&self, job_id: &str, edit: impl FnOnce(&mut Job)) {
        let mut jobs = self.jobs.lock().expect("job map");
        if let Some(job) = jobs.get_mut(job_id) {
            edit(job);
            job.touch();
        }
    }

    fn evict_locked(&self, jobs: &mut HashMap<String, Job>) {
        let retention = self.cfg.job_retention;
        jobs.retain(|_, job| match job.terminal_at {
            Some(at) => at.elapsed() < retention,
            None => true,
        });
    }
}

fn static_code(reasons: &[String]) -> &'static str {
    if reasons.iter().any(|reason| reason == REFUSE_EARLIER) {
        REFUSE_EARLIER
    } else if reasons
        .iter()
        .any(|reason| reason == policy::REFUSE_BOOKENDS)
    {
        policy::REFUSE_BOOKENDS
    } else if reasons
        .iter()
        .any(|reason| reason == policy::REFUSE_LEG_RATE)
    {
        policy::REFUSE_LEG_RATE
    } else if reasons.iter().any(|reason| reason == UNREADABLE_REASON) {
        UNREADABLE_REASON
    } else {
        "refused"
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}
