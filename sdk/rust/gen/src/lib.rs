//! CASES.md v2 candidate files. Deterministic. No networking.
#![deny(unsafe_code)]

mod negatives;

use pcoc::checkpoint::checkpoint;
use pcoc::compare::{anchor_window, leg_rate_floor};
use pcoc::constants::HASHCASH_DIFFICULTY;
use pcoc::draw::{draw_six, draw_thorough};
use pcoc::hash::sha256;
use pcoc::hashcash::{hashcash_challenge, hashcash_solve, header_hash, leading_zero_bits};
use pcoc::hexutil::{hex_decode_exact, hex_encode};
use pcoc::hike::produce_hike;
use pcoc::jsonutil::unknown_kind_record;
use pcoc::object::encode_object;
use pcoc::profile::Profile;
use pcoc::step::{slot_index, step};
use pcoc::terrain::{lanes, nl4_base_word, seed, seed_preimage, terrain_from_seed};
use std::path::{Path, PathBuf};

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
        assert!(!key.is_empty());
        assert!(!value.contains('\n') && !value.contains('\t') && !value.ends_with(' '));
        self.rows.push((key, value));
    }

    fn hex(&mut self, key: impl Into<String>, bytes: &[u8]) {
        self.add(key, hex_encode(bytes));
    }

    fn u64hex(&mut self, key: impl Into<String>, n: u64) {
        self.add(key, format!("{n:016x}"));
    }

    fn finish(mut self) -> String {
        self.rows.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        for w in self.rows.windows(2) {
            assert!(w[0].0.as_bytes() < w[1].0.as_bytes(), "duplicate {}", w[0].0);
        }
        let mut body = String::new();
        for (k, v) in &self.rows {
            body.push_str(k);
            body.push('\t');
            body.push_str(v);
            body.push('\n');
        }
        body
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

fn write_file(dir: &Path, name: &str, body: String) -> Result<(), String> {
    let path = dir.join(name);
    std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Write the 14 CASES.md files into `dir`. Stops if any negative equals honest.
pub fn write_candidates(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("mkdir: {e}"))?;
    let pk_a = pk(PK_A);
    let pk_b = pk(PK_B);
    let seed_beacon = beacon_named("beacon/seed");
    let interrupts = interrupts();

    write_file(dir, "seed.txt", seed_lines(&pk_a, &pk_b, &seed_beacon))?;
    write_file(dir, "rate.txt", rate_lines())?;
    write_file(dir, "anchor.txt", anchor_lines())?;
    write_file(dir, "draw.txt", draw_lines())?;
    write_file(dir, "hashcash.txt", hashcash_lines())?;

    let s1 = Profile::conformance_small(32, 64, 65_536, 100).expect("S1");
    let (terrain, d, lane) = terrain_from_seed(&pk_a, &seed_beacon, &s1);
    let s1_digest = sha256(&terrain);
    write_file(dir, "nl4.txt", nl4_lines(&s1, &lane, &terrain))?;
    let traced = {
        let (body, leg) = fold_lines(&s1, &terrain, &d);
        write_file(dir, "fold.txt", body)?;
        leg
    };
    let legs = [1u32; 18];
    let (obj, d2, outs) = produce_hike(&s1, &pk_a, &seed_beacon, 1000, &interrupts, &legs, &terrain);
    if d != d2 || traced != outs[0].leg_results[0] {
        return Err("S1 fold leg and hike leg differ".into());
    }
    write_file(dir, "kat-small.txt", hike_lines("kat-small.S1", &s1_digest, &outs))?;
    write_file(dir, "wes.txt", wes_lines(&d, &outs[0].waypoint))?;
    write_file(dir, "unknown-kind.txt", unknown_lines(&obj))?;
    let neg_body = negative_lines(&s1, &terrain, &d, &lane, &pk_b, &seed_beacon, &s1_digest, traced)?;
    write_file(dir, "negatives.txt", neg_body)?;
    drop(terrain);

    let s2 = Profile::conformance_small(64, 256, 1_048_576, 1_000).expect("S2");
    let (terrain, _, _) = terrain_from_seed(&pk_a, &seed_beacon, &s2);
    let s2_digest = sha256(&terrain);
    let legs3 = [3u32; 18];
    let (_obj, _, outs) = produce_hike(&s2, &pk_a, &seed_beacon, 1000, &interrupts, &legs3, &terrain);
    write_file(dir, "test-3leg.txt", hike_lines("test-3leg.S2", &s2_digest, &outs))?;
    drop(terrain);

    let prod = Profile::production();
    let (terrain, d, _) = terrain_from_seed(&pk_a, &seed_beacon, &prod);
    let prod_digest = sha256(&terrain);
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&d[..8]);
    let mut s = u64::from_le_bytes(raw);
    for _ in 0..prod.steps_per_leg {
        s = step(&terrain, &prod, s).1;
    }
    let cp = checkpoint(&d, &d, &terrain, &prod, s);
    let mut lines = Lines::new();
    lines.hex("terrain.S1.terrain_sha256", &s1_digest);
    lines.hex("terrain.S2.terrain_sha256", &s2_digest);
    lines.hex("terrain.PROD.terrain_sha256", &prod_digest);
    write_file(dir, "terrain.txt", lines.finish())?;
    let mut prod_lines = Lines::new();
    prod_lines.hex("prod.PROD.terrain_sha256", &prod_digest);
    prod_lines.add("prod.PROD.stage.0.leg.0.legs_le_hex", le_hex(s));
    prod_lines.hex("prod.PROD.stage.0.leg.0.checkpoint", &cp);
    write_file(dir, "prod.txt", prod_lines.finish())?;
    Ok(())
}

