//! Single-use nonce rows for the locked §15.1 wire.
//!
//! A row is keyed by `nonce`. The compare-and-set that consumes a fresh
//! nonce is the single-use lock. It does not admit a provisional grant.
//! A result read of an unknown nonce returns `pending` and writes nothing.

use crate::wire::{self, ClientClass, OUTCOME_PENDING};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub origin: String,
    pub exp: u64,
    pub object_sha: [u8; 32],
    pub signature: [u8; 64],
    pub client: ClientClass,
    pub outcome: String,
    /// Set when the check order consumes the nonce. A recorded non-consuming
    /// failure has no stamp. A later consuming submit replaces that outcome.
    pub consumed: bool,
    /// Unix second when `outcome` left `pending`. `None` while it is pending.
    pub settled_at: Option<u64>,
    /// Holder key recorded when a consuming submit knows the key.
    pub public_key: Option<[u8; 32]>,
    /// Bookend `N` when the verifier record has a number.
    pub legs: Option<i64>,
    /// Bookend `S` when the verifier record has a number.
    pub span_s: Option<i64>,
    /// Decision line that opens a claim window. Set on accept, return, and echo.
    pub log_day: Option<String>,
    pub log_seq: Option<u64>,
    /// Thorough progress, published once the walks finish.
    pub deep: Option<Deep>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deep {
    pub runs_done: u64,
    pub legs_matched: u64,
    pub legs_total: u64,
    pub salt: String,
    pub indices: Vec<Vec<u64>>,
    pub commitment: String,
}

pub struct SignInRows {
    rows: Mutex<HashMap<[u8; 32], Row>>,
    /// Unix second at which this store could have lost rows. Moves forward
    /// on process start and on [`Self::flush`].
    store_start: Mutex<u64>,
    retention_s: u64,
    skew_s: u64,
}

impl SignInRows {
    /// Test helper. Retention does not expire rows. Skew is 0. Start is 0.
    /// A door uses [`Self::open`] with the policy values.
    pub fn new() -> Self {
        Self::open(0, u64::MAX, 0)
    }

    /// `now` is the nonce-store start. `retention_s` and `skew_s` come from
    /// the loaded policy.
    pub fn open(now: u64, retention_s: u64, skew_s: u64) -> Self {
        Self {
            rows: Mutex::new(HashMap::new()),
            store_start: Mutex::new(now),
            retention_s,
            skew_s,
        }
    }

    pub fn store_start(&self) -> u64 {
        *self.store_start.lock().expect("nonce store start")
    }

    pub fn retention_seconds(&self) -> u64 {
        self.retention_s
    }

    pub fn skew_seconds(&self) -> u64 {
        self.skew_s
    }

    /// Drop every row and move the start forward. A flush with no new process.
    pub fn flush(&self, now: u64) {
        self.rows.lock().expect("sign-in rows").clear();
        self.note_loss(now);
    }

    /// The store could have lost rows. The start only moves forward.
    pub fn note_loss(&self, now: u64) {
        let mut start = self.store_start.lock().expect("nonce store start");
        if now > *start {
            *start = now;
        }
    }

    /// Replace the nonce-store start. Production records it at process open
    /// and only moves it forward ([`Self::note_loss`]). A test that pins the
    /// door clock to a fixture epoch calls this so the third window
    /// comparison uses that epoch, not the wall clock.
    pub fn set_start(&self, now: u64) {
        *self.store_start.lock().expect("nonce store start") = now;
    }

    fn prune(&self, rows: &mut HashMap<[u8; 32], Row>, now: u64) {
        let retention = self.retention_s;
        let skew = self.skew_s;
        rows.retain(|_, row| row_live(row, now, retention, skew));
    }

    pub fn outcome(&self, nonce: &[u8; 32], now: u64) -> String {
        let mut rows = self.rows.lock().expect("sign-in rows");
        self.prune(&mut rows, now);
        rows.get(nonce)
            .map(|row| row.outcome.clone())
            .unwrap_or_else(|| OUTCOME_PENDING.to_owned())
    }

