//! Reads the `hike` pin from the workspace manifest and lock file and
//! exposes it to the About page. It reports the pin; it does not refuse one.

use std::path::PathBuf;

fn main() {
    let core_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = core_dir.parent().expect("workspace root").to_path_buf();
    let manifest_path = root.join("Cargo.toml");
    let lock_path = root.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    println!("cargo:rerun-if-changed={}", lock_path.display());

    let hike = std::fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|m| {
            m.get("workspace")
                .and_then(|w| w.get("dependencies"))
                .and_then(|d| d.get("hike"))
                .and_then(|h| h.as_table())
                .cloned()
        })
        .unwrap_or_default();

    let str_of = |key: &str| hike.get(key).and_then(|v| v.as_str()).map(str::to_owned);
    let (kind, pin) = if let Some(tag) = str_of("tag") {
        ("tag", tag)
    } else if let Some(rev) = str_of("rev") {
        ("rev", rev)
    } else if let Some(branch) = str_of("branch") {
        ("branch", branch)
    } else if let Some(path) = str_of("path") {
        ("path", path)
    } else {
        ("unknown", "unknown".to_owned())
    };

    let (version, commit) = std::fs::read_to_string(&lock_path)
        .ok()
        .and_then(|lock| hike_in_lock(&lock))
        .unwrap_or_else(|| ("unknown".to_owned(), "unresolved".to_owned()));

    println!("cargo:rustc-env=PCOC_REF_PIN_KIND={kind}");
    println!("cargo:rustc-env=PCOC_REF_PIN={pin}");
    println!("cargo:rustc-env=PCOC_HIKE_VERSION={version}");
    println!("cargo:rustc-env=PCOC_REF_COMMIT={commit}");
}

/// Version and resolved commit of `hike` in the lock file. A path or patched
/// source has no `#commit`; that reads as "local".
fn hike_in_lock(lock: &str) -> Option<(String, String)> {
    let parsed: toml::Table = lock.parse().ok()?;
    let packages = parsed.get("package")?.as_array()?;
    for package in packages {
        let table = package.as_table()?;
        if table.get("name").and_then(|v| v.as_str()) != Some("hike") {
            continue;
        }
        let version = table.get("version")?.as_str()?.to_owned();
        let commit = table
            .get("source")
            .and_then(|v| v.as_str())
            .and_then(|s| s.rsplit_once('#'))
            .map(|(_, c)| c.to_owned())
            .unwrap_or_else(|| "local".to_owned());
        return Some((version, commit));
    }
    None
}