fn seed_lines(pk_a: &[u8; 32], pk_b: &[u8; 32], seed_beacon: &[u8; 32]) -> String {
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
    lines.finish()
}

fn rate_lines() -> String {
    let mut lines = Lines::new();
    for legs in [0_i128, 1, 5, 6, 1000] {
        for span in [1_i128, 300, 3600] {
            for (p, q) in [(1_i128, 5_i128), (1, 4), (7, 20)] {
                let word = leg_rate_floor(legs, span, p, q);
                lines.add(format!("rate.legs.{legs}.span.{span}.floor.{p}_{q}"), word);
            }
        }
    }
    lines.finish()
}

fn anchor_lines() -> String {
    let mut lines = Lines::new();
    for h in 100u64..=106 {
        lines.add(format!("anchor.H.100.height.{h}"), anchor_window(100, h));
    }
    lines.finish()
}

fn join_indices(idx: &[u64]) -> String {
    idx.iter().map(|q| q.to_string()).collect::<Vec<_>>().join(",")
}

fn draw_lines() -> String {
    let mut lines = Lines::new();
    lines.add("draw.b0_label", "pcoc-pack-2.2/draw");
    lines.add("draw.thorough_rounds", "3");
    for leg_total in [6u64, 18, 54] {
        let mut chain = DrawChain::new();
        let six = draw_six(leg_total, &mut || chain.next8()).expect("six-leg draw");
        lines.add(
            format!("draw.legs.{leg_total}.six_leg.indices"),
            join_indices(&six),
        );
        let mut chain = DrawChain::new();
        let thorough = draw_thorough(leg_total, 3, &mut || chain.next8()).expect("thorough draw");
        lines.add(
            format!("draw.legs.{leg_total}.thorough.indices"),
            join_indices(&thorough),
        );
    }
    lines.finish()
}

