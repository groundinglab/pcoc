//! Candidate vectors for CASES.md. Deterministic. No networking.
#![forbid(unsafe_code)]

mod negatives;

use pcoc_sdk::checkpoint::checkpoint;
use pcoc_sdk::compare::{anchor_window, leg_rate_floor};
use pcoc_sdk::draw::draw_six;
use pcoc_sdk::hash::sha256;
use pcoc_sdk::hashcash::{hashcash_challenge, hashcash_solve_spec, nonce_decimal};
use pcoc_sdk::hexutil::{hex_decode_exact, hex_encode};
use pcoc_sdk::hike::produce_hike;
use pcoc_sdk::jsonutil::unknown_kind_record;
use pcoc_sdk::profile::Profile;
use pcoc_sdk::step::{leg_result, slot_index, step};
use pcoc_sdk::terrain::{lanes, nl4_base_word, seed, seed_preimage, terrain_from_seed};
use pcoc_sdk::constants::HASHCASH_DIFFICULTY;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Instant;

const PK_A: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
const PK_B: &str = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";

struct Lines {
    rows: Vec<(String, String)>,
}

impl Lines {
    fn new() -> Self {
        Self { rows: Vec::new() }
    }

    fn add(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        assert!(!key.is_empty(), "empty key");
        assert!(!value.contains('\n') && !value.contains('\t') && !value.ends_with(' '), "bad value {key}");
        self.rows.push((key, value));
    }

    fn hex(&mut self, key: impl Into<String>, bytes: &[u8]) {
        self.add(key, hex_encode(bytes));
    }

    fn u64hex(&mut self, key: impl Into<String>, n: u64) {
        self.add(key, format!("{n:016x}"));
    }

    fn write(mut self, path: PathBuf) {
        self.rows.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        for w in self.rows.windows(2) {
            assert!(w[0].0.as_bytes() < w[1].0.as_bytes(), "key order {}", w[0].0);
        }
        let mut body = String::new();
        for (k, v) in &self.rows {
            body.push_str(k);
            body.push('\t');
            body.push_str(v);
            body.push('\n');
        }
        std::fs::write(&path, body).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }
}

fn note(msg: &str) {
    eprintln!("{msg}");
    let _ = io::stderr().flush();
}

fn differ(name: &str, same: bool) {
    if same {
        eprintln!("collision {name}");
        std::process::exit(1);
    }
}

fn pk(hex: &str) -> [u8; 32] {
    hex_decode_exact(hex).expect("public key")
}

fn beacon_named(name: &str) -> [u8; 32] {
    sha256(format!("pcoc-pack-2.2/{name}").as_bytes())
}

fn interrupts() -> [([u8; 32], u64); 18] {
    let mut out = [([0u8; 32], 0u64); 18];
    for k in 0..18 {
        out[k] = (beacon_named(&format!("beacon/{k}")), 1001 + k as u64);
    }
    out
}

fn out_dir() -> PathBuf {
    let mut args = std::env::args().skip(1);
    let mut out = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => {
                out = Some(PathBuf::from(args.next().unwrap_or_else(|| {
                    eprintln!("--out needs a directory");
                    std::process::exit(2);
                })));
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    out.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("candidates"))
}

fn le_hex(n: u64) -> String {
    hex_encode(&n.to_le_bytes())
}

struct DrawChain {
    block: [u8; 32],
    pos: usize,
}

impl DrawChain {
    fn new() -> Self {
        Self {
            block: sha256(b"pcoc-pack-2.2/draw"),
            pos: 0,
        }
    }

    fn next8(&mut self) -> Option<[u8; 8]> {
        if self.pos == 32 {
            self.block = sha256(&self.block);
            self.pos = 0;
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.block[self.pos..self.pos + 8]);
        self.pos += 8;
        Some(b)
    }
}

