//! One-screen Windows producer. The egui shell is `main.rs`. This library
//! holds the session, the nsec-only key rule, and the finish path.
//!
//! The screen has no second page. History stays a list on this screen.

pub mod finish;
pub mod keys_ui;
pub mod produce_run;
pub mod session;
pub mod success;

pub use session::{Session, StartOpts};

pub const LABEL_GENERATE: &str = "Generate";
pub const LABEL_SAVE: &str = "Save key";
pub const LABEL_START: &str = "Start";
pub const LABEL_STOP: &str = "Stop";
pub const LABEL_HISTORY: &str = "Show history";
pub const LABEL_FOLDER: &str = "Open folder";

/// Program used to open the history folder. Windows uses Explorer.
pub fn folder_program() -> &'static str {
    if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    }
}

pub fn open_folder(path: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new(folder_program())
        .arg(path)
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_source_is_one_screen() {
        let main = include_str!("main.rs");
        assert!(main.contains("CentralPanel"));
        assert!(!main.contains("SidePanel"));
        assert!(!main.contains("Settings"));
        assert!(!main.contains("Verify"));
        for label in [
            "LABEL_GENERATE",
            "LABEL_SAVE",
            "LABEL_START",
            "LABEL_STOP",
            "LABEL_HISTORY",
            "LABEL_FOLDER",
        ] {
            assert!(main.contains(label), "{label}");
        }
        assert_eq!(LABEL_GENERATE, "Generate");
        assert_eq!(LABEL_SAVE, "Save key");
        assert_eq!(LABEL_START, "Start");
        assert_eq!(LABEL_STOP, "Stop");
        assert_eq!(LABEL_HISTORY, "Show history");
        assert_eq!(LABEL_FOLDER, "Open folder");
    }

    #[test]
    fn open_folder_uses_explorer_on_windows() {
        if cfg!(windows) {
            assert_eq!(folder_program(), "explorer");
        } else {
            assert_eq!(folder_program(), "xdg-open");
        }
    }
}
