//! Application state. Lives on the GTK main thread. Worker threads send
//! events over channels and never touch widgets.
//!
//! The nsec, when there is one, lives only in `secret`. It is never written.

use pcoc_hike_core::config::Config;
use pcoc_hike_core::keys::SecretKey;
use pcoc_hike_core::library::Entry;
use pcoc_hike_core::produce::live::LiveRun;
use pcoc_hike_core::produce::model::ProduceView;
use pcoc_hike_core::signin::{Challenge, NotALink};
use pcoc_hike_core::verify_job::VerifyRun;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Produce,
    Verify,
    SignIn,
    Settings,
}

impl Page {
    pub const ALL: [Page; 4] = [Page::Produce, Page::Verify, Page::SignIn, Page::Settings];

    pub fn name(self) -> &'static str {
        match self {
            Page::Produce => "produce",
            Page::Verify => "verify",
            Page::SignIn => "sign-in",
            Page::Settings => "settings",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Produce => "Produce",
            Page::Verify => "Verify",
            Page::SignIn => "Sign-in",
            Page::Settings => "Settings",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Page::Produce => "network-cellular-symbolic",
            Page::Verify => "document-properties-symbolic",
            Page::SignIn => "channel-secure-symbolic",
            Page::Settings => "emblem-system-symbolic",
        }
    }

    pub fn from_name(name: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.name() == name)
    }
}

#[derive(Clone, Debug, Default)]
pub enum VerifyState {
    #[default]
    Empty,
    /// Picked from the hike list or a file dialog, not started.
    Selected(PathBuf),
    Running(PathBuf),
    /// A bad or old file. Idle, with one plain error. Verify stays usable.
    IdleError {
        path: PathBuf,
        message: String,
    },
    Done(Box<VerifyRun>),
}

#[derive(Clone, Debug, Default)]
pub enum SignInState {
    #[default]
    Empty,
    Bad(NotALink),
    Parsed(Challenge),
    /// The one press happened. The POST is in flight.
    Sending,
    /// The form is replaced by this sentence.
    Done(String),
}

pub struct State {
    pub config: Config,
    /// Session nsec. Memory only. Dropped on exit.
    pub secret: Option<SecretKey>,
    /// A bad nsec, or a start that could not make a run folder.
    pub key_error: Option<String>,
    pub library: Vec<Entry>,
    pub produce: ProduceView,
    pub produce_run: Option<LiveRun>,
    pub produce_output: Option<PathBuf>,
    /// A finished hike opened from the left column. Absent while setup or live is showing.
    pub result_path: Option<PathBuf>,
    pub verify: VerifyState,
    pub sign_in: SignInState,
    pub sign_in_text: String,
    /// The PCOC a sign-in will sign. A file path, not a secret.
    pub sign_in_path: Option<PathBuf>,
    /// Test admission on Produce.
    pub test_note: Option<String>,
    pub test_busy: bool,
    pub page: Page,
    pub narrow: bool,
    /// The sample build pins the clock to the design's moment.
    pub fixed_now_ms: Option<u64>,
    pub sample: bool,
}

impl State {
    pub fn now_ms(&self) -> u64 {
        self.fixed_now_ms
            .unwrap_or_else(pcoc_hike_core::format::now_unix_ms)
    }

    pub fn now_s(&self) -> u64 {
        self.now_ms() / 1000
    }

    pub fn learn(&self) -> bool {
        self.config.learn_mode
    }

    /// The sample build reads this. The shipping screens keep the secret on `secret`.
    #[cfg_attr(not(feature = "sample"), allow(dead_code))]
    pub fn active_public_key(&self) -> Option<[u8; 32]> {
        self.secret.as_ref().map(|key| key.public_key().0)
    }

    /// The PCOC a test admission submits: the one just written, else the newest hike.
    pub fn admission_target(&self) -> Option<PathBuf> {
        if let Some(object) = &self.produce.object {
            return Some(object.path.clone());
        }
        self.library.first().map(|entry| entry.path.clone())
    }

    pub fn save_config(&mut self) {
        if self.sample {
            return;
        }
        if let Err(err) = self.config.save(&pcoc_hike_core::config::default_path()) {
            eprintln!("pcoc-hike: config: {err}");
        }
    }

    pub fn rescan(&mut self) {
        if self.sample {
            return;
        }
        self.library = pcoc_hike_core::library::scan(&self.config.data_folder);
    }
}
