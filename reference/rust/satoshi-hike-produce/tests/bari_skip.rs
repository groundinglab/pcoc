//! A missed Bari path is a short skip. Produce does not sit on the GET.
//!
//! The asks are the live book: one plant, one GET at each anchor-window
//! offset (H+2, H+3, H+4). The tip is a finite script, so no header is jumped
//! and no clock is waited on. `PCOC_TEST_PAUSE_MS=0` records the 15–45s draw
//! and returns. Unset, that pause still sleeps. The drawn wait stays on the
//! `path_ask_wait` line.

use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn local_listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, format!("127.0.0.1:{port}"))
}

fn serve_204_then_404(listener: TcpListener, posts: Arc<AtomicUsize>, gets: Arc<AtomicUsize>) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut seen = Vec::new();
                let mut buf = [0u8; 2048];
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            seen.extend_from_slice(&buf[..n]);
                            if seen.windows(4).any(|window| window == b"\r\n\r\n") {
                                break;
                            }
                        }
                    }
                }
                let head = String::from_utf8_lossy(&seen);
                let response = if head.starts_with("POST ") {
                    posts.fetch_add(1, Ordering::SeqCst);
                    "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                } else {
                    gets.fetch_add(1, Ordering::SeqCst);
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 12\r\nConnection: close\r\n\r\nnot included"
                };
                let _ = stream.write_all(response.as_bytes());
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
}

/// One settlement at a time. The ops capture buffer is process-wide.
fn one_hike() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

struct TipScript {
    tips: Vec<([u8; 32], u64)>,
    at: usize,
}

impl TipScript {
    fn next(&mut self) -> Result<([u8; 32], u64), hike::ProduceFailure> {
        let tip = self.tips.get(self.at).copied().ok_or_else(|| {
            hike::ProduceFailure::new("tip script ended before the hike returned")
        })?;
        self.at += 1;
        Ok(tip)
    }

    fn current(&self) -> ([u8; 32], u64) {
        self.tips[self.at - 1]
    }
}

/// One seed plus one interrupt for each of the 18 stages.
///
/// Stage 0 posts nothing. Stage 1's beacon is the first interrupt, height H.
/// The anchor-window headers are applied on the book inside that stage's post.
/// They are not stage interrupts: a different tip closes the stage.
fn scripted_tips() -> Vec<([u8; 32], u64)> {
    (0..19)
        .map(|i| {
            let mut hash = [0u8; 32];
            hash[0] = i as u8;
            hash[31] = 0x5a;
            (hash, 100 + i)
        })
        .collect()
}

#[test]
fn path_404_skips_and_produce_continues_without_a_hang() {
    let _one = one_hike();
    let started = Instant::now();
    hike::begin_ops_capture();

    let posts = Arc::new(AtomicUsize::new(0));
    let gets = Arc::new(AtomicUsize::new(0));
    let (bari_listener, bari_addr) = local_listener();
    let posts_srv = Arc::clone(&posts);
    let gets_srv = Arc::clone(&gets);
    thread::spawn(move || serve_204_then_404(bari_listener, posts_srv, gets_srv));
    let bari_url = format!("http://{bari_addr}");

    let dir = std::env::temp_dir().join(format!(
        "satoshi-hike-produce-bari-skip-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("out.cbor");

    // The live draw is still 15–45s. This test records it and does not sleep.
    // `PCOC_LIVE_PATH_ASK_WAIT_MS` does not change that draw.
    let previous_pause = std::env::var("PCOC_TEST_PAUSE_MS").ok();
    std::env::set_var("PCOC_TEST_PAUSE_MS", "0");

    let tips = Arc::new(Mutex::new(TipScript {
        tips: scripted_tips(),
        at: 0,
    }));
    let book = Rc::new(RefCell::new(hike::LivePathBook::new(
        hike::produce_http_agent(),
        bari_url,
    )));
    let produced = {
        let tips_observe = Arc::clone(&tips);
        let tips_open = Arc::clone(&tips);
        let book_open = Rc::clone(&book);
        // The old CLI cut-off was the first plant's three asks plus the
        // stage_closed that follows them. A later plant's window would add
        // GETs. Stop before that post. finish() is not reached, so path_skip
        // is not written.
        let plants = Rc::new(Cell::new(0u32));
        hike::produce_with_settlement_sized(
            [0xab; 32],
            hike::Sizes::conformance_small(),
            move || {
                tips_observe
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .next()
            },
            move |waypoint, beacon_height| {
                if plants.get() >= 1 {
                    return Err(hike::ProduceFailure::new(
                        "stop after the first plant's anchor window",
                    ));
                }
                plants.set(1);
                let (tip, height) = tips_open
                    .lock()
                    .unwrap_or_else(|err| err.into_inner())
                    .current();
                hike::emit_tip_in_hashcash(&tip, height);
                let mut book = book_open.borrow_mut();
                book.post_observing(waypoint, &tip, height, beacon_height, 8, |_after| {
                    Ok(Vec::new())
                })?;
                // One plant, asked at H+2, H+3, and H+4. Each header is one
                // step, so a jump cannot skip an offset.
                for step in 1..=hike::HEADERS_AFTER_POST {
                    let mut hash = [0x90; 32];
                    hash[0] = step as u8;
                    book.on_tip(hash, beacon_height + step)?;
                }
                Ok(Vec::new())
            },
            |_stages, _cut_off| Ok(()),
        )
    };

    match previous_pause {
        Some(value) => std::env::set_var("PCOC_TEST_PAUSE_MS", value),
        None => std::env::remove_var("PCOC_TEST_PAUSE_MS"),
    }

    let text = hike::take_ops_capture();
    let err = produced.expect_err(&text);
    assert!(
        err.to_string().contains("stop after the first plant"),
        "{err}\n{text}"
    );
    assert!(
        !output.exists(),
        "object is written after the leg, not on skip"
    );

    let fetches = text.matches("path_fetch call_site=fetch_path").count();
    assert!(
        fetches >= 3 && text.contains("stage_closed call_site=produce"),
        "produce did not ask the anchor window and continue\n{text}"
    );
    assert!(
        posts.load(Ordering::SeqCst) >= 1,
        "stage 0 is not posted; W_1 is\n{text}"
    );
    assert_eq!(
        gets.load(Ordering::SeqCst),
        3,
        "one plant is asked at each anchor-window offset\n{text}"
    );
    let heights: Vec<u64> = text
        .lines()
        .filter(|line| line.starts_with("path_fetch call_site=fetch_path"))
        .filter_map(|line| {
            line.split("tip_height=")
                .nth(1)
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|height| height.parse().ok())
        })
        .collect();
    assert_eq!(
        heights,
        vec![103, 104, 105],
        "the three asks are H+2, H+3, and H+4\n{text}"
    );
    let waits: Vec<u64> = text
        .lines()
        .filter(|line| line.starts_with("path_ask_wait call_site=fetch_path"))
        .filter_map(|line| {
            line.split("sleep_ms=")
                .nth(1)
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|ms| ms.parse().ok())
        })
        .collect();
    assert_eq!(waits.len(), 3, "{text}");
    assert!(
        waits.iter().all(|ms| (15_000..=45_000).contains(ms)),
        "each ask still draws 15–45s: {waits:?}\n{text}"
    );
    assert!(
        !text.contains("connectivity_wait call_site=fetch_path"),
        "{text}"
    );
    assert!(
        !text.contains("path_skip"),
        "path_skip is the step after the cut-off, not an ask result\n{text}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the anchor-window asks slept: {:?}",
        started.elapsed()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
