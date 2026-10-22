//! Append-only public decision log for Satoshi Hike 2.2.
//!
//! The process creates the directory. A chain whose lines are not this
//! spec is not loaded and is not rewritten. Archiving a v1.35 log is a
//! later phase. Day paths are served from `time_unix_s` on the one chain
//! `{log_dir}/chain.jsonl`.
//!
//! One writer. A line is fsynced, including its newline, before it is
//! current. `/latest.json` is rewritten after the line is durable, and again
//! when the log is opened, from the chain. A crash between those two writes
//! is repaired on the next open. A failure of the hint after a new line does
//! not undo that line. A later line may name a lower `bitcoin_tip_height`.
//! The hash chain still holds. A reorg is recorded.
//!
//! A thorough `verify_summary` is the section 14 record, including that
//! record's trailing newline. The newline sits inside the JSON object, before
//! the decision's closing brace. Readers take JSON values. Splitting the file
//! on every newline splits a thorough line. The file's own newline after that
//! brace is the line separator and is not part of the hashed bytes.

use crate::canon::{self, event_hash, genesis_hash, hex32, GENESIS_HEX};
use crate::verifier::SPEC_NAME;
use serde_json::{json, Map, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const FEED: &str = "pcoc-gate-feed-v2";

#[derive(Debug)]
pub enum LogError {
    Io(io::Error),
    /// The directory holds a log this gate does not serve.
    NotThisLineage(String),
    Unusable(String),
}

impl From<io::Error> for LogError {
    fn from(error: io::Error) -> Self {
        LogError::Io(error)
    }
}

pub struct StoredLine {
    pub raw: Vec<u8>,
    pub value: Value,
    pub hash: [u8; 32],
}

pub struct DecisionLog {
    dir: PathBuf,
    lines: Vec<StoredLine>,
    next_seq: u64,
    prev_hash: [u8; 32],
    /// Copied into `/latest.json` when set. The chain does not store it.
    policy_hint: Option<Value>,
}

pub struct DecisionWrite {
    pub time_unix_s: u64,
    /// `None` is JSON null. A known height is that integer. Never `0` for unknown.
    pub bitcoin_tip_height: Option<u64>,
    pub phase: &'static str,
    pub decision: &'static str,
    pub fast_seq: Option<u64>,
    pub submission_sha256: [u8; 32],
    pub public_key: [u8; 32],
    pub object_sha256: [u8; 32],
    pub object: Vec<u8>,
    pub reasons: Vec<String>,
    pub reason: Option<String>,
    /// Section 14 record bytes, or `{}` when this line has no record.
    /// A real record includes its trailing newline. The encoder splices
    /// these bytes. It does not parse them. `None` omits `verify_summary`
    /// and writes `measures` instead.
    pub summary: Option<String>,
    /// Verifier numbers. A thorough line writes these instead of `summary`.
    /// A fast refusal writes both: `measures`, then `verify_summary` last.
    pub measures: Option<Value>,
    /// Gate policy id. Omitted on a fast line, a provisional, and a resume
    /// that still stores the section 14 record.
    pub policy_id: Option<String>,
    /// `[{field, min, met}]`. Omitted when this line has no policy.
    pub policy_checks: Option<Value>,
    /// One stable snake_case code on a refusal. Never the catch-all `rejected`.
    pub reason_code: Option<String>,
    /// `below_policy`, `invalid`, or `operational`.
    pub class: Option<String>,
    /// Thorough accept this return points at.
    pub access_ref_day: Option<String>,
    pub access_ref_seq: Option<u64>,
    /// Thorough result: runs, legs matched, revealed draw and commitment.
    /// Monitor feed only; not on the sign-in result body.
    pub deep: Option<Value>,
    /// Gate 0.5.4: the `network` of each index document this decision rests
    /// on (distinct, sorted), `null` when it rests on none. Lines written
    /// before 0.5.4 have no such key; readers treat that as unknown.
    pub index_network: Option<Value>,
    /// The policy's `[index] expected_network` (gl-admit-v7 on).
    pub expected_network: Option<String>,
}

impl DecisionWrite {
    /// A line with no policy, no measures, and an empty `verify_summary`.
    /// Key order matches the encoder that hashed existing chains.
    pub fn bare(
        time_unix_s: u64,
        bitcoin_tip_height: Option<u64>,
        phase: &'static str,
        decision: &'static str,
    ) -> Self {
        Self {
            time_unix_s,
            bitcoin_tip_height,
            phase,
            decision,
            fast_seq: None,
            submission_sha256: [0u8; 32],
            public_key: [0u8; 32],
            object_sha256: [0u8; 32],
            object: Vec::new(),
            reasons: Vec::new(),
            reason: None,
            summary: Some("{}".to_owned()),
            measures: None,
            policy_id: None,
            policy_checks: None,
            reason_code: None,
            class: None,
            access_ref_day: None,
            access_ref_seq: None,
            deep: None,
            index_network: None,
            expected_network: None,
        }
    }
}

impl DecisionLog {
    /// Create `dir` when it is missing. Refuse a chain that is not v2.0.
    pub fn open(dir: &Path) -> Result<Self, LogError> {
        if dir.exists() && !dir.is_dir() {
            return Err(LogError::Unusable(format!(
                "{} is not a directory",
                dir.display()
            )));
        }
        let chain = dir.join("chain.jsonl");
        let mut blob = if chain.exists() {
            fs::read(&chain)?
        } else {
            Vec::new()
        };
        let parsed = parse_chain(&blob)?;
        if parsed.torn {
            if parsed.lines.is_empty() {
                return Err(LogError::NotThisLineage(format!(
                    "{} has no complete Satoshi Hike 2.2 line; this gate does not rewrite it",
                    chain.display()
                )));
            }
            blob.truncate(parsed.keep);
            let file = OpenOptions::new().write(true).open(&chain)?;
            file.set_len(parsed.keep as u64)?;
            file.sync_all()?;
            if let Some(parent) = chain.parent() {
                sync_dir(parent)?;
            }
        }
        fs::create_dir_all(dir)?;
        fs::create_dir_all(dir.join("objects"))?;
        let mut log = Self {
            dir: dir.to_path_buf(),
            lines: Vec::new(),
            next_seq: 0,
            prev_hash: genesis_hash(),
            policy_hint: None,
        };
        log.install(parsed.lines)?;
        log.repair_latest()?;
        Ok(log)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn lines(&self) -> &[StoredLine] {
        &self.lines
    }

    /// Put `policy` on the next `/latest.json`. Rewrites the hint when a line exists.
    pub fn set_policy_hint(&mut self, policy: Value) -> Result<(), LogError> {
        self.policy_hint = Some(policy);
        if self.lines.is_empty() {
            return Ok(());
        }
        self.rewrite_latest()
    }

    pub fn object_bytes(&self, sha_hex: &str) -> Option<Vec<u8>> {
        fs::read(self.dir.join("objects").join(sha_hex)).ok()
    }

    pub fn latest_bytes(&self) -> Option<Vec<u8>> {
        if self.lines.is_empty() {
            return None;
        }
        let path = self.dir.join("latest.json");
        if !path.exists() {
            return None;
        }
        fs::read(path).ok()
    }

    pub fn day_bytes(&self, day: &str) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        for line in &self.lines {
            let time = line
                .value
                .get("time_unix_s")
                .and_then(|item| item.as_u64())?;
            if utc_day(time) == day {
                out.extend_from_slice(&line.raw);
                out.push(b'\n');
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    pub fn append(&mut self, write: &DecisionWrite) -> Result<u64, LogError> {
        if !canon::in_json_bound(write.time_unix_s)
            || write
                .bitcoin_tip_height
                .is_some_and(|tip| !canon::in_json_bound(tip))
        {
            return Err(LogError::Unusable(
                "time or tip is outside the JSON integer bound".to_owned(),
            ));
        }
        let object_sha = hex32(&write.object_sha256);
        publish_new(&self.dir.join("objects").join(&object_sha), &write.object)?;
        let seq = self.next_seq;
        let raw = encode_decision(seq, &hex32(&self.prev_hash), write)?;
        self.commit_line(raw)
    }

    /// One decision object whose `seq` matches and whose `time_unix_s` falls
    /// on that UTC day. The bytes are the stored line.
    pub fn line_on_day(&self, day: &str, seq: u64) -> Option<&StoredLine> {
        self.lines.iter().find(|line| {
            let line_seq = line.value.get("seq").and_then(|item| item.as_u64());
            let time = line.value.get("time_unix_s").and_then(|item| item.as_u64());
            line_seq == Some(seq) && time.is_some_and(|time| utc_day(time) == day)
        })
    }

    fn install(&mut self, lines: Vec<StoredLine>) -> Result<(), LogError> {
        for line in lines {
            let seq = line
                .value
                .get("seq")
                .and_then(|item| item.as_u64())
                .ok_or_else(|| LogError::Unusable("seq".to_owned()))?;
            match line.value.get("bitcoin_tip_height") {
                Some(Value::Number(number)) if number.as_u64().is_some() => {}
                Some(Value::Null) => {}
                _ => return Err(LogError::Unusable("bitcoin_tip_height".to_owned())),
            }
            self.prev_hash = line.hash;
            self.next_seq = seq + 1;
            self.lines.push(line);
        }
        Ok(())
    }

    /// `/latest.json` is a hint. The chain is the record. Opening the log
    /// writes the hint from the last line, or removes a leftover hint when
    /// the chain has no line.
    fn repair_latest(&self) -> Result<(), LogError> {
        if self.lines.is_empty() {
            let path = self.dir.join("latest.json");
            if path.exists() {
                fs::remove_file(&path)?;
                sync_dir(&self.dir)?;
            }
            return Ok(());
        }
        self.rewrite_latest()
    }

    fn commit_line(&mut self, raw: Vec<u8>) -> Result<u64, LogError> {
        let hash = event_hash(&raw);
        let seq = self.next_seq;
        let path = self.dir.join("chain.jsonl");
        {
            let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
            file.write_all(&raw)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
        }
        sync_dir(&self.dir)?;
        let value: Value = serde_json::from_slice(&raw)
            .map_err(|err| LogError::Unusable(format!("line was not JSON after encode: {err}")))?;
        self.prev_hash = hash;
        self.next_seq = seq + 1;
        self.lines.push(StoredLine { raw, value, hash });
        if let Err(error) = self.rewrite_latest() {
            eprintln!("admission-service latest.json was not rewritten: {error:?}");
        }
        Ok(seq)
    }

    fn rewrite_latest(&self) -> Result<(), LogError> {
        let last = self
            .lines
            .last()
            .ok_or_else(|| LogError::Unusable("no line".to_owned()))?;
        let time = last
            .value
            .get("time_unix_s")
            .and_then(|item| item.as_u64())
            .unwrap_or(0);
        let mut map = Map::new();
        map.insert("feed".to_owned(), json!(FEED));
        map.insert("spec".to_owned(), json!(SPEC_NAME));
        if let Some(policy) = self.policy_hint.as_ref() {
            map.insert("policy".to_owned(), policy.clone());
        }
        map.insert("last_seq".to_owned(), json!(last.value["seq"].clone()));
        map.insert("last_event_hash".to_owned(), json!(hex32(&last.hash)));
        map.insert("today".to_owned(), json!(utc_day(time)));
        map.insert("published_unix_s".to_owned(), json!(unix_now()));
        let bytes = serde_json::to_vec(&Value::Object(map))
            .map_err(|err| LogError::Unusable(err.to_string()))?;
        let path = self.dir.join("latest.json");
        let tmp = self.dir.join("latest.json.tmp");
        {
            let mut file = File::create(&tmp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        fs::rename(&tmp, &path)?;
        sync_dir(&self.dir)?;
        Ok(())
    }
}

struct Parsed {
    lines: Vec<StoredLine>,
    torn: bool,
    keep: usize,
}

fn parse_chain(blob: &[u8]) -> Result<Parsed, LogError> {
    if blob.is_empty() {
        return Ok(Parsed {
            lines: Vec::new(),
            torn: false,
            keep: 0,
        });
    }
    let objects = split_decisions(blob)?;
    let mut lines = Vec::new();
    let mut prev_hash = genesis_hash();
    for (index, raw) in objects.raw.iter().enumerate() {
        let value: Value = serde_json::from_slice(raw).map_err(|err| {
            LogError::NotThisLineage(format!("log line {index} is not a 2.2 line: {err}"))
        })?;
        if value.get("spec").and_then(|item| item.as_str()) != Some(SPEC_NAME) {
            return Err(LogError::NotThisLineage(format!(
                "log line {index} is not a Satoshi Hike 2.2 decision; this gate does not serve it"
            )));
        }
        let seq = value
            .get("seq")
            .and_then(|item| item.as_u64())
            .ok_or_else(|| LogError::Unusable(format!("log line {index} has no seq")))?;
        if seq != index as u64 {
            return Err(LogError::Unusable(format!(
                "log seq {seq} is not position {index}"
            )));
        }
        let prev = value
            .get("prev_event_hash")
            .and_then(|item| item.as_str())
            .ok_or_else(|| LogError::Unusable("prev_event_hash".to_owned()))?;
        let expect = if index == 0 {
            GENESIS_HEX.to_owned()
        } else {
            hex32(&prev_hash)
        };
        if prev != expect {
            return Err(LogError::Unusable(
                "prev_event_hash does not match the previous line".to_owned(),
            ));
        }
        let hash = event_hash(raw);
        prev_hash = hash;
        lines.push(StoredLine {
            raw: raw.to_vec(),
            value,
            hash,
        });
    }
    Ok(Parsed {
        lines,
        torn: objects.torn,
        keep: objects.keep,
    })
}

fn encode_decision(seq: u64, prev: &str, write: &DecisionWrite) -> Result<Vec<u8>, LogError> {
    let mut map = Map::new();
    map.insert("event".to_owned(), json!("gate.decision"));
    map.insert("seq".to_owned(), json!(seq));
    map.insert("prev_event_hash".to_owned(), json!(prev));
    map.insert("time_unix_s".to_owned(), json!(write.time_unix_s));
    map.insert(
        "bitcoin_tip_height".to_owned(),
        match write.bitcoin_tip_height {
            Some(height) => json!(height),
            None => Value::Null,
        },
    );
    map.insert("spec".to_owned(), json!(SPEC_NAME));
    map.insert("gate_version".to_owned(), json!(env!("CARGO_PKG_VERSION")));
    map.insert("gate_commit".to_owned(), json!(env!("ADMISSION_COMMIT")));
    map.insert("phase".to_owned(), json!(write.phase));
    map.insert("decision".to_owned(), json!(write.decision));
    if let Some(fast_seq) = write.fast_seq {
        map.insert("fast_seq".to_owned(), json!(fast_seq));
    }
    map.insert(
        "submission_sha256".to_owned(),
        json!(hex32(&write.submission_sha256)),
    );
    map.insert("public_key".to_owned(), json!(hex32(&write.public_key)));
    map.insert(
        "object_sha256".to_owned(),
        json!(hex32(&write.object_sha256)),
    );
    map.insert(
        "reasons".to_owned(),
        Value::Array(write.reasons.iter().cloned().map(Value::String).collect()),
    );
    if let Some(reason) = &write.reason {
        map.insert("reason".to_owned(), json!(reason));
    }
    if let Some(code) = &write.reason_code {
        map.insert("reason_code".to_owned(), json!(code));
    }
    if let Some(class) = &write.class {
        map.insert("class".to_owned(), json!(class));
    }
    if let Some(policy_id) = &write.policy_id {
        map.insert("policy_id".to_owned(), json!(policy_id));
    }
    if let Some(checks) = &write.policy_checks {
        map.insert("policy_checks".to_owned(), checks.clone());
    }
    if let Some(day) = &write.access_ref_day {
        map.insert("access_ref_day".to_owned(), json!(day));
    }
    if let Some(seq) = write.access_ref_seq {
        map.insert("access_ref_seq".to_owned(), json!(seq));
    }
    if let Some(deep) = &write.deep {
        map.insert("deep".to_owned(), deep.clone());
    }
    if let Some(expected) = &write.expected_network {
        map.insert("expected_network".to_owned(), json!(expected));
    }
    if let Some(networks) = &write.index_network {
        map.insert("index_network".to_owned(), networks.clone());
    }
    let mut text = serde_json::to_string(&Value::Object(map))
        .map_err(|err| LogError::Unusable(err.to_string()))?;
    // New keys are already in the object, immediately before the summary splice.
    // The splice stays last so a section 14 newline still sits inside the object.
    text.pop();
    if let Some(measures) = &write.measures {
        let encoded =
            serde_json::to_string(measures).map_err(|err| LogError::Unusable(err.to_string()))?;
        text.push_str(",\"measures\":");
        text.push_str(&encoded);
    }
    if let Some(summary) = &write.summary {
        // The record's trailing newline is inside the object. Re-encoding
        // would drop it. `profile` stays, and the section 14 key order stays.
        text.push_str(",\"verify_summary\":");
        text.push_str(summary);
    }
    text.push('}');
    Ok(text.into_bytes())
}

struct DecisionSplit<'a> {
    raw: Vec<&'a [u8]>,
    torn: bool,
    keep: usize,
}

/// One JSON object per decision. Whitespace between objects is the separator.
/// A newline inside `verify_summary` is still inside the object.
fn split_decisions(blob: &[u8]) -> Result<DecisionSplit<'_>, LogError> {
    let mut raw = Vec::new();
    let mut i = 0;
    let mut keep = 0;
    while i < blob.len() {
        let before_gap = i;
        while i < blob.len() && is_json_ws(blob[i]) {
            i += 1;
        }
        if i >= blob.len() {
            keep = blob.len();
            break;
        }
        if blob[i] != b'{' {
            return Err(LogError::NotThisLineage(
                "a decision line is not a JSON object".to_owned(),
            ));
        }
        let start = i;
        match end_of_value(blob, start) {
            Some(end) => {
                raw.push(&blob[start..end]);
                i = end;
                keep = end;
            }
            None => {
                // Drop the incomplete tail. Keep the separator after the
                // previous object so the file still ends on that break.
                keep = before_gap;
                return Ok(DecisionSplit {
                    raw,
                    torn: true,
                    keep,
                });
            }
        }
    }
    Ok(DecisionSplit {
        raw,
        torn: false,
        keep,
    })
}

fn is_json_ws(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// End index of one JSON value starting at `start`, or `None` when the tail
/// is cut off. Strings hide braces. A raw newline outside a string does not
/// end the value.
fn end_of_value(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    while i < bytes.len() {
        let byte = bytes[i];
        i += 1;
        if in_string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

pub fn parse_hex32(text: &str) -> Option<[u8; 32]> {
    let raw = hex::decode(text).ok()?;
    if raw.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&raw);
    Some(out)
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn publish_new(path: &Path, bytes: &[u8]) -> Result<(), LogError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        sync_dir(parent)?;
    }
    Ok(())
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Howard Hinnant's civil-from-days, UTC. Unix day 0 is 1970-01-01.
pub fn utc_day(unix_s: u64) -> String {
    let z = (unix_s / 86_400) as i64;
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-gate-log-{label}-{}-{}",
            std::process::id(),
            unix_now()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn sample(decision: &'static str, tip: u64) -> DecisionWrite {
        let object = b"object-bytes".to_vec();
        DecisionWrite {
            time_unix_s: 1_700_000_000,
            bitcoin_tip_height: Some(tip),
            phase: "fast",
            decision,
            fast_seq: None,
            submission_sha256: [7u8; 32],
            public_key: [9u8; 32],
            object_sha256: canon::sha256(&object),
            object,
            reasons: Vec::new(),
            reason: None,
            summary: Some("{}".to_owned()),
            measures: None,
            policy_id: None,
            policy_checks: None,
            reason_code: None,
            class: None,
            access_ref_day: None,
            access_ref_seq: None,
            deep: None,
            index_network: None,
            expected_network: None,
        }
    }

    /// A line without the 0.5.4 network fields encodes exactly as before,
    /// and a log mixing both shapes opens and replays.
    #[test]
    fn network_fields_are_additive_and_old_shapes_still_parse() {
        let dir = temp_dir("network-fields");
        let mut log = DecisionLog::open(&dir).unwrap();
        let old = sample("provisional", 1);
        let raw = encode_decision(0, GENESIS_HEX, &old).unwrap();
        let text = String::from_utf8(raw).unwrap();
        assert!(!text.contains("index_network") && !text.contains("expected_network"));
        log.append(&old).unwrap();
        let mut new = sample("refuse", 2);
        new.expected_network = Some("main".to_owned());
        new.index_network = Some(serde_json::json!(["synthetic"]));
        log.append(&new).unwrap();
        drop(log);
        let log = DecisionLog::open(&dir).unwrap();
        assert_eq!(log.lines().len(), 2);
        assert!(log.lines()[0].value.get("index_network").is_none());
        assert_eq!(
            log.lines()[1].value["index_network"],
            serde_json::json!(["synthetic"])
        );
        assert_eq!(log.lines()[1].value["expected_network"], "main");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fast_refusal_writes_measures_then_the_record_last() {
        let mut write = sample("refuse", 1);
        let record = "{\"profile\":\"production\",\"N\":1499,\"S\":9557}\n";
        write.summary = Some(record.to_owned());
        write.measures = Some(serde_json::json!({"legs": 1499, "span_s": 9557}));
        let raw = String::from_utf8(encode_decision(0, "00", &write).unwrap()).unwrap();
        let measures_at = raw.find(",\"measures\":").expect("measures");
        let summary_at = raw.find(",\"verify_summary\":").expect("verify_summary");
        assert!(measures_at < summary_at, "{raw}");
        assert!(raw.ends_with(&format!("{record}}}")), "{raw}");
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(value["measures"]["legs"], 1499);
        assert_eq!(value["verify_summary"]["S"], 9557);
    }

    #[test]
    fn unix_epoch_is_the_first_utc_day() {
        assert_eq!(utc_day(0), "1970-01-01");
        assert_eq!(utc_day(86_399), "1970-01-01");
        assert_eq!(utc_day(86_400), "1970-01-02");
    }

    #[test]
    fn a_missing_directory_is_created_empty() {
        let dir = temp_dir("empty");
        assert!(!dir.exists());
        let log = DecisionLog::open(&dir).unwrap();
        assert!(dir.is_dir());
        assert!(log.lines().is_empty());
        assert!(log.latest_bytes().is_none());
        assert!(!dir.join("chain.jsonl").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_v135_chain_is_not_read_or_rewritten() {
        let dir = temp_dir("v135");
        fs::create_dir_all(&dir).unwrap();
        let chain = dir.join("chain.jsonl");
        let original = b"{\"event\":\"gate.decision\",\"seq\":0,\"decision\":\"accept\",\"policy_profile_id\":\"default\"}\n";
        fs::write(&chain, original).unwrap();
        match DecisionLog::open(&dir) {
            Err(LogError::NotThisLineage(_)) => {}
            Err(error) => panic!("expected a foreign log to be refused, got {error:?}"),
            Ok(_) => panic!("a v1.35 chain was opened"),
        }
        assert_eq!(fs::read(&chain).unwrap(), original);
        assert!(!dir.join("latest.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_torn_tail_on_a_v2_line_is_dropped() {
        let dir = temp_dir("torn");
        let mut log = DecisionLog::open(&dir).unwrap();
        let seq = log.append(&sample("provisional", 10)).unwrap();
        assert_eq!(seq, 0);
        let chain = dir.join("chain.jsonl");
        let mut torn = fs::read(&chain).unwrap();
        torn.extend_from_slice(b"{\"torn\":");
        fs::write(&chain, &torn).unwrap();
        let log = DecisionLog::open(&dir).unwrap();
        assert_eq!(log.lines().len(), 1);
        assert!(log.latest_bytes().is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_lower_tip_is_recorded() {
        let dir = temp_dir("tip");
        let mut log = DecisionLog::open(&dir).unwrap();
        log.append(&sample("provisional", 5)).unwrap();
        log.append(&sample("provisional", 4)).unwrap();
        assert_eq!(log.lines().len(), 2);
        assert_eq!(log.lines()[1].value["bitcoin_tip_height"], 4);
        let reopened = DecisionLog::open(&dir).unwrap();
        assert_eq!(reopened.lines().len(), 2);
        assert_eq!(reopened.lines()[1].value["bitcoin_tip_height"], 4);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reopen_rebuilds_a_stale_latest_json() {
        let dir = temp_dir("stale-latest");
        let mut log = DecisionLog::open(&dir).unwrap();
        log.append(&sample("provisional", 5)).unwrap();
        log.append(&sample("provisional", 6)).unwrap();
        drop(log);
        fs::write(
            dir.join("latest.json"),
            br#"{"feed":"pcoc-gate-feed-v2","spec":"Satoshi Hike v2.0","last_seq":0}"#,
        )
        .unwrap();
        let log = DecisionLog::open(&dir).unwrap();
        let latest: serde_json::Value =
            serde_json::from_slice(&log.latest_bytes().expect("latest")).unwrap();
        assert_eq!(latest["last_seq"], 1);
        assert_eq!(log.lines().len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reopen_writes_latest_json_when_the_file_was_removed() {
        let dir = temp_dir("missing-latest");
        let mut log = DecisionLog::open(&dir).unwrap();
        log.append(&sample("provisional", 5)).unwrap();
        log.append(&sample("provisional", 6)).unwrap();
        drop(log);
        fs::remove_file(dir.join("latest.json")).unwrap();
        let log = DecisionLog::open(&dir).unwrap();
        assert_eq!(log.lines().len(), 2);
        let latest: serde_json::Value =
            serde_json::from_slice(&log.latest_bytes().expect("latest")).unwrap();
        assert_eq!(latest["last_seq"], 1);
        assert_eq!(latest["feed"], FEED);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_record_newline_stays_inside_the_decision_and_round_trips() {
        let dir = temp_dir("record-nl");
        let mut log = DecisionLog::open(&dir).unwrap();
        let record = "{\"profile\":\"production\",\"shape\":\"match\",\"N\":\"not defined\"}\n";
        let mut write = sample("refuse", 3);
        write.phase = "thorough";
        write.summary = Some(record.to_owned());
        log.append(&write).unwrap();
        let chain = fs::read(dir.join("chain.jsonl")).unwrap();
        let text = String::from_utf8(chain.clone()).unwrap();
        let marker = "\"verify_summary\":";
        let start = text.find(marker).unwrap() + marker.len();
        assert!(text[start..].starts_with(record));
        assert_eq!(text.as_bytes()[start + record.len()], b'}');
        let reopened = DecisionLog::open(&dir).unwrap();
        assert_eq!(reopened.lines().len(), 1);
        assert_eq!(reopened.lines()[0].raw, chain[..chain.len() - 1]);
        assert_eq!(
            reopened.lines()[0].value["verify_summary"]["profile"],
            "production"
        );
        assert_eq!(
            reopened.lines()[0].value["verify_summary"]["N"],
            "not defined"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
