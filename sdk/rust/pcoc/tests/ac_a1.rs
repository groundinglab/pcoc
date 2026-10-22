//! AC-A1. Every file under `sdk/rust/` is added on this branch.
//! The test does not read `sdk/go` or `sdk/java`.
//!
//! The integration branch also changes consumers, the other language SDKs,
//! and CI. A lock that those paths must stay untouched belonged to the
//! Rust-only branch. It is not the check that the Rust tree is new.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
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

/// Workspace paths the removed tree also used. The layout requires these names.
/// Each one on this branch is a different blob from `origin/main`.
const REPLACED_ON_MAIN: &[&str] = &[
    "sdk/rust/Cargo.toml",
    "sdk/rust/Cargo.lock",
    "sdk/rust/rust-toolchain.toml",
];

#[test]
fn files_added_on_this_branch() {
    let repo = repo();
    let mut files = Vec::new();
    walk(&repo.join("sdk/rust"), &mut files);
    assert!(!files.is_empty());
    let first = {
        let out = git(&repo, &["rev-list", "--reverse", "origin/main..HEAD"]);
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        text.lines().next().unwrap().trim().to_string()
    };
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
