//! Bari-owned header source.
//!
//! SPEC §9 names the live header source. That source, and the Bari client a
//! producer would dial, are Phase C and are not in the 2.2 SDK. This module
//! keeps the synth plane the test gate already runs, and fails closed on any
//! live network. It is not a pin to an older SDK.
//!
//! The synth wire is newline-delimited JSON, one tip per line:
//! `{"hash","height","previous","n_time"}`. Hash and previous are natural-order
//! hex. `n_time` is the header's unix seconds (SPEC §9 header time). Zero is a
//! real time.

use crate::{decode_bitcoin_id, Hash32};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

/// One gossip tip. `height` is the block height. `n_time` is header seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TipTick {
    pub hash: Hash32,
    pub height: u64,
    pub previous: Option<Hash32>,
    pub n_time: u32,
}

/// Which header wire this clock speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GossipNetwork {
    /// Synth plane. Not Bitcoin Core.
    Regtest,
    /// Live Bitcoin headers. Not in this build (SPEC §9, Phase C).
    Mainnet,
}

/// Why a header read stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderError {
    pub message: String,
}

enum ClockState {
    /// Live source. Every wait fails closed.
    Closed { message: String },
    Synth(TcpStream),
}

/// Blocking header clock. `wait_tips_after` is called from a blocking thread.
pub struct HeaderClock {
    state: ClockState,
}

impl HeaderClock {
    /// Open a clock. A live network does not dial. Regtest retries the synth peer.
    pub fn start(peer: &str, network: GossipNetwork) -> Self {
        match network {
            GossipNetwork::Mainnet => Self {
                state: ClockState::Closed {
                    message: "live header source is not in this build".to_owned(),
                },
            },
            GossipNetwork::Regtest => {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                loop {
                    match TcpStream::connect(peer) {
                        Ok(stream) => {
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                            let _ = stream.set_nodelay(true);
                            return Self {
                                state: ClockState::Synth(stream),
                            };
                        }
                        Err(error) => {
                            if std::time::Instant::now() > deadline {
                                return Self {
                                    state: ClockState::Closed {
                                        message: format!("synth peer {peer} did not accept: {error}"),
                                    },
                                };
                            }
                            thread::sleep(Duration::from_millis(20));
                        }
                    }
                }
            }
        }
    }

    /// The next tip after `after`. `None` is the opening tip.
    ///
    /// One tip per call, in chain order, so a caller that applies every tick
    /// stays on a continuous previous-hash link.
    pub fn wait_tips_after(&mut self, _after: Option<Hash32>) -> Result<Vec<TipTick>, HeaderError> {
        match &mut self.state {
            ClockState::Closed { message } => Err(HeaderError {
                message: message.clone(),
            }),
            ClockState::Synth(stream) => {
                let line = read_line(stream)?;
                let tick = parse_tick(&line)?;
                Ok(vec![tick])
            }
        }
    }
}

/// Bind `bind` and emit a fresh chain on every connection, one tip per `interval`.
///
/// Blocks until the listener closes. A dropped client ends that connection
/// and the next accept starts again at the opening tip.
pub fn serve_synth(bind: &str, interval: Duration) -> Result<(), String> {
    let listener = TcpListener::bind(bind).map_err(|error| error.to_string())?;
    for conn in listener.incoming() {
        let mut stream = match conn {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        let _ = stream.set_nodelay(true);
        let mut height = 100u64;
        let mut previous: Option<Hash32> = None;
        loop {
            let tick = TipTick {
                hash: tip_hash(height),
                height,
                previous,
                n_time: 1_700_000_000u32.saturating_add(height as u32),
            };
            if write_tick(&mut stream, &tick).is_err() {
                break;
            }
            previous = Some(tick.hash);
            height = height.saturating_add(1);
            thread::sleep(interval);
        }
    }
    Ok(())
}

fn tip_hash(height: u64) -> Hash32 {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"pcoc-bari-synth-tip-v1");
    hasher.update(height.to_le_bytes());
    hasher.finalize().into()
}

