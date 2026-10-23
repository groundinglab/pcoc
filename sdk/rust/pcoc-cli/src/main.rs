//! SPEC §18 verify CLI, plus selftest, candidates, and scripted produce.
#![deny(unsafe_code)]

use pcoc::profile::Profile;
use pcoc::verify::{EmptyIndex, Verified};
use pcoc_produce::bari::{event_json, HttpBari, DEFAULT_BARI_URL, RESEND_AFTER};
use pcoc_produce::live::{produce_live, write_atomically, LiveRunOpts, LiveStop};
use pcoc_produce::source::GossipSource;
use pcoc_produce::{encode_produced, path_skip_stderr, produce_on_rig, Case, Progress};
use pcoc_testing::LabelChain;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        return print_version();
    }
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        usage();
        return ExitCode::from(2);
    }
    match args[0].as_str() {
        "verify" => cmd_verify(&args[1..]),
        "selftest" => cmd_selftest(),
        "quickstart" => cmd_quickstart(),
        "candidates" => cmd_candidates(&args[1..]),
        "produce" => cmd_produce(&args[1..]),
        _ => {
            usage();
            ExitCode::from(2)
        }
    }
}

fn print_version() -> ExitCode {
    let info = pcoc::build_info();
    println!("{}", info.version);
    println!("{}", info.spec_tag);
    println!("{}", info.pack_pin);
    ExitCode::SUCCESS
}

fn cmd_selftest() -> ExitCode {
    let info = pcoc::build_info();
    println!("version: {}", info.version);
    println!("spec: {}", info.spec_tag);
    println!("pack: {}", info.pack_pin);
    println!("sha256: {}", info.sha_backend);
    ExitCode::SUCCESS
}

fn usage() {
    eprintln!(
        "usage: pcoc verify [--profile production|conformance-small] [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N] [--draw-label LABEL] [--input PATH] [PATH]\n\
         pcoc selftest\n\
         pcoc quickstart\n\
         pcoc candidates [--out DIR]\n\
         pcoc produce --rig --case s1|s2|prod --public-key HEX --out PATH\n\
         pcoc produce --live --public-key HEX --out PATH [--gossip-peer HOST:PORT] [--bari URL]\n\
         pcoc --version"
    );
}

struct VerifyOpts {
    profile: Profile,
    draw_label: Option<String>,
    input: Option<PathBuf>,
}

fn parse_u64(flag: &str, value: Option<&str>) -> Result<u64, ExitCode> {
    let Some(v) = value else {
        eprintln!("{flag} needs an integer");
        return Err(ExitCode::from(2));
    };
    v.parse::<u64>().map_err(|_| {
        eprintln!("{flag} needs an integer");
        ExitCode::from(2)
    })
}

fn cmd_verify(args: &[String]) -> ExitCode {
    let parsed = match parse_verify(args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let mut bytes = Vec::new();
    if let Some(path) = &parsed.input {
        let mut file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("unreadable file");
                return ExitCode::from(1);
            }
        };
        if file.read_to_end(&mut bytes).is_err() {
            eprintln!("unreadable file");
            return ExitCode::from(1);
        }
    } else {
        usage();
        return ExitCode::from(2);
    }
    if let Some(label) = &parsed.draw_label {
        eprintln!("draw: injected label (rig input)");
        let mut chain = LabelChain::new(label);
        let mut next = || chain.next8();
        let mut index = EmptyIndex;
        emit(pcoc_testing::verify_from_bytes(
            &bytes,
            &parsed.profile,
            &mut index,
            &mut next,
        ))
    } else {
        let mut index = EmptyIndex;
        emit(pcoc::verify(&bytes, &parsed.profile, &mut index))
    }
}

