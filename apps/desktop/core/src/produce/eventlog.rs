//! The crate's `{output}.events` sibling (section 13.5): block, waypoint,
//! posted, checked and fetch. It is not the hike object and not a per-leg
//! feed. The app reads it for post and ask results and for the log drawer.

use super::events::{AskOutcome, ProduceEvent};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "event")]
pub enum Record {
    #[serde(rename = "hike.produce.block")]
    Block { height: u64, tip: String },
    #[serde(rename = "hike.produce.waypoint")]
    Waypoint { stage: u64, waypoint: String },
    #[serde(rename = "hike.produce.posted")]
    Posted {
        stage: u64,
        waypoint: String,
        posted: bool,
        succeeded: bool,
        #[serde(default)]
        status: Option<u16>,
    },
    #[serde(rename = "hike.produce.checked")]
    Checked { waypoint: String, checked: bool },
    #[serde(rename = "hike.produce.fetch")]
    Fetch {
        waypoint: String,
        worked: bool,
        #[serde(default)]
        status: Option<u16>,
        #[serde(default)]
        block: Option<String>,
        #[serde(default)]
        block_height: Option<u32>,
        #[serde(default)]
        miss: Option<String>,
    },
}

pub fn parse_line(line: &str) -> Option<Record> {
    serde_json::from_str(line).ok()
}

/// One readable drawer line per record. `tip` in a block record is natural order.
pub fn drawer_line(record: &Record) -> String {
    match record {
        Record::Block { height, tip } => format!(
            "block {height} tip {} (natural order)",
            crate::hex::short(tip)
        ),
        Record::Waypoint { stage, waypoint } => {
            format!("waypoint W_{stage} {}", crate::hex::short(waypoint))
        }
        Record::Posted {
            stage,
            status,
            succeeded,
            ..
        } => match status {
            Some(status) => format!(
                "posted W_{stage} status {status}{}",
                if *succeeded { "" } else { ", not appended" }
            ),
            None => format!("posted W_{stage} did not connect"),
        },
        Record::Checked { waypoint, checked } => {
            format!("checked {} {}", crate::hex::short(waypoint), checked)
        }
        Record::Fetch {
            waypoint,
            worked,
            status,
            block_height,
            miss,
            ..
        } => {
            let mut line = format!("fetch {}", crate::hex::short(waypoint));
            if let Some(status) = status {
                line.push_str(&format!(" status {status}"));
            }
            if *worked {
                line.push_str(" served");
                if let Some(h) = block_height {
                    line.push_str(&format!(" block {h}"));
                }
            } else {
                line.push_str(" miss");
                if let Some(miss) = miss {
                    line.push_str(&format!(": {miss}"));
                }
            }
            line
        }
    }
}

/// Turns records into post and ask events.
#[derive(Default)]
pub struct Follower {
    stage_of: HashMap<String, u64>,
    last_block: Option<u64>,
}

impl Follower {
    pub fn apply(&mut self, record: &Record) -> Vec<ProduceEvent> {
        let mut out = vec![ProduceEvent::Log(redact(&drawer_line(record)))];
        match record {
            Record::Block { height, .. } => self.last_block = Some(*height),
            Record::Waypoint { stage, waypoint } => {
                self.stage_of.insert(waypoint.clone(), *stage);
            }
            Record::Posted {
                stage,
                waypoint,
                succeeded,
                status,
                ..
            } => {
                self.stage_of.insert(waypoint.clone(), *stage);
                out.push(ProduceEvent::WaypointPosted {
                    g: *stage,
                    appended: *succeeded,
                    status: *status,
                });
            }
            Record::Checked { .. } => {}
            Record::Fetch {
                waypoint,
                worked,
                miss,
                ..
            } => {
                if let Some(&g) = self.stage_of.get(waypoint) {
                    out.push(ProduceEvent::PathAsk {
                        g,
                        at_height: self.last_block.unwrap_or(0),
                        outcome: if *worked {
                            AskOutcome::Served
                        } else {
                            AskOutcome::Miss(miss.clone())
                        },
                    });
                }
            }
        }
        out
    }
}

