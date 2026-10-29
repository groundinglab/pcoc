//! Session state for the one screen. The nsec stays in memory. Save key is
//! the only write of a key file. Prefs and the public log do not hold it.

use crate::keys_ui::{accept_ui_secret, save_key_file, UiKeyError};
use crate::open_folder;
use crate::produce_run::{self, Job, StartMode, WorkerDone};
use pcoc_windows_core::dm;
use pcoc_windows_core::history::{self, WINDOWS_HISTORY_DISPLAY};
use pcoc_windows_core::keys::SecretKey;
use pcoc_windows_core::signin;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use zeroize::Zeroize;

pub struct StartOpts {
    pub mode: StartMode,
    pub door: String,
    /// `None` uses the default relays. An empty list tries none.
    pub relays: Option<Vec<String>>,
}

impl StartOpts {
    pub fn live() -> Self {
        Self {
            mode: StartMode::Live,
            door: signin::DOOR.to_owned(),
            relays: None,
        }
    }
}

pub struct Session {
    pub nsec_input: String,
    pub save_path: String,
    pub sign_in_link: String,
    pub npub: String,
    pub key_message: String,
    pub status: String,
    pub report: String,
    pub show_history: bool,
    pub history_lines: Vec<String>,
    pub history_override: Option<PathBuf>,
    running: bool,
    cancel: Option<Arc<AtomicBool>>,
    progress: Arc<Mutex<String>>,
    worker: Option<JoinHandle<WorkerDone>>,
}

impl Session {
    pub fn new(history_override: Option<PathBuf>) -> Self {
        Self {
            nsec_input: String::new(),
            save_path: String::new(),
            sign_in_link: String::new(),
            npub: String::new(),
            key_message: String::new(),
            status: "Ready.".to_owned(),
            report: String::new(),
            show_history: false,
            history_lines: Vec::new(),
            history_override,
            running: false,
            cancel: None,
            progress: Arc::new(Mutex::new(String::new())),
            worker: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn history_dir(&self) -> PathBuf {
        history::history_root(self.history_override.as_deref())
    }

    pub fn history_display(&self) -> String {
        match &self.history_override {
            Some(path) => path.display().to_string(),
            None => WINDOWS_HISTORY_DISPLAY.to_owned(),
        }
    }

    /// JSON a prefs file may store. No nsec field.
    pub fn persisted_json(&self) -> String {
        let root = self
            .history_override
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default();
        format!(
            "{{\"history_root\":{}}}\n",
            serde_json::to_string(&root).unwrap_or_else(|_| "\"\"".to_owned())
        )
    }

    /// Words that may be logged. The nsec box is not included.
    pub fn public_log(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}",
            self.npub,
            self.status,
            self.report,
            self.key_message,
            self.history_lines.join("\n")
        )
    }

    pub fn refresh_key(&mut self) {
        match accept_ui_secret(&self.nsec_input) {
            Ok(secret) => {
                self.npub = secret.public_key().npub();
                self.key_message.clear();
            }
            Err(UiKeyError::Empty) => {
                self.npub.clear();
                self.key_message.clear();
            }
            Err(err) => {
                self.npub.clear();
                self.key_message = err.sentence().to_owned();
            }
        }
    }

    pub fn generate(&mut self) {
        let secret = SecretKey::generate();
        self.npub = secret.public_key().npub();
        self.nsec_input = secret.to_nsec().to_string();
        self.key_message =
            "Generated a new key. It stays in memory until you press Save key.".to_owned();
    }

    pub fn save_key(&mut self) {
        let secret = match accept_ui_secret(&self.nsec_input) {
            Ok(secret) => secret,
            Err(err) => {
                self.key_message = err.sentence().to_owned();
                return;
            }
        };
        if self.save_path.trim().is_empty() {
            self.key_message = "Type a path, then press Save key.".to_owned();
            return;
        }
        let path = PathBuf::from(self.save_path.trim());
        match save_key_file(&path, secret.to_nsec().as_str()) {
            Ok(()) => self.key_message = format!("Saved the key to {}.", path.display()),
            Err(err) => self.key_message = format!("Could not save the key: {err}"),
        }
    }

