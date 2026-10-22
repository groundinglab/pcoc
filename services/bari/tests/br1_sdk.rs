//! Rule BR1 in the Rust SDK, compared with this service's fee window.
//!
//! The SDK implements SPEC §13.6, including a live-plane retarget. This
//! build's `fee_window` answers `after` only on the synthetic network. A
//! live-plane after that the SDK accepts is `Outside` here. That is reported,
//! not papered over: the test-plane rows must still agree.

#![forbid(unsafe_code)]

use pcoc::br1::{br1_status, br1_window, Br1Plane, Br1Tip, Br1Window};
use pcoc::hexutil::hex_decode_exact;
use pcoc_bari::hashcash::{self, FeeWindow};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn fields() -> BTreeMap<String, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../sdk/rust/candidates/br1.txt");
    let text = std::fs::read_to_string(&path).expect("br1.txt");
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('\t').expect("tab");
        out.insert(key.to_owned(), value.to_owned());
    }
    out
}

fn tip(map: &BTreeMap<String, String>, id: &str) -> Br1Tip {
    let hash: [u8; 32] = hex_decode_exact(map[&format!("br1.{id}.tip_hash")].as_str()).unwrap();
    let previous: [u8; 32] =
        hex_decode_exact(map[&format!("br1.{id}.tip_previous")].as_str()).unwrap();
    Br1Tip {
        hash,
        previous,
        height: map[&format!("br1.{id}.tip_height")].parse().unwrap(),
        time: map[&format!("br1.{id}.tip_time")].parse().unwrap(),
        bits: u32::from_str_radix(&map[&format!("br1.{id}.tip_bits")], 16).unwrap(),
        period_first_time: map[&format!("br1.{id}.period_first_time")].parse().unwrap(),
    }
}

fn plane(name: &str) -> Br1Plane {
    match name {
        "test" => Br1Plane::Test,
        "live" => Br1Plane::Live,
        other => panic!("plane {other}"),
    }
}

#[test]
fn sdk_rows_match_bari_where_bari_implements_the_window() {
    let map = fields();
    let ids = [
        "current",
        "before",
        "after_test",
        "after_live_copy",
        "after_live",
        "outside",
        "wrong_origin",
        "display_current",
        "display_before",
        "display_after",
        "wrong_nbits_test",
        "wrong_nbits_live",
    ];
    for id in ids {
        let header: [u8; 80] =
            hex_decode_exact(map[&format!("br1.{id}.fee_header")].as_str()).unwrap();
        let waypoint: [u8; 32] =
            hex_decode_exact(map[&format!("br1.{id}.waypoint")].as_str()).unwrap();
        let origin = map[&format!("br1.{id}.origin")].as_str();
        let nonce = map[&format!("br1.{id}.nonce")].as_str();
        let tip = tip(&map, id);
        let plane = plane(map[&format!("br1.{id}.plane")].as_str());
        let status = br1_status(&waypoint, nonce, &map[&format!("br1.{id}.fee_header")], origin, &tip, plane);
        assert_eq!(status.to_string(), map[&format!("br1.{id}.status")], "{id}");
        let window = br1_window(&header, &tip, plane);
        let window_s = match window {
            Br1Window::Current => "current",
            Br1Window::Before => "before",
            Br1Window::After => "after",
            Br1Window::Outside => "outside",
        };
        assert_eq!(window_s, map[&format!("br1.{id}.window")], "{id}");

        let network = match plane {
            Br1Plane::Test => "synthetic",
            Br1Plane::Live => "main",
        };
        let bari = hashcash::fee_window(&header, &tip.hash, Some(&tip.previous), network);
        let bari_s = match bari {
            FeeWindow::Current => "current",
            FeeWindow::Before => "before",
            FeeWindow::After => "after",
            FeeWindow::Outside => "outside",
        };
        if matches!(id, "after_live" | "after_live_copy") {
            assert_eq!(window, Br1Window::After, "{id}");
            assert_eq!(status, 204, "{id}");
            assert_eq!(
                bari,
                FeeWindow::Outside,
                "{id}: Bari's fee_window does not implement a live-plane after"
            );
            continue;
        }
        assert_eq!(bari_s, window_s, "{id} bari vs sdk");
    }
}