fn main() {
    if cfg!(debug_assertions) {
        note("gen-candidates debug build: the PROD leg walks every section 6 word. Use --release.");
    }
    let out = out_dir();
    std::fs::create_dir_all(&out).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let pk_a = pk(PK_A);
    let pk_b = pk(PK_B);
    let seed_beacon = beacon_named("beacon/seed");
    let interrupts = interrupts();

    write_seed(&out, &pk_a, &pk_b, &seed_beacon);
    write_rate(&out);
    write_anchor(&out);
    write_draw(&out);
    write_unknown(&out);
    write_hashcash(&out);

    let s1 = Profile::conformance_small(32, 64, 65536, 100).expect("S1");
    note("S1 terrain fill start");
    let (terrain, d, lane) = terrain_from_seed(&pk_a, &seed_beacon, &s1);
    let s1_digest = sha256(&terrain);
    write_nl4(&out, &s1, &lane, &terrain);
    let traced = write_fold(&out, &s1, &terrain, &d);
    let legs = [1u32; 18];
    let (obj, d2, outs) = produce_hike(&s1, &pk_a, &seed_beacon, 1000, &interrupts, &legs, &terrain);
    assert_eq!(d, d2);
    assert_eq!(traced, outs[0].leg_results[0]);
    let _ = obj;
    write_hike(&out, "kat-small.S1.pk_A", &outs);
    write_wes(&out, &d, &outs[0].waypoint);
    write_s1_negatives(
        &out,
        &s1,
        &terrain,
        &d,
        &lane,
        &pk_b,
        &seed_beacon,
        &s1_digest,
        outs[0].leg_results[0],
    );
    drop(terrain);

    let s2 = Profile::conformance_small(64, 256, 1_048_576, 1000).expect("S2");
    note("S2 terrain fill start");
    let (terrain, _, _) = terrain_from_seed(&pk_a, &seed_beacon, &s2);
    let s2_digest = sha256(&terrain);
    let legs3 = [3u32; 18];
    let (_obj, _, outs) = produce_hike(&s2, &pk_a, &seed_beacon, 1000, &interrupts, &legs3, &terrain);
    write_hike(&out, "test-3leg.S2.pk_A", &outs);
    drop(terrain);
    // terrain.txt is written once, after the PROD digest is known.
    let mut terrain_lines = Lines::new();
    terrain_lines.hex("terrain.S1.pk_A.digest", &s1_digest);
    terrain_lines.hex("terrain.S2.pk_A.digest", &s2_digest);

    let prod = Profile::production();
    note(&format!(
        "PROD profile slot_bytes={} sample_bytes={} terrain_bytes={} steps_per_leg={}",
        prod.slot_bytes, prod.sample_bytes, prod.terrain_bytes, prod.steps_per_leg
    ));
    note("PROD terrain fill start");
    let t0 = Instant::now();
    let (terrain, d, _) = terrain_from_seed(&pk_a, &seed_beacon, &prod);
    note(&format!("PROD terrain_ms={}", t0.elapsed().as_millis()));
    let prod_digest = sha256(&terrain);
    terrain_lines.hex("terrain.PROD.pk_A.digest", &prod_digest);
    terrain_lines.write(out.join("terrain.txt"));

    note("PROD leg start");
    let t1 = Instant::now();
    let lr = leg_result(&terrain, &prod, &d);
    note(&format!("PROD leg_ms={}", t1.elapsed().as_millis()));
    let cp = checkpoint(&d, &d, &terrain, &prod, lr);
    let mut prod_lines = Lines::new();
    prod_lines.hex("prod.PROD.pk_A.digest", &prod_digest);
    prod_lines.add("prod.PROD.pk_A.stage.0.leg.0.legs_le_hex", le_hex(lr));
    prod_lines.hex("prod.PROD.pk_A.stage.0.leg.0.checkpoint", &cp);
    prod_lines.write(out.join("prod.txt"));

    note("PROD partial_2047 start");
    let t2 = Instant::now();
    let short = negatives::partial_slot_read_2047(&terrain, &d);
    note(&format!("PROD partial_2047_ms={}", t2.elapsed().as_millis()));
    differ("partial_slot_read_2047", short == lr);
    let mut neg = Lines::new();
    // S1 negative rows were written into negatives.txt already. Reload and append.
    let prior = std::fs::read_to_string(out.join("negatives.txt")).expect("s1 negatives");
    for line in prior.split_inclusive('\n') {
        if line.is_empty() {
            continue;
        }
        let line = line.trim_end_matches('\n');
        let (k, v) = line.split_once('\t').expect("tab");
        neg.add(k, v);
    }
    neg.add("negatives.PROD.pk_A.partial_slot_read_2047.legs_le_hex", le_hex(short));
    neg.add("negatives.PROD.pk_A.partial_slot_read_2047.matches_honest", "false");
    neg.write(out.join("negatives.txt"));
    drop(terrain);
    note("gen-candidates wrote 14 files");
}

