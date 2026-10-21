//! `satoshi-hike-produce` — CLI v0.1.
#![forbid(unsafe_code)]

use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = match take_demo_mnemonic(&args) {
        Ok(DemoFront::Show(key)) => {
            print!("{}", nip06::demo_mnemonic_report(&key));
            return 0;
        }
        Ok(DemoFront::Produce(args)) => args,
        Err(()) => {
            usage();
            return 2;
        }
    };
    let env_peer = std::env::var("PCOC_LIVE_GOSSIP_PEER").ok();
    let test_peer = std::env::var("PCOC_TEST_GOSSIP_PEER").ok();
    let test_bari = std::env::var("PCOC_TEST_BARI_URL").ok();
    let args = hike::apply_live_gossip_env(args, env_peer.as_deref());
    let args = hike::apply_test_produce_env(args, test_peer.as_deref(), test_bari.as_deref());
    let parsed = match hike::parse_produce(&args) {
        Ok(parsed) => parsed,
        Err(_) => {
            usage();
            return 2;
        }
    };
    if let Err(err) = hike::parent_dir_exists(&parsed.output) {
        eprintln!("satoshi-hike-produce: {err}");
        let report = parsed
            .report
            .as_deref()
            .filter(|path| hike::parent_dir_exists(path).is_ok());
        return finish(1, &hike::Report::produce_fail(), report);
    }
    if let Some(path) = &parsed.report {
        if let Err(err) = hike::parent_dir_exists(path) {
            eprintln!("satoshi-hike-produce: {err}");
            return finish(1, &hike::Report::produce_fail(), None);
        }
    }

    let agent = hike::produce_http_agent();
    let bari_url = parsed.bari_url.clone();
    let bits = hike::effective_bits();
    let profile = match parsed.profile {
        hike::PlaneProfile::Test => "test",
        hike::PlaneProfile::Live => "live",
        hike::PlaneProfile::SlowLan => "slow-lan",
    };
    if parsed.profile == hike::PlaneProfile::Live {
        eprintln!(
            "satoshi-hike-produce: profile={profile}; progress is the event log, this stderr, and the final report; the object file is written only after the report"
        );
    } else {
        eprintln!(
            "satoshi-hike-produce: profile={profile}; progress is this stderr and the final report; the object file is written only after the report"
        );
    }
    let produced = match parsed.clock {
        hike::ProduceClock::Http { .. } => {
            eprintln!("satoshi-hike-produce: HTTP tip is not a produce Observe");
            usage();
            return 2;
        }
        hike::ProduceClock::Gossip { dial, network } => {
            let clock = match dial {
                hike::GossipDial::Named(peer) => hike::HeaderClock::start(peer, network),
                hike::GossipDial::MainnetDnsSeeds => hike::HeaderClock::start_mainnet_dns_seeds(),
            };
            let mut book = hike::LivePathBook::new(agent, bari_url);
            if parsed.profile == hike::PlaneProfile::Live {
                let log_path = hike::produce_event_log_path(&parsed.output);
                match book.open_event_log(&parsed.output) {
                    Ok(()) => {
                        eprintln!("satoshi-hike-produce: event log {}", log_path.display());
                    }
                    Err(err) => {
                        eprintln!(
                            "satoshi-hike-produce: event log {}: {err}",
                            log_path.display()
                        );
                    }
                }
            }
            match produce_anchored(book, bits, &clock, parsed.public_key) {
                Ok(bytes) => {
                    print_hike_summary(&bytes, &clock);
                    Ok(bytes)
                }
                Err(err) => Err(err),
            }
        }
    };
    let bytes = match produced {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("satoshi-hike-produce: {err}");
            return finish(1, &hike::Report::produce_fail(), parsed.report.as_deref());
        }
    };
    let tmp = match stage_object(&parsed.output, &bytes) {
        Ok(tmp) => tmp,
        Err(err) => {
            eprintln!("satoshi-hike-produce: {err}");
            return finish(1, &hike::Report::produce_fail(), parsed.report.as_deref());
        }
    };
    let report = hike::Report::produce_pass(parsed.output.clone());
    if let Err(err) = hike::emit_report(&report, parsed.report.as_deref()) {
        eprintln!("satoshi-hike-produce: {err}");
        let _ = std::fs::remove_file(&tmp);
        return 1;
    }
    if let Err(err) = std::fs::rename(&tmp, &parsed.output) {
        eprintln!("satoshi-hike-produce: {err}");
        let _ = std::fs::remove_file(&tmp);
        return 1;
    }
    0
}