fn hashcash_lines() -> String {
    let origin = "https://example.com";
    let h0 = sha256(b"pcoc-pack-2.2/hashcash/fee_header/0");
    let h1 = sha256(b"pcoc-pack-2.2/hashcash/fee_header/1");
    let h2 = sha256(b"pcoc-pack-2.2/hashcash/fee_header/2");
    let mut fee = Vec::with_capacity(80);
    fee.extend_from_slice(&h0);
    fee.extend_from_slice(&h1);
    fee.extend_from_slice(&h2[..16]);
    let hh = header_hash(&fee);
    let wp = sha256(b"pcoc-pack-2.2/hashcash/waypoint");
    let challenge = hashcash_challenge(origin, &hh);
    let (nonce, digest) = hashcash_solve(&challenge, &wp, HASHCASH_DIFFICULTY).expect("nonce");
    let bits = leading_zero_bits(&digest);
    let mut lines = Lines::new();
    lines.add("hashcash.bari_origin", origin);
    lines.hex("hashcash.fee_header", &fee);
    lines.hex("hashcash.header_hash", &hh);
    lines.hex("hashcash.waypoint", &wp);
    lines.add("hashcash.difficulty_bits", format!("{HASHCASH_DIFFICULTY}"));
    lines.hex("hashcash.challenge", &challenge);
    lines.add("hashcash.nonce", nonce.to_string());
    lines.hex("hashcash.digest", &digest);
    lines.add("hashcash.leading_zero_bits", format!("{bits}"));
    lines.finish()
}

fn read_stored(terrain: &[u8], slot_bytes: usize, slot: usize, j: usize) -> u64 {
    let off = slot * slot_bytes + j * 8;
    let mut b = [0u8; 8];
    b.copy_from_slice(&terrain[off..off + 8]);
    u64::from_le_bytes(b)
}

fn nl4_lines(profile: &Profile, lane: &[u64; 4], terrain: &[u8]) -> String {
    let mut lines = Lines::new();
    for slot in [0usize, 1, 1023] {
        for j in 0..8u64 {
            let base = nl4_base_word(lane, slot as u64, j);
            lines.u64hex(format!("nl4.S1.slot.{slot}.word.{j}.base_u64"), base);
            let stored = read_stored(terrain, profile.slot_bytes, slot, j as usize);
            lines.u64hex(format!("nl4.S1.slot.{slot}.word.{j}.stored_u64"), stored);
        }
    }
    lines.finish()
}

fn fold_lines(profile: &Profile, terrain: &[u8], d: &[u8; 32]) -> (String, u64) {
    let mut lines = Lines::new();
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&d[..8]);
    let mut s = u64::from_le_bytes(raw);
    for t in 0..4 {
        let i = slot_index(profile, s);
        let (acc, s_prime) = step(terrain, profile, s);
        lines.u64hex(format!("fold.S1.step.{t}.S_u64"), s);
        lines.add(format!("fold.S1.step.{t}.slot_index"), format!("{i}"));
        lines.u64hex(format!("fold.S1.step.{t}.acc_u64"), acc);
        lines.u64hex(format!("fold.S1.step.{t}.S_next_u64"), s_prime);
        s = s_prime;
    }
    for _ in 4..profile.steps_per_leg {
        s = step(terrain, profile, s).1;
    }
    lines.u64hex("fold.S1.leg_result_u64", s);
    (lines.finish(), s)
}

fn hike_lines(prefix: &str, digest: &[u8; 32], outs: &[pcoc::hike::StageOut]) -> String {
    let mut lines = Lines::new();
    lines.hex(format!("{prefix}.terrain_sha256"), digest);
    for (g, st) in outs.iter().enumerate() {
        lines.hex(format!("{prefix}.stage.{g}.waypoint"), &st.waypoint);
        for (j, (cp, lr)) in st.checkpoints.iter().zip(st.leg_results.iter()).enumerate() {
            lines.add(format!("{prefix}.stage.{g}.leg.{j}.legs_le_hex"), le_hex(*lr));
            lines.hex(format!("{prefix}.stage.{g}.leg.{j}.checkpoint"), cp);
        }
    }
    lines.finish()
}

fn wes_lines(d: &[u8; 32], waypoint: &[u8; 32]) -> String {
    let mut lines = Lines::new();
    lines.hex("wes.S1.D", d);
    lines.hex("wes.S1.stage.0.waypoint", waypoint);
    lines.add("wes.S1.equal", if d == waypoint { "true" } else { "false" });
    lines.finish()
}

