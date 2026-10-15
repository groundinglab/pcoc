//! `nostr.json` is a pure function of `claims/index.json`.
//! Stage 1 has no name rows, so the file is `{"names":{}}`.

use serde_json::{Map, Value};

pub const EMPTY: &str = "{\"names\":{}}";

/// Rebuild the NIP-05 document from index bytes. Names sort by raw UTF-8.
/// The result has no spaces and no trailing newline.
pub fn from_index(index_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let value: Value =
        serde_json::from_slice(index_bytes).map_err(|err| format!("index is not json: {err}"))?;
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| "index has no items".to_owned())?;
    let mut names: Vec<(String, String)> = Vec::new();
    for item in items {
        if item.get("type").and_then(Value::as_str) != Some("name") {
            continue;
        }
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        let pubkey = item.get("pubkey").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() || pubkey.is_empty() {
            continue;
        }
        names.push((name.to_owned(), pubkey.to_owned()));
    }
    names.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    let mut map = Map::new();
    for (name, pubkey) in names {
        map.insert(name, Value::String(pubkey));
    }
    let body = Value::Object({
        let mut root = Map::new();
        root.insert("names".into(), Value::Object(map));
        root
    });
    serde_json::to_vec(&body).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_index_and_a_shuffled_name_list_are_stable() {
        let empty = br#"{"feed":"groundinglab-claims-v1","items":[]}"#;
        assert_eq!(from_index(empty).unwrap(), EMPTY.as_bytes());
        let shuffled = br#"{"feed":"groundinglab-claims-v1","items":[{"type":"name","id":"bb","pubkey":"22","name":"zeta","created_at":2},{"type":"badge","id":"aa","pubkey":"11","name":null,"created_at":1},{"type":"name","id":"cc","pubkey":"33","name":"alpha","created_at":3}]}"#;
        let again = br#"{"feed":"groundinglab-claims-v1","items":[{"type":"name","id":"cc","pubkey":"33","name":"alpha","created_at":9},{"type":"name","id":"bb","pubkey":"22","name":"zeta","created_at":1}]}"#;
        assert_eq!(from_index(shuffled).unwrap(), from_index(again).unwrap());
        assert_eq!(
            from_index(shuffled).unwrap(),
            br#"{"names":{"alpha":"33","zeta":"22"}}"#
        );
    }
}
