//! SPEC §13.1 path asks. No clock, no sleep, no timer.
//! An ask is sent from this thread when a due header is already learned and the
//! post has returned 204. The walk never calls into this file.

use super::bari_client::{BariClient, BariResponse, PathGet};
use super::events;
use super::state::{Run, PHASE_PLANTED};
use crate::constants::ANCHOR_WINDOW_OFFSETS;
use crate::hexutil::hex_encode;
use crate::path::{decode_path, peek_claimed, validate_for};
use std::sync::atomic::Ordering;

pub(crate) fn dispatch(run: &Run, bari: &dyn BariClient, bari_url: &str) {
    if run.closed.load(Ordering::SeqCst) || run.cancel.load(Ordering::SeqCst) {
        return;
    }
    let heights: Vec<u64> = run.learned.lock().unwrap().iter().map(|h| h.height).collect();
    for g in 1..=16 {
        loop {
            if run.closed.load(Ordering::SeqCst) || run.cancel.load(Ordering::SeqCst) {
                return;
            }
            let next = next_due(run, g, &heights);
            let Some((offset_i, waypoint, plant_h)) = next else {
                break;
            };
            {
                let mut plants = run.plants.lock().unwrap();
                plants[g].asked[offset_i] = true;
            }
            if run.closed.load(Ordering::SeqCst) {
                return;
            }
            let leaf = hex_encode(&waypoint);
            let ask = PathGet {
                url: format!("{bari_url}/path/{leaf}"),
                leaf_hex: leaf,
            };
            run.asks.fetch_add(1, Ordering::Relaxed);
            let resp = bari.get_path(&ask);
            let stored = finish_ask(run, g, &waypoint, plant_h, &resp);
            if stored {
                break;
            }
        }
    }
}

fn next_due(run: &Run, g: usize, heights: &[u64]) -> Option<(usize, [u8; 32], u64)> {
    if run.path_present(g) {
        return None;
    }
    let plants = run.plants.lock().unwrap();
    let plant = &plants[g];
    if plant.phase != PHASE_PLANTED {
        return None;
    }
    for (i, off) in ANCHOR_WINDOW_OFFSETS.iter().enumerate() {
        if plant.asked[i] {
            continue;
        }
        let due = plant.height.saturating_add(*off);
        if heights.iter().any(|h| *h == due) {
            return Some((i, plant.waypoint, plant.height));
        }
    }
    None
}

fn finish_ask(run: &Run, g: usize, waypoint: &[u8; 32], plant_h: u64, resp: &BariResponse) -> bool {
    let (status, body) = match resp {
        BariResponse::NoStatus => (None, &[][..]),
        BariResponse::Http { status, body } => (Some(*status), body.as_slice()),
    };
    let checked = status == Some(200) && path_ok(body, waypoint, plant_h);
    let block = if status == Some(200) {
        decode_path(body)
            .map(|p| (p.block_hash, p.block_height))
            .or_else(|| peek_claimed(body))
    } else {
        None
    };
    let stored = if checked {
        let mut paths = run.paths.lock().unwrap();
        if run.closed.load(Ordering::SeqCst) {
            false
        } else {
            paths[g] = Some(body.to_vec());
            true
        }
    } else {
        false
    };
    if run.live_log() {
        run.emit(events::fetch_line(waypoint, stored, status, block, body));
        if status == Some(200) {
            // `checked` is the section 13.1 body check. The close may still refuse the store.
            run.emit(events::checked_line(waypoint, checked));
        }
    }
    stored
}

fn path_ok(body: &[u8], waypoint: &[u8; 32], plant_h: u64) -> bool {
    let Some(decoded) = decode_path(body) else {
        return false;
    };
    if !validate_for(&decoded, waypoint) {
        return false;
    }
    let bh = decoded.block_height as u64;
    ANCHOR_WINDOW_OFFSETS
        .iter()
        .any(|off| plant_h.saturating_add(*off) == bh)
}
