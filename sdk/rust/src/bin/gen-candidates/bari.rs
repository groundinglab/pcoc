//! Rule BR1 paid-waypoint vectors. Easy proof-of-work only.
//! A live retarget header at `0x1d00ffff`, when one was found, is a constant.

use super::Lines;
use pcoc_sdk::bari_rule::{
    br1_status, display_hash, fee_header_class, hash_meets_nbits, header_hash,
    header_nbits, header_prev, header_time, mainnet_next_nbits, mine_nonce, BariTip, Plane, WaypointSubmission,
    MAINNET_POW_LIMIT_NBITS, SYNTH_NBITS,
};
use pcoc_sdk::hash::sha256;
use pcoc_sdk::hashcash::{hashcash_challenge, hashcash_digest, hashcash_solve_spec, leading_zero_bits, nonce_decimal};
use pcoc_sdk::hexutil::{hex_decode_exact, hex_encode};
use std::path::Path;

const ORIGIN: &str = "https://bari.example";
const OTHER: &str = "https://other.example";

/// One header that meets mainnet `0x1d00ffff`, found by a single search outside this generator.
/// Tip is height 2015, previous block the genesis header, `nTime` = genesis time + 1_209_600.
const LIVE_BOUNDARY_CHILD: &str = "00000020508e181c65f9257d28e8facf63778a8232d0fac500ef2641a84f69c74ac12d5dd8f90000639bede451bb722d066c8044eb9101b11c3fce4f15190b646488dd0581227249ffff001d71fa0000";
const LIVE_BOUNDARY_TIP: &str = "000000206fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d61900000000000b506250e8b7dc447ad506c0adda0af95bf518873d60e1ebf797d08d8064f3b529207249ffff001d00000000";