    /// Step 2 and the transport edge. Store `malformed` or `rejected` only
    /// when the object bytes hash to `object` and this nonce has no consumed
    /// row. A hash mismatch, and a consumed nonce, leave the stored outcome.
    pub fn note_unconsumed(&self, nonce: &[u8; 32], hash_matches: bool, outcome: &str, now: u64) {
        if !hash_matches {
            return;
        }
        let mut rows = self.rows.lock().expect("sign-in rows");
        self.prune(&mut rows, now);
        if rows.get(nonce).is_some_and(|row| row.consumed) {
            return;
        }
        let settled_at = (outcome != OUTCOME_PENDING).then_some(now);
        match rows.get_mut(nonce) {
            Some(row) => {
                row.outcome = outcome.to_owned();
                row.settled_at = settled_at;
            }
            None => {
                rows.insert(
                    *nonce,
                    Row {
                        origin: String::new(),
                        exp: 0,
                        object_sha: [0u8; 32],
                        signature: [0u8; 64],
                        client: ClientClass::Malformed,
                        outcome: outcome.to_owned(),
                        consumed: false,
                        settled_at,
                        public_key: None,
                        legs: None,
                        span_s: None,
                        log_day: None,
                        log_seq: None,
                        deep: None,
                    },
                );
            }
        }
    }

    /// Step 6. A signature miss is not a consume. A later consuming submit
    /// replaces this outcome.
    pub fn note_bad_signature(&self, body: &wire::LockedBody, now: u64) -> Option<Replay> {
        let mut rows = self.rows.lock().expect("sign-in rows");
        self.prune(&mut rows, now);
        if let Some(row) = rows.get(&body.nonce) {
            if row.consumed {
                return Some(replay_of(row, &body.object_sha, &body.signature));
            }
            let row = rows.get_mut(&body.nonce).expect("row");
            row.outcome = wire::OUTCOME_BAD_SIGNATURE.to_owned();
            row.client = ClientClass::BadSignature;
            row.settled_at = Some(now);
            row.object_sha = body.object_sha;
            row.signature = body.signature;
            row.origin = body.origin.clone();
            row.exp = body.exp;
            return None;
        }
        rows.insert(
            body.nonce,
            Row {
                origin: body.origin.clone(),
                exp: body.exp,
                object_sha: body.object_sha,
                signature: body.signature,
                client: ClientClass::BadSignature,
                outcome: wire::OUTCOME_BAD_SIGNATURE.to_owned(),
                consumed: false,
                settled_at: Some(now),
                public_key: None,
                legs: None,
                span_s: None,
                log_day: None,
                log_seq: None,
                deep: None,
            },
        );
        None
    }

    /// Compare-and-set. The winner consumes the nonce. A nonce that is already
    /// consumed comes back as a replay and is not consumed again.
    ///
    /// The lock is held only for the map update. Policy runs after this
    /// returns, so the set does not admit a provisional grant. Two racers
    /// serialize on the mutex: the second sees `consumed` and replays.
    pub fn consume(
        &self,
        body: &wire::LockedBody,
        client: ClientClass,
        outcome: &str,
        now: u64,
    ) -> Result<(), Replay> {
        let mut rows = self.rows.lock().expect("sign-in rows");
        self.prune(&mut rows, now);
        if let Some(row) = rows.get(&body.nonce) {
            if row.consumed {
                return Err(replay_of(row, &body.object_sha, &body.signature));
            }
        }
        let settled_at = (outcome != OUTCOME_PENDING).then_some(now);
        let next = Row {
            origin: body.origin.clone(),
            exp: body.exp,
            object_sha: body.object_sha,
            signature: body.signature,
            client,
            outcome: outcome.to_owned(),
            consumed: true,
            settled_at,
            public_key: None,
            legs: None,
            span_s: None,
            log_day: None,
            log_seq: None,
            deep: None,
        };
        rows.insert(body.nonce, next);
        Ok(())
    }

