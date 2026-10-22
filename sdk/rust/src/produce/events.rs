//! SPEC §13.5 live event records. Compact JSON, keys in the order that section lists.

use crate::hexutil::hex_encode;

fn json_bool(v: bool) -> &'static str {
    if v {
        "true"
    } else {
        "false"
    }
}

pub fn block_line(height: u64, tip: &[u8; 32]) -> String {
    format!(
        "{{\"event\":\"hike.produce.block\",\"height\":{height},\"tip\":\"{}\"}}\n",
        hex_encode(tip)
    )
}

pub fn waypoint_line(stage: usize, waypoint: &[u8; 32]) -> String {
    format!(
        "{{\"event\":\"hike.produce.waypoint\",\"stage\":{stage},\"waypoint\":\"{}\"}}\n",
        hex_encode(waypoint)
    )
}

pub fn posted_line(
    stage: usize,
    waypoint: &[u8; 32],
    attempt: u8,
    posted: bool,
    succeeded: bool,
    dropped: bool,
    status: Option<u16>,
) -> String {
    let mut line = format!(
        "{{\"event\":\"hike.produce.posted\",\"stage\":{stage},\"waypoint\":\"{}\",\"attempt\":{attempt},\"posted\":{},\"succeeded\":{},\"dropped\":{}",
        hex_encode(waypoint),
        json_bool(posted),
        json_bool(succeeded),
        json_bool(dropped),
    );
    if let Some(status) = status {
        line.push_str(&format!(",\"status\":{status}"));
    }
    line.push_str("}\n");
    line
}

pub fn checked_line(waypoint: &[u8; 32], checked: bool) -> String {
    format!(
        "{{\"event\":\"hike.produce.checked\",\"waypoint\":\"{}\",\"checked\":{}}}\n",
        hex_encode(waypoint),
        json_bool(checked)
    )
}

/// `miss` is set only when `body` is exactly one of the five §13.5 phrases.
pub fn fetch_line(
    waypoint: &[u8; 32],
    worked: bool,
    status: Option<u16>,
    block: Option<([u8; 32], u32)>,
    body: &[u8],
) -> String {
    let mut line = format!(
        "{{\"event\":\"hike.produce.fetch\",\"waypoint\":\"{}\",\"worked\":{}",
        hex_encode(waypoint),
        json_bool(worked)
    );
    if let Some(status) = status {
        line.push_str(&format!(",\"status\":{status}"));
    }
    if let Some((hash, height)) = block {
        line.push_str(&format!(
            ",\"block\":\"{}\",\"block_height\":{height}",
            hex_encode(&hash)
        ));
    }
    if let Some(phrase) = miss_phrase(body) {
        line.push_str(&format!(",\"miss\":\"{phrase}\""));
    }
    line.push_str("}\n");
    line
}

pub fn alert_line(alert: &str, stage: usize, height: Option<u64>) -> String {
    let mut line = format!(
        "{{\"event\":\"hike.produce.alert\",\"alert\":\"{alert}\",\"stage\":{stage}"
    );
    if let Some(height) = height {
        line.push_str(&format!(",\"height\":{height}"));
    }
    line.push_str("}\n");
    line
}

/// Append records off the walk. `recv_timeout` so a live sender in `Run` does not stall shutdown.
pub(crate) fn write_loop(
    mut file: std::fs::File,
    rx: std::sync::mpsc::Receiver<String>,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::io::Write;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(line) => {
                let _ = file.write_all(line.as_bytes());
            }
            Err(RecvTimeoutError::Timeout) => {
                if cancel.load(Ordering::SeqCst) {
                    while let Ok(line) = rx.try_recv() {
                        let _ = file.write_all(line.as_bytes());
                    }
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                while let Ok(line) = rx.try_recv() {
                    let _ = file.write_all(line.as_bytes());
                }
                break;
            }
        }
    }
}

fn miss_phrase(body: &[u8]) -> Option<&'static str> {
    const PHRASES: &[&str] = &[
        "still in the pool",
        "sealed but not in a block yet",
        "not included",
        "already archived after it was served",
        "unknown",
    ];
    let text = std::str::from_utf8(body).ok()?;
    PHRASES.iter().copied().find(|p| *p == text)
}
