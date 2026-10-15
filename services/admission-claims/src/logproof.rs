//! One shot at `GET {gate_base}/log/{day}/{seq}`. No retry.
//!
//! A thorough `accept`, or a `return` that cites one, opens the window when
//! the line carries `policy_id` and `now - window <= time_unix_s <= now + skew`.
//! A chain of returns is refused. Policy, object, and the `r` target come from
//! the accept.

use crate::error::Fail;
use crate::nostr::is_lower_hex;
use crate::timeutil::utc_day;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Admission {
    pub pubkey: String,
    pub policy_id: String,
    pub object_sha256: String,
    pub accept_day: String,
    pub accept_seq: i64,
    pub admitted_unix_s: i64,
}

struct Fetch<'a> {
    client: &'a reqwest::Client,
    gate_base: &'a str,
    max_bytes: usize,
    timeout: Duration,
}

struct Limits {
    now: i64,
    window_secs: i64,
    future_skew: i64,
}

pub struct ProofRequest<'a> {
    pub client: &'a reqwest::Client,
    pub gate_base: &'a str,
    pub day: &'a str,
    pub seq: i64,
    pub now: i64,
    pub window_secs: i64,
    pub future_skew: i64,
    pub max_bytes: usize,
    pub timeout: Duration,
}

pub async fn prove(request: ProofRequest<'_>) -> Result<Admission, Fail> {
    let fetch = Fetch {
        client: request.client,
        gate_base: request.gate_base,
        max_bytes: request.max_bytes,
        timeout: request.timeout,
    };
    let limits = Limits {
        now: request.now,
        window_secs: request.window_secs,
        future_skew: request.future_skew,
    };
    let line = get_line(&fetch, request.day, request.seq).await?;
    classify(&line, request.day, request.seq, &limits, &fetch).await
}

async fn classify(
    line: &Value,
    day: &str,
    seq: i64,
    limits: &Limits,
    fetch: &Fetch<'_>,
) -> Result<Admission, Fail> {
    if line_seq(line)? != seq {
        return Err(Fail::no_access());
    }
    let time = line_i64(line, "time_unix_s")?;
    if utc_day(time) != day {
        return Err(Fail::no_access());
    }
    let phase = line_str(line, "phase").unwrap_or("");
    let decision = line_str(line, "decision").unwrap_or("");
    if phase == "provisional" || decision == "provisional" {
        return Err(Fail::provisional());
    }
    let policy = match line_str(line, "policy_id") {
        Some(value) if !value.is_empty() => value.to_owned(),
        _ => return Err(Fail::no_access()),
    };
    if decision == "return" || phase == "return" {
        return prove_return(line, &policy, time, limits, fetch).await;
    }
    if decision != "accept" || phase != "thorough" {
        return Err(Fail::no_access());
    }
    check_window(time, limits)?;
    let pubkey = require_pubkey(line)?;
    let object = require_object(line)?;
    Ok(Admission {
        pubkey,
        policy_id: policy,
        object_sha256: object,
        accept_day: day.to_owned(),
        accept_seq: seq,
        admitted_unix_s: time,
    })
}

async fn prove_return(
    line: &Value,
    policy: &str,
    time: i64,
    limits: &Limits,
    fetch: &Fetch<'_>,
) -> Result<Admission, Fail> {
    check_window(time, limits)?;
    let pubkey = require_pubkey(line)?;
    let ref_day = line_str(line, "access_ref_day").ok_or_else(Fail::no_access)?;
    let ref_seq = line
        .get("access_ref_seq")
        .and_then(json_i64)
        .ok_or_else(Fail::no_access)?;
    if ref_seq < 1 {
        return Err(Fail::no_access());
    }
    let accept = get_line(fetch, ref_day, ref_seq).await?;
    if line_str(&accept, "phase") != Some("thorough")
        || line_str(&accept, "decision") != Some("accept")
    {
        return Err(Fail::no_access());
    }
    if line_str(&accept, "decision") == Some("return")
        || line_str(&accept, "phase") == Some("return")
    {
        return Err(Fail::no_access());
    }
    let accept_policy = line_str(&accept, "policy_id").ok_or_else(Fail::no_access)?;
    if accept_policy.is_empty() || accept_policy != policy {
        return Err(Fail::no_access());
    }
    let accept_pubkey = require_pubkey(&accept)?;
    if accept_pubkey != pubkey {
        return Err(Fail::no_access());
    }
    let object = require_object(&accept)?;
    if let Some(return_object) = line_str(line, "object_sha256") {
        if return_object != object {
            return Err(Fail::no_access());
        }
    } else {
        return Err(Fail::no_access());
    }
    let accept_time = line_i64(&accept, "time_unix_s")?;
    if utc_day(accept_time) != ref_day {
        return Err(Fail::no_access());
    }
    if line_seq(&accept)? != ref_seq {
        return Err(Fail::no_access());
    }
    Ok(Admission {
        pubkey,
        policy_id: policy.to_owned(),
        object_sha256: object,
        accept_day: ref_day.to_owned(),
        accept_seq: ref_seq,
        admitted_unix_s: accept_time,
    })
}

fn check_window(time: i64, limits: &Limits) -> Result<(), Fail> {
    if time < limits.now.saturating_sub(limits.window_secs)
        || time > limits.now.saturating_add(limits.future_skew)
    {
        return Err(Fail::window());
    }
    Ok(())
}

async fn get_line(fetch: &Fetch<'_>, day: &str, seq: i64) -> Result<Value, Fail> {
    let url = format!("{}/log/{day}/{seq}", fetch.gate_base);
    let response = tokio::time::timeout(fetch.timeout, fetch.client.get(&url).send())
        .await
        .map_err(|_| Fail::unavailable())?
        .map_err(|_| Fail::unavailable())?;
    let status = response.status();
    if status.as_u16() == 404 {
        return Err(Fail::no_access());
    }
    if !status.is_success() {
        return Err(Fail::unavailable());
    }
    let bytes = read_capped(response, fetch.max_bytes).await?;
    serde_json::from_slice(&bytes).map_err(|_| Fail::unavailable())
}

async fn read_capped(mut response: reqwest::Response, max_bytes: usize) -> Result<Vec<u8>, Fail> {
    let mut buf = Vec::new();
    loop {
        let chunk = response.chunk().await.map_err(|_| Fail::unavailable())?;
        let Some(chunk) = chunk else {
            break;
        };
        if buf.len().saturating_add(chunk.len()) > max_bytes {
            return Err(Fail::unavailable());
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

fn require_pubkey(line: &Value) -> Result<String, Fail> {
    let pubkey = line_str(line, "public_key").ok_or_else(Fail::no_access)?;
    if !is_lower_hex(pubkey, 64) {
        return Err(Fail::no_access());
    }
    Ok(pubkey.to_owned())
}

fn require_object(line: &Value) -> Result<String, Fail> {
    let object = line_str(line, "object_sha256").ok_or_else(Fail::no_access)?;
    if !is_lower_hex(object, 64) {
        return Err(Fail::no_access());
    }
    Ok(object.to_owned())
}

fn line_str<'a>(line: &'a Value, key: &str) -> Option<&'a str> {
    line.get(key).and_then(Value::as_str)
}

fn line_i64(line: &Value, key: &str) -> Result<i64, Fail> {
    line.get(key).and_then(json_i64).ok_or_else(Fail::no_access)
}

fn line_seq(line: &Value) -> Result<i64, Fail> {
    line_i64(line, "seq")
}

fn json_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
}