fn emit(v: Verified) -> ExitCode {
    match v {
        Verified::Record(s) | Verified::UnknownKind(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Verified::DecodeFailure => {
            eprintln!("section 7.3: no record");
            ExitCode::from(1)
        }
    }
}

fn parse_verify(args: &[String]) -> Result<VerifyOpts, ExitCode> {
    let mut profile_name: Option<String> = None;
    let mut sample: Option<u64> = None;
    let mut slot: Option<u64> = None;
    let mut terrain: Option<u64> = None;
    let mut steps: Option<u64> = None;
    let mut draw_label = None;
    let mut input = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let next = args.get(i + 1).map(String::as_str);
        match a.as_str() {
            "--profile" => {
                let Some(v) = next else {
                    eprintln!("--profile needs a name");
                    return Err(ExitCode::from(2));
                };
                profile_name = Some(v.to_string());
                i += 2;
            }
            "--sample-bytes" => {
                sample = Some(parse_u64("--sample-bytes", next)?);
                i += 2;
            }
            "--slot-bytes" => {
                slot = Some(parse_u64("--slot-bytes", next)?);
                i += 2;
            }
            "--terrain-bytes" => {
                terrain = Some(parse_u64("--terrain-bytes", next)?);
                i += 2;
            }
            "--steps-per-leg" => {
                steps = Some(parse_u64("--steps-per-leg", next)?);
                i += 2;
            }
            "--draw-label" => {
                let Some(v) = next else {
                    eprintln!("--draw-label needs a label");
                    return Err(ExitCode::from(2));
                };
                draw_label = Some(v.to_string());
                i += 2;
            }
            "--input" => {
                let Some(v) = next else {
                    eprintln!("--input needs a path");
                    return Err(ExitCode::from(2));
                };
                if input.is_some() {
                    usage();
                    return Err(ExitCode::from(2));
                }
                input = Some(PathBuf::from(v));
                i += 2;
            }
            "--version" | "-V" => return Err(print_version()),
            s if s.starts_with('-') => {
                usage();
                return Err(ExitCode::from(2));
            }
            s => {
                if input.is_some() {
                    usage();
                    return Err(ExitCode::from(2));
                }
                input = Some(PathBuf::from(s));
                i += 1;
            }
        }
    }
    let ints = [sample, slot, terrain, steps];
    let any_int = ints.iter().any(|v| v.is_some());
    let all_int = ints.iter().all(|v| v.is_some());
    let name = profile_name.as_deref().unwrap_or("production");
    let profile = match name {
        "production" | "test" | "live" => {
            if any_int {
                eprintln!("--profile {name} does not take profile integers");
                return Err(ExitCode::from(2));
            }
            Profile::production()
        }
        "conformance-small" => {
            if !all_int {
                eprintln!("--profile conformance-small needs all four integers");
                return Err(ExitCode::from(2));
            }
            match Profile::conformance_small(
                sample.unwrap() as usize,
                slot.unwrap() as usize,
                terrain.unwrap() as usize,
                steps.unwrap(),
            ) {
                Ok(p) => p,
                Err(err) => {
                    eprintln!("{err}");
                    return Err(ExitCode::from(2));
                }
            }
        }
        _ => {
            usage();
            return Err(ExitCode::from(2));
        }
    };
    Ok(VerifyOpts {
        profile,
        draw_label,
        input,
    })
}

fn cmd_candidates(args: &[String]) -> ExitCode {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => {
            usage();
            return ExitCode::from(2);
        }
    };
    let gen = exe.parent().unwrap().join("gen-candidates");
    if !gen.is_file() {
        eprintln!("gen-candidates is not built beside this binary");
        return ExitCode::from(2);
    }
    let status = std::process::Command::new(gen).args(args).status();
    match status {
        Ok(st) => ExitCode::from(st.code().unwrap_or(1) as u8),
        Err(_) => ExitCode::from(2),
    }
}