    /// Replace a consumed row's website outcome after policy settles.
    /// A nonce that is not consumed is left alone.
    pub fn settle(&self, nonce: &[u8; 32], outcome: &str, now: u64) -> bool {
        let mut rows = self.rows.lock().expect("sign-in rows");
        let Some(row) = rows.get_mut(nonce) else {
            return false;
        };
        if !row.consumed {
            return false;
        }
        row.outcome = outcome.to_owned();
        if outcome != OUTCOME_PENDING {
            row.settled_at = Some(now);
        }
        true
    }

    /// Step 9. The key is known once the nonce is consumed. A row that is
    /// not consumed is left alone.
    pub fn note_holder(&self, nonce: &[u8; 32], public_key: [u8; 32]) {
        let mut rows = self.rows.lock().expect("sign-in rows");
        let Some(row) = rows.get_mut(nonce) else {
            return;
        };
        if !row.consumed {
            return;
        }
        row.public_key = Some(public_key);
    }

    /// Bookend pair from a verifier record. Stored only on a consumed row.
    /// The result read publishes them only after `accepted`.
    pub fn note_bookends(&self, nonce: &[u8; 32], legs: Option<i64>, span_s: Option<i64>) {
        let mut rows = self.rows.lock().expect("sign-in rows");
        let Some(row) = rows.get_mut(nonce) else {
            return;
        };
        if !row.consumed {
            return;
        }
        row.legs = legs;
        row.span_s = span_s;
    }

    /// The line that opens the claim window. A return and an echo omit measurements.
    pub fn note_access(&self, nonce: &[u8; 32], day: Option<String>, seq: Option<u64>) {
        let mut rows = self.rows.lock().expect("sign-in rows");
        let Some(row) = rows.get_mut(nonce) else {
            return;
        };
        if !row.consumed {
            return;
        }
        row.log_day = day;
        row.log_seq = seq;
    }

    /// `GET /v1/sign-in/result/<nonce>`.
    ///
    /// An unknown nonce is exactly `{"outcome":"pending"}`. A consumed row
    /// that is still pending adds `submitted`. `accepted` adds the holder
    /// key, the object hash, and numeric bookends. Other tokens stay one field.
    pub fn result_value(&self, nonce: &[u8; 32], now: u64) -> Value {
        let mut rows = self.rows.lock().expect("sign-in rows");
        self.prune(&mut rows, now);
        let Some(row) = rows.get(nonce) else {
            return json!({ "outcome": OUTCOME_PENDING });
        };
        let mut body = serde_json::Map::new();
        body.insert("outcome".to_owned(), json!(row.outcome));
        if row.consumed && row.outcome == OUTCOME_PENDING {
            body.insert("submitted".to_owned(), json!(true));
        }
        if row.outcome == wire::OUTCOME_ACCEPTED {
            if let Some(key) = row.public_key {
                body.insert("public_key".to_owned(), json!(crate::canon::hex32(&key)));
            }
            body.insert(
                "object_sha256".to_owned(),
                json!(crate::canon::hex32(&row.object_sha)),
            );
            if let Some(legs) = row.legs {
                body.insert("legs".to_owned(), json!(legs));
            }
            if let Some(span_s) = row.span_s {
                body.insert("span_s".to_owned(), json!(span_s));
            }
            if let Some(day) = &row.log_day {
                body.insert("log_day".to_owned(), json!(day));
            }
            if let Some(seq) = row.log_seq {
                body.insert("log_seq".to_owned(), json!(seq));
            }
        }
        Value::Object(body)
    }

    pub fn note_deep(&self, nonce: &[u8; 32], deep: Deep) {
        let mut rows = self.rows.lock().expect("sign-in rows");
        let Some(row) = rows.get_mut(nonce) else {
            return;
        };
        if row.consumed {
            row.deep = Some(deep);
        }
    }

    pub fn consumed(&self, nonce: &[u8; 32]) -> bool {
        self.rows
            .lock()
            .expect("sign-in rows")
            .get(nonce)
            .is_some_and(|row| row.consumed)
    }

