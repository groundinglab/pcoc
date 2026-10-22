//! The Hike objects list (DESIGN-v2 sections 2.10 and 10). Objects are
//! rescanned from the data folder on start. The index holds paths, SHA-256,
//! public key and leg counts. It holds no keys.

use crate::format::{day_month_time, thousands};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Hike objects are small. Anything larger is not read for the list.
const MAX_OBJECT_BYTES: u64 = 64 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Kind `v2.2-pcoc-object`.
    V2,
    /// Rule A5. Any other kind text.
    Unknown(String),
    Unreadable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    /// Run directory name when this PCOC lives in one. Empty otherwise.
    pub run_name: String,
    pub bytes: u64,
    pub sha256: [u8; 32],
    pub modified_unix_s: u64,
    pub kind: Kind,
    pub public_key: Option<[u8; 32]>,
    /// Checkpoints stored across the stages.
    pub legs: Option<u64>,
}

impl Entry {
    pub fn read(path: &Path) -> Option<Entry> {
        let meta = std::fs::metadata(path).ok()?;
        if !meta.is_file() || meta.len() > MAX_OBJECT_BYTES {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        let modified_unix_s = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        Some(Self::from_bytes(path.to_owned(), &bytes, modified_unix_s))
    }

    pub fn from_bytes(path: PathBuf, bytes: &[u8], modified_unix_s: u64) -> Entry {
        let too_deep = crate::object_key::nesting_too_deep(bytes);
        let summary = if too_deep { None } else { summarize(bytes) };
        let kind = if too_deep {
            Kind::Unreadable
        } else {
            match pcoc::precheck(bytes, &pcoc::profile::Profile::production()) {
                pcoc::verify::Precheck::UnknownKind(record) => Kind::Unknown(kind_text_of(&record)),
                pcoc::verify::Precheck::Shape { .. } => Kind::V2,
                pcoc::verify::Precheck::DecodeFailure | pcoc::verify::Precheck::Malformed => {
                    Kind::Unreadable
                }
            }
        };
        let public_key = if too_deep {
            None
        } else {
            crate::object_key::public_key(bytes)
        };
        Entry {
            path,
            run_name: String::new(),
            bytes: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
            modified_unix_s,
            kind,
            public_key,
            legs: summary.and_then(|s| s.legs),
        }
    }

    pub fn file_name(&self) -> String {
        if !self.run_name.is_empty() {
            return self.run_name.clone();
        }
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The second line in the sidebar.
    pub fn line(&self, active_public_key: Option<[u8; 32]>) -> String {
        match &self.kind {
            Kind::Unknown(_) => "unknown kind".to_owned(),
            Kind::Unreadable => "not a hike object this verifier decodes".to_owned(),
            Kind::V2 => {
                if active_public_key.is_some()
                    && self.public_key.is_some()
                    && self.public_key != active_public_key
                {
                    return "opened · other npub".to_owned();
                }
                match self.legs {
                    Some(legs) => format!(
                        "{} · {} legs",
                        day_month_time(self.modified_unix_s),
                        thousands(legs)
                    ),
                    None => day_month_time(self.modified_unix_s),
                }
            }
        }
    }
}

/// The PCOC inside a run directory. Prefers `pcoc.cbor`. Hidden files are skipped.
pub fn pcoc_file(dir: &Path) -> Option<PathBuf> {
    let preferred = dir.join(crate::format::PCOC_FILE_NAME);
    if preferred.is_file() {
        return Some(preferred);
    }
    let dir = std::fs::read_dir(dir).ok()?;
    let mut files: Vec<PathBuf> = dir
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.extension().is_some_and(|ext| ext == "cbor")
                && !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        })
        .collect();
    files.sort();
    files.into_iter().next()
}

/// How a hike is labelled in the left column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HikeStatus {
    InProgress,
    Finished,
    Stopped,
}

impl HikeStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::InProgress => "in progress",
            Self::Finished => "finished",
            Self::Stopped => "stopped",
        }
    }

    /// In progress opens the live view. Finished opens the result.
    /// Stopped opens setup.
    pub fn open(self) -> &'static str {
        match self {
            Self::InProgress => "live",
            Self::Finished => "result",
            Self::Stopped => "setup",
        }
    }
}

/// One row in the left column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HikeRow {
    pub name: String,
    pub path: PathBuf,
    pub status: HikeStatus,
}

