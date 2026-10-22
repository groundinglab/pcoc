//! Rule BR1 candidate lines. The four SDKs emit this file byte for byte.
//! The live retarget header is the first header, at the first `nTime` at or
//! after 1233062596, whose SHA-256d meets the mainnet pow limit. It is a
//! constant so candidate regeneration does not search.

use pcoc::br1::{
    assemble_header, br1_status, br1_window, required_bits, Br1Plane, Br1Tip, Br1Window,
    SYNTH_NBITS,
};
use pcoc::hash::sha256;
use pcoc::hashcash::{hashcash_challenge, hashcash_solve};
use pcoc::hexutil::{hex_decode_exact, hex_encode};
use pcoc::constants::HASHCASH_DIFFICULTY;
use pcoc::hashcash::header_hash;

use super::Lines;

/// 80-byte fee header for `after_live`. Empty until the one-time search is pasted in.
pub const LIVE_AFTER_HEADER_HEX: &str = "0100000007fd9d1abc8d5b4c25c9f451e215c0c90b68b1a46917374e5274cbc6086327b051b29fc516c3c3aecac4a77c019cb535bde7a446cd0ae8e32afc61d094b7fd37c50a7f49ffff001d27167cb1";

pub fn lines() -> String {
    let origin = "https://example.com";
    let waypoint = sha256(b"pcoc-pack-2.2/br1/waypoint");
    let tip_header = assemble_header(&[0x11; 32], 1_700_000_000, SYNTH_NBITS, 4);
    let tip_hash = header_hash(&tip_header);
    let before_header = assemble_header(&[0x33; 32], 1_700_000_001, SYNTH_NBITS, 5);
    let before_hash = header_hash(&before_header);
    let outside_header = assemble_header(&[0x55; 32], 1_700_000_002, SYNTH_NBITS, 9);
    let test_tip = Br1Tip {
        hash: tip_hash,
        previous: [0x11; 32],
        height: 100,
        time: 1_700_000_000,
        bits: SYNTH_NBITS,
        period_first_time: 0,
    };
    let after_test = header_with_pow(&tip_hash, 1_700_000_010, SYNTH_NBITS);
    let mut reversed_tip = tip_hash;
    reversed_tip.reverse();
    let display_after = header_with_pow(&reversed_tip, 1_700_000_011, SYNTH_NBITS);
    let wrong_bits_test = assemble_header(&tip_hash, 1_700_000_012, 0x1d00_ffff, 1);

    let live_hash = sha256(b"pcoc-pack-2.2/br1/live-tip");
    let live_prev = sha256(b"pcoc-pack-2.2/br1/live-prev");
    let live_tip = Br1Tip {
        hash: live_hash,
        previous: live_prev,
        height: 2015,
        time: 1_233_061_996,
        bits: 0x1d00_ffff,
        period_first_time: 1_231_006_505,
    };
    let live_header: [u8; 80] = hex_decode_exact(LIVE_AFTER_HEADER_HEX).expect("live header hex");
    assert_eq!(&live_header[4..36], &live_hash[..], "live header prev is the tip hash");
    assert_eq!(
        required_bits(&live_tip, Br1Plane::Live),
        0x1d00_ffff,
        "historical retarget clamps to the pow limit"
    );
    let wrong_bits_live = header_with_pow(&live_hash, 1_233_062_000, SYNTH_NBITS);

    let mut lines = Lines::new();
    emit(
        &mut lines,
        "current",
        origin,
        &waypoint,
        &tip_header,
        &test_tip,
        Br1Plane::Test,
    );
    let mut before_tip = test_tip;
    before_tip.previous = before_hash;
    emit(
        &mut lines,
        "before",
        origin,
        &waypoint,
        &before_header,
        &before_tip,
        Br1Plane::Test,
    );
    emit(
        &mut lines,
        "after_test",
        origin,
        &waypoint,
        &after_test,
        &test_tip,
        Br1Plane::Test,
    );
    emit(
        &mut lines,
        "after_live_copy",
        origin,
        &waypoint,
        &after_test,
        &test_tip,
        Br1Plane::Live,
    );
    emit(
        &mut lines,
        "after_live",
        origin,
        &waypoint,
        &live_header,
        &live_tip,
        Br1Plane::Live,
    );
    emit(
        &mut lines,
        "outside",
        origin,
        &waypoint,
        &outside_header,
        &test_tip,
        Br1Plane::Test,
    );
    emit_at(
        &mut lines,
        "wrong_origin",
        origin,
        "https://other.example",
        &waypoint,
        &tip_header,
        &test_tip,
        Br1Plane::Test,
    );
    let mut display_current_tip = test_tip;
    display_current_tip.hash = reversed_tip;
    emit(
        &mut lines,
        "display_current",
        origin,
        &waypoint,
        &tip_header,
        &display_current_tip,
        Br1Plane::Test,
    );
    let mut display_before_tip = test_tip;
    let mut reversed_before = before_hash;
    reversed_before.reverse();
    display_before_tip.previous = reversed_before;
    emit(
        &mut lines,
        "display_before",
        origin,
        &waypoint,
        &before_header,
        &display_before_tip,
        Br1Plane::Test,
    );
    emit(
        &mut lines,
        "display_after",
        origin,
        &waypoint,
        &display_after,
        &test_tip,
        Br1Plane::Test,
    );
    emit(
        &mut lines,
        "wrong_nbits_test",
        origin,
        &waypoint,
        &wrong_bits_test,
        &test_tip,
        Br1Plane::Test,
    );
    emit(
        &mut lines,
        "wrong_nbits_live",
        origin,
        &waypoint,
        &wrong_bits_live,
        &live_tip,
        Br1Plane::Live,
    );
    lines.finish()
}

