//! `satoshi-hike-verify` — CLI v0.1, lab `path-chain-bind`.
//!
//! Stdout is a measurement record when the object decodes. Exit 0 means that
//! record was produced. Exit 1 means the file could not be read or decoded.
//! Exit 2 is usage. Streams do not carry a policy verdict.
#![forbid(unsafe_code)]

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let test_index = std::env::var("PCOC_TEST_INDEX_URL").ok();
    let args = hike::apply_test_index_env(args, test_index.as_deref());
    let parsed = match hike::parse_verify(&args) {
        Ok(parsed) => parsed,
        Err(_) => {
            usage();
            return 2;
        }
    };
    let chain_rpc = match chain_rpc(&parsed) {
        Ok(chain_rpc) => chain_rpc,
        Err(()) => {
            usage();
            eprintln!(
                "satoshi-hike-verify: direct mainnet needs HIKE_CHAIN_RPC_USER and HIKE_CHAIN_RPC_PASSWORD"
            );
            return 2;
        }
    };
    let bytes = match std::fs::read(&parsed.input) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("satoshi-hike-verify: {err}");
            if parsed.log.emits(false) {
                emit_record(&hike::VerifyObservation::unopened(&parsed.input));
            }
            return 1;
        }
    };
    let draw = match parsed.audit_seed {
        Some(seed) => hike::AuditDraw::PinnedSeed(seed),
        None => hike::AuditDraw::OsCsprng,
    };
    let agent = hike::http_agent();
    let index_url = parsed.index_url.clone();
    let measurements = hike::verify_sized(
        &bytes,
        draw,
        |height| hike::read_index_beacon(&agent, &index_url, height).map_err(|_| ()),
        |loc| match &chain_rpc {
            Some(rpc) => match rpc.facts(loc) {
                Ok(facts) => match hike::direct_matches(&facts, loc) {
                    Ok(()) => hike::ChainRead::Match,
                    Err(()) => hike::ChainRead::Mismatch,
                },
                Err(_) => hike::ChainRead::NotRead,
            },
            None => match hike::index_document(&agent, &index_url, u64::from(loc.block_height)) {
                Ok(document) => match hike::proxy_matches(&document, loc) {
                    Ok(()) => hike::ChainRead::Match,
                    Err(()) => hike::ChainRead::Mismatch,
                },
                Err(_) => hike::ChainRead::NotRead,
            },
        },
        parsed.measure.sizes(),
    );
    // Exit 1 only when no top-level map decodes (a well-formed item that is
    // not a map included). Any other input gets its record and exit 0.
    let decoded = hike::top_level_map(&bytes);
    if parsed.log.emits(decoded) {
        emit_record(&hike::VerifyObservation::opened(
            &parsed.input,
            &bytes,
            &measurements,
        ));
    }
    if !decoded {
        return 1;
    }
    let profile = parsed.measure.record_name();
    let record = hike::verify_record_json(profile, &measurements);
    write_out(record.as_bytes(), parsed.report.as_deref())
}

fn chain_rpc(parsed: &hike::VerifyArgs) -> Result<Option<hike_net::ChainRpc>, ()> {
    if parsed.profile != hike::PlaneProfile::Live {
        return Ok(None);
    }
    let url = if let Some(url) = parsed.chain_rpc.clone() {
        url
    } else {
        match std::env::var("HIKE_CHAIN_RPC_URL") {
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Ok(url) if hike::absolute_http_without_trailing_slash(&url) => url,
            _ => return Err(()),
        }
    };
    let user = std::env::var("HIKE_CHAIN_RPC_USER").map_err(|_| ())?;
    let password = std::env::var("HIKE_CHAIN_RPC_PASSWORD").map_err(|_| ())?;
    if user.is_empty() || password.is_empty() {
        return Err(());
    }
    Ok(Some(hike_net::ChainRpc::new(url, user, password)))
}

fn usage() {
    eprintln!(
        "usage: satoshi-hike-verify --input PATH --index-url URL [--audit-seed HEX] [--report PATH] [--profile production|conformance-small|test|live] [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N] [--chain-rpc URL] [--log quiet|verbose]\n       default profile is test. Omitted --index-url is PCOC_TEST_INDEX_URL. Unset is a usage error. This tree does not fill that host. It does not fill https://index.groundinglab.io.\n       --profile live fills that index. --chain-rpc is live-only.\n       --profile conformance-small requires all four integers. Any of them with another profile is a usage error, exit 2.\n       --log defaults to quiet. verbose writes the measurement record to stderr. stdout is the measurement record when the object decodes."
    );
}

/// Subscribe to the record `hike::verify` already returned. Quiet writes nothing.
fn emit_record(observation: &hike::VerifyObservation) {
    let mut sink = StderrSubscriber;
    hike::publish_verify(observation, &mut sink);
}

struct StderrSubscriber;

impl hike::VerifySubscriber for StderrSubscriber {
    fn on_verify_event(&mut self, event: &hike::VerifyEvent) {
        eprintln!("{}", event.line());
    }
}

fn write_out(bytes: &[u8], report_path: Option<&str>) -> i32 {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if stdout.write_all(bytes).is_err() || stdout.flush().is_err() {
        eprintln!("satoshi-hike-verify: stdout");
        return 1;
    }
    if let Some(path) = report_path {
        if let Err(err) = std::fs::write(path, bytes) {
            eprintln!("satoshi-hike-verify: {path}: {err}");
        }
    }
    0
}
