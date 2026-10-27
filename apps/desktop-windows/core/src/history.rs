//! History root and the finished `.cbor` write.
//!
//! Ask A6: the Windows default is `%USERPROFILE%\PCOC\hikes\`, cleartext.
//! Encryption is later. This module has no encryption API. A finished
//! produce always writes the `.cbor`. Tests pass an override root so the
//! suite does not need a Windows profile.

use crate::endpoints::{BARI, INDEX};
use crate::pin::STAGES;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Documented Windows default, as a display string. The resolver builds the
/// same path from `USERPROFILE` when that variable is set.
pub const WINDOWS_HISTORY_DISPLAY: &str = r"%USERPROFILE%\PCOC\hikes\";

/// Cleartext now. Encryption is a later cut. There is no encrypt function.
pub const CLEARTEXT_NOTE: &str =
    "History is cleartext. Encryption is later. The finished .cbor is the object bytes.";

/// Extremely basic preferences. Endpoints and the stage count are hard-coded
/// defaults. There is no field for an nsec or a hex secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prefs {
    /// Empty selects the history root from [`history_root`].
    #[serde(default)]
    pub history_root: PathBuf,
    /// Empty selects [`crate::endpoints::BARI`].
    #[serde(default)]
    pub bari_url: String,
    /// Empty selects [`crate::endpoints::INDEX`].
    #[serde(default)]
    pub index_url: String,
    /// Worker hint. Clamped on load. Not a secret.
    #[serde(default = "default_threads")]
    pub threads: u32,
}

fn default_threads() -> u32 {
    6
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            history_root: PathBuf::new(),
            bari_url: String::new(),
            index_url: String::new(),
            threads: default_threads(),
        }
    }
}

impl Prefs {
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("prefs serialize");
        text.push('\n');
        text
    }

    /// Load prefs. Unknown fields (including a planted secret) are refused:
    /// the load falls back to defaults and the secret is not kept.
    pub fn load(text: &str) -> Self {
        serde_json::from_str::<Prefs>(text)
            .map(|mut prefs| {
                prefs.threads = prefs.threads.clamp(1, 32);
                prefs
            })
            .unwrap_or_default()
    }

    pub fn effective_bari(&self) -> &str {
        if self.bari_url.is_empty() {
            BARI
        } else {
            &self.bari_url
        }
    }

    pub fn effective_index(&self) -> &str {
        if self.index_url.is_empty() {
            INDEX
        } else {
            &self.index_url
        }
    }
}

/// Where finished objects go.
///
/// `override_root` wins. That is the test double and any explicit path.
/// Otherwise, on a Windows target, or when `USERPROFILE` is set and `HOME`
/// is not, the root is `{USERPROFILE}/PCOC/hikes` (the `%USERPROFILE%\PCOC\hikes\`
/// default). On the Linux harness the usual home is `HOME`, joined as
/// `PCOC/hikes`, so native tests can resolve a profile without pretending
/// to be Windows.
pub fn history_root(override_root: Option<&Path>) -> PathBuf {
    if let Some(root) = override_root {
        return root.to_path_buf();
    }
    if let Some(profile) = windows_profile_dir() {
        return profile.join("PCOC").join("hikes");
    }
    unix_home()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("PCOC")
        .join("hikes")
}

/// The Windows default, when this process should use it.
///
/// - Native Windows: `USERPROFILE`, then the profile home.
/// - A non-Windows harness that has set `USERPROFILE` and has not set `HOME`
///   is treated as the Windows default under test.
fn windows_profile_dir() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE").map(PathBuf::from);
    if cfg!(windows) {
        return profile.or_else(home_dir);
    }
    let home_set = std::env::var_os("HOME").is_some();
    if home_set {
        return None;
    }
    profile.filter(|path| !path.as_os_str().is_empty())
}

fn unix_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn home_dir() -> Option<PathBuf> {
    unix_home().or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

/// File name for one finished object. The public identifier is not a secret.
pub fn object_file_name(public_hex: &str) -> String {
    format!("{public_hex}.cbor")
}

/// Write `bytes` as `{root}/{public_hex}.cbor`. Creates `root` when needed.
/// A finished produce always calls this. The bytes are cleartext.
pub fn save_object(root: &Path, public_hex: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(root)?;
    let path = root.join(object_file_name(public_hex));
    let tmp = path.with_extension("cbor.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// True when the saved object uses a `.cbor` file name. `Path::ends_with`
/// treats a dotted name as one component, so the check is on the file name.
pub fn is_cbor_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "cbor")
}

/// Stage count the producer is built for. Exposed so prefs tests can name it
/// without a second constant.
pub fn stages() -> usize {
    STAGES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_root_wins_over_the_profile() {
        let root = PathBuf::from("/tmp/pcoc-windows-history-override");
        assert_eq!(history_root(Some(&root)), root);
    }

    #[test]
    fn userprofile_without_home_is_the_windows_default() {
        // SAFETY: the test process is single-threaded for this assertion.
        // cargo test may run tests in parallel, so use a unique lock via
        // serialising on a process-local mutex.
        let _guard = env_lock();
        let saved_home = std::env::var_os("HOME");
        let saved_profile = std::env::var_os("USERPROFILE");
        std::env::remove_var("HOME");
        std::env::set_var("USERPROFILE", r"C:\Users\pcoc");
        let root = history_root(None);
        match saved_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match saved_profile {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
        assert_eq!(
            root,
            PathBuf::from(r"C:\Users\pcoc").join("PCOC").join("hikes")
        );
        assert_eq!(WINDOWS_HISTORY_DISPLAY, r"%USERPROFILE%\PCOC\hikes\");
    }

    #[test]
    fn finished_produce_always_writes_cbor_under_the_root() {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-windows-history-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = b"cleartext-object-bytes";
        let path = save_object(&dir, "ab".repeat(32).as_str(), bytes).unwrap();
        assert!(is_cbor_file(&path), "{}", path.display());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(CLEARTEXT_NOTE.contains("cleartext"));
        assert!(!CLEARTEXT_NOTE.to_lowercase().contains("encrypt the"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
    }
}