    pub fn start(&mut self, opts: StartOpts) {
        if self.running {
            return;
        }
        self.refresh_key();
        if accept_ui_secret(&self.nsec_input).is_err() {
            if self.key_message.is_empty() {
                self.key_message = UiKeyError::Empty.sentence().to_owned();
            }
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new("Waiting for the next block.".to_owned()));
        let relays = opts
            .relays
            .unwrap_or_else(|| dm::RELAYS.iter().map(|url| (*url).to_owned()).collect());
        let job = Job {
            nsec: zeroize::Zeroizing::new(self.nsec_input.clone()),
            history: self.history_dir(),
            sign_in_link: self.sign_in_link.clone(),
            door: opts.door,
            relays,
            mode: opts.mode,
            cancel: Arc::clone(&cancel),
            progress: Arc::clone(&progress),
        };
        self.progress = progress;
        self.cancel = Some(cancel);
        self.running = true;
        self.report.clear();
        self.status = "Waiting for the next block.".to_owned();
        self.worker = Some(
            std::thread::Builder::new()
                .name("pcoc-produce".into())
                .spawn(move || produce_run::run(job))
                .expect("spawn produce"),
        );
    }

    pub fn stop(&mut self) {
        if let Some(flag) = &self.cancel {
            flag.store(true, Ordering::SeqCst);
            self.status = "Stopping.".to_owned();
        }
    }

