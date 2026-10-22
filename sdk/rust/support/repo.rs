//! Locate this checkout. Tests fail, rather than skip, when the walk misses.

use std::path::PathBuf;

/// Directory that holds both `sdk/VERSION` and `spec/`.
///
/// Walks up from the crate manifest. Panics with a clear message when no
/// directory on that walk holds both. A missing checkout is a failure.
pub fn repo_root() -> PathBuf {
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut dir = start.clone();
    loop {
        let version = dir.join("sdk").join("VERSION");
        let spec = dir.join("spec");
        if version.is_file() && spec.is_dir() {
            return dir;
        }
        if !dir.pop() {
            panic!(
                "repo root not found walking up from {}: no directory holds both sdk/VERSION and spec/",
                start.display()
            );
        }
    }
}
