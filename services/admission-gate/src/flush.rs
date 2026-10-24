//! One-time challenges for a signed flush of one identity's login cache.
//!
//! A row is keyed by nonce. A successful submit consumes it. A bad signature
//! does not, so the holder can retry inside the window. This store is not
//! the decision log and it is not the sign-in result table.

use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub origin: String,
    pub exp: u64,
    pub consumed: bool,
}

pub struct Challenges {
    rows: Mutex<HashMap<[u8; 32], Row>>,
}

impl Challenges {
    pub fn new() -> Self {
        Self {
            rows: Mutex::new(HashMap::new()),
        }
    }

    /// Store a fresh nonce. A collision with an open row draws again.
    pub fn mint(&self, origin: String, exp: u64) -> Result<[u8; 32], String> {
        let mut rows = self.rows.lock().expect("flush challenges");
        for _ in 0..8 {
            let nonce = random_nonce()?;
            if rows.contains_key(&nonce) {
                continue;
            }
            rows.insert(
                nonce,
                Row {
                    origin,
                    exp,
                    consumed: false,
                },
            );
            return Ok(nonce);
        }
        Err("flush challenge nonce collided".to_owned())
    }

    pub fn get(&self, nonce: &[u8; 32]) -> Option<Row> {
        self.rows
            .lock()
            .expect("flush challenges")
            .get(nonce)
            .cloned()
    }

    /// Mark the nonce consumed. `false` when it is missing or already consumed.
    pub fn consume_fresh(&self, nonce: &[u8; 32]) -> bool {
        let mut rows = self.rows.lock().expect("flush challenges");
        let Some(row) = rows.get_mut(nonce) else {
            return false;
        };
        if row.consumed {
            return false;
        }
        row.consumed = true;
        true
    }
}

fn random_nonce() -> Result<[u8; 32], String> {
    let mut nonce = [0u8; 32];
    let mut file = std::fs::File::open("/dev/urandom").map_err(|err| err.to_string())?;
    file.read_exact(&mut nonce).map_err(|err| err.to_string())?;
    Ok(nonce)
}
