//! SPEC §18. Verify CLI. Stdout is one section 14 record and one trailing newline.
#![forbid(unsafe_code)]

use pcoc_sdk::draw::next8_from;
use pcoc_sdk::hexutil::hex_decode;
use pcoc_sdk::profile::Profile;
use pcoc_sdk::verify::{verify, EmptyIndex, Verified};
use pcoc_sdk::VERSION;
use std::io::Cursor;

fn usage() -> ! {
    eprintln!("usage: pcoc-verify [--profile production|conformance-small|test|live] [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N] [--draw-hex HEX] [object]");
    std::process::exit(2);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut profile_name: Option<String> = None;
    let mut sample: Option<u64> = None;
    let mut slot: Option<u64> = None;
    let mut terrain: Option<u64> = None;
    let mut steps: Option<u64> = None;
    let mut draw_hex: Option<String> = None;
    let mut object: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--version" => {
                println!("{VERSION}");
                return;
            }
            "--profile" => profile_name = Some(args.next().unwrap_or_else(|| usage())),
            "--sample-bytes" => sample = Some(parse_u(&args.next().unwrap_or_else(|| usage()))),
            "--slot-bytes" => slot = Some(parse_u(&args.next().unwrap_or_else(|| usage()))),
            "--terrain-bytes" => terrain = Some(parse_u(&args.next().unwrap_or_else(|| usage()))),
            "--steps-per-leg" => steps = Some(parse_u(&args.next().unwrap_or_else(|| usage()))),
            "--draw-hex" => draw_hex = Some(args.next().unwrap_or_else(|| usage())),
            s if s.starts_with("--") => usage(),
            s => {
                if object.is_some() {
                    usage();
                }
                object = Some(s.to_string());
            }
        }
    }
    let object = object.unwrap_or_else(|| usage());
    let name = profile_name.as_deref().unwrap_or("production");
    let profile = match name {
        "production" | "test" | "live" => Profile::production(),
        "conformance-small" => {
            let (Some(sample), Some(slot), Some(terrain), Some(steps)) = (sample, slot, terrain, steps)
            else {
                usage();
            };
            match Profile::conformance_small(sample as usize, slot as usize, terrain as usize, steps)
            {
                Ok(p) => p,
                Err(msg) => {
                    eprintln!("{msg}");
                    std::process::exit(2);
                }
            }
        }
        _ => usage(),
    };
    if name != "conformance-small" && (sample.is_some() || slot.is_some() || terrain.is_some() || steps.is_some())
    {
        usage();
    }
    let bytes = match std::fs::read(&object) {
        Ok(b) => b,
        Err(_) => std::process::exit(1),
    };
    let mut cursor = draw_hex.as_ref().and_then(|h| hex_decode(h)).map(Cursor::new);
    if draw_hex.is_some() && cursor.is_none() {
        usage();
    }
    let mut empty = EmptyIndex;
    let result = if let Some(cur) = cursor.as_mut() {
        let mut next = || next8_from(cur);
        verify(&bytes, &profile, &mut empty, Some(&mut next))
    } else {
        verify(&bytes, &profile, &mut empty, None)
    };
    match result {
        Verified::Unreadable => std::process::exit(1),
        Verified::Record(s) | Verified::UnknownKind(s) => {
            println!("{s}");
        }
    }
}

fn parse_u(s: &str) -> u64 {
    match s.parse::<u64>() {
        Ok(n) => n,
        Err(_) => usage(),
    }
}