/// Sign-in fields taken from this session.
///
/// `nsec_in_memory` does not copy the secret and nothing here is written.
/// `pcoc` is the object this session produced. An older hike is not selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPrefill {
    pub nsec_in_memory: bool,
    pub pcoc: Option<PathBuf>,
}

pub fn session_prefill(nsec_in_memory: bool, produced: Option<PathBuf>) -> SessionPrefill {
    SessionPrefill {
        nsec_in_memory,
        pcoc: produced,
    }
}

/// Hikes in the data folder, newest run name first.
///
/// A directory with a PCOC is finished. The run whose output is `running`
/// is in progress. A run directory with no PCOC is stopped. `running` is
/// the output path (`…/pcoc.cbor`), not the directory.
pub fn scan_hikes(folder: &Path, running: Option<&Path>) -> Vec<HikeRow> {
    let running_dir = running.and_then(|path| path.parent()).map(Path::to_owned);
    let Ok(dir) = std::fs::read_dir(folder) else {
        return running_row(running);
    };
    let mut rows = Vec::new();
    let mut saw_running = false;
    for child in dir.filter_map(Result::ok) {
        let path = child.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if name.starts_with('.') || !is_run_dir(&name) {
            continue;
        }
        let in_progress = running_dir.as_ref().is_some_and(|dir| dir == &path);
        if in_progress {
            saw_running = true;
        }
        if let Some(pcoc) = pcoc_file(&path) {
            if !in_progress {
                rows.push(HikeRow {
                    name,
                    path: pcoc,
                    status: HikeStatus::Finished,
                });
                continue;
            }
        }
        rows.push(HikeRow {
            name,
            path: running.filter(|_| in_progress).unwrap_or(&path).to_owned(),
            status: if in_progress {
                HikeStatus::InProgress
            } else {
                HikeStatus::Stopped
            },
        });
    }
    if !saw_running {
        rows.extend(running_row(running));
    }
    rows.sort_by(|a, b| b.name.cmp(&a.name).then(a.path.cmp(&b.path)));
    rows
}

fn running_row(running: Option<&Path>) -> Vec<HikeRow> {
    let Some(path) = running else {
        return Vec::new();
    };
    let name = path
        .parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    vec![HikeRow {
        name,
        path: path.to_owned(),
        status: HikeStatus::InProgress,
    }]
}

/// `YYYY-MM-DD_…`, including a `-2` suffix used when that second is taken.
fn is_run_dir(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() >= 11
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'_'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

/// Run directories in the data folder, newest name first.
///
/// A run directory is `YYYY-MM-DD_HHMMSS_<first 10 chars of npub>`. Loose
/// files in the data folder are not listed. Hidden directories are skipped.
/// A directory with no PCOC is skipped.
pub fn scan(folder: &Path) -> Vec<Entry> {
    let Ok(dir) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for child in dir.filter_map(Result::ok) {
        let path = child.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let Some(pcoc) = pcoc_file(&path) else {
            continue;
        };
        let Some(mut entry) = Entry::read(&pcoc) else {
            continue;
        };
        entry.run_name = name;
        entries.push(entry);
    }
    entries.sort_by(|a, b| b.run_name.cmp(&a.run_name).then(a.path.cmp(&b.path)));
    entries
}

/// One library entry, matched by SHA-256.
pub fn find_by_sha256<'a>(entries: &'a [Entry], sha256: &[u8; 32]) -> Option<&'a Entry> {
    entries.iter().find(|e| &e.sha256 == sha256)
}

/// The hike object a sign-in uses.
///
/// A pick that is still in the library wins. Otherwise the produced object,
/// when that file is in the library. Otherwise the first entry, which the
/// scan orders newest-first. `None` when the library is empty.
pub fn choose_sign_in_object<'a>(
    entries: &'a [Entry],
    produced: Option<&[u8; 32]>,
    picked: Option<&[u8; 32]>,
) -> Option<&'a Entry> {
    if let Some(sha) = picked {
        if let Some(entry) = find_by_sha256(entries, sha) {
            return Some(entry);
        }
    }
    if let Some(sha) = produced {
        if let Some(entry) = find_by_sha256(entries, sha) {
            return Some(entry);
        }
    }
    entries.first()
}

struct Summary {
    legs: Option<u64>,
}

/// Reads the stage checkpoint counts out of the object's CBOR map without
/// decoding the rest. Only definite lengths are read; anything else is
/// "no count" and the list shows no leg figure.
fn kind_text_of(record: &str) -> String {
    serde_json::from_str::<serde_json::Value>(record)
        .ok()
        .and_then(|v| v.get("kind_text").and_then(|t| t.as_str()).map(str::to_owned))
        .unwrap_or_default()
}

