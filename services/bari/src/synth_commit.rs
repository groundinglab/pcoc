//! Test-plane plant. A simulated commit under synth tips. No Bitcoin Core.
//!
//! Live plant and inclusion stay on [`crate::bitcoin_rpc::BitcoinRpc`]. This
//! backend is the test command path only. Header wire magic on the synth peer
//! may still be Bitcoin's regtest magic. That magic is not a Core node.

use crate::clock::{ChainBackend, ChainError, ChainPostCheck, ChainTip, PlantError, PlantReceipt};
use crate::proof::{commitment_script, CanonicalPathBody};
use crate::storage::StoredInclusion;
use crate::Hash32;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

/// Portable-index `network` for a test-plane tip. Verifiers accept this name.
/// It is not Bitcoin Core regtest or testnet4.
pub const SYNTH_INDEX_NETWORK: &str = "synthetic";

/// `StoredPlant::fee_source` for a simulated commit. No fee was paid to Core.
pub const SYNTH_FEE_SOURCE: &str = "synth-commit";

struct Simulated {
    root: Hash32,
    vout: u32,
    /// Set on the inclusion tip (the tip after the seal). Absent until then.
    inclusion: Option<StoredInclusion>,
}

struct Ledger {
    by_txid: HashMap<Hash32, Simulated>,
}

/// In-process commitment surface for the test plane.
///
/// `plant` records one commit and does not broadcast. `included_in_tip`
/// anchors that commit into the tip the notary asks about, once. The clock
/// asks on the next continuous tip and does not ask again. A later tip does
/// not pick the commit up.
pub struct SynthCommit {
    plants: AtomicUsize,
    best_tip_calls: AtomicUsize,
    next_plant: AtomicU64,
    ledger: Mutex<Ledger>,
}

impl SynthCommit {
    pub fn new() -> Self {
        Self {
            plants: AtomicUsize::new(0),
            best_tip_calls: AtomicUsize::new(0),
            next_plant: AtomicU64::new(1),
            ledger: Mutex::new(Ledger {
                by_txid: HashMap::new(),
            }),
        }
    }

    pub fn plant_count(&self) -> usize {
        self.plants.load(Ordering::SeqCst)
    }

    pub fn best_tip_calls(&self) -> usize {
        self.best_tip_calls.load(Ordering::SeqCst)
    }

    fn ledger(&self) -> std::sync::MutexGuard<'_, Ledger> {
        self.ledger
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for SynthCommit {
    fn default() -> Self {
        Self::new()
    }
}

/// Domain-separated id for one simulated commit. Not a Core transaction id.
fn simulated_txid(root: &Hash32, plant_number: u64) -> Hash32 {
    let mut hasher = Sha256::new();
    hasher.update(b"pcoc-synth-commit-v1");
    hasher.update(root);
    hasher.update(plant_number.to_be_bytes());
    let first = hasher.finalize();
    Sha256::digest(first).into()
}

impl ChainBackend for SynthCommit {
    fn network(&self) -> Result<String, ChainError> {
        Ok(SYNTH_INDEX_NETWORK.to_owned())
    }

    fn initial_block_download(&self) -> Result<bool, ChainError> {
        Ok(false)
    }

    fn best_tip(&self) -> Result<ChainTip, ChainError> {
        self.best_tip_calls.fetch_add(1, Ordering::SeqCst);
        Err(ChainError(
            "synth commit does not read a Bitcoin Core tip; test tips come from the synth sequencer"
                .to_owned(),
        ))
    }

    fn plant(&self, root: &Hash32) -> Result<PlantReceipt, PlantError> {
        let plant_number = self.next_plant.fetch_add(1, Ordering::SeqCst);
        let txid = simulated_txid(root, plant_number);
        let mut ledger = self.ledger();
        ledger.by_txid.insert(
            txid,
            Simulated {
                root: *root,
                vout: 0,
                inclusion: None,
            },
        );
        self.plants.fetch_add(1, Ordering::SeqCst);
        Ok(PlantReceipt {
            txid,
            vout: 0,
            fee_rate_sat_vb: 0.0,
            fee_source: SYNTH_FEE_SOURCE.to_owned(),
            fee_cap_sat_vb: None,
            fee_sat: None,
            vsize: None,
        })
    }

    fn included_in_tip(
        &self,
        txid: &Hash32,
        tip: &ChainTip,
    ) -> Result<Option<StoredInclusion>, ChainError> {
        let mut ledger = self.ledger();
        let Some(planted) = ledger.by_txid.get_mut(txid) else {
            return Ok(None);
        };
        if let Some(existing) = planted.inclusion.clone() {
            return Ok(
                (existing.block_hash == tip.hash && existing.block_height == tip.height)
                    .then_some(existing),
            );
        }
        let inclusion = StoredInclusion {
            block_hash: tip.hash,
            block_height: tip.height,
        };
        planted.inclusion = Some(inclusion.clone());
        Ok(Some(inclusion))
    }