fn unknown_lines(obj: &pcoc::object::HikeObject) -> String {
    let labels = [
        "v2.1-pcoc-object",
        "satoshi-hike-v2.0-object",
        "",
        "v2.2-pcoc-objecT",
    ];
    let mut lines = Lines::new();
    for (n, text) in labels.iter().enumerate() {
        let mut replaced = obj.clone();
        replaced.kind = (*text).to_string();
        let bytes = encode_object(&replaced);
        if bytes.is_empty() {
            panic!("replaced kind did not encode");
        }
        lines.hex(
            format!("unknown-kind.{n}.kind_text_hex"),
            text.as_bytes(),
        );
        let rec = unknown_kind_record(text);
        lines.hex(format!("unknown-kind.{n}.record_hex"), rec.as_bytes());
    }
    lines.finish()
}

fn negative_lines(
    profile: &Profile,
    terrain: &[u8],
    d: &[u8; 32],
    lane: &[u64; 4],
    pk_b: &[u8; 32],
    seed_beacon: &[u8; 32],
    honest_digest: &[u8; 32],
    honest_leg: u64,
) -> Result<String, String> {
    let legs = negatives::leg_negatives(terrain, profile, d);
    if legs.cached_revisit != Some((34, 101, 20)) {
        return Err(format!(
            "cached_leg first revisit {:?}, expected step 34 slot 101 first-read step 20",
            legs.cached_revisit
        ));
    }
    let lane_b = lanes(&seed(pk_b, seed_beacon));
    let f3 = negatives::f3_shared_terrain(profile, d, &lane_b);
    let named: [(&str, u64); 10] = [
        ("slot_summary_fold", legs.slot_summary_fold),
        ("xor_accumulator_fold", legs.xor_accumulator_fold),
        ("xor_w_fold", legs.xor_w_fold),
        ("s_plus_w_fold", legs.s_plus_w_fold),
        ("prefold_hoist", legs.prefold_hoist),
        ("final_stage_hoist", legs.final_stage_hoist),
        ("partial_slot_read", legs.partial_slot_read),
        ("partial_slot_read_2047", legs.partial_slot_read_2047),
        ("f3_shared_terrain", f3),
        ("cached_leg", legs.cached_leg),
    ];
    let mut seen = Vec::new();
    let mut lines = Lines::new();
    lines.add("negatives.honest.legs_le_hex", le_hex(honest_leg));
    lines.hex("negatives.honest.terrain_sha256", honest_digest);
    for (name, value) in named {
        if value == honest_leg {
            return Err(format!("{name} equals honest"));
        }
        let hex = le_hex(value);
        if seen.contains(&hex) {
            return Err(format!("{name} is not pairwise distinct"));
        }
        seen.push(hex.clone());
        let same = value == honest_leg;
        lines.add(format!("negatives.{name}.legs_le_hex"), hex);
        lines.add(
            format!("negatives.{name}.matches_honest"),
            if same { "true" } else { "false" },
        );
    }
    let terrains = [
        ("one_lane_flip", negatives::one_lane_flip(profile, lane)),
        ("all4_kernel", negatives::all4_kernel(profile, lane)),
        ("squeezed_seed_terrain", negatives::squeezed_seed_terrain(profile, lane)),
    ];
    let mut seen_t = Vec::new();
    for (name, dig) in terrains {
        if dig == *honest_digest {
            return Err(format!("{name} equals honest"));
        }
        let hex = hex_encode(&dig);
        if seen_t.contains(&hex) {
            return Err(format!("{name} is not pairwise distinct"));
        }
        seen_t.push(hex);
        let same = dig == *honest_digest;
        lines.hex(format!("negatives.{name}.terrain_sha256"), &dig);
        lines.add(
            format!("negatives.{name}.matches_honest"),
            if same { "true" } else { "false" },
        );
    }
    Ok(lines.finish())
}

pub use negatives::{
    all4_kernel, f3_shared_terrain, leg_negatives, one_lane_flip, squeezed_seed_terrain,
    LegNegatives,
};

/// Default output directory: `sdk/rust/candidates`.
pub fn default_out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../candidates")
}