fn write_tick(stream: &mut TcpStream, tick: &TipTick) -> Result<(), ()> {
    let previous = match tick.previous {
        Some(hash) => format!("\"{}\"", hex::encode(hash)),
        None => "null".to_owned(),
    };
    let line = format!(
        "{{\"hash\":\"{}\",\"height\":{},\"previous\":{},\"n_time\":{}}}\n",
        hex::encode(tick.hash),
        tick.height,
        previous,
        tick.n_time
    );
    stream.write_all(line.as_bytes()).map_err(|_| ())?;
    stream.flush().map_err(|_| ())
}

fn read_line(stream: &mut TcpStream) -> Result<String, HeaderError> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => {
                return Err(HeaderError {
                    message: "synth closed before a tip".to_owned(),
                });
            }
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                buf.push(byte[0]);
                if buf.len() > 4096 {
                    return Err(HeaderError {
                        message: "synth tip line is too long".to_owned(),
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(HeaderError {
                    message: format!("synth read failed: {error}"),
                });
            }
        }
    }
    String::from_utf8(buf).map_err(|_| HeaderError {
        message: "synth tip line is not utf-8".to_owned(),
    })
}

fn parse_tick(line: &str) -> Result<TipTick, HeaderError> {
    let value: Value = serde_json::from_str(line).map_err(|error| HeaderError {
        message: format!("synth tip is not json: {error}"),
    })?;
    let hash = parse_hash(value.get("hash"))?;
    let height = value.get("height").and_then(Value::as_u64).ok_or_else(|| HeaderError {
        message: "synth tip has no height".to_owned(),
    })?;
    let previous = match value.get("previous") {
        None | Some(Value::Null) => None,
        Some(_) => Some(parse_hash(value.get("previous"))?),
    };
    let n_time = value
        .get("n_time")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| HeaderError {
            message: "synth tip has no n_time".to_owned(),
        })?;
    Ok(TipTick {
        hash,
        height,
        previous,
        n_time,
    })
}

fn parse_hash(value: Option<&Value>) -> Result<Hash32, HeaderError> {
    let text = value.and_then(Value::as_str).ok_or_else(|| HeaderError {
        message: "synth tip hash is not a string".to_owned(),
    })?;
    let raw = hex::decode(text).map_err(|error| HeaderError {
        message: format!("synth tip hash: {error}"),
    })?;
    raw.try_into().map_err(|_| HeaderError {
        message: "synth tip hash is not 32 bytes".to_owned(),
    })
}

/// Header hash (natural order) and height from a stored portable-index body.
///
/// The stored header hash is Bitcoin display hex. This returns natural order,
/// which is what `ChainTip.hash` carries.
pub fn parse_index_document(body: &str) -> Result<(Hash32, u64), String> {
    let value: Value = serde_json::from_str(body).map_err(|error| error.to_string())?;
    let hash = value
        .pointer("/header/hash")
        .and_then(Value::as_str)
        .ok_or_else(|| "index document has no header hash".to_owned())?;
    let height = value
        .pointer("/header/height")
        .and_then(Value::as_u64)
        .ok_or_else(|| "index document has no header height".to_owned())?;
    Ok((decode_bitcoin_id(hash)?, height))
}

/// `true` when `url` names a host that is not the local test plane.
pub fn live_plane_host(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    match parsed.host_str() {
        Some(host) => {
            let host = host.trim_start_matches('[').trim_end_matches(']');
            !matches!(host, "127.0.0.1" | "localhost" | "::1" | "0.0.0.0")
        }
        None => false,
    }
}

/// What the test-plane gate observed. This is not admission policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestGateMeasurement {
    pub planted: bool,
    pub path_status: u16,
    pub index_is_live: bool,
    pub index_matches_tip: bool,
}

/// Pass only when the test plane planted, served the path, and the index
/// matches the tip, and the index host is not a live plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateJudgment {
    Pass,
    Fail,
}

/// A skip recorded beside the test gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateSkip {
    pub reason: String,
}

/// Test-plane judgment. A live index host cannot pass.
pub fn judge_test_policy(measurement: &TestGateMeasurement) -> GateJudgment {
    if measurement.planted
        && measurement.path_status == 200
        && measurement.index_matches_tip
        && !measurement.index_is_live
    {
        GateJudgment::Pass
    } else {
        GateJudgment::Fail
    }
}