fn summarize(bytes: &[u8]) -> Option<Summary> {
    let mut r = Reader { b: bytes, at: 0 };
    let (major, pairs) = r.head()?;
    if major != 5 {
        return None;
    }
    let mut legs = None;
    for _ in 0..pairs {
        let key = r.text()?;
        if key == "stages" {
            let (major, n) = r.head()?;
            if major != 4 {
                return None;
            }
            let mut total = 0u64;
            for _ in 0..n {
                let (major, fields) = r.head()?;
                if major != 5 {
                    return None;
                }
                for _ in 0..fields {
                    let field = r.text()?;
                    if field == "checkpoints" {
                        let (major, count) = r.head()?;
                        if major != 4 {
                            return None;
                        }
                        total += count;
                        for _ in 0..count {
                            r.skip()?;
                        }
                    } else {
                        r.skip()?;
                    }
                }
            }
            legs = Some(total);
        } else {
            r.skip()?;
        }
    }
    Some(Summary { legs })
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let v = *self.b.get(self.at)?;
        self.at += 1;
        Some(v)
    }

    fn head(&mut self) -> Option<(u8, u64)> {
        let first = self.byte()?;
        let (major, info) = (first >> 5, first & 0x1f);
        let arg = match info {
            0..=23 => u64::from(info),
            24..=27 => {
                let n = 1usize << (info - 24);
                let mut v = 0u64;
                for _ in 0..n {
                    v = (v << 8) | u64::from(self.byte()?);
                }
                v
            }
            _ => return None,
        };
        Some((major, arg))
    }

    fn take(&mut self, n: u64) -> Option<&[u8]> {
        let end = self.at.checked_add(usize::try_from(n).ok()?)?;
        let s = self.b.get(self.at..end)?;
        self.at = end;
        Some(s)
    }

    fn text(&mut self) -> Option<String> {
        let (major, n) = self.head()?;
        if major != 3 {
            return None;
        }
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }

    fn skip(&mut self) -> Option<()> {
        let (major, arg) = self.head()?;
        match major {
            0 | 1 | 7 => Some(()),
            2 | 3 => self.take(arg).map(|_| ()),
            4 => (0..arg).try_for_each(|_| self.skip()),
            5 => (0..arg).try_for_each(|_| {
                self.skip()?;
                self.skip()
            }),
            6 => self.skip(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o1() -> Vec<u8> {
        std::fs::read(format!(
            "{}/../../../sdk/rust/objects/O1-S1.cbor",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn counts_checkpoints_and_reads_the_key() {
        let bytes = o1();
        let e = Entry::from_bytes(PathBuf::from("/x/O1-S1.cbor"), &bytes, 1_791_322_260);
        assert_eq!(e.kind, Kind::V2);
        assert_eq!(e.legs, Some(18));
        assert_eq!(
            e.public_key,
            Some(
                crate::hex::decode32(
                    "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
                )
                .unwrap()
            )
        );
        assert_eq!(e.line(e.public_key), "6 Oct 21:31 UTC · 18 legs");
        assert_eq!(e.line(Some([0x22; 32])), "opened · other npub");
    }

    #[test]
    fn sign_in_prefers_a_pick_then_the_produced_object_then_the_newest() {
        let older = Entry::from_bytes(PathBuf::from("older.cbor"), b"older", 1);
        let newer = Entry::from_bytes(PathBuf::from("newer.cbor"), b"newer", 2);
        let entries = vec![newer.clone(), older.clone()];
        assert!(choose_sign_in_object(&[], None, None).is_none());
        assert_eq!(
            choose_sign_in_object(&entries, None, None).map(|e| e.sha256),
            Some(newer.sha256)
        );
        assert_eq!(
            choose_sign_in_object(&entries, Some(&older.sha256), None).map(|e| e.sha256),
            Some(older.sha256)
        );
        assert_eq!(
            choose_sign_in_object(&entries, Some(&newer.sha256), Some(&older.sha256))
                .map(|e| e.sha256),
            Some(older.sha256)
        );
        let missing = [0xee; 32];
        assert_eq!(
            choose_sign_in_object(&entries, Some(&older.sha256), Some(&missing)).map(|e| e.sha256),
            Some(older.sha256)
        );
    }

    #[test]
    fn unknown_kind_and_unreadable_rows() {
        let bytes = pcoc::cbor::encode_map(&[(
            "kind".into(),
            pcoc::cbor::encode_text("not-v2.2-pcoc-object"),
        )]);
        let e = Entry::from_bytes(PathBuf::from("run-b-stages18.cbor"), &bytes, 0);
        assert_eq!(e.kind, Kind::Unknown("not-v2.2-pcoc-object".into()));
        assert_eq!(e.line(None), "unknown kind");

        let junk = Entry::from_bytes(PathBuf::from("junk.cbor"), &[0xff, 0x00], 0);
        assert_eq!(junk.kind, Kind::Unreadable);
        assert_eq!(junk.legs, None);
    }

    #[test]
    fn scan_lists_run_directories_and_skips_loose_files() {
        let dir = std::env::temp_dir().join(format!("pcoc-lib-{}", std::process::id()));
        let run = dir.join("2026-10-09_070431_npub16896f");
        std::fs::create_dir_all(&run).unwrap();
        let bytes = o1();
        std::fs::write(run.join("pcoc.cbor"), &bytes).unwrap();
        std::fs::write(run.join("pcoc.cbor.events"), b"{}\n").unwrap();
        std::fs::write(run.join(".hidden.cbor"), &bytes).unwrap();
        std::fs::write(dir.join("loose.cbor"), &bytes).unwrap();
        let older = dir.join("2026-10-08_010203_npub1aaaaa");
        std::fs::create_dir_all(&older).unwrap();
        std::fs::write(older.join("other.cbor"), &bytes).unwrap();
        let entries = scan(&dir);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].file_name(), "2026-10-09_070431_npub16896f");
        assert!(entries[0].path.ends_with("pcoc.cbor"));
        assert_eq!(entries[1].file_name(), "2026-10-08_010203_npub1aaaaa");
        assert!(entries[1].path.ends_with("other.cbor"));
        let sha: [u8; 32] = Sha256::digest(&bytes).into();
        assert!(find_by_sha256(&entries, &sha).is_some());
        assert!(find_by_sha256(&entries, &[0; 32]).is_none());
        assert!(!entries[0].file_name().contains("nsec"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn hikes_are_labelled_in_progress_finished_or_stopped() {
        let dir = std::env::temp_dir().join(format!("pcoc-hikes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let finished = dir.join("2026-10-13_010000_npub1finish");
        let stopped = dir.join("2026-10-13_020000_npub1stopxx");
        let running_dir = dir.join("2026-10-13_030000_npub1live00");
        std::fs::create_dir_all(&finished).unwrap();
        std::fs::create_dir_all(&stopped).unwrap();
        std::fs::create_dir_all(&running_dir).unwrap();
        std::fs::write(finished.join("pcoc.cbor"), b"pcoc").unwrap();
        let running = running_dir.join("pcoc.cbor");
        let rows = scan_hikes(&dir, Some(&running));
        let labels: Vec<_> = rows
            .iter()
            .map(|row| (row.name.as_str(), row.status))
            .collect();
        assert!(labels.contains(&("2026-10-13_030000_npub1live00", HikeStatus::InProgress)));
        assert!(labels.contains(&("2026-10-13_010000_npub1finish", HikeStatus::Finished)));
        assert!(labels.contains(&("2026-10-13_020000_npub1stopxx", HikeStatus::Stopped)));
        assert_eq!(HikeStatus::InProgress.open(), "live");
        assert_eq!(HikeStatus::Finished.open(), "result");
        assert_eq!(HikeStatus::Stopped.open(), "setup");
        assert_eq!(HikeStatus::InProgress.label(), "in progress");
        assert_eq!(HikeStatus::Finished.label(), "finished");
        assert_eq!(HikeStatus::Stopped.label(), "stopped");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn session_prefill_is_this_sessions_pcoc_and_writes_nothing() {
        let dir = std::env::temp_dir().join(format!("pcoc-prefill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let produced = dir.join("2026-10-13_010203_npub1xxxxx").join("pcoc.cbor");
        let older = dir.join("older").join("pcoc.cbor");
        let filled = session_prefill(true, Some(produced.clone()));
        assert!(filled.nsec_in_memory);
        assert_eq!(filled.pcoc.as_ref(), Some(&produced));
        assert_ne!(filled.pcoc.as_ref(), Some(&older));
        let empty = session_prefill(false, None);
        assert!(!empty.nsec_in_memory);
        assert!(empty.pcoc.is_none());
        assert!(std::fs::read_dir(&dir).unwrap().next().is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
