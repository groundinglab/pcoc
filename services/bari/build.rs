//! Stamp `BARI_COMMIT` into the binary.
//!
//! `COMMIT`, when set to a non-empty single line, is that stamp. Otherwise
//! `git rev-parse HEAD` in this tree is the stamp. A build with neither
//! records `unknown`. Request time does not re-read git.

fn main() {
    println!("cargo:rerun-if-env-changed=COMMIT");
    println!("cargo:rerun-if-changed=build.rs");
    let commit = std::env::var("COMMIT")
        .ok()
        .and_then(|raw| single_line(&raw))
        .or_else(git_head)
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=BARI_COMMIT={commit}");
}

fn single_line(raw: &str) -> Option<String> {
    let line = raw.trim();
    if line.is_empty() || line.chars().any(|c| c.is_whitespace()) {
        None
    } else {
        Some(line.to_owned())
    }
}

fn git_head() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    single_line(&String::from_utf8(output.stdout).ok()?)
}
