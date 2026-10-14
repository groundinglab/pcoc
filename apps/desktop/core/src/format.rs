//! Numbers and clocks the way the screens print them. Tabular, English, UTC.

use std::path::{Path, PathBuf};

/// `3,559`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `3 h 13 m 29 s`, `1 h 07 m`, `2 m 03 s`, `12 s`.
pub fn duration_hms(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    if h > 0 {
        format!("{h} h {m:02} m {s:02} s")
    } else if m > 0 {
        format!("{m} m {s:02} s")
    } else {
        format!("{s} s")
    }
}

/// `1 h 57 m`, `46 m`. Minutes are floored.
pub fn duration_hm(seconds: u64) -> String {
    let (h, m) = (seconds / 3600, (seconds % 3600) / 60);
    if h > 0 {
        format!("{h} h {m:02} m")
    } else {
        format!("{m} m")
    }
}

/// `0.308`. Three places, rounded half away from zero.
pub fn three_places(numerator: u64, denominator: u64) -> Option<String> {
    if denominator == 0 {
        return None;
    }
    let scaled = (numerator as u128 * 1000 * 2 + denominator as u128) / (denominator as u128 * 2);
    Some(format!("{}.{:03}", scaled / 1000, scaled % 1000))
}

/// Civil date from Unix days (proleptic Gregorian, UTC).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub struct Utc {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

pub fn utc(unix_s: u64) -> Utc {
    let days = (unix_s / 86_400) as i64;
    let rem = unix_s % 86_400;
    let (year, month, day) = civil(days);
    Utc {
        year,
        month,
        day,
        hour: (rem / 3600) as u32,
        minute: ((rem % 3600) / 60) as u32,
        second: (rem % 60) as u32,
    }
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `07:04:31`.
pub fn clock(unix_s: u64) -> String {
    let t = utc(unix_s);
    format!("{:02}:{:02}:{:02}", t.hour, t.minute, t.second)
}

/// `10:08`.
pub fn clock_hm(unix_s: u64) -> String {
    let t = utc(unix_s);
    format!("{:02}:{:02}", t.hour, t.minute)
}

/// `6 Oct 21:31 UTC`.
pub fn day_month_time(unix_s: u64) -> String {
    let t = utc(unix_s);
    format!(
        "{} {} {:02}:{:02} UTC",
        t.day,
        MONTHS[(t.month - 1) as usize],
        t.hour,
        t.minute
    )
}

/// `2026-10-09 07:04:31 UTC`.
pub fn iso(unix_s: u64) -> String {
    let t = utc(unix_s);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}

/// The file name inside a run directory.
pub const PCOC_FILE_NAME: &str = "pcoc.cbor";

/// `2026-10-09_070431_npub16896f`. UTC, then the first 10 characters of the npub.
/// The name is a single path component and holds no secret.
/// A run directory that does not already exist. A restart in the same
/// second gets a `-2` suffix so the stopped directory is left in place.
pub fn unique_run_dir(folder: &Path, unix_s: u64, npub: &str) -> PathBuf {
    let name = run_dir_name(unix_s, npub);
    let first = folder.join(&name);
    if !first.exists() {
        return first;
    }
    for n in 2..1000 {
        let alt = folder.join(format!("{name}-{n}"));
        if !alt.exists() {
            return alt;
        }
    }
    first
}

pub fn run_dir_name(unix_s: u64, npub: &str) -> String {
    let t = utc(unix_s);
    let prefix: String = npub.chars().take(10).collect();
    format!(
        "{:04}-{:02}-{:02}_{:02}{:02}{:02}_{prefix}",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}

pub fn now_unix_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `~/PCOC/hikes/x.cbor` when the path is under `$HOME`.
pub fn tilde(path: &std::path::Path) -> String {
    if let Some(home) = std::env::var_os("HOME") {
        if let Ok(rest) = path.strip_prefix(&home) {
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_sample_figures() {
        assert_eq!(thousands(3559), "3,559");
        assert_eq!(thousands(186_432), "186,432");
        assert_eq!(thousands(971_205), "971,205");
        assert_eq!(thousands(17), "17");
        assert_eq!(three_places(1228, 3982).unwrap(), "0.308");
        assert_eq!(three_places(3094, 10_200).unwrap(), "0.303");
        assert_eq!(duration_hms(11_609), "3 h 13 m 29 s");
        assert_eq!(duration_hms(123), "2 m 03 s");
        assert_eq!(duration_hm(10_200), "2 h 50 m");
        assert_eq!(clock(1_791_529_471), "07:04:31");
        assert_eq!(clock(1_791_541_080), "10:18:00");
        assert_eq!(iso(1_791_529_471), "2026-10-09 07:04:31 UTC");
        assert_eq!(
            run_dir_name(
                1_791_529_471,
                "npub16896f5x95hys6mssl9hz7jtffhm3hzrzcmwd0a0ayq2kjn0walfqasq744"
            ),
            "2026-10-09_070431_npub16896f"
        );
        assert!(!run_dir_name(1_791_529_471, "npub16896f5x95hys").contains("nsec"));
        assert_eq!(day_month_time(1_791_322_260), "6 Oct 21:31 UTC");
    }

    #[test]
    fn a_restart_in_the_same_second_gets_its_own_directory() {
        let dir = std::env::temp_dir().join(format!("pcoc-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let npub = "npub16896f5x95hys6mssl9hz7jtffhm3hzrzcmwd0a0ayq2kjn0walfqasq744";
        let first = unique_run_dir(&dir, 1_791_529_471, npub);
        std::fs::create_dir_all(&first).unwrap();
        let second = unique_run_dir(&dir, 1_791_529_471, npub);
        assert_ne!(first, second);
        assert!(second.ends_with("2026-10-09_070431_npub16896f-2"));
        assert!(!second.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