fn cmd_produce(args: &[String]) -> ExitCode {
    let mut rig = false;
    let mut live = false;
    let mut case: Option<Case> = None;
    let mut key: Option<[u8; 32]> = None;
    let mut out: Option<PathBuf> = None;
    let mut peer: Option<String> = None;
    let mut bari_url: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let next = args.get(i + 1).map(String::as_str);
        match a.as_str() {
            "--rig" => {
                rig = true;
                i += 1;
            }
            "--live" => {
                live = true;
                i += 1;
            }
            "--gossip-peer" => {
                let Some(v) = next else {
                    eprintln!("--gossip-peer needs HOST:PORT");
                    return ExitCode::from(2);
                };
                peer = Some(v.to_owned());
                i += 2;
            }
            "--bari" => {
                let Some(v) = next else {
                    eprintln!("--bari needs a URL");
                    return ExitCode::from(2);
                };
                bari_url = Some(v.to_owned());
                i += 2;
            }
            "--case" => {
                let Some(v) = next else {
                    eprintln!("--case needs s1, s2, or prod");
                    return ExitCode::from(2);
                };
                case = Some(match v {
                    "s1" => Case::S1,
                    "s2" => Case::S2,
                    "prod" => Case::Prod,
                    _ => {
                        eprintln!("--case needs s1, s2, or prod");
                        return ExitCode::from(2);
                    }
                });
                i += 2;
            }
            "--public-key" => {
                let Some(v) = next else {
                    eprintln!("--public-key needs 64 hex digits");
                    return ExitCode::from(2);
                };
                let Some(bytes) = parse_public_key(v) else {
                    eprintln!("--public-key needs 64 hex digits");
                    return ExitCode::from(2);
                };
                key = Some(bytes);
                i += 2;
            }
            "--out" => {
                let Some(v) = next else {
                    eprintln!("--out needs a path");
                    return ExitCode::from(2);
                };
                out = Some(PathBuf::from(v));
                i += 2;
            }
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    if live == rig {
        usage();
        return ExitCode::from(2);
    }
    let (Some(key), Some(out)) = (key, out) else {
        usage();
        return ExitCode::from(2);
    };
    if live {
        return cmd_produce_live(key, out, peer, bari_url);
    }
    let Some(case) = case else {
        usage();
        return ExitCode::from(2);
    };
    let profile = pcoc_produce::cases_profile(case);
    let schedule = pcoc_produce::cases_rig(case);
    let cancel = AtomicBool::new(false);
    let outcome = produce_on_rig(&key, &profile, &schedule, &cancel, |ev| {
        let line = match ev {
            Progress::TerrainFilled => "{\"event\":\"terrain\"}\n".to_string(),
            Progress::Leg {
                stage,
                leg,
                leg_result,
            } => format!(
                "{{\"event\":\"leg\",\"stage\":{stage},\"leg\":{leg},\"leg_result\":\"{}\"}}\n",
                pcoc::hexutil::hex_encode(&leg_result.to_le_bytes())
            ),
            Progress::Interrupt { stage, height } => {
                format!("{{\"event\":\"interrupt\",\"stage\":{stage},\"height\":{height}}}\n")
            }
            Progress::Closed => "{\"event\":\"closed\"}\n".to_string(),
        };
        let _ = std::io::stderr().write_all(line.as_bytes());
    });
    match outcome {
        pcoc_produce::ProduceOutcome::Cancelled => ExitCode::SUCCESS,
        pcoc_produce::ProduceOutcome::Object(obj) => {
            let bytes = encode_produced(&obj);
            if std::fs::write(&out, &bytes).is_err() {
                eprintln!("unreadable file");
                return ExitCode::from(1);
            }
            let _ = std::io::stderr().write_all(path_skip_stderr().as_bytes());
            ExitCode::SUCCESS
        }
    }
}

fn cmd_produce_live(
    key: [u8; 32],
    out: PathBuf,
    peer: Option<String>,
    bari_url: Option<String>,
) -> ExitCode {
    let cancel = Arc::new(AtomicBool::new(false));
    let source = GossipSource::spawn(peer.as_deref().unwrap_or(""), Arc::clone(&cancel));
    let progress: pcoc_produce::bari::Progress = Arc::new(|event| {
        let _ = std::io::stderr().write_all(event_json(&event).as_bytes());
    });
    let bari = match HttpBari::open(
        bari_url.as_deref().unwrap_or(DEFAULT_BARI_URL),
        RESEND_AFTER,
        source.tip_fn(),
        Arc::clone(&progress),
    ) {
        Ok(bari) => bari,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let profile = Profile::production();
    let opts = LiveRunOpts {
        public_key: &key,
        profile: &profile,
        out: &out,
        cancel: &cancel,
    };
    match produce_live(&opts, &source, &bari, &progress) {
        Ok(product) => {
            if write_atomically(&out, &product.bytes).is_err() {
                eprintln!("could not write the object");
                return ExitCode::from(1);
            }
            let _ = std::io::stderr().write_all(product.stderr.as_bytes());
            cancel.store(true, Ordering::SeqCst);
            ExitCode::SUCCESS
        }
        Err(LiveStop::Cancelled) => ExitCode::SUCCESS,
        Err(LiveStop::Failed(message)) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn find_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..8 {
        if dir.join("sdk/SPEC_PIN").is_file() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn pin_value(text: &str, key: &str) -> String {
    let prefix = format!("{key}=");
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(&prefix) {
            return rest.to_string();
        }
    }
    String::new()
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let rest = text.split_once(start)?.1;
    Some(rest.split_once(end)?.0)
}

fn results_are_match(body: &str) -> bool {
    let mut rest = body;
    let mut n = 0;
    while let Some(i) = rest.find("\"result\":\"") {
        let v = &rest[i + 10..];
        if !v.starts_with("match\"") {
            return false;
        }
        n += 1;
        rest = &v[6..];
    }
    n > 0
}

fn measured_ok(record: &str) -> bool {
    let Some(six) = between(record, "\"six_leg\":", ",\"six_leg_runs\":") else {
        return false;
    };
    if six.contains("mismatch") || six.matches("\"match\"").count() != 6 {
        return false;
    }
    let Some(checkpoints) = between(record, "\"checkpoints\":", ",\"waypoints\":") else {
        return false;
    };
    let Some(waypoints) = between(record, "\"waypoints\":", ",\"paths\":") else {
        return false;
    };
    results_are_match(checkpoints) && results_are_match(waypoints)
}

/// Six lines. The production leg is the pinned `legs_le_hex`. This command does not time a leg.
fn cmd_quickstart() -> ExitCode {
    let Some(root) = find_root() else {
        eprintln!("SPEC_PIN not found");
        return ExitCode::from(1);
    };
    let pin_text = match std::fs::read_to_string(root.join("sdk/SPEC_PIN")) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("SPEC_PIN not read");
            return ExitCode::from(1);
        }
    };
    let manifest = match std::fs::read(root.join("spec/conformance/pack/manifest.json")) {
        Ok(bytes) => bytes,
        Err(_) => {
            eprintln!("manifest not read");
            return ExitCode::from(1);
        }
    };
    if pcoc::pin::check_pin(&pin_text, &manifest).is_err() {
        eprintln!("pin mismatch");
        return ExitCode::from(1);
    }
    let vector = pin_value(&pin_text, "vector-sha256");
    let tag = pin_value(&pin_text, "tag");
    if tag != "spec-2.3.0" || vector.len() < 16 {
        eprintln!("pin tag is not spec-2.3.0");
        return ExitCode::from(1);
    }
    let object = match std::fs::read(root.join("sdk/rust/objects/O1-S1.cbor")) {
        Ok(bytes) => bytes,
        Err(_) => {
            eprintln!("O1-S1 not read");
            return ExitCode::from(1);
        }
    };
    let object_sha = pcoc::hexutil::hex_encode(&pcoc::hash::sha256(&object));
    const O1: &str = "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246";
    if object_sha != O1 {
        eprintln!("O1-S1 sha256 {object_sha}");
        return ExitCode::from(1);
    }
    let profile = match Profile::conformance_small(32, 64, 65_536, 100) {
        Ok(profile) => profile,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };
    let mut chain = LabelChain::new("pcoc-pack-2.2/draw");
    let mut next = || chain.next8();
    let mut index = EmptyIndex;
    let record = match pcoc_testing::verify_from_bytes(&object, &profile, &mut index, &mut next) {
        Verified::Record(text) => text,
        Verified::UnknownKind(_) => {
            eprintln!("O1-S1 unknown kind");
            return ExitCode::from(1);
        }
        Verified::DecodeFailure => {
            eprintln!("O1-S1 decode failure");
            return ExitCode::from(1);
        }
    };
    if !measured_ok(&record) {
        eprintln!("O1-S1 six_leg, checkpoints, or waypoints differ");
        return ExitCode::from(1);
    }
    let negatives = match std::fs::read_to_string(root.join("sdk/rust/candidates/negatives.txt")) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("negatives not read");
            return ExitCode::from(1);
        }
    };
    let n = negatives
        .lines()
        .filter(|line| line.contains("matches_honest\tfalse"))
        .count();
    if n != 13 {
        eprintln!("negatives {n}");
        return ExitCode::from(1);
    }
    let prod = match std::fs::read_to_string(root.join("sdk/rust/candidates/prod.txt")) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("prod not read");
            return ExitCode::from(1);
        }
    };
    if !prod
        .lines()
        .any(|line| line == "prod.PROD.stage.0.leg.0.legs_le_hex\t88a04ac5beacb8a7")
    {
        eprintln!("prod leg");
        return ExitCode::from(1);
    }
    let sha = pcoc::sha_backend();
    println!(
        "pcoc {} (rust) {} pack {}: pin match",
        pcoc::VERSION,
        tag,
        &vector[..16]
    );
    println!("kernel scalar, sha256 {sha}");
    println!("O1-S1 object sha256 {O1}");
    println!("O1-S1 six_leg match, checkpoints match, waypoints match");
    println!("negatives 13 non-match");
    println!("PROD leg 0 legs_le_hex 88a04ac5beacb8a7");
    ExitCode::SUCCESS
}

