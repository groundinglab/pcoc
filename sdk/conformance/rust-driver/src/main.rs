//! Conformance driver for the Rust verifier.
//! The verify CLI always passes an empty index (sdk/rust/pcoc-cli/src/main.rs).
//! This program calls the same `verify_from_bytes` entry the CLI calls for
//! `--draw-label`, and reads `GET http://127.0.0.1/index/{height}`.
#![deny(unsafe_code)]

use pcoc::profile::Profile;
use pcoc::verify::{MapIndex, Verified};
use pcoc_testing::LabelChain;
use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::time::Duration;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(code) => code,
    }
}

fn run() -> Result<ExitCode, ExitCode> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut index_url: Option<String> = None;
    let mut profile_name: Option<String> = None;
    let mut sample: Option<u64> = None;
    let mut slot: Option<u64> = None;
    let mut terrain: Option<u64> = None;
    let mut steps: Option<u64> = None;
    let mut draw_label: Option<String> = None;
    let mut input: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let next = args.get(i + 1).map(String::as_str);
        match a.as_str() {
            "--index-url" => {
                index_url = Some(need(next, "--index-url")?.to_string());
                i += 2;
            }
            "--profile" => {
                profile_name = Some(need(next, "--profile")?.to_string());
                i += 2;
            }
            "--sample-bytes" => {
                sample = Some(parse_u64(need(next, "--sample-bytes")?)?);
                i += 2;
            }
            "--slot-bytes" => {
                slot = Some(parse_u64(need(next, "--slot-bytes")?)?);
                i += 2;
            }
            "--terrain-bytes" => {
                terrain = Some(parse_u64(need(next, "--terrain-bytes")?)?);
                i += 2;
            }
            "--steps-per-leg" => {
                steps = Some(parse_u64(need(next, "--steps-per-leg")?)?);
                i += 2;
            }
            "--draw-label" => {
                draw_label = Some(need(next, "--draw-label")?.to_string());
                i += 2;
            }
            s if s.starts_with('-') => {
                usage();
                return Err(ExitCode::from(2));
            }
            s => {
                if input.is_some() {
                    usage();
                    return Err(ExitCode::from(2));
                }
                input = Some(s.to_string());
                i += 1;
            }
        }
    }
    let (Some(index_url), Some(input)) = (index_url, input) else {
        usage();
        return Err(ExitCode::from(2));
    };
    let port = loopback_port(&index_url)?;
    let ints = [sample, slot, terrain, steps];
    let name = profile_name.as_deref().unwrap_or("production");
    let profile = match name {
        "production" | "test" | "live" => {
            if ints.iter().any(|v| v.is_some()) {
                usage();
                return Err(ExitCode::from(2));
            }
            Profile::production()
        }
        "conformance-small" => {
            if ints.iter().any(|v| v.is_none()) {
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
    let bytes = match std::fs::read(&input) {
        Ok(b) => b,
        Err(_) => return Err(ExitCode::from(1)),
    };
    let mut docs = HashMap::new();
    for height in 989u64..=1018 {
        if let Some(body) = http_get(port, height) {
            docs.insert(height, body);
        }
    }
    let mut index = MapIndex { docs };
    if let Some(label) = &draw_label {
        eprintln!("draw: injected label (rig input)");
        let mut chain = LabelChain::new(label);
        let mut next = || chain.next8();
        emit(pcoc_testing::verify_from_bytes(
            &bytes,
            &profile,
            &mut index,
            &mut next,
        ))
    } else {
        emit(pcoc::verify(&bytes, &profile, &mut index))
    }
}

fn emit(v: Verified) -> Result<ExitCode, ExitCode> {
    match v {
        Verified::Record(s) | Verified::UnknownKind(s) => {
            println!("{s}");
            Ok(ExitCode::SUCCESS)
        }
        Verified::DecodeFailure => {
            eprintln!("section 7.3: no record");
            Err(ExitCode::from(1))
        }
    }
}

fn need<'a>(v: Option<&'a str>, flag: &str) -> Result<&'a str, ExitCode> {
    match v {
        Some(s) => Ok(s),
        None => {
            eprintln!("{flag} needs a value");
            Err(ExitCode::from(2))
        }
    }
}

fn parse_u64(v: &str) -> Result<u64, ExitCode> {
    v.parse::<u64>().map_err(|_| {
        eprintln!("integer flag is not an integer");
        ExitCode::from(2)
    })
}

fn loopback_port(url: &str) -> Result<u16, ExitCode> {
    let Some(rest) = url.strip_prefix("http://127.0.0.1:") else {
        eprintln!("index url must be http://127.0.0.1:PORT");
        return Err(ExitCode::from(2));
    };
    if rest.contains('/') || rest.is_empty() {
        eprintln!("index url must be http://127.0.0.1:PORT");
        return Err(ExitCode::from(2));
    }
    rest.parse::<u16>().map_err(|_| {
        eprintln!("index url must be http://127.0.0.1:PORT");
        ExitCode::from(2)
    })
}

fn http_get(port: u16, height: u64) -> Option<Vec<u8>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    stream.set_write_timeout(Some(Duration::from_secs(10))).ok()?;
    let req = format!(
        "GET /index/{height} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    let sep = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = &buf[..sep];
    if !head.starts_with(b"HTTP/1.") {
        return None;
    }
    let status = head.split(|&c| c == b' ').nth(1)?;
    if status != b"200" {
        return None;
    }
    Some(buf[sep + 4..].to_vec())
}

fn usage() {
    eprintln!(
        "usage: xverify-rust --index-url http://127.0.0.1:PORT [--profile production|conformance-small] [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N] [--draw-label LABEL] OBJECT"
    );
}