/// Test and live. Post `W_g` at stage open. Ask at the anchor-window offsets from H.
///
/// H is the stage beacon height passed into the post, not the Hashcash tip.
/// Not a one-minute stagger, and not a fetch at stage open. Every header since
/// the last tip is applied, so an ask header is not skipped when a later tip
/// is already in hand. There is no fee bump, RBF, replace, or repair of a plant.
/// Stage 0 is not posted.
fn produce_anchored(
    book: hike::LivePathBook,
    bits: u32,
    clock: &hike::HeaderClock,
    public_key: [u8; 32],
) -> Result<Vec<u8>, hike::ProduceFailure> {
    let book = std::cell::RefCell::new(book);
    let mut stage = 0usize;
    hike::produce_with_settlement(
        public_key,
        || {
            let mut book = book.borrow_mut();
            hike::note_produce_headers(clock, &mut book)
        },
        |waypoint, beacon_height| {
            stage += 1;
            eprintln!(
                "satoshi-hike-produce: stage {stage} opening; progress is stderr and the report, not the object file"
            );
            let (tip, height) = {
                let mut book = book.borrow_mut();
                hike::note_produce_headers(clock, &mut book)?
            };
            hike::emit_tip_in_hashcash(&tip, height);
            book.borrow_mut().post_observing(
                waypoint,
                &tip,
                height,
                beacon_height,
                bits,
                |after| {
                    clock.headers_since(after).map(|ticks| {
                        ticks
                            .into_iter()
                            .map(|tick| (tick.hash, tick.height))
                            .collect()
                    })
                },
            )?;
            eprintln!(
                "satoshi-hike-produce: stage {stage} posted; the path is asked at the anchor-window offsets from this stage's beacon height"
            );
            Ok(Vec::new())
        },
        |stages, cut_off| {
            book.borrow_mut().finish(stages, cut_off, |after| {
                let ticks = clock.wait_tips_after(after)?;
                Ok(ticks
                    .into_iter()
                    .map(|tick| (tick.hash, tick.height))
                    .collect())
            })
        },
    )
}

/// One line for the measurement runs: legs, the first and last beacon, and legs/s.
fn print_hike_summary(bytes: &[u8], clock: &hike::HeaderClock) {
    let Ok(hike) = hike::decoded_hike(bytes) else {
        return;
    };
    let legs: u64 = hike.stages.iter().map(|stage| stage.leg_count).sum();
    let (first_height, first_time) = match hike.stages.first() {
        Some(stage) => (
            Some(stage.beacon_height),
            clock.n_time(&stage.beacon),
        ),
        None => (None, None),
    };
    let last_time = clock.n_time(&hike.last_interrupt);
    let steps = legs.saturating_mul(hike::STEPS_PER_LEG);
    let raw_rate = rate_per_s(legs, first_time, last_time);
    let raw_steps = rate_per_s(steps, first_time, last_time);
    let first_mtp = hike
        .stages
        .first()
        .and_then(|stage| clock.median_time_past(&stage.beacon));
    let last_mtp = clock.median_time_past(&hike.last_interrupt);
    let mtp_rate = rate_per_s(legs, first_mtp, last_mtp);
    let mtp_steps = rate_per_s(steps, first_mtp, last_mtp);
    let show = |value: Option<u64>| value.map(|item| item.to_string()).unwrap_or_else(|| "not read".to_owned());
    let show_u32 = |value: Option<u32>| value.map(|item| item.to_string()).unwrap_or_else(|| "not read".to_owned());
    eprintln!(
        "satoshi-hike-produce: hike_summary legs={legs} steps={steps} first_beacon_height={} first_beacon_time={} last_beacon_height={} last_beacon_time={} legs_per_s_raw_ntime={} steps_per_s_raw_ntime={} first_beacon_mtp={} last_beacon_mtp={} legs_per_s_mtp={} steps_per_s_mtp={}",
        show(first_height),
        show_u32(first_time),
        hike.last_interrupt_height,
        show_u32(last_time),
        show_rate(raw_rate),
        show_rate(raw_steps),
        show_u32(first_mtp),
        show_u32(last_mtp),
        show_rate(mtp_rate),
        show_rate(mtp_steps)
    );
}

fn rate_per_s(legs: u64, first: Option<u32>, last: Option<u32>) -> Option<f64> {
    match (first, last) {
        (Some(first), Some(last)) if last > first => Some(legs as f64 / f64::from(last - first)),
        _ => None,
    }
}

fn show_rate(rate: Option<f64>) -> String {
    rate.map(|item| format!("{item:.6}"))
        .unwrap_or_else(|| "not read".to_owned())
}

fn finish(status: i32, report: &hike::Report, report_path: Option<&str>) -> i32 {
    if let Err(err) = hike::emit_report(report, report_path) {
        eprintln!("satoshi-hike-produce: {err}");
        return 1;
    }
    status
}