/// Follows a growing file by offset. Partial last lines wait for their newline.
pub struct Tail {
    path: PathBuf,
    offset: u64,
    partial: String,
}

impl Tail {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            offset: 0,
            partial: String::new(),
        }
    }

    pub fn poll(&mut self) -> Vec<String> {
        let Ok(mut file) = std::fs::File::open(&self.path) else {
            return Vec::new();
        };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut chunk = String::new();
        let Ok(n) = file.read_to_string(&mut chunk) else {
            return Vec::new();
        };
        self.offset += n as u64;
        self.partial.push_str(&chunk);
        let mut lines = Vec::new();
        while let Some(at) = self.partial.find('\n') {
            let line: String = self.partial.drain(..=at).collect();
            let line = line.trim_end().to_owned();
            if !line.is_empty() {
                lines.push(line);
            }
        }
        lines
    }
}

/// K12. Replace any `nsec1…` or `ncryptsec1…` run with `[redacted]`.
pub fn redact(line: &str) -> String {
    const CHARSET: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let next = ["ncryptsec1", "nsec1"]
            .iter()
            .filter_map(|p| rest.find(p).map(|i| (i, *p)))
            .min_by_key(|(i, p)| (*i, std::cmp::Reverse(p.len())));
        let Some((at, prefix)) = next else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..at]);
        let tail = &rest[at + prefix.len()..];
        let run = tail
            .char_indices()
            .find(|(_, c)| !CHARSET.contains(c.to_ascii_lowercase()))
            .map(|(i, _)| i)
            .unwrap_or(tail.len());
        out.push_str("[redacted]");
        rest = &tail[run..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_parse_into_posts_and_asks() {
        let lines = [
            r#"{"event":"hike.produce.block","height":971206,"tip":"227f548967452301efcdab907856341290ef56cd34ab00000000000000000000"}"#,
            r#"{"event":"hike.produce.waypoint","stage":1,"waypoint":"aa"}"#,
            r#"{"event":"hike.produce.posted","stage":1,"waypoint":"aa","posted":true,"succeeded":true,"status":204}"#,
            r#"{"event":"hike.produce.posted","stage":2,"waypoint":"bb","posted":false,"succeeded":false}"#,
            r#"{"event":"hike.produce.block","height":971207,"tip":"00"}"#,
            r#"{"event":"hike.produce.fetch","waypoint":"aa","worked":true,"status":200,"block_height":971207}"#,
            r#"{"event":"hike.produce.fetch","waypoint":"cc","worked":false,"miss":"still in the pool"}"#,
        ];
        let mut follower = Follower::default();
        let events: Vec<ProduceEvent> = lines
            .iter()
            .flat_map(|l| follower.apply(&parse_line(l).unwrap()))
            .filter(|e| !matches!(e, ProduceEvent::Log(_)))
            .collect();
        assert_eq!(
            events,
            [
                ProduceEvent::WaypointPosted {
                    g: 1,
                    appended: true,
                    status: Some(204)
                },
                ProduceEvent::WaypointPosted {
                    g: 2,
                    appended: false,
                    status: None
                },
                ProduceEvent::PathAsk {
                    g: 1,
                    at_height: 971_207,
                    outcome: AskOutcome::Served
                },
            ]
        );
    }

    #[test]
    fn tail_waits_for_whole_lines() {
        let path = std::env::temp_dir().join(format!("pcoc-tail-{}.events", std::process::id()));
        std::fs::write(&path, "one\ntw").unwrap();
        let mut tail = Tail::new(&path);
        assert_eq!(tail.poll(), ["one"]);
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"o\nthree\n")
            .unwrap();
        assert_eq!(tail.poll(), ["two", "three"]);
        assert!(tail.poll().is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn secrets_are_redacted() {
        assert_eq!(
            redact("key nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5 end"),
            "key [redacted] end"
        );
        assert_eq!(
            redact("x ncryptsec1qgg9947rlpvqu76pj5 y nsec1acd"),
            "x [redacted] y [redacted]"
        );
        assert_eq!(redact("nsec1qq, next"), "[redacted], next");
        assert_eq!(redact("npub1abc stays"), "npub1abc stays");
    }
}