    /// Step 4, before a later step can replace the row. `None` means this
    /// nonce has no consumed row.
    pub fn replay(&self, body: &wire::LockedBody, now: u64) -> Option<Replay> {
        let mut rows = self.rows.lock().expect("sign-in rows");
        self.prune(&mut rows, now);
        let row = rows.get(&body.nonce)?;
        if !row.consumed {
            return None;
        }
        Some(replay_of(row, &body.object_sha, &body.signature))
    }
}

impl Default for SignInRows {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Replay {
    /// Same object and same signature. Return the original client class and
    /// the original website outcome. Do not run policy again.
    Same {
        client: ClientClass,
        outcome: String,
    },
    /// Any other object or signature. Client `malformed`. The stored outcome
    /// stays.
    Other,
}

fn replay_of(row: &Row, object_sha: &[u8; 32], signature: &[u8; 64]) -> Replay {
    if row.object_sha == *object_sha && row.signature == *signature {
        Replay::Same {
            client: row.client,
            outcome: row.outcome.clone(),
        }
    } else {
        Replay::Other
    }
}

fn row_live(row: &Row, now: u64, retention_s: u64, skew_s: u64) -> bool {
    // A consumed row stays through the challenge window, even when retention
    // is shorter, so a replay inside the window still finds the row.
    if row.consumed {
        let until = u128::from(row.exp).saturating_add(u128::from(skew_s));
        if u128::from(now) < until {
            return true;
        }
    }
    match row.settled_at {
        Some(at) if row.outcome != OUTCOME_PENDING => {
            u128::from(now) < u128::from(at).saturating_add(u128::from(retention_s))
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon::hex32;
    use crate::wire::{ClientClass, LockedBody, OUTCOME_ACCEPTED};

    fn body(nonce: [u8; 32]) -> LockedBody {
        LockedBody {
            origin: "https://groundinglab.io".to_owned(),
            nonce,
            exp: 10,
            object_sha: [9u8; 32],
            object: vec![1, 2, 3],
            signature: [7u8; 64],
        }
    }

    #[test]
    fn result_value_adds_measurements_only_on_accepted() {
        let rows = SignInRows::new();
        let nonce = [1u8; 32];
        assert_eq!(
            rows.result_value(&nonce, 1),
            json!({ "outcome": "pending" })
        );

        rows.consume(&body(nonce), ClientClass::DeliveryOk, OUTCOME_PENDING, 1)
            .unwrap();
        let pending = rows.result_value(&nonce, 1);
        assert_eq!(pending["outcome"], "pending");
        assert_eq!(pending["submitted"], true);
        assert!(pending.get("public_key").is_none());
        assert!(pending.get("legs").is_none());

        let key = [4u8; 32];
        rows.note_holder(&nonce, key);
        rows.note_bookends(&nonce, Some(1348), Some(5294));
        let still = rows.result_value(&nonce, 1);
        assert_eq!(still["submitted"], true);
        assert!(still.get("public_key").is_none());
        assert!(still.get("legs").is_none());

        assert!(rows.settle(&nonce, OUTCOME_ACCEPTED, 2));
        let accepted = rows.result_value(&nonce, 2);
        assert_eq!(accepted["outcome"], "accepted");
        assert!(accepted.get("submitted").is_none());
        assert!(accepted.get("leg_rate_floor").is_none());
        assert_eq!(accepted["legs"], 1348);
        assert_eq!(accepted["span_s"], 5294);
        assert_eq!(accepted["public_key"], hex32(&key));
        assert_eq!(accepted["object_sha256"], hex32(&[9u8; 32]));
    }

    #[test]
    fn note_holder_ignores_an_unconsumed_row() {
        let rows = SignInRows::new();
        let nonce = [2u8; 32];
        rows.note_unconsumed(&nonce, true, wire::OUTCOME_REJECTED, 1);
        rows.note_holder(&nonce, [3u8; 32]);
        rows.note_bookends(&nonce, Some(1), Some(2));
        let body = rows.result_value(&nonce, 1);
        assert_eq!(body, json!({ "outcome": "rejected" }));
    }
}