fn write_seed(out: &PathBuf, pk_a: &[u8; 32], pk_b: &[u8; 32], seed_beacon: &[u8; 32]) {
    let mut lines = Lines::new();
    for (name, key) in [("pk_A", pk_a), ("pk_B", pk_b)] {
        let pre = seed_preimage(key, seed_beacon);
        let d = seed(key, seed_beacon);
        let p = lanes(&d);
        lines.hex(format!("seed.{name}.preimage"), &pre);
        lines.hex(format!("seed.{name}.D"), &d);
        for k in 0..4 {
            lines.u64hex(format!("seed.{name}.P{k}_u64"), p[k]);
        }
    }
    lines.write(out.join("seed.txt"));
}

fn write_rate(out: &PathBuf) {
    let mut lines = Lines::new();
    for legs in [0_i128, 1, 5, 6, 1000] {
        for span in [1_i128, 300, 3600] {
            for (p, q) in [(1_i128, 5_i128), (1, 4), (7, 20)] {
                let word = leg_rate_floor(legs, span, p, q);
                lines.add(
                    format!("rate.hike_legs.{legs}.hike_span.{span}.p.{p}.q.{q}"),
                    word,
                );
            }
        }
    }
    lines.write(out.join("rate.txt"));
}

fn write_anchor(out: &PathBuf) {
    let mut lines = Lines::new();
    for h in 100u64..=106 {
        lines.add(
            format!("anchor.H.100.header_height.{h}"),
            anchor_window(100, h),
        );
    }
    lines.write(out.join("anchor.txt"));
}

fn write_draw(out: &PathBuf) {
    let mut lines = Lines::new();
    for leg_total in [6u64, 18, 54] {
        let mut chain = DrawChain::new();
        let mut next = || chain.next8();
        let idx = draw_six(leg_total, &mut next).unwrap_or_else(|_| {
            eprintln!("collision draw leg_total {leg_total}");
            std::process::exit(1);
        });
        assert_eq!(idx.len(), 6);
        for (i, q) in idx.iter().enumerate() {
            lines.add(format!("draw.leg_total.{leg_total}.index.{i}"), format!("{q}"));
        }
    }
    lines.write(out.join("draw.txt"));
}

fn write_unknown(out: &PathBuf) {
    let mut lines = Lines::new();
    let labels = [
        ("v2.1-pcoc-object", "v2.1-pcoc-object"),
        ("satoshi-hike-v2.0-object", "satoshi-hike-v2.0-object"),
        ("empty", ""),
        ("v2.2-pcoc-objecT", "v2.2-pcoc-objecT"),
    ];
    for (key, text) in labels {
        let rec = unknown_kind_record(text);
        lines.hex(format!("unknown-kind.{key}"), rec.as_bytes());
    }
    lines.write(out.join("unknown-kind.txt"));
}

fn write_hashcash(out: &PathBuf) {
    let origin = "https://bari.example";
    let header_name = "pcoc-pack-2.2/hashcash/header_hash";
    let wp_name = "pcoc-pack-2.2/hashcash/waypoint";
    let header = sha256(header_name.as_bytes());
    let wp = sha256(wp_name.as_bytes());
    let challenge = hashcash_challenge(origin, &header);
    let (nonce, digest) = hashcash_solve_spec(&challenge, &wp).expect("hashcash nonce");
    let mut lines = Lines::new();
    lines.add("hashcash.origin", origin);
    lines.add("hashcash.origin_label", origin);
    lines.add("hashcash.header_hash_name", header_name);
    lines.hex("hashcash.header_hash", &header);
    lines.add("hashcash.waypoint_name", wp_name);
    lines.hex("hashcash.waypoint", &wp);
    lines.hex("hashcash.challenge", &challenge);
    lines.add("hashcash.difficulty", format!("{HASHCASH_DIFFICULTY}"));
    lines.add("hashcash.nonce", nonce_decimal(nonce));
    lines.hex("hashcash.digest", &digest);
    lines.write(out.join("hashcash.txt"));
}

fn read_stored(terrain: &[u8], slot_bytes: usize, slot: usize, j: usize) -> u64 {
    let off = slot * slot_bytes + j * 8;
    let mut b = [0u8; 8];
    b.copy_from_slice(&terrain[off..off + 8]);
    u64::from_le_bytes(b)
}

fn write_nl4(out: &PathBuf, profile: &Profile, lane: &[u64; 4], terrain: &[u8]) {
    let mut lines = Lines::new();
    let n = profile.n_slots as usize;
    let slots = [0usize, 1, n - 1];
    for slot in slots {
        for j in 0..16u64 {
            let base = nl4_base_word(lane, slot as u64, j);
            lines.u64hex(format!("nl4.S1.pk_A.slot.{slot}.j.{j}.base_u64"), base);
            if (j as usize) < profile.words_per_slot {
                let stored = read_stored(terrain, profile.slot_bytes, slot, j as usize);
                lines.u64hex(format!("nl4.S1.pk_A.slot.{slot}.j.{j}.stored_u64"), stored);
            }
        }
    }
    lines.write(out.join("nl4.txt"));
}