pub fn write_bari(out: &Path) {
    let mut lines = Lines::new();
    let genesis = genesis_hash();
    let parent = easy(&genesis, "pcoc-pack-2.2/bari/test/parent", 1_700_000_000, SYNTH_NBITS);
    let parent_hash = header_hash(&parent);
    let tip = easy(&parent_hash, "pcoc-pack-2.2/bari/test/tip", 1_700_000_600, SYNTH_NBITS);
    let tip_hash = header_hash(&tip);
    let tip_h = 200u64;
    let test_tip = BariTip {
        header: tip,
        height: tip_h,
        hash_natural: tip_hash,
        plane: Plane::Test,
        period_first_time: None,
    };

    let (wp_cur, nonce_cur) = paid(ORIGIN, &tip, "pcoc-pack-2.2/bari/waypoint/current");
    put(&mut lines, "test.current", &test_tip, &tip, &wp_cur, &nonce_cur, "current", 204, ORIGIN);
    lines.add("bari.test.current.tip_hash_is", "header-hash");

    let (wp_before, nonce_before) = paid(ORIGIN, &parent, "pcoc-pack-2.2/bari/waypoint/before");
    put(&mut lines, "test.before", &test_tip, &parent, &wp_before, &nonce_before, "before", 204, ORIGIN);
    lines.hex("bari.test.before.parent_header", &parent);

    let after = easy(&tip_hash, "pcoc-pack-2.2/bari/test/after", 1_700_001_200, SYNTH_NBITS);
    assert_eq!(header_nbits(&after), SYNTH_NBITS);
    assert!(hash_meets_nbits(&header_hash(&after), SYNTH_NBITS));
    let (wp_after, nonce_after) = paid(ORIGIN, &after, "pcoc-pack-2.2/bari/waypoint/after");
    put(&mut lines, "test.after", &test_tip, &after, &wp_after, &nonce_after, "after", 204, ORIGIN);

    let outside_prev = sha256(b"pcoc-pack-2.2/bari/outside-prev");
    let outside = easy(&outside_prev, "pcoc-pack-2.2/bari/test/outside", 1_700_001_800, SYNTH_NBITS);
    assert_ne!(header_hash(&outside), tip_hash);
    assert_ne!(header_hash(&outside), header_prev(&tip));
    assert_ne!(header_prev(&outside), tip_hash);
    let (wp_out, nonce_out) = paid(ORIGIN, &outside, "pcoc-pack-2.2/bari/waypoint/outside");
    put(&mut lines, "test.outside", &test_tip, &outside, &wp_out, &nonce_out, "outside", 402, ORIGIN);

    let (wp_wrong, nonce_wrong) = paid_other(&tip);
    put(&mut lines, "test.wrong-origin", &test_tip, &tip, &wp_wrong, &nonce_wrong, "wrong-origin", 402, ORIGIN);
    lines.add("bari.test.wrong-origin.solved_origin", OTHER);

    let mut shown = test_tip.clone();
    shown.hash_natural = display_hash(&tip_hash);
    assert_eq!(display_hash(&header_hash(&tip)), shown.hash_natural);
    assert_ne!(header_hash(&tip), shown.hash_natural);
    put(&mut lines, "test.display-current", &shown, &tip, &wp_cur, &nonce_cur, "display-current", 402, ORIGIN);
    lines.add("bari.test.display-current.tip_hash_is", "display-of-header-hash");

    let mut disp_before = tip;
    disp_before[4..36].copy_from_slice(&display_hash(&parent_hash));
    let disp_before_tip = BariTip {
        header: disp_before,
        height: tip_h,
        hash_natural: header_hash(&disp_before),
        plane: Plane::Test,
        period_first_time: None,
    };
    assert_eq!(header_prev(&disp_before), display_hash(&parent_hash));
    assert_ne!(header_hash(&parent), header_prev(&disp_before));
    put(
        &mut lines,
        "test.display-before",
        &disp_before_tip,
        &parent,
        &wp_before,
        &nonce_before,
        "display-before",
        402,
        ORIGIN,
    );
    lines.add("bari.test.display-before.tip_prev_is", "display-of-parent-hash");

    let rev_tip = display_hash(&tip_hash);
    let disp_after = easy(&rev_tip, "pcoc-pack-2.2/bari/test/display-after", 1_700_002_400, SYNTH_NBITS);
    assert_eq!(header_prev(&disp_after), rev_tip);
    assert_ne!(header_prev(&disp_after), tip_hash);
    assert!(hash_meets_nbits(&header_hash(&disp_after), SYNTH_NBITS));
    let (wp_da, nonce_da) = paid(ORIGIN, &disp_after, "pcoc-pack-2.2/bari/waypoint/display-after");
    put(&mut lines, "test.display-after", &test_tip, &disp_after, &wp_da, &nonce_da, "display-after", 402, ORIGIN);
    lines.add("bari.test.display-after.fee_prev_is", "display-of-tip-hash");

    let wrong_bits = 0x207f_fffeu32;
    let wrong = easy(&tip_hash, "pcoc-pack-2.2/bari/test/wrong-nbits", 1_700_003_000, wrong_bits);
    assert_eq!(header_nbits(&wrong), wrong_bits);
    assert!(hash_meets_nbits(&header_hash(&wrong), wrong_bits));
    assert_ne!(header_nbits(&wrong), SYNTH_NBITS);
    let (wp_wn, nonce_wn) = paid(ORIGIN, &wrong, "pcoc-pack-2.2/bari/waypoint/test-wrong-nbits");
    put(&mut lines, "test.wrong-nbits", &test_tip, &wrong, &wp_wn, &nonce_wn, "wrong-nbits", 402, ORIGIN);
    lines.add("bari.test.wrong-nbits.required_nbits", format!("{SYNTH_NBITS:08x}"));

    live_easy(&mut lines);
    live_retarget(&mut lines);
    lines.write(out.join("bari.txt"));
}