#[cfg(test)]
mod tests {
    #[test]
    fn candidate_marks_the_paid_cases_204() {
        let text = super::lines();
        for id in ["current", "before", "after_test", "after_live", "after_live_copy"] {
            assert!(
                text.contains(&format!("br1.{id}.status\t204\n")),
                "{id}"
            );
        }
        for id in [
            "outside",
            "wrong_origin",
            "display_current",
            "display_before",
            "display_after",
            "wrong_nbits_test",
            "wrong_nbits_live",
        ] {
            assert!(
                text.contains(&format!("br1.{id}.status\t402\n")),
                "{id}"
            );
        }
    }
}

fn header_with_pow(previous: &[u8; 32], time: u32, bits: u32) -> [u8; 80] {
    let tip = Br1Tip {
        hash: *previous,
        previous: [0; 32],
        height: 100,
        time,
        bits,
        period_first_time: 0,
    };
    let plane = if bits == SYNTH_NBITS {
        Br1Plane::Test
    } else {
        Br1Plane::Live
    };
    for nonce in 0..4096u32 {
        let header = assemble_header(previous, time, bits, nonce);
        if br1_window(&header, &tip, plane) == Br1Window::After {
            return header;
        }
    }
    panic!("no proof-of-work header");
}

fn emit(
    lines: &mut Lines,
    id: &str,
    origin: &str,
    waypoint: &[u8; 32],
    header: &[u8; 80],
    tip: &Br1Tip,
    plane: Br1Plane,
) {
    emit_at(lines, id, origin, origin, waypoint, header, tip, plane);
}

fn emit_at(
    lines: &mut Lines,
    id: &str,
    check_origin: &str,
    solve_origin: &str,
    waypoint: &[u8; 32],
    header: &[u8; 80],
    tip: &Br1Tip,
    plane: Br1Plane,
) {
    let hash = header_hash(header);
    let challenge = hashcash_challenge(solve_origin, &hash);
    let (nonce, _) = hashcash_solve(&challenge, waypoint, HASHCASH_DIFFICULTY).expect("nonce");
    let nonce_s = nonce.to_string();
    let header_hex = hex_encode(header);
    let status = br1_status(waypoint, &nonce_s, &header_hex, check_origin, tip, plane);
    let window = br1_window(header, tip, plane);
    let expect_status: u16 = match id {
        "current" | "before" | "after_test" | "after_live" | "after_live_copy" => 204,
        _ => 402,
    };
    let expect_window = match id {
        "current" | "wrong_origin" => "current",
        "before" => "before",
        "after_test" | "after_live" | "after_live_copy" => "after",
        _ => "outside",
    };
    assert_eq!(status, expect_status, "{id}");
    let window_s = match window {
        Br1Window::Current => "current",
        Br1Window::Before => "before",
        Br1Window::After => "after",
        Br1Window::Outside => "outside",
    };
    assert_eq!(window_s, expect_window, "{id} window");
    let plane_s = match plane {
        Br1Plane::Test => "test",
        Br1Plane::Live => "live",
    };
    let p = format!("br1.{id}");
    lines.add(format!("{p}.plane"), plane_s);
    lines.add(format!("{p}.origin"), check_origin);
    lines.add(format!("{p}.solve_origin"), solve_origin);
    lines.hex(format!("{p}.waypoint"), waypoint);
    lines.add(format!("{p}.nonce"), nonce_s);
    lines.add(format!("{p}.fee_header"), header_hex);
    lines.hex(format!("{p}.tip_hash"), &tip.hash);
    lines.hex(format!("{p}.tip_previous"), &tip.previous);
    lines.add(format!("{p}.tip_height"), tip.height.to_string());
    lines.add(format!("{p}.tip_time"), tip.time.to_string());
    lines.add(format!("{p}.tip_bits"), format!("{:08x}", tip.bits));
    lines.add(
        format!("{p}.period_first_time"),
        tip.period_first_time.to_string(),
    );
    lines.add(format!("{p}.window"), window_s);
    lines.add(format!("{p}.status"), status.to_string());
}
