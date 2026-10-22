//! SPEC §18 verify CLI, plus selftest, candidates, and scripted produce.
#![deny(unsafe_code)]

use pcoc::profile::Profile;
use pcoc::verify::{EmptyIndex, Verified};
use pcoc_produce::{encode_produced, path_skip_stderr, produce_on_rig, Case, Progress};
use pcoc_testing::LabelChain;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

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
         pcoc candidates [--out DIR]\n\
         pcoc produce --rig --case s1|s2|prod --public-key HEX --out PATH\n\
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
    let status = std::process::Command::new(gen)
        .args(args)
        .status();
    match status {
        Ok(st) => ExitCode::from(st.code().unwrap_or(1) as u8),
        Err(_) => ExitCode::from(2),
    }
}

fn cmd_produce(args: &[String]) -> ExitCode {
    let mut rig = false;
    let mut case: Option<Case> = None;
    let mut key: Option<[u8; 32]> = None;
    let mut out: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let next = args.get(i + 1).map(String::as_str);
        match a.as_str() {
            "--rig" => {
                rig = true;
                i += 1;
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
                let Some(bytes) = pcoc::hexutil::hex_decode_exact::<32>(v) else {
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
    if !rig {
        eprintln!("live header source is not in this build");
        return ExitCode::from(2);
    }
    let (Some(case), Some(key), Some(out)) = (case, key, out) else {
        usage();
        return ExitCode::from(2);
    };
    let profile = pcoc_produce::cases_profile(case);
    let schedule = pcoc_produce::cases_rig(case);
    let cancel = AtomicBool::new(false);
    let outcome = produce_on_rig(&key, &profile, &schedule, &cancel, |ev| {
        let line = match ev {
            Progress::TerrainFilled => "{\"event\":\"terrain\"}\n".to_string(),
            Progress::Leg { stage, leg, leg_result } => format!(
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