fn write_fold(out: &PathBuf, profile: &Profile, terrain: &[u8], d: &[u8; 32]) -> u64 {
    let mut lines = Lines::new();
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&d[..8]);
    let mut s = u64::from_le_bytes(raw);
    for t in 0..4 {
        let i = slot_index(profile, s);
        let (acc, s_prime) = step(terrain, profile, s);
        let p = format!("fold.S1.pk_A.stage.0.leg.0.step.{t}");
        lines.u64hex(format!("{p}.S_u64"), s);
        lines.add(format!("{p}.i"), format!("{i}"));
        lines.u64hex(format!("{p}.acc_u64"), acc);
        lines.u64hex(format!("{p}.S'_u64"), s_prime);
        s = s_prime;
    }
    for _ in 4..profile.steps_per_leg {
        s = step(terrain, profile, s).1;
    }
    lines.u64hex("fold.S1.pk_A.stage.0.leg.0.leg_result_u64", s);
    lines.write(out.join("fold.txt"));
    s
}

fn write_hike(out: &PathBuf, prefix: &str, outs: &[pcoc_sdk::hike::StageOut]) {
    let mut lines = Lines::new();
    let file = if prefix.starts_with("kat-small") {
        "kat-small.txt"
    } else {
        "test-3leg.txt"
    };
    for (g, st) in outs.iter().enumerate() {
        lines.hex(format!("{prefix}.stage.{g}.waypoint"), &st.waypoint);
        for (j, (cp, lr)) in st.checkpoints.iter().zip(st.leg_results.iter()).enumerate() {
            lines.add(format!("{prefix}.stage.{g}.leg.{j}.legs_le_hex"), le_hex(*lr));
            lines.hex(format!("{prefix}.stage.{g}.leg.{j}.checkpoint"), cp);
        }
    }
    lines.write(out.join(file));
}

fn write_wes(out: &PathBuf, d: &[u8; 32], waypoint: &[u8; 32]) {
    let mut lines = Lines::new();
    lines.hex("wes.S1.pk_A.D", d);
    lines.hex("wes.S1.pk_A.waypoint", waypoint);
    assert_eq!(d, waypoint, "stage-0 waypoint is the seed");
    lines.add("wes.S1.pk_A.equal", "true");
    lines.write(out.join("wes.txt"));
}

fn write_s1_negatives(
    out: &PathBuf,
    profile: &Profile,
    terrain: &[u8],
    d: &[u8; 32],
    lane: &[u64; 4],
    pk_b: &[u8; 32],
    seed_beacon: &[u8; 32],
    honest_digest: &[u8; 32],
    honest_leg: u64,
) {
    let legs = negatives::s1_legs(terrain, profile, d);
    let mut lines = Lines::new();
    let named = [
        ("slot_summary_fold", legs.slot_summary_fold),
        ("xor_accumulator_fold", legs.xor_accumulator_fold),
        ("xor_w_fold", legs.xor_w_fold),
        ("s_plus_w_fold", legs.s_plus_w_fold),
        ("prefold_hoist", legs.prefold_hoist),
        ("partial_slot_read", legs.partial_slot_read),
        ("cached_leg", legs.cached_leg),
    ];
    for (name, value) in named {
        differ(name, value == honest_leg);
        lines.add(format!("negatives.S1.pk_A.{name}.legs_le_hex"), le_hex(value));
        lines.add(format!("negatives.S1.pk_A.{name}.matches_honest"), "false");
    }
    let lane_b = lanes(&seed(pk_b, seed_beacon));
    let terrains = [
        ("one_lane_flip", negatives::one_lane_flip(profile, lane)),
        ("all4_kernel", negatives::all4_kernel(profile, lane)),
        ("f3_shared_terrain", negatives::f3_shared_terrain(profile, &lane_b)),
        ("squeezed_seed_terrain", negatives::squeezed_seed_terrain(profile, lane)),
    ];
    for (name, dig) in terrains {
        differ(name, dig == *honest_digest);
        lines.hex(format!("negatives.S1.pk_A.{name}.digest"), &dig);
        lines.add(format!("negatives.S1.pk_A.{name}.matches_honest"), "false");
    }
    lines.write(out.join("negatives.txt"));
}
