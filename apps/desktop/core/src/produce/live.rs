//! Live produce fails closed.
//!
//! Phase C owns the live header source. This build does not dial Bitcoin
//! gossip. The error text is the same sentence the `pcoc` CLI prints when
//! produce is started without `--rig`.

use super::events::ProduceEvent;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

pub type Emit = Arc<dyn Fn(ProduceEvent) + Send + Sync>;

/// The sentence returned to the caller and shown in the UI.
pub const LIVE_HEADER_SOURCE: &str = "live header source is not in this build";

#[derive(Clone, Debug)]
pub struct LiveConfig {
    pub public_key: [u8; 32],
    pub output: PathBuf,
    /// Kept so a later phase can name a peer. This build does not dial it.
    pub gossip_peer: Option<String>,
}

pub struct LiveRun {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl LiveRun {
    /// Ask the run to stop. The run has already failed closed; nothing is written.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    pub fn finished(&self) -> bool {
        self.thread.as_ref().is_none_or(|t| t.is_finished())
    }
}

pub fn start(config: LiveConfig, emit: Emit) -> LiveRun {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("pcoc-produce".into())
        .spawn(move || run(config, &thread_stop, emit))
        .expect("spawn produce thread");
    LiveRun {
        stop,
        thread: Some(thread),
    }
}

fn run(_config: LiveConfig, _stop: &AtomicBool, emit: Emit) {
    emit(ProduceEvent::Failed {
        message: LIVE_HEADER_SOURCE.to_owned(),
    });
}

/// Temporary sibling, sync, then rename. A failure leaves no partial object.
pub fn write_atomically(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn live_produce_fails_closed_without_a_header_source() {
        let (tx, rx) = std::sync::mpsc::channel();
        let emit: Emit = Arc::new(move |event| {
            let _ = tx.send(event);
        });
        let run = start(
            LiveConfig {
                public_key: [0x11; 32],
                output: PathBuf::from("/tmp/pcoc-live-not-written.cbor"),
                gossip_peer: Some("192.0.2.7:8333".into()),
            },
            emit,
        );
        let event = rx.recv_timeout(Duration::from_secs(5)).expect("event");
        match event {
            ProduceEvent::Failed { message } => assert_eq!(message, LIVE_HEADER_SOURCE),
            other => panic!("expected the live-source error, got {other:?}"),
        }
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        for _ in 0..50 {
            if run.finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(run.finished());
        assert!(!Path::new("/tmp/pcoc-live-not-written.cbor").exists());
    }

    #[test]
    fn atomic_write_leaves_only_the_object() {
        let dir = std::env::temp_dir().join(format!("pcoc-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("hike.cbor");
        write_atomically(&dest, b"abc").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"abc");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