fn live_easy(lines: &mut Lines) {
    let prev = sha256(b"pcoc-pack-2.2/bari/live/parent-hash");
    let tip = easy(&prev, "pcoc-pack-2.2/bari/live/tip", 1_710_000_000, SYNTH_NBITS);
    let tip_hash = header_hash(&tip);
    let required = mainnet_next_nbits(100, SYNTH_NBITS, 1_710_000_000, None).unwrap();
    assert_eq!(required, SYNTH_NBITS, "non-boundary copies the tip nBits");
    let tip_view = BariTip {
        header: tip,
        height: 100,
        hash_natural: tip_hash,
        plane: Plane::Live,
        period_first_time: None,
    };
    let child = easy(&tip_hash, "pcoc-pack-2.2/bari/live/after", 1_710_000_600, SYNTH_NBITS);
    assert!(hash_meets_nbits(&header_hash(&child), SYNTH_NBITS));
    let (wp, nonce) = paid(ORIGIN, &child, "pcoc-pack-2.2/bari/waypoint/live-after");
    put(lines, "live.after", &tip_view, &child, &wp, &nonce, "after", 204, ORIGIN);
    lines.add("bari.live.after.required_nbits", format!("{required:08x}"));
    lines.add("bari.live.after.retarget", "false");

    let wrong_bits = 0x207f_fffeu32;
    let wrong = easy(&tip_hash, "pcoc-pack-2.2/bari/live/wrong-nbits", 1_710_001_200, wrong_bits);
    assert!(hash_meets_nbits(&header_hash(&wrong), wrong_bits));
    let (wp, nonce) = paid(ORIGIN, &wrong, "pcoc-pack-2.2/bari/waypoint/live-wrong-nbits");
    put(lines, "live.wrong-nbits", &tip_view, &wrong, &wp, &nonce, "wrong-nbits", 402, ORIGIN);
    lines.add("bari.live.wrong-nbits.required_nbits", format!("{SYNTH_NBITS:08x}"));
}

fn live_retarget(lines: &mut Lines) {
    let first = 1_231_006_505u32;
    let tip_time = first + 1_209_600;
    let clamped = mainnet_next_nbits(2015, SYNTH_NBITS, tip_time, Some(first)).unwrap();
    assert_eq!(clamped, MAINNET_POW_LIMIT_NBITS);
    lines.add("bari.live.retarget.clamp.tip_nbits", format!("{SYNTH_NBITS:08x}"));
    lines.add("bari.live.retarget.clamp.period_first_time", first.to_string());
    lines.add("bari.live.retarget.clamp.tip_time", tip_time.to_string());
    lines.add("bari.live.retarget.clamp.required_nbits", format!("{clamped:08x}"));

    let prev = sha256(b"pcoc-pack-2.2/bari/live/clamp-tip-prev");
    let tip = easy(&prev, "pcoc-pack-2.2/bari/live/clamp-tip", tip_time, SYNTH_NBITS);
    let tip_hash = header_hash(&tip);
    let tip_view = BariTip {
        header: tip,
        height: 2015,
        hash_natural: tip_hash,
        plane: Plane::Live,
        period_first_time: Some(first),
    };
    let wrong = easy(&tip_hash, "pcoc-pack-2.2/bari/live/clamp-wrong", tip_time + 600, SYNTH_NBITS);
    let (wp, nonce) = paid(ORIGIN, &wrong, "pcoc-pack-2.2/bari/waypoint/live-clamp-wrong");
    put(lines, "live.retarget.clamp", &tip_view, &wrong, &wp, &nonce, "wrong-nbits", 402, ORIGIN);

    let required = mainnet_next_nbits(2015, MAINNET_POW_LIMIT_NBITS, tip_time, Some(first)).unwrap();
    assert_eq!(required, MAINNET_POW_LIMIT_NBITS);
    lines.add("bari.live.retarget.boundary.required_nbits", format!("{required:08x}"));
    lines.add("bari.live.retarget.boundary.period_first_time", first.to_string());
    lines.add("bari.live.retarget.boundary.tip_time", tip_time.to_string());
    let tip: [u8; 80] = hex_decode_exact(LIVE_BOUNDARY_TIP).expect("boundary tip");
    let child: [u8; 80] = hex_decode_exact(LIVE_BOUNDARY_CHILD).expect("boundary child");
    assert_eq!(header_time(&tip), tip_time);
    assert_eq!(header_nbits(&tip), MAINNET_POW_LIMIT_NBITS);
    assert_eq!(header_prev(&child), header_hash(&tip));
    assert_eq!(header_nbits(&child), required);
    assert!(hash_meets_nbits(&header_hash(&child), required));
    let tip_view = BariTip {
        header: tip,
        height: 2015,
        hash_natural: header_hash(&tip),
        plane: Plane::Live,
        period_first_time: Some(first),
    };
    let (wp, nonce) = paid(ORIGIN, &child, "pcoc-pack-2.2/bari/waypoint/live-boundary");
    put(lines, "live.retarget.boundary", &tip_view, &child, &wp, &nonce, "after", 204, ORIGIN);
    lines.add("bari.live.retarget.boundary.accepted", "true");
}