    fn block_is_selected(&self, block_hash: &Hash32) -> Result<bool, ChainError> {
        let ledger = self.ledger();
        Ok(ledger.by_txid.values().any(|planted| {
            planted
                .inclusion
                .as_ref()
                .is_some_and(|inclusion| inclusion.block_hash == *block_hash)
        }))
    }

    fn post_check(&self, body: &CanonicalPathBody) -> Result<ChainPostCheck, ChainError> {
        let ledger = self.ledger();
        let Some(planted) = ledger.by_txid.get(&body.txid) else {
            return Ok(ChainPostCheck {
                transaction_in_block: false,
                output_exists: false,
                zero_value: false,
                exact_script: false,
                selected_chain: false,
                carried_height: body.block_height,
                authenticated_height: None,
                height_matches: None,
            });
        };
        let script_matches = planted.root == body.root
            && planted.vout == body.vout
            && commitment_script(&planted.root) == commitment_script(&body.root);
        let authenticated_height = planted.inclusion.as_ref().and_then(|inclusion| {
            (inclusion.block_hash == body.block_hash).then_some(inclusion.block_height)
        });
        let transaction_in_block = authenticated_height.is_some();
        let output_ok = script_matches && transaction_in_block;
        Ok(ChainPostCheck {
            transaction_in_block,
            output_exists: output_ok,
            zero_value: output_ok,
            exact_script: output_ok,
            selected_chain: transaction_in_block,
            carried_height: body.block_height,
            authenticated_height,
            height_matches: authenticated_height.map(|height| height == body.block_height),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::BariClock;
    use crate::storage::FileStore;
    use std::sync::Arc;

    fn tip(byte: u8, previous: Option<u8>, height: u32) -> ChainTip {
        ChainTip {
            hash: [byte; 32],
            previous_hash: previous.map(|value| [value; 32]),
            height,
            time: None,
            bits: 0,
            period_first_time: 0,
        }
    }

    fn open(backend: Arc<SynthCommit>, tip0: ChainTip) -> (BariClock, tempfile::TempDir) {
        let temp = tempfile::tempdir().unwrap();
        let clock = BariClock::open_at_supplied_tip(
            backend,
            FileStore::open(temp.path()).unwrap(),
            tip0,
            SYNTH_INDEX_NETWORK,
            None,
        )
        .unwrap();
        (clock, temp)
    }

    #[test]
    fn simulated_commit_serves_on_the_tip_after_plant_without_core() {
        let backend = Arc::new(SynthCommit::new());
        let (mut clock, temp) = open(backend.clone(), tip(1, None, 10));
        let leaf = [0x11; 32];
        clock.post_waypoint(leaf).unwrap();

        let seal = clock.on_tip(tip(2, Some(1), 11)).unwrap();
        assert!(seal.closed_batch.is_some());
        assert!(seal.included.is_none());
        assert!(seal.missed.is_none());
        assert!(clock.path_for(&leaf).unwrap().is_none());
        assert!(clock.index_snapshot().plant.is_none());
        assert!(clock.index_at_height(11).unwrap().unwrap().plant.is_none());
        assert_eq!(backend.plant_count(), 1);
        assert_eq!(backend.best_tip_calls(), 0);

        let inclusion_tip = tip(3, Some(2), 12);
        let included = clock.on_tip(inclusion_tip).unwrap();
        assert!(included.included.is_some());
        assert!(included.missed.is_none());
        let body = clock.path_for(&leaf).unwrap().expect("path while serving");
        assert_eq!(body.block_hash, inclusion_tip.hash);
        assert_eq!(body.block_height, inclusion_tip.height);
        let snapshot = clock.index_snapshot();
        assert_eq!(snapshot.network, SYNTH_INDEX_NETWORK);
        let plant = snapshot.plant.expect("anchored plant");
        assert_eq!(plant.block_hash, inclusion_tip.hash);
        assert_eq!(plant.block_height, inclusion_tip.height);
        assert_eq!(plant.root, body.root);
        assert_eq!(plant.txid, body.txid);
        assert_eq!(plant.vout, 0);
        let stored = clock.index_at_height(12).unwrap().unwrap();
        assert_eq!(stored.network, SYNTH_INDEX_NETWORK);
        let stored_plant = stored.plant.expect("height file carries the anchor");
        assert_eq!(
            stored_plant.block_hash,
            crate::encode_bitcoin_id(&inclusion_tip.hash)
        );
        assert_eq!(stored_plant.block_height, 12);
        assert_eq!(stored_plant.root, crate::encode_hash32(&body.root));
        let check = clock.post_check(&body).unwrap();
        assert!(check.is_valid());
        assert_eq!(check.height_matches, Some(true));
        assert_eq!(backend.plant_count(), 1);
        assert_eq!(backend.best_tip_calls(), 0);

        let still = clock.on_tip(tip(4, Some(3), 13)).unwrap();
        assert!(still.archived.is_none());
        assert!(clock.path_for(&leaf).unwrap().is_some());
        assert!(clock
            .on_tip(tip(5, Some(4), 14))
            .unwrap()
            .archived
            .is_none());
        assert!(clock.path_for(&leaf).unwrap().is_some());
        let later = clock.on_tip(tip(6, Some(5), 15)).unwrap();
        assert!(later.archived.is_none());
        let still_served = clock
            .path_for(&leaf)
            .unwrap()
            .expect("confirmed path stays");
        assert_eq!(still_served.block_height, inclusion_tip.height);
        assert_eq!(backend.plant_count(), 1);

        clock.set_born_height(body.block_height);
        assert_eq!(clock.born_height(), body.block_height);
        assert!(clock.path_for(&leaf).unwrap().is_some());
        let kept = temp
            .path()
            .join("index")
            .join(format!("height-{:010}.json", body.block_height));
        assert!(kept.is_file());
        clock.set_born_height(body.block_height + 1);
        assert!(clock.path_for(&leaf).unwrap().is_none());
        assert!(clock.index_at_height(body.block_height).unwrap().is_none());
        assert!(kept.is_file(), "history below the floor stays on disk");
    }

    #[test]
    fn a_gap_keeps_the_simulated_commit_and_does_not_retry() {
        let backend = Arc::new(SynthCommit::new());
        let (mut clock, _temp) = open(backend.clone(), tip(1, None, 10));
        let leaf = [0x22; 32];
        clock.post_waypoint(leaf).unwrap();
        clock.on_tip(tip(2, Some(1), 11)).unwrap();
        assert_eq!(backend.plant_count(), 1);

        // A gap is not a skip. The simulated commit includes on the first tip
        // that asks, so the path stays served and the plant is not sent again.
        let gap = clock.on_tip(tip(9, Some(1), 19)).unwrap();
        assert!(gap.missed.is_none());
        assert!(gap.included.is_some());
        let body = clock
            .path_for(&leaf)
            .unwrap()
            .expect("a gap does not drop the path");
        assert_eq!(body.block_height, 19);

        let later = clock.on_tip(tip(10, Some(9), 20)).unwrap();
        assert!(later.included.is_none());
        assert!(later.missed.is_none());
        assert_eq!(clock.path_for(&leaf).unwrap().unwrap().block_height, 19);
        assert_eq!(backend.plant_count(), 1);
        assert_eq!(backend.best_tip_calls(), 0);
    }

    #[test]
    fn synth_commit_cannot_open_the_live_poll_path() {
        let backend = Arc::new(SynthCommit::new());
        let temp = tempfile::tempdir().unwrap();
        let error = match BariClock::open_for_network(
            backend.clone(),
            FileStore::open(temp.path()).unwrap(),
            SYNTH_INDEX_NETWORK,
        ) {
            Err(error) => error,
            Ok(_) => panic!("live poll path opened on synth commit"),
        };
        let message = error.to_string();
        assert!(message.contains("synth sequencer"), "{message}");
        assert_eq!(backend.best_tip_calls(), 1);
        assert_eq!(backend.plant_count(), 0);
    }

    #[test]
    fn unknown_txid_is_not_mined_and_a_forged_body_fails_post_check() {
        let backend = SynthCommit::new();
        let tip = tip(4, Some(3), 12);
        assert!(backend
            .included_in_tip(&[0xAB; 32], &tip)
            .unwrap()
            .is_none());
        let forged = CanonicalPathBody {
            path: crate::merkle::MerkleTree::build(vec![[0x11; 32]])
                .unwrap()
                .path(&[0x11; 32])
                .unwrap(),
            root: [0x11; 32],
            txid: [0xAB; 32],
            vout: 0,
            block_hash: tip.hash,
            block_height: tip.height,
        };
        let check = backend.post_check(&forged).unwrap();
        assert!(!check.is_valid());
        assert!(!backend.block_is_selected(&tip.hash).unwrap());
    }

    #[test]
    fn the_same_inclusion_tip_is_idempotent_and_a_later_tip_is_not() {
        let backend = SynthCommit::new();
        let root = [0x44; 32];
        let receipt = backend.plant(&root).unwrap();
        assert_eq!(receipt.fee_source, SYNTH_FEE_SOURCE);
        assert_eq!(receipt.vout, 0);
        let first = tip(5, Some(4), 20);
        let again = backend
            .included_in_tip(&receipt.txid, &first)
            .unwrap()
            .unwrap();
        let repeat = backend
            .included_in_tip(&receipt.txid, &first)
            .unwrap()
            .unwrap();
        assert_eq!(again, repeat);
        assert!(backend
            .included_in_tip(&receipt.txid, &tip(6, Some(5), 21))
            .unwrap()
            .is_none());
        assert!(backend.block_is_selected(&first.hash).unwrap());
        assert!(!backend
            .block_is_selected(&tip(6, Some(5), 21).hash)
            .unwrap());
    }
}