    pub fn poll(&mut self) {
        if let Ok(progress) = self.progress.lock() {
            if self.running && !progress.is_empty() && self.status != "Stopping." {
                self.status = progress.clone();
            }
        }
        let finished = self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished());
        if !finished {
            return;
        }
        let Some(worker) = self.worker.take() else {
            return;
        };
        self.running = false;
        self.cancel = None;
        match worker.join() {
            Ok(WorkerDone::Report { status, report }) => {
                self.status = status;
                self.report = report;
            }
            Ok(WorkerDone::Stopped) => {
                self.status = "Stopped. No file was written.".to_owned();
            }
            Ok(WorkerDone::Failed(message)) => {
                self.status = message;
            }
            Err(_) => {
                self.status =
                    "The produce thread stopped unexpectedly. No file was written.".to_owned();
            }
        }
    }

    pub fn toggle_history(&mut self) {
        self.show_history = !self.show_history;
        if self.show_history {
            self.refresh_history();
        }
    }

    pub fn refresh_history(&mut self) {
        self.history_lines = history::list_objects(&self.history_dir())
            .into_iter()
            .map(|path| path.display().to_string())
            .collect();
    }

    pub fn open_history_folder(&mut self) {
        let root = self.history_dir();
        if let Err(err) = std::fs::create_dir_all(&root) {
            self.status = format!("Could not open the folder: {err}");
            return;
        }
        match open_folder(&root) {
            Ok(()) => self.status = format!("Opened {}.", root.display()),
            Err(err) => self.status = format!("Could not open the folder: {err}"),
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(flag) = &self.cancel {
            flag.store(true, Ordering::SeqCst);
        }
        self.nsec_input.zeroize();
        drop(self.worker.take());
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Session")
            .field("npub", &self.npub)
            .field("status", &self.status)
            .field("nsec", &"[redacted]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::success::CLAIM_LINE;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    fn temp_session() -> (Session, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-windows-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (Session::new(Some(dir.clone())), dir)
    }

    #[test]
    fn generate_does_not_write_a_key_until_save_and_logs_omit_it() {
        let (mut session, dir) = temp_session();
        session.generate();
        let nsec = session.nsec_input.clone();
        assert!(nsec.starts_with(pcoc_windows_core::keys::NSEC_PREFIX));
        assert!(session.npub.starts_with("npub1"));
        assert!(!dir.join("pcoc.nsec").exists());
        assert!(!session.public_log().contains(&nsec));
        assert!(!format!("{:?}", session).contains(&nsec));
        assert!(!session.persisted_json().contains(&nsec));
        assert!(!session.persisted_json().contains("nsec"));
        session.save_path = dir.join("pcoc.nsec").display().to_string();
        session.save_key();
        assert_eq!(
            std::fs::read_to_string(dir.join("pcoc.nsec")).unwrap(),
            nsec
        );
        session.nsec_input = "ab".repeat(32);
        session.refresh_key();
        assert!(session.key_message.contains("not accepted"));
        assert!(session.npub.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stop_during_a_tip_wait_writes_no_file() {
        let (mut session, dir) = temp_session();
        session.generate();
        session.start(StartOpts {
            mode: StartMode::TipWait,
            door: "http://127.0.0.1:9/v1/sign-in".into(),
            relays: Some(Vec::new()),
        });
        std::thread::sleep(Duration::from_millis(120));
        let started = Instant::now();
        session.stop();
        loop {
            session.poll();
            if !session.is_running() {
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(3), "stop hung");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            session.status.contains("No file was written"),
            "{}",
            session.status
        );
        assert!(history::list_objects(&dir).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn scripted_finish_posts_once_measures_and_keeps_the_file_when_dm_fails() {
        let (door, server) = spawn_signin();
        let (mut session, dir) = temp_session();
        session.generate();
        session.sign_in_link = format!(
            "https://groundinglab.io/sign-in/?v=1&nonce={}&exp=1893456000",
            "cd".repeat(32)
        );
        session.start(StartOpts {
            mode: StartMode::Scripted,
            door,
            relays: Some(Vec::new()),
        });
        let started = Instant::now();
        loop {
            session.poll();
            if !session.is_running() {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "scripted hike hung"
            );
            std::thread::sleep(Duration::from_millis(30));
        }
        let req = server.join().unwrap();
        assert_eq!(req.matches("POST ").count(), 1, "{req}");
        assert!(req.contains("POST /v1/sign-in "), "{req}");
        assert!(
            req.contains("\"origin\":\"https://groundinglab.io\""),
            "{req}"
        );
        let marker = concat!("/v1/sign-in/", "result");
        assert!(!req.contains(marker), "{req}");
        assert!(session.status.contains("Finished"), "{}", session.status);
        assert!(session.report.contains("Saved:"), "{}", session.report);
        assert!(
            session.report.contains("Measurement record"),
            "{}",
            session.report
        );
        assert!(
            session.report.contains("not a gate decision"),
            "{}",
            session.report
        );
        assert!(
            session.report.contains("Sign-in submitted"),
            "{}",
            session.report
        );
        assert!(session.report.contains("Not sent"), "{}", session.report);
        assert!(session.report.contains(CLAIM_LINE), "{}", session.report);
        assert!(!session.report.to_ascii_lowercase().contains("admitted"));
        assert!(!session.report.to_ascii_lowercase().contains("refused"));
        assert!(!session.report.contains(&session.nsec_input));
        let files = history::list_objects(&dir);
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(std::fs::metadata(&files[0]).unwrap().len() > 0);
        session.toggle_history();
        assert_eq!(session.history_lines.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_bad_link_does_not_post() {
        let (mut session, dir) = temp_session();
        session.generate();
        session.sign_in_link = "not a link".into();
        session.start(StartOpts {
            mode: StartMode::Scripted,
            door: "http://127.0.0.1:1/v1/sign-in".into(),
            relays: Some(Vec::new()),
        });
        wait_idle(&mut session);
        assert!(
            session.report.contains("not a sign-in link"),
            "{}",
            session.report
        );
        assert!(
            !session.report.contains("Sign-in submitted"),
            "{}",
            session.report
        );
        assert!(!history::list_objects(&dir).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn wait_idle(session: &mut Session) {
        let started = Instant::now();
        loop {
            session.poll();
            if !session.is_running() {
                return;
            }
            assert!(started.elapsed() < Duration::from_secs(60));
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    fn spawn_signin() -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let start = Instant::now();
            let mut sock = loop {
                match listener.accept() {
                    Ok((sock, _)) => break sock,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        if start.elapsed() > Duration::from_secs(40) {
                            return "timeout".to_owned();
                        }
                        std::thread::sleep(Duration::from_millis(15));
                    }
                    Err(err) => return format!("accept {err}"),
                }
            };
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 8192];
            let header_end = loop {
                let n = match sock.read(&mut tmp) {
                    Ok(0) | Err(_) => break buf.len(),
                    Ok(n) => n,
                };
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
            };
            let header = String::from_utf8_lossy(&buf[..header_end.min(buf.len())]).to_string();
            let len = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            while buf.len() < header_end + len {
                match sock.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                }
            }
            let req = String::from_utf8_lossy(&buf).to_string();
            let body = b"{\"delivery\":\"ok\"}";
            let ok = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = sock.write_all(ok.as_bytes());
            let _ = sock.write_all(body);
            req
        });
        (format!("http://127.0.0.1:{port}/v1/sign-in"), handle)
    }
}
