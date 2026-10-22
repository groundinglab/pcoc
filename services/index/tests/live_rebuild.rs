//! Live evidence. Ignored by `cargo test`. Run it by hand:
//!
//! ```text
//! cargo test --locked --release --test live_rebuild -- --ignored --nocapture
//! ```
//!
//! Rebuilds heights 971497..=971516 from `https://mempool.space/api` (the
//! window `~/pcoc-build/pepe.cbor` asks the index for, plus one block), diffs
//! them against `https://index.groundinglab.io`, and runs `pcoc verify`
//! against both. Off-chain fields are the only allowed difference:
//! `header.timestamp_unix_ms` and `plant.tree_size` / `plants[].tree_size`.
//! A block with more than one plant may also differ in `plants` order and in
//! which plant the single `plant` member names. Bari writes serving-batch
//! order. A chain scan writes transaction order. The verifier matches any
//! one plant (specification P14.5).

use pcoc_index::chain::{
    EsploraSource, LimitedGet, UreqGet, DEFAULT_ESPLORA_BASE, DEFAULT_MAX_RETRIES,
    DEFAULT_MIN_INTERVAL_MS, DEFAULT_REORG_DEPTH,
};
use pcoc_index::rebuild::{rebuild, RebuildOptions};
use serde_json::{Map, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const FROM: u32 = 971_497;
const TO: u32 = 971_516;
const HOSTED: &str = "https://index.groundinglab.io";
const PCOC: &str = "/home/christian/pcoc-build/pepe.cbor";

#[test]
#[ignore = "live mempool.space and index.groundinglab.io; not part of CI"]
fn rebuild_matches_hosted_index_and_the_verifier() {
    let out = tempfile::tempdir().unwrap();
    let http = LimitedGet::new(
        UreqGet::new(Duration::from_secs(90)),
        Duration::from_millis(DEFAULT_MIN_INTERVAL_MS),
        DEFAULT_MAX_RETRIES,
    );
    let source = EsploraSource::new(DEFAULT_ESPLORA_BASE, http);
    let report = rebuild(
        &source,
        &RebuildOptions {
            from_height: FROM,
            to_height: TO,
            out_dir: out.path().to_path_buf(),
            reorg_depth: DEFAULT_REORG_DEPTH,
        },
    )
    .unwrap_or_else(|error| panic!("rebuild: {error}"));
    eprintln!(
        "rebuild wrote {} unchanged {} reorg {}",
        report.written, report.unchanged, report.rewritten
    );

    let mut diffs = Vec::new();
    for height in FROM..=TO {
        let rebuilt = fs::read(out.path().join(format!("height-{height:010}.json")))
            .unwrap_or_else(|error| panic!("missing rebuilt {height}: {error}"));
        let hosted = http_get(&format!("{HOSTED}/index/{height}"));
        let left: Value = serde_json::from_slice(&rebuilt).unwrap();
        let right: Value = serde_json::from_slice(hosted.trim().as_bytes()).unwrap();
        if let Some(diff) = compare(&left, &right) {
            diffs.push(format!("{height}: {diff}"));
        }
    }
    assert!(
        diffs.is_empty(),
        "rebuilt index differs from {HOSTED} beyond off-chain fields:\n{}",
        diffs.join("\n")
    );

    let pcoc = Path::new(PCOC);
    assert!(pcoc.is_file(), "sample PCOC missing at {PCOC}");
    let verify = verify_bin();
    let hosted_record = run_verify(&verify, pcoc, HOSTED);
    let bind = "127.0.0.1:38091";
    let mut child = Command::new(env!("CARGO_BIN_EXE_pcoc-index"))
        .env("PCOC_INDEX_DATA_DIR", out.path())
        .env("PCOC_INDEX_BIND_ADDR", bind)
        .spawn()
        .unwrap_or_else(|error| panic!("serve rebuilt index: {error}"));
    wait_until_up(&format!("http://{bind}/index/{FROM}"));
    let rebuilt_record = run_verify(&verify, pcoc, &format!("http://{bind}"));
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(
        hosted_record, rebuilt_record,
        "verifier records differ\nhosted:\n{hosted_record}\nrebuilt:\n{rebuilt_record}"
    );
    eprintln!("verifier records match ({} bytes)", hosted_record.len());
}

fn compare(rebuilt: &Value, hosted: &Value) -> Option<String> {
    let mut rebuilt = rebuilt.clone();
    let mut hosted = hosted.clone();
    strip_off_chain(&mut rebuilt);
    strip_off_chain(&mut hosted);
    // Multi-plant order is Bari's serving-batch order, not chain order.
    let rebuilt_set = plant_set(&rebuilt);
    let hosted_set = plant_set(&hosted);
    if rebuilt_set != hosted_set {
        return Some(format!(
            "plant set rebuilt={rebuilt_set:?} hosted={hosted_set:?}"
        ));
    }
    drop_plants(&mut rebuilt);
    drop_plants(&mut hosted);
    if rebuilt == hosted {
        None
    } else {
        Some(format!("document rebuilt={rebuilt} hosted={hosted}"))
    }
}

fn strip_off_chain(value: &mut Value) {
    if let Some(header) = value.get_mut("header").and_then(Value::as_object_mut) {
        header.remove("timestamp_unix_ms");
    }
    strip_tree_size(value.get_mut("plant"));
    if let Some(plants) = value.get_mut("plants").and_then(Value::as_array_mut) {
        for plant in plants {
            strip_tree_size(Some(plant));
        }
    }
}

fn strip_tree_size(slot: Option<&mut Value>) {
    if let Some(Value::Object(plant)) = slot {
        plant.remove("tree_size");
    }
}

fn plant_set(value: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(plants) = value.get("plants").and_then(Value::as_array) {
        for plant in plants {
            keys.push(plant_key(plant));
        }
    } else if let Some(plant) = value.get("plant").filter(|plant| plant.is_object()) {
        keys.push(plant_key(plant));
    }
    keys.sort();
    keys
}

fn plant_key(plant: &Value) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        plant.get("root").and_then(Value::as_str).unwrap_or(""),
        plant.get("txid").and_then(Value::as_str).unwrap_or(""),
        plant.get("vout").and_then(Value::as_u64).unwrap_or(0),
        plant
            .get("block_hash")
            .and_then(Value::as_str)
            .unwrap_or(""),
        plant
            .get("block_height")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    )
}

