//! UTC civil time from a unix second. No extra clock crate.

/// `YYYY-MM-DD` for `unix` in UTC. Negative instants stay on the proleptic day.
pub fn utc_day(unix: i64) -> String {
    let (y, m, d) = civil_ymd(unix.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// `YYYYMMDDTHHMMSSZ`.
pub fn utc_basic(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let sod = unix.rem_euclid(86_400) as u32;
    let (y, m, d) = civil_ymd(days);
    let hh = sod / 3600;
    let mm = (sod % 3600) / 60;
    let ss = sod % 60;
    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

/// `YYYY-MM-DD HH:MM:SS UTC`.
pub fn utc_words(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let sod = unix.rem_euclid(86_400) as u32;
    let (y, m, d) = civil_ymd(days);
    let hh = sod / 3600;
    let mm = (sod % 3600) / 60;
    let ss = sod % 60;
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02} UTC")
}

/// True when `text` is `YYYY-MM-DD` and that civil day exists.
pub fn valid_day(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let Ok(y) = text[0..4].parse::<i32>() else {
        return false;
    };
    let Ok(m) = text[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(d) = text[8..10].parse::<u32>() else {
        return false;
    };
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&m) || d == 0 {
        return false;
    }
    d <= days_in_month(y, m)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Howard Hinnant's `civil_from_days`, shifted so day 0 is 1970-01-01.
fn civil_ymd(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_and_a_known_afternoon() {
        assert_eq!(utc_day(0), "1970-01-01");
        assert_eq!(utc_basic(0), "19700101T000000Z");
        assert_eq!(utc_day(86_400), "1970-01-02");
        assert_eq!(utc_day(86_400 * 365), "1971-01-01");
        // 2023-11-14 22:13:20 UTC.
        assert_eq!(utc_day(1_700_000_000), "2023-11-14");
        assert_eq!(utc_basic(1_700_000_000), "20231114T221320Z");
        assert_eq!(utc_words(1_700_000_000), "2023-11-14 22:13:20 UTC");
    }

    #[test]
    fn rejects_impossible_days() {
        assert!(valid_day("2026-10-14"));
        assert!(valid_day("2024-02-29"));
        assert!(!valid_day("2026-02-29"));
        assert!(!valid_day("2026-13-01"));
        assert!(!valid_day("26-10-14"));
        assert!(!valid_day("2026-1-01"));
    }
}