fn stage_object(path: &str, bytes: &[u8]) -> Result<std::path::PathBuf, String> {
    let dest = Path::new(path);
    let parent = match dest.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = dest
        .file_name()
        .ok_or_else(|| "output path has no file name".to_owned())?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(err) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(err.to_string());
    }
    Ok(tmp)
}

enum DemoFront {
    Show(nip06::DemoKey),
    Produce(Vec<String>),
}

fn usage() {
    eprintln!(
        "usage: satoshi-hike-produce --pubkey-hex HEX --output PATH --gossip-peer HOST:PORT --bari-url URL [--report PATH] [--profile test|live|slow-lan] [--clock-source gossip] [--gossip-network mainnet|regtest]\n       satoshi-hike-produce --demo-mnemonic PHRASE [--output PATH] [--report PATH] [--profile test|live|slow-lan] [--gossip-peer HOST:PORT] [--bari-url URL] [--clock-source gossip] [--gossip-network mainnet|regtest]\n       default profile is test. Observe is the synth tip sequence. Omitted --gossip-peer is PCOC_TEST_GOSSIP_PEER. Omitted --bari-url is PCOC_TEST_BARI_URL. Unset is a usage error. This tree does not fill those hosts. Omitted --gossip-network is regtest wire magic only (header network magic), not a Bitcoin Core tip clock or plant. slow-lan defaults to that same wire magic. It does not fill a live origin.\n       --profile live Observes mainnet gossip. With no --gossip-peer and no PCOC_LIVE_GOSSIP_PEER, the starting list is Bitcoin Core's mainnet DNS seeds. A lookup returns node addresses. The producer connects and asks those nodes for more peers. A bad address fails and is discarded. A named --gossip-peer or PCOC_LIVE_GOSSIP_PEER (host:port) still wins, and the command line wins over the environment variable. This tree does not invent that host or port. The network is mainnet. An omitted --bari-url is https://bari.groundinglab.io. --gossip-network regtest on live is a usage error.\n       --tip-url and --clock-source http are usage errors. Bitcoin Core, regtest, and testnet4 are not the test tip clock or plant. The test profile and slow-lan do not use the mainnet DNS seeds.\n       A fresh test key is `satoshi-hike-keygen --pub-out FILE`. The producer takes only --pubkey-hex. It does not read a secret."
    );
}

fn eprint_demo(key: &nip06::DemoKey) {
    eprintln!(
        "satoshi-hike-produce: DEMO ONLY mnemonic on the command line; empty passphrase; NIP-06 {}; public_key={}",
        nip06::PATH,
        key.public_key_hex()
    );
}

/// Lab flag. Not a Lock amend of CLI v0.1. The locked parser still does not know this flag.
/// With no `--output`, print the x-only public key and exit. With `--output`, derive that key and produce.
fn take_demo_mnemonic(args: &[String]) -> Result<DemoFront, ()> {
    let mut phrase = None;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--demo-mnemonic" || arg.starts_with("--demo-mnemonic=") {
            if phrase.is_some() {
                eprintln!("satoshi-hike-produce: --demo-mnemonic repeated");
                return Err(());
            }
            let value = if let Some(value) = arg.strip_prefix("--demo-mnemonic=") {
                if value.is_empty() {
                    eprintln!("satoshi-hike-produce: --demo-mnemonic needs the 12 words");
                    return Err(());
                }
                value.to_owned()
            } else {
                i += 1;
                let Some(next) = args.get(i) else {
                    eprintln!("satoshi-hike-produce: --demo-mnemonic needs the 12 words");
                    return Err(());
                };
                if next.starts_with('-') {
                    eprintln!("satoshi-hike-produce: --demo-mnemonic needs the 12 words");
                    return Err(());
                }
                next.clone()
            };
            phrase = Some(value);
        } else {
            rest.push(arg.clone());
        }
        i += 1;
    }
    let Some(phrase) = phrase else {
        return Ok(DemoFront::Produce(rest));
    };
    if rest
        .iter()
        .any(|arg| arg == "--pubkey-hex" || arg.starts_with("--pubkey-hex="))
    {
        eprintln!("satoshi-hike-produce: --demo-mnemonic and --pubkey-hex together");
        return Err(());
    }
    let key = match nip06::from_mnemonic(&phrase) {
        Ok(key) => key,
        Err(err) => {
            eprintln!("satoshi-hike-produce: {err}");
            return Err(());
        }
    };
    eprint_demo(&key);
    let has_output = rest
        .iter()
        .any(|arg| arg == "--output" || arg.starts_with("--output="));
    if !has_output {
        return Ok(DemoFront::Show(key));
    }
    let mut produce_args = Vec::with_capacity(rest.len() + 2);
    produce_args.push("--pubkey-hex".to_owned());
    produce_args.push(key.public_key_hex());
    produce_args.extend(rest);
    Ok(DemoFront::Produce(produce_args))
}