fn put(
    lines: &mut Lines,
    key: &str,
    tip: &BariTip,
    fee: &[u8; 80],
    waypoint: &[u8; 32],
    nonce: &str,
    class: &str,
    answer: u16,
    origin: &str,
) {
    let fee_hex = hex_encode(fee);
    let sub = WaypointSubmission {
        body: waypoint,
        nonce_header: nonce,
        fee_header_hex: &fee_hex,
    };
    let got = br1_status(origin, &sub, tip);
    assert_eq!(got, answer, "{key}");
    let fee_class = fee_header_class(&sub, tip);
    if answer == 204 {
        assert_eq!(fee_class, Some(class), "{key}");
    } else if key.ends_with("wrong-origin") {
        assert_eq!(fee_class, Some("current"), "{key}");
    } else {
        assert!(fee_class.is_none(), "{key} class {fee_class:?}");
    }
    lines.add(format!("bari.{key}.plane"), tip.plane.label());
    lines.add(format!("bari.{key}.class"), class);
    lines.add(format!("bari.{key}.answer"), answer.to_string());
    lines.add(format!("bari.{key}.origin"), origin);
    lines.add(format!("bari.{key}.tip_height"), tip.height.to_string());
    lines.hex(format!("bari.{key}.tip_header"), &tip.header);
    lines.hex(format!("bari.{key}.tip_hash"), &tip.hash_natural);
    lines.hex(format!("bari.{key}.fee_header"), fee);
    lines.hex(format!("bari.{key}.waypoint"), waypoint);
    lines.add(format!("bari.{key}.nonce"), nonce);
}

fn paid(origin: &str, fee: &[u8; 80], label: &str) -> ([u8; 32], String) {
    let waypoint = sha256(label.as_bytes());
    let challenge = hashcash_challenge(origin, &header_hash(fee));
    let (nonce, _) = hashcash_solve_spec(&challenge, &waypoint).expect(label);
    (waypoint, nonce_decimal(nonce))
}

fn paid_other(fee: &[u8; 80]) -> ([u8; 32], String) {
    for salt in 0..8u32 {
        let waypoint = sha256(format!("pcoc-pack-2.2/bari/waypoint/wrong-origin/{salt}").as_bytes());
        let challenge = hashcash_challenge(OTHER, &header_hash(fee));
        let (nonce, _) = hashcash_solve_spec(&challenge, &waypoint).expect("other");
        let real = hashcash_digest(&hashcash_challenge(ORIGIN, &header_hash(fee)), &waypoint, nonce);
        if leading_zero_bits(&real) < 21 {
            return (waypoint, nonce_decimal(nonce));
        }
    }
    panic!("wrong-origin digest also paid the real origin");
}

fn easy(prev: &[u8; 32], label: &str, time: u32, nbits: u32) -> [u8; 80] {
    for salt in 0..32u32 {
        let mut merkle = sha256(label.as_bytes());
        merkle[..4].copy_from_slice(&salt.to_le_bytes());
        if let Some(header) = mine_nonce(prev, &merkle, time, nbits, 64) {
            return header;
        }
    }
    panic!("no easy header for {label}");
}

fn genesis_hash() -> [u8; 32] {
    let header: [u8; 80] = hex_decode_exact(
        "0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c",
    )
    .unwrap();
    let hash = header_hash(&header);
    assert_eq!(
        hex_encode(&hash),
        "6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000"
    );
    hash
}
