//! Stamp `ADMISSION_COMMIT` and `PCOC_SDK_VERSION` into the binary.
//!
//! `PCOC_SDK_VERSION` is the version of the `hike` package (the Rust SDK) that
//! `Cargo.lock` resolved, plus where it came from: `2.0.0-rc.1 (path)` for the
//! in-tree `sdk/rust`, or `<version> (git <commit>)` for a git pin.
//!
//! `COMMIT`, when set to a non-empty single line, is that stamp. Otherwise
//! `git rev-parse HEAD` in this tree is the stamp. A build with neither
//! records `unknown`. The stamp is read at build time. Cargo reruns this
//! script when `COMMIT` changes, or when `.git/HEAD` or `.git/refs` changes.
//! A worktree's `.git` is a file, so the resolved `git rev-parse --git-path`
//! locations are watched as well.

fn main() {
    println!("cargo:rerun-if-env-changed=COMMIT");
    watch_git();
    let commit = std::env::var("COMMIT")
        .ok()
        .and_then(|raw| single_line(&raw))
        .or_else(git_head)
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=ADMISSION_COMMIT={commit}");
    println!("cargo:rerun-if-changed=Cargo.lock");
    let sdk = std::fs::read_to_string("Cargo.lock")
        .ok()
        .and_then(|lock| sdk_in_lock(&lock))
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=PCOC_SDK_VERSION={sdk}");
}

/// `hike` in Cargo.lock: `<version> (path)` or `<version> (git <short commit>)`.
fn sdk_in_lock(lock: &str) -> Option<String> {
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line != "[[package]]" {
            continue;
        }
        let mut name = None;
        let mut version = None;
        let mut source = None;
        for field in lines.by_ref() {
            if field.is_empty() {
                break;
            }
            if let Some(v) = field.strip_prefix("name = ") {
                name = Some(v.trim_matches('"'));
            } else if let Some(v) = field.strip_prefix("version = ") {
                version = Some(v.trim_matches('"'));
            } else if let Some(v) = field.strip_prefix("source = ") {
                source = Some(v.trim_matches('"'));
            }
        }
        if name != Some("hike") {
            continue;
        }
        let version = version?;
        return Some(match source.and_then(|s| s.rsplit_once('#')) {
            Some((_, commit)) => format!("{version} (git {})", &commit[..commit.len().min(7)]),
            None if source.is_none() => format!("{version} (path)"),
            None => format!("{version} ({})", source.unwrap_or("unknown")),
        });
    }
    None
}

/// Rebuild when HEAD or a ref moves. Paths are absolute so they resolve from
/// this package directory, not from a `.git` that is not here.
fn watch_git() {
    if let Some(top) = git_line(&["rev-parse", "--show-toplevel"]) {
        for rel in [".git/HEAD", ".git/refs"] {
            let path = format!("{top}/{rel}");
            if std::path::Path::new(&path).exists() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
    for spec in ["HEAD", "refs"] {
        if let Some(path) = git_line(&["rev-parse", "--git-path", spec]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

fn git_head() -> Option<String> {
    git_line(&["rev-parse", "HEAD"])
}

fn git_line(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    single_line(&String::from_utf8(output.stdout).ok()?)
}

fn single_line(raw: &str) -> Option<String> {
    let line = raw.trim();
    if line.is_empty() || line.chars().any(|c| c.is_whitespace()) {
        None
    } else {
        Some(line.to_owned())
    }
}
