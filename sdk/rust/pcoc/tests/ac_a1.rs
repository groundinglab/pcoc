//! AC-A1. Every file under `sdk/rust/` is added on this branch.
//! The test does not read `sdk/go` or `sdk/java`.
//!
//! The integration branch also changes consumers, the other language SDKs,
//! and CI. A lock that those paths must stay untouched belonged to the
//! Rust-only branch. It is not the check that the Rust tree is new.
//!
//! The check was met once the 2.2 Rust SDK reached main. It returns before
//! the per-file walk when `origin/main` already contains that tree, and when
//! `origin/main..HEAD` is empty (this checkout is main, or the branch is
//! already merged).

#[path = "../../support/repo.rs"]
mod repo_mod;

use repo_mod::repo_root;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    repo_root()
}

fn git(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for ent in std::fs::read_dir(dir).unwrap() {
        let ent = ent.unwrap();
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if name == "target" {
            continue;
        }
        let path = ent.path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Paths the removed tree also used. Each blob on this branch differs from `origin/main`.
const REPLACED_ON_MAIN: &[&str] = &[
    "sdk/rust/Cargo.toml",
    "sdk/rust/Cargo.lock",
    "sdk/rust/rust-toolchain.toml",
    "sdk/rust/README.md",
];

/// Crate manifest the pre-2.2 tree did not have. The workspace files in
/// `REPLACED_ON_MAIN` existed on that tree, so they do not mark 2.2.
const SDK_22_MARKER: &str = "sdk/rust/pcoc/Cargo.toml";

#[test]
fn files_added_on_this_branch() {
    let repo = repo();
    let marker_ref = format!("origin/main:{SDK_22_MARKER}");
    let marker = git(&repo, &["cat-file", "-e", &marker_ref]);
    if marker.status.success() {
        eprintln!(
            "AC-A1: origin/main already contains the 2.2 Rust SDK ({SDK_22_MARKER}); skipping the per-file check"
        );
        return;
    }
    let out = git(&repo, &["rev-list", "--reverse", "origin/main..HEAD"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let Some(first) = text
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
    else {
        eprintln!("AC-A1: origin/main..HEAD is empty; skipping the per-file check");
        return;
    };
    let mut files = Vec::new();
    walk(&repo.join("sdk/rust"), &mut files);
    assert!(!files.is_empty());
    for file in &files {
        let rel = file
            .strip_prefix(&repo)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let at_cut = format!("{first}:{rel}");
        let cut = git(&repo, &["cat-file", "-e", &at_cut]);
        assert!(
            !cut.status.success(),
            "{rel} exists at the branch cut {first}"
        );
        let on_main = format!("origin/main:{rel}");
        let main = git(&repo, &["cat-file", "-e", &on_main]);
        if main.status.success() {
            assert!(
                REPLACED_ON_MAIN.contains(&rel.as_str()),
                "{rel} is on origin/main and is not a replaced workspace path"
            );
            let old = git(&repo, &["rev-parse", &on_main]);
            let new = git(&repo, &["hash-object", file.to_str().unwrap()]);
            let old = String::from_utf8(old.stdout).unwrap();
            let new = String::from_utf8(new.stdout).unwrap();
            assert_ne!(old.trim(), new.trim(), "{rel} matches origin/main");
        }
    }

    // `git diff <cut>` is the working tree against that commit, so a clean
    // tree is HEAD against the branch cut. The Rust tree has to be part of
    // that diff. Other paths are the integration branch's own work.
    let diff = git(&repo, &["diff", "--name-only", &first]);
    assert!(diff.status.success());
    let changed: Vec<String> = String::from_utf8(diff.stdout)
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();
    assert!(
        changed.iter().any(|path| path.starts_with("sdk/rust/")),
        "the branch diff does not touch sdk/rust"
    );
}

#[test]
fn step_is_only_in_step_rs() {
    let root = repo().join("sdk/rust");
    let mut files = Vec::new();
    walk(&root, &mut files);
    let mut hits = Vec::new();
    for file in files {
        if file.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&file).unwrap();
        let defines = text.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with("fn step(")
                || t.starts_with("pub fn step(")
                || t.starts_with("pub(crate) fn step(")
        });
        if defines {
            hits.push(file);
        }
    }
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].ends_with("pcoc/src/step.rs"));
}