/// SPEC 2.3 P13.12 and §13. A public-key input is 64 hex digits under §7.2,
/// with an optional leading `0x` or `0X`. Exactly those two prefix bytes are
/// stripped. The remaining spelling is 64 digits, `0-9` `a-f` `A-F`, no spaces.
fn parse_public_key(text: &str) -> Option<[u8; 32]> {
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    pcoc::hexutil::hex_decode_exact::<32>(hex)
}

#[cfg(test)]
mod tests {
    use super::parse_public_key;

    const DIGITS: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

    #[test]
    fn public_key_with_and_without_0x_is_the_same_key() {
        let raw = parse_public_key(DIGITS).unwrap();
        assert_eq!(parse_public_key(&format!("0x{DIGITS}")).unwrap(), raw);
        assert_eq!(parse_public_key(&format!("0X{DIGITS}")).unwrap(), raw);
        assert_eq!(parse_public_key(&DIGITS.to_ascii_uppercase()).unwrap(), raw);
        assert_eq!(
            parse_public_key(&format!("0x{}", DIGITS.to_ascii_uppercase())).unwrap(),
            raw
        );
    }

    #[test]
    fn public_key_bad_hex_is_refused() {
        assert!(parse_public_key("zz").is_none());
        assert!(parse_public_key(&format!("0x{DIGITS}ff")).is_none());
        assert!(parse_public_key(&format!("0x{}", &DIGITS[..62])).is_none());
        assert!(parse_public_key(&format!("0xzz{}", &DIGITS[..60])).is_none());
        assert!(parse_public_key(&format!("0x {DIGITS}")).is_none());
        assert!(parse_public_key(&format!("{DIGITS} ")).is_none());
        assert!(parse_public_key("0x").is_none());
    }
}
