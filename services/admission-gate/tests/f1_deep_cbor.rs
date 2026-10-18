//! F1 regression: a deeply nested CBOR object in the one pre-signature field the
//! door decodes (the sign-in `object`, via `hike::yielded_public_key` at
//! `door.rs` step 6, BEFORE `verify_signature`) must be refused as malformed and
//! must NOT abort the process.
//!
//! This test binds a real `axum::serve` on loopback and POSTs over TCP, so the
//! handler runs on a tokio worker thread with the production ~2 MiB stack — the
//! same path the live gate uses. A `oneshot`/tower test would poll the handler
//! on the test's own large stack and hide the overflow, so we do not use it.
//!
//! In-process only; binds 127.0.0.1:0, never a live host.
//!
//! On 565add5 (no depth cap) the worker overflows its stack and the whole test
//! binary aborts (`tokio-rt-worker … stack overflow, aborting`, SIGABRT). After
//! the depth cap lands (decode returns a `Fail` → `malformed`), the POST
//! returns HTTP 400 `{"error":{"code":"bad_signature"}}` (door maps Undecodable→no key) and the server keeps
//! serving, so this test passes.

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
    let dir = std::env::temp_dir().join(format!("pcoc-gate-f1-{label}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// CBOR map `{"public_key": h'11'*32, "zzz": [[[ … 1 ]]]}`, `depth` arrays deep.
/// The map yields a 32-byte key, so the unfixed decoder recurses `depth` frames
/// before the key is returned and the signature is ever checked.
fn nested_object(depth: usize) -> Vec<u8> {
    let mut obj = vec![0xa2u8, 0x6a];
    obj.extend_from_slice(b"public_key");
    obj.extend_from_slice(&[0x58, 0x20]);
    obj.extend_from_slice(&[0x11; 32]);
    obj.extend_from_slice(&[0x63, b'z', b'z', b'z']);
    obj.extend(std::iter::repeat(0x81u8).take(depth));
    obj.push(0x01);
    obj
}

fn signin_json(object: &[u8]) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    format!(
        r#"{{"origin":"{}","nonce":"{}","exp":{},"object":"{}","object_b64":"{}","signature":"{}"}}"#,
        // TEST_SERVED_ORIGIN used by Config::for_tests
        "https://groundinglab.io",
        "aa".repeat(32),
        now + 60,
        sha256_hex(object),
        b64(object),
        "00".repeat(64),
    )
}

/// Minimal blocking HTTP/1.1 POST. Returns the status line, or None if the peer
/// closed without a response (what a crashed worker looks like from the client).
fn post_signin(addr: std::net::SocketAddr, body: &str) -> Option<String> {
    let mut stream = TcpStream::connect(addr).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_deeply_nested_signin_object_is_malformed_and_the_gate_survives() {
    let dir = scratch("deep");
    let cfg = Config::for_tests(dir.clone(), "http://127.0.0.1:9");
    let service = Service::open(cfg).expect("gate");
    // Pin now via the signed body's exp window instead of the clock (real serve
    // uses the wall clock), so exp = now+60 is inside the window.
    let app = service
        .router()
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    // ~200 KB object (under the 2 MiB object cap), far past any sane depth cap.
    let deep = nested_object(200_000);
    let body = signin_json(&deep);
    let line = tokio::task::spawn_blocking(move || post_signin(addr, &body))
        .await
        .unwrap();

    // On the unfixed gate the worker aborts the whole process and this is never
    // reached. On the fixed gate the decode fails closed: HTTP 400 malformed.
    let line = line.expect("worker crashed: peer closed with no response (F1 present)");
    assert!(line.contains(" 400 "), "deep object must be 400, got: {line}");
    // Door maps Undecodable (SDK "malformed cbor") to client bad_signature at step 6
    // (no key yielded -> signature cannot verify). Same as locked_wire.rs.
    assert!(line.contains("bad_signature"), "expected bad_signature for over-deep nesting, got: {line}");

    // Survival probe: the server still answers after the deep POST.
    let shallow = nested_object(4);
    let body2 = signin_json(&shallow);
    let line2 = tokio::task::spawn_blocking(move || post_signin(addr, &body2))
        .await
        .unwrap()
        .expect("server should still serve after the deep POST");
    assert!(line2.contains(" 400 "), "shallow unsigned object should be 400: {line2}");

    let _ = std::fs::remove_dir_all(&dir);
}
