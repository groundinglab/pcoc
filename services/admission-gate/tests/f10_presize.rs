//! A 2 MiB object of 64 nested array heads, each count equal to the bytes left.
//! The sign-in door decodes it before the signature check. The reservation must
//! not follow that count: the POST is refused, the process stays up, and the
//! address-space high-water mark stays one array rather than one per level.
//!
//! In-process only. Binds 127.0.0.1:0.

#![forbid(unsafe_code)]

use base64::Engine as _;
use pcoc_admission::{Config, Service};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn scratch(label: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("pcoc-gate-f10-{label}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// 64 array heads (`9a` + count) then zeros. The count is the bytes still left.
fn amp(size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size];
    let mut n = 0usize;
    for _ in 0..64 {
        let cnt = (size - n - 5) as u32;
        out[n] = 0x9a;
        out[n + 1..n + 5].copy_from_slice(&cnt.to_be_bytes());
        n += 5;
    }
    out
}

fn signin_json(object: &[u8], lifetime_s: u64, skew_s: u64) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    // Issued at now + skew: past the nonce-store start, inside the lifetime.
    let exp = now.saturating_add(lifetime_s).saturating_add(skew_s);
    format!(
        r#"{{"origin":"{}","nonce":"{}","exp":{},"object":"{}","object_b64":"{}","signature":"{}"}}"#,
        "https://groundinglab.io",
        "aa".repeat(32),
        exp,
        sha256_hex(object),
        b64(object),
        "00".repeat(64),
    )
}

fn post_signin(addr: std::net::SocketAddr, body: &str) -> Option<String> {
    let mut stream = TcpStream::connect(addr).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok()?;
    let req = format!(
        "POST /v1/sign-in HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).ok()?;
    if buf.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(&buf);
    Some(text.lines().next().unwrap_or("").to_owned() + "\n" + &text)
}

fn vm_hwm_kb() -> u64 {
    let text = std::fs::read_to_string("/proc/self/status").unwrap();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("VmHWM:") else {
            continue;
        };
        return rest.split_whitespace().next().unwrap().parse().unwrap();
    }
    panic!("VmHWM");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_nested_count_does_not_presize_and_the_gate_survives() {
    let dir = scratch("amp");
    let cfg = Config::for_tests(dir.clone(), "http://127.0.0.1:9");
    let service = Service::open(cfg).expect("gate");
    let lifetime_s = service.door().policy().challenge_lifetime_seconds;
    let skew_s = service.door().policy().challenge_clock_skew_seconds;
    let app = service
        .router()
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    let before = vm_hwm_kb();
    // Largest object the JSON sign-in body can carry: base64 of 2 MiB plus the
    // six fields is 108 bytes over the transport limit, so this is 2 MiB − 80.
    let object = amp(2_097_072);
    let body = signin_json(&object, lifetime_s, skew_s);
    let line = tokio::task::spawn_blocking(move || post_signin(addr, &body))
        .await
        .unwrap()
        .expect("worker crashed: peer closed with no response");
    let after = vm_hwm_kb();
    assert!(line.contains(" 400 "), "nested heads must be 400, got: {line}");
    assert!(
        line.contains("bad_signature") || line.contains("malformed"),
        "expected a closed refusal, got: {line}"
    );
    // The decoded array is one value per input byte. 64 full-size reservations
    // would move this by gigabytes.
    let grew = after.saturating_sub(before);
    assert!(grew < 256 * 1024, "VmHWM grew {grew} KiB");

    let again = amp(4096);
    let body2 = signin_json(&again, lifetime_s, skew_s);
    let line2 = tokio::task::spawn_blocking(move || post_signin(addr, &body2))
        .await
        .unwrap()
        .expect("server should still serve after the nested POST");
    assert!(line2.contains(" 400 "), "follow-up should be 400: {line2}");
    let _ = std::fs::remove_dir_all(&dir);
}