fn drop_plants(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    object.remove("plants");
    if let Some(plant) = object.get_mut("plant") {
        if plant.is_object() {
            *plant = Value::Object(Map::new());
        }
    }
}

fn http_get(url: &str) -> String {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build();
    agent
        .get(url)
        .call()
        .unwrap_or_else(|error| panic!("GET {url}: {error}"))
        .into_string()
        .unwrap()
}

fn wait_until_up(url: &str) {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(2))
        .build();
    for _ in 0..50 {
        if agent.get(url).call().is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("rebuilt index did not answer {url}");
}

fn verify_bin() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("PCOC_VERIFY") {
        return Path::new(&path).to_path_buf();
    }
    let go_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/go");
    let dest = std::env::temp_dir().join("pcoc-verify-live");
    let go = std::env::var("GO_BIN").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
        format!("{home}/opt/go/bin/go")
    });
    let status = Command::new(&go)
        .args(["build", "-o"])
        .arg(&dest)
        .arg("./cmd/pcoc")
        .current_dir(&go_dir)
        .status()
        .unwrap_or_else(|error| panic!("build pcoc with {go}: {error}"));
    assert!(status.success(), "go build ./cmd/pcoc failed");
    dest
}

fn run_verify(bin: &Path, pcoc: &Path, index_url: &str) -> String {
    let output = Command::new(bin)
        .args([
            "--input",
            pcoc.to_str().unwrap(),
            "--index-url",
            index_url,
            "--profile",
            "live",
            "--audit-seed",
            &"11".repeat(32),
            "--log",
            "quiet",
        ])
        .output()
        .unwrap_or_else(|error| panic!("run {}: {error}", bin.display()));
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success(),
        "verify against {index_url} failed: {stderr}\n{stdout}"
    );
    stdout
}
