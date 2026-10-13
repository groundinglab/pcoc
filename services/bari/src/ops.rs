//! Operator-visible Bari lifecycle lines.
//!
//! The crate has no `tracing` or `log` facade. Live process output is stderr
//! (`eprintln`), the same stream as the `tip height=` line in `main`. These
//! events are unconditional: there is no env var and no feature flag.
//!
//! Each line is one JSON object. `event` is a `bari.ops.` name. `level` is
//! `info` or `warn`. Leaf and root ids are natural-order hex (`encode_hash32`).
//! Tip hashes, txids, and block hashes are Bitcoin display hex
//! (`encode_bitcoin_id`), the same split `events.jsonl` already uses.

use serde_json::{Map, Value};

pub const WAYPOINT_ACCEPT: &str = "bari.ops.waypoint.accept";
pub const WAYPOINT_REJECT: &str = "bari.ops.waypoint.reject";
pub const OPEN_POOL_ADMIT: &str = "bari.ops.open_pool.admit";
pub const SEAL: &str = "bari.ops.seal";
/// One line per seal decision: block D, its p10, the trigger, the bid, and
/// `plant`, `defer`, or `empty`. Distinct from [`SEAL`] so an empty handoff
/// still has one `pool_empty_handoff` line.
pub const SEAL_DECISION: &str = "bari.ops.seal.decision";
pub const PLANT_UTXO: &str = "bari.ops.plant.utxo";
pub const PLANT_BROADCAST: &str = "bari.ops.plant.broadcast";
pub const PLANT_INCLUDED: &str = "bari.ops.plant.included";
pub const PLANT_SKIP: &str = "bari.ops.plant.skip";
pub const PLANT_DEFERRED: &str = "bari.ops.plant.deferred";
/// `estimatesmartfee` failed or returned no usable feerate. The mempool
/// minimum still gates. Not emitted when the estimate is present.
pub const FEE_ESTIMATE: &str = "bari.ops.fee.estimate";
pub const PATH_SERVE: &str = "bari.ops.path.serve";
/// No longer emitted. A confirmed path stays servable. The name remains so a
/// test can show the archive line is absent.
#[cfg_attr(not(test), allow(dead_code))]
pub const PATH_ARCHIVE: &str = "bari.ops.path.archive";
pub const CONFIRMATION_WINDOW_MISSED: &str = "bari.ops.confirmation_window.missed";
/// One warn line per reconciliation rule per tip when the operator page
/// finds that rule does not hold.
pub const PAGE_CHECK: &str = "bari.ops.page.check";

pub fn info(event: &str, fields: Value) {
    emit("info", event, fields);
}

pub fn warn(event: &str, fields: Value) {
    emit("warn", event, fields);
}

fn emit(level: &str, event: &str, fields: Value) {
    let line = match format_line(level, event, fields) {
        Ok(line) => line,
        Err(error) => format!(
            "{{\"event\":{event:?},\"level\":\"warn\",\"error\":\"ops serialize failed: {error}\"}}"
        ),
    };
    #[cfg(test)]
    {
        recorded()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(line.clone());
    }
    eprintln!("{line}");
}

fn format_line(level: &str, event: &str, fields: Value) -> Result<String, serde_json::Error> {
    let mut out = Map::new();
    out.insert("event".to_owned(), Value::String(event.to_owned()));
    out.insert("level".to_owned(), Value::String(level.to_owned()));
    if let Value::Object(map) = fields {
        for (key, value) in map {
            if key != "event" && key != "level" {
                out.insert(key, value);
            }
        }
    } else {
        out.insert("fields".to_owned(), fields);
    }
    serde_json::to_string(&Value::Object(out))
}

#[cfg(test)]
fn recorded() -> &'static std::sync::Mutex<Vec<String>> {
    static LINES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    &LINES
}

#[cfg(test)]
pub(crate) fn lines_containing(needle: &str) -> Vec<String> {
    recorded()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .filter(|line| line.contains(needle))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn info_line_is_one_json_object_with_event_and_level() {
        info(
            SEAL,
            json!({
                "tree_size": 2,
                "leaves": ["aa", "bb"],
            }),
        );
        let line = lines_containing(SEAL)
            .into_iter()
            .filter(|line| line.contains("\"event\":\"bari.ops.seal\""))
            .filter(|line| !line.contains("\"event\":\"bari.ops.seal.decision\""))
            .last()
            .expect("seal line");
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["event"], SEAL);
        assert_eq!(value["level"], "info");
        assert_eq!(value["tree_size"], 2);
        assert_eq!(value["leaves"][0], "aa");
        assert!(line.starts_with('{') && line.ends_with('}'));
        assert!(!line.contains('\n'));
    }

    #[test]
    fn warn_level_is_explicit() {
        warn(WAYPOINT_REJECT, json!({"reason": "header required"}));
        let line = lines_containing(WAYPOINT_REJECT)
            .into_iter()
            .last()
            .expect("reject line");
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["level"], "warn");
        assert_eq!(value["reason"], "header required");
    }
}
