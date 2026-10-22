//! PCOC Hike for Linux. GTK4 and libadwaita, with the PCOC Rust SDK
//! in-process (DESIGN-v2 section 10).
#![forbid(unsafe_code)]

mod pages;
mod state;
mod style;
mod ui;
mod widgets;

use adw::prelude::*;
use pcoc_hike_core::config::{self, Config};
use state::{Page, SignInState, State, VerifyState};

fn main() -> gtk::glib::ExitCode {
    let application = adw::Application::builder()
        .application_id(pcoc_hike_core::about::APP_ID)
        .build();
    application.connect_startup(|_| {
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_label_select_on_focus(false);
        }
    });
    application.connect_activate(|application| {
        if let Some(window) = application.active_window() {
            window.present();
            return;
        }
        let app = ui::App::new(application, initial_state());
        // Widgets hold weak references to the App. The window owns the
        // one strong reference until it is destroyed.
        let owner = std::cell::RefCell::new(Some(app.clone()));
        app.window.connect_destroy(move |_| drop(owner.take()));
        app.window.present();
    });
    application.connect_shutdown(|application| {
        for window in application.windows() {
            window.destroy();
        }
    });
    application.run_with_args::<&str>(&[])
}

fn blank_state(config: Config) -> State {
    State {
        config,
        secret: None,
        key_error: None,
        library: Vec::new(),
        produce: Default::default(),
        produce_run: None,
        produce_output: None,
        result_path: None,
        verify: VerifyState::Empty,
        sign_in: SignInState::Empty,
        sign_in_text: String::new(),
        sign_in_path: None,
        test_note: None,
        test_busy: false,
        phone_note: None,
        phone_busy: false,
        page: Page::Produce,
        narrow: false,
        fixed_now_ms: None,
        sample: false,
    }
}

fn initial_state() -> State {
    #[cfg(feature = "sample")]
    if let Some(state) = sample::state() {
        return state;
    }
    let config = Config::load(&config::default_path());
    let _ = std::fs::create_dir_all(&config.data_folder);
    let mut state = blank_state(config);
    state.rescan();
    if let Some(page) = std::env::var("PCOC_PAGE")
        .ok()
        .as_deref()
        .and_then(Page::from_name)
    {
        state.page = page;
    }
    state
}

/// The DESIGN-v2 section 2 sample, for the screenshot run. Selected with
/// `PCOC_SAMPLE=r|f`. Nothing in this mode writes the config or sends.
#[cfg(feature = "sample")]
mod sample {
    use super::{blank_state, Page, SignInState, State, VerifyState};
    use pcoc_hike_core::config::{Config, Theme};
    use pcoc_hike_core::keys::SecretKey;
    use pcoc_hike_core::library::{Entry, Kind};
    use pcoc_hike_core::produce::model::ProduceView;
    use pcoc_hike_core::sample as s;
    use pcoc_hike_core::signin;

    fn env(name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }

    pub fn state() -> Option<State> {
        let moment_f = match env("PCOC_SAMPLE")?.as_str() {
            "r" => false,
            "f" => true,
            _ => return None,
        };
        let page = env("PCOC_PAGE")
            .as_deref()
            .and_then(Page::from_name)
            .unwrap_or(Page::Produce);
        let config = Config {
            theme: Some(if env("PCOC_THEME").as_deref() == Some("light") {
                Theme::Light
            } else {
                Theme::Dark
            }),
            learn_mode: env("PCOC_LEARN").as_deref() != Some("off"),
            ..Config::default()
        };
        let mut state = blank_state(config);
        if env("PCOC_KEYS").as_deref() == Some("unlocked") {
            state.secret = Some(SecretKey::generate());
        }
        state.sample = true;
        state.page = page;
        let active = state.active_public_key();

        let mut produce = ProduceView::default();
        for e in if moment_f {
            s::moment_f()
        } else {
            s::moment_r()
        } {
            produce.apply(e);
        }
        state.produce = produce;

        let other = [0x11u8; 32];
        let mut library = Vec::new();
        if moment_f {
            library.push(entry(
                s::object_path(),
                s::OBJECT_BYTES,
                s::sha256(),
                s::WRITTEN_S,
                Kind::V2,
                active,
                Some(3_559),
            ));
        }
        let hikes = s::object_path().parent().unwrap().to_owned();
        library.push(entry(
            hikes.join("hike-2026-10-06-2131.cbor"),
            172_204,
            [0x21; 32],
            1_791_322_260,
            Kind::V2,
            active,
            Some(3_212),
        ));
        library.push(entry(
            hikes.join("alice-hike-1004.cbor"),
            160_998,
            [0x31; 32],
            1_791_101_000,
            Kind::V2,
            Some(other),
            Some(2_904),
        ));
        library.push(entry(
            hikes.join("run-b-stages18.cbor"),
            98_310,
            [0x41; 32],
            1_790_900_000,
            Kind::Unknown("not-v2.2-pcoc-object".into()),
            None,
            None,
        ));
        state.library = library;

        let now_s = match page {
            Page::Verify => s::VERIFIED_S,
            Page::SignIn => s::ADMISSION_NOW_S,
            _ if moment_f => s::NOW_F_S,
            _ => s::NOW_R_S,
        };
        state.fixed_now_ms = Some(now_s * 1000);
        if moment_f {
            state.verify = VerifyState::Done(Box::new(s::verify_run()));
            let link = s::admission_link();
            state.sign_in = SignInState::Parsed(signin::parse_link(&link).expect("sample link"));
            state.sign_in_text = link;
            state.sign_in_path = Some(s::object_path());
        }
        Some(state)
    }

    fn entry(
        path: std::path::PathBuf,
        bytes: u64,
        sha256: [u8; 32],
        modified_unix_s: u64,
        kind: Kind,
        public_key: Option<[u8; 32]>,
        legs: Option<u64>,
    ) -> Entry {
        Entry {
            path,
            run_name: String::new(),
            bytes,
            sha256,
            modified_unix_s,
            kind,
            public_key,
            legs,
        }
    }
}
