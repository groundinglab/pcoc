//! Presenting the section 14 record. Measurements only: the page shows the
//! fields, the tokens, what each token means, and arithmetic over the counts.
//! It does not add a verdict, a score, or a threshold (§14.5, P4.2).

use crate::format::{thousands, three_places};
use serde_json::Value;

/// An integer field, or the token the record wrote in its place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Val {
    Int(u64),
    Token(String),
}

impl Val {
    fn of(value: Option<&Value>) -> Val {
        match value {
            Some(Value::Number(n)) => n
                .as_u64()
                .map(Val::Int)
                .unwrap_or_else(|| Val::Token(n.to_string())),
            Some(Value::String(s)) => Val::Token(s.clone()),
            Some(other) => Val::Token(other.to_string()),
            None => Val::Token("absent".to_owned()),
        }
    }

    pub fn int(&self) -> Option<u64> {
        match self {
            Val::Int(n) => Some(*n),
            Val::Token(_) => None,
        }
    }

    pub fn token(&self) -> Option<&str> {
        match self {
            Val::Int(_) => None,
            Val::Token(t) => Some(t),
        }
    }

    /// `3,094` or `not defined`.
    pub fn text(&self) -> String {
        match self {
            Val::Int(n) => thousands(*n),
            Val::Token(t) => t.clone(),
        }
    }

    /// `10,200 s` or the token.
    pub fn seconds(&self) -> String {
        match self {
            Val::Int(n) => format!("{} s", thousands(*n)),
            Val::Token(t) => t.clone(),
        }
    }
}

/// Counts of each token, in order of first appearance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally(pub Vec<(String, usize)>);

impl Tally {
    fn add(&mut self, token: &str) {
        match self.0.iter_mut().find(|(t, _)| t == token) {
            Some((_, n)) => *n += 1,
            None => self.0.push((token.to_owned(), 1)),
        }
    }

    pub fn total(&self) -> usize {
        self.0.iter().map(|(_, n)| n).sum()
    }

    /// The one token when every entry has it.
    pub fn uniform(&self) -> Option<&str> {
        match self.0.as_slice() {
            [(token, _)] => Some(token),
            _ => None,
        }
    }

    /// `every one is match` or `3,550 match · 9 mismatch`.
    pub fn sentence(&self) -> String {
        if self.0.is_empty() {
            return "none recorded".to_owned();
        }
        if let Some(token) = self.uniform() {
            return format!("every one is {token}");
        }
        self.0
            .iter()
            .map(|(t, n)| format!("{} {t}", thousands(*n as u64)))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderRow {
    /// `seed beacon`, `I_0` … `I_17`.
    pub role: String,
    pub height: Val,
    pub stored_hash: Option<String>,
    pub index_comparison: String,
    pub t: Val,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathRow {
    pub g: u64,
    pub path_state: String,
    pub plant: String,
    pub height_p_g: Val,
    pub d_g: Val,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SixLegRow {
    /// Global checkpoint index, from 0.
    pub index: u64,
    /// Stage and `j` from the per-stage counts, when those counts are integers.
    pub stage: Option<usize>,
    pub j: Option<u64>,
    pub in_stage: Option<u64>,
    pub result: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordView {
    pub profile: String,
    pub shape: String,
    pub shape_item: Vec<String>,
    pub seed_value: String,
    pub headers: Vec<HeaderRow>,
    pub checkpoint_results: Tally,
    pub waypoint_results: Tally,
    pub stage_counts: Vec<Val>,
    pub stage_spans: Vec<Val>,
    /// `W_1` … `W_16`.
    pub paths: Vec<PathRow>,
    pub w17_path: String,
    pub anchors: Vec<String>,
    pub anchored_total: Val,
    pub skip_list: Vec<u64>,
    pub a: Val,
    pub b: Val,
    pub n: Val,
    pub s: Val,
    pub delta_h: Val,
    pub fixed_start: [Val; 3],
    pub height_p_b: Val,
    pub height_i_a_minus_1: Val,
    pub whole_hike_legs: Val,
    pub whole_hike_span: Val,
    pub legs_outside: Val,
    /// `None` when the record wrote a token for `six_leg`.
    pub six_leg: Option<Vec<SixLegRow>>,
    pub six_leg_token: Option<String>,
    pub six_leg_runs: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presented {
    /// Rule A5: `version` `earlier version`. No stage matrix.
    Earlier {
        kind: String,
    },
    Record(Box<RecordView>),
}

/// The four lines on the verify screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrimmedRecord {
    pub checkpoints: String,
    pub stages: String,
    pub span: String,
    pub rate: String,
}

/// The record bytes do not parse as a section 14 JSON object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotARecord;

pub fn present(record_json: &str) -> Result<Presented, NotARecord> {
    let value: Value = serde_json::from_str(record_json.trim_end()).map_err(|_| NotARecord)?;
    let obj = value.as_object().ok_or(NotARecord)?;
    if obj.get("version").and_then(Value::as_str) == Some("earlier version") {
        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        return Ok(Presented::Earlier { kind });
    }
    let profile = obj
        .get("profile")
        .and_then(Value::as_str)
        .ok_or(NotARecord)?
        .to_owned();
    let text = |key: &str| {
        obj.get(key)
            .and_then(Value::as_str)
            .unwrap_or("absent")
            .to_owned()
    };

    let headers = obj
        .get("headers")
        .and_then(Value::as_array)
        .map(|rows| {
            let last = rows.len().saturating_sub(1);
            rows.iter()
                .enumerate()
                .map(|(k, row)| HeaderRow {
                    role: match k {
                        0 => "seed beacon".to_owned(),
                        k if k == last && k == 18 => "I_17".to_owned(),
                        k => format!("I_{}", k - 1),
                    },
                    height: Val::of(row.get("stored_height")),
                    stored_hash: row
                        .get("stored_hash")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    index_comparison: row
                        .get("index_comparison")
                        .and_then(Value::as_str)
                        .unwrap_or("absent")
                        .to_owned(),
                    t: Val::of(row.get("T")),
                })
                .collect()
        })
        .unwrap_or_default();

    let mut checkpoint_results = Tally::default();
    if let Some(stages) = obj.get("checkpoints").and_then(Value::as_array) {
        for stage in stages {
            for leg in stage
                .get("legs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                checkpoint_results.add(
                    leg.get("result")
                        .and_then(Value::as_str)
                        .unwrap_or("absent"),
                );
            }
        }
    }
    let mut waypoint_results = Tally::default();
    for row in obj
        .get("waypoints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        waypoint_results.add(
            row.get("result")
                .and_then(Value::as_str)
                .unwrap_or("absent"),
        );
    }

    let analysis = obj.get("analysis");
    let per_stage: Vec<&Value> = analysis
        .and_then(|a| a.get("per_stage"))
        .and_then(Value::as_array)
        .map(|rows| rows.iter().collect())
        .unwrap_or_default();
    let stage_counts: Vec<Val> = per_stage
        .iter()
        .map(|r| Val::of(r.get("checkpoint_count")))
        .collect();
    let stage_spans: Vec<Val> = per_stage.iter().map(|r| Val::of(r.get("span"))).collect();

    let mut paths = Vec::new();
    let mut w17_path = "absent".to_owned();
    for row in obj
        .get("paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let g = row.get("g").and_then(Value::as_u64).unwrap_or(0);
        if g == 17 {
            w17_path = row
                .get("w17_path")
                .and_then(Value::as_str)
                .unwrap_or("absent")
                .to_owned();
            continue;
        }
        paths.push(PathRow {
            g,
            path_state: row
                .get("path_state")
                .and_then(Value::as_str)
                .unwrap_or("absent")
                .to_owned(),
            plant: row
                .get("plant")
                .and_then(Value::as_str)
                .unwrap_or("absent")
                .to_owned(),
            height_p_g: Val::of(row.get("height_P_g")),
            d_g: Val::of(row.get("d_g")),
        });
    }

    let anchors = obj
        .get("anchors")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|t| t.as_str().unwrap_or("absent").to_owned())
                .collect()
        })
        .unwrap_or_default();
    let skip_list = obj
        .get("skip_list")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();

    let fixed = obj.get("fixed_start");
    let fixed_start = [
        Val::of(fixed.and_then(|f| f.get("N"))),
        Val::of(fixed.and_then(|f| f.get("S"))),
        Val::of(fixed.and_then(|f| f.get("delta_h"))),
    ];

    let counts_int: Option<Vec<u64>> = stage_counts.iter().map(Val::int).collect();
    let (six_leg, six_leg_token) = match obj.get("six_leg") {
        Some(Value::Object(draw)) => {
            let indices: Vec<u64> = draw
                .get("indices")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default();
            let results: Vec<String> = draw
                .get("results")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|r| r.as_str().unwrap_or("absent").to_owned())
                        .collect()
                })
                .unwrap_or_default();
            let rows = indices
                .iter()
                .enumerate()
                .map(|(i, &index)| {
                    let located = counts_int.as_deref().and_then(|c| locate(c, index));
                    SixLegRow {
                        index,
                        stage: located.map(|l| l.0),
                        j: located.map(|l| l.1),
                        in_stage: located.map(|l| l.2),
                        result: results
                            .get(i)
                            .cloned()
                            .unwrap_or_else(|| "absent".to_owned()),
                    }
                })
                .collect();
            (Some(rows), None)
        }
        Some(Value::String(token)) => (None, Some(token.clone())),
        _ => (None, Some("absent".to_owned())),
    };

    Ok(Presented::Record(Box::new(RecordView {
        profile,
        shape: text("shape"),
        shape_item: obj
            .get("shape_item")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|t| t.as_str().unwrap_or("absent").to_owned())
                    .collect()
            })
            .unwrap_or_default(),
        seed_value: text("seed_value"),
        headers,
        checkpoint_results,
        waypoint_results,
        stage_counts,
        stage_spans,
        paths,
        w17_path,
        anchors,
        anchored_total: Val::of(obj.get("anchored_total")),
        skip_list,
        a: Val::of(obj.get("a")),
        b: Val::of(obj.get("b")),
        n: Val::of(obj.get("N")),
        s: Val::of(obj.get("S")),
        delta_h: Val::of(obj.get("delta_h")),
        fixed_start,
        height_p_b: Val::of(obj.get("height_P_b")),
        height_i_a_minus_1: Val::of(obj.get("height_I_a_minus_1")),
        whole_hike_legs: Val::of(analysis.and_then(|a| a.get("whole_hike_legs"))),
        whole_hike_span: Val::of(analysis.and_then(|a| a.get("whole_hike_span"))),
        legs_outside: Val::of(analysis.and_then(|a| a.get("legs_outside"))),
        six_leg,
        six_leg_token,
        six_leg_runs: obj
            .get("six_leg_runs")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
    })))
}

/// Global checkpoint index to `(stage, j, checkpoints in that stage)`.
fn locate(counts: &[u64], index: u64) -> Option<(usize, u64, u64)> {
    let mut start = 0u64;
    for (g, &count) in counts.iter().enumerate() {
        if index < start + count {
            return Some((g, index - start, count));
        }
        start += count;
    }
    None
}

/// One row of the "where the checkpoints sit" ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerRow {
    pub count: u64,
    pub label: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ledger {
    pub rows: Vec<LedgerRow>,
    pub total: u64,
    /// `3,094 + 465 = 3,559 · 465 is stages 0, 16 and 17, outside the bookends`.
    pub outside_identity: String,
    /// `3,094 + 108 + 180 = 3,382 · 3,382 is whole_hike_legs, stages 1–17`.
    pub whole_identity: String,
}

fn stage_list(stages: &[usize]) -> String {
    match stages {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!(
            "{} and {last}",
            rest.iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

impl RecordView {
    /// The breakdown, when `a`, `b`, `N` and every per-stage count are integers (A4.7, A4.10).
    pub fn ledger(&self) -> Option<Ledger> {
        let counts: Vec<u64> = self
            .stage_counts
            .iter()
            .map(Val::int)
            .collect::<Option<_>>()?;
        if counts.len() != 18 {
            return None;
        }
        let (a, b, n) = (
            self.a.int()? as usize,
            self.b.int()? as usize,
            self.n.int()?,
        );
        if a == 0 || b <= a || b > 17 {
            return None;
        }
        let total: u64 = counts.iter().sum();
        let mut rows = vec![LedgerRow {
            count: n,
            label: if b - 1 == a {
                format!("stage {a}")
            } else {
                format!("stages {a}–{}", b - 1)
            },
            note: format!("bookend N, a = {a}, b = {b}"),
        }];
        let outside: Vec<usize> = (1..a).chain(b..18).collect();
        for &g in &outside {
            rows.push(LedgerRow {
                count: counts[g],
                label: format!("stage {g}"),
                note: String::new(),
            });
        }
        rows.push(LedgerRow {
            count: counts[0],
            label: "stage 0".to_owned(),
            note: "not in whole_hike_legs".to_owned(),
        });

        let legs_outside = self.legs_outside.int().unwrap_or(total - n);
        let mut outside_with_zero = vec![0usize];
        outside_with_zero.extend(&outside);
        let outside_identity = format!(
            "{} + {} = {} · {} is stages {}, outside the bookends",
            thousands(n),
            thousands(legs_outside),
            thousands(n + legs_outside),
            thousands(legs_outside),
            stage_list(&outside_with_zero)
        );
        let whole: u64 = n + outside.iter().map(|&g| counts[g]).sum::<u64>();
        let mut parts = vec![thousands(n)];
        parts.extend(outside.iter().map(|&g| thousands(counts[g])));
        let whole_identity = format!(
            "{} = {} · {} is whole_hike_legs, stages 1–17",
            parts.join(" + "),
            thousands(self.whole_hike_legs.int().unwrap_or(whole)),
            thousands(self.whole_hike_legs.int().unwrap_or(whole))
        );
        Some(Ledger {
            rows,
            total,
            outside_identity,
            whole_identity,
        })
    }

    /// Checkpoints, stages, span and rate. The verify screen shows these
    /// and does not draw the rest of the section 14 record.
    pub fn trimmed(&self) -> TrimmedRecord {
        let checkpoints = thousands(self.checkpoint_results.total() as u64);
        let stages = if self.stage_counts.is_empty() {
            "—".to_owned()
        } else {
            self.stage_counts.len().to_string()
        };
        TrimmedRecord {
            checkpoints,
            stages,
            span: self.whole_hike_span.seconds(),
            rate: self.n_over_s().unwrap_or_else(|| "—".to_owned()),
        }
    }

    /// `N ÷ S` to three places, display only (A4.9). Not a record key.
    pub fn n_over_s(&self) -> Option<String> {
        three_places(self.n.int()?, self.s.int()?)
    }

    /// Plant reads with a measured result.
    pub fn plant_reads_measured(&self) -> usize {
        self.paths
            .iter()
            .filter(|p| p.plant == "match" || p.plant == "mismatch")
            .count()
    }

    pub fn header_comparisons(&self) -> Tally {
        let mut tally = Tally::default();
        for header in &self.headers {
            tally.add(&header.index_comparison);
        }
        tally
    }

    /// `19 stored headers, each index_comparison match, from seed 971,204 through I_17 at 971,222`.
    pub fn header_sentence(&self) -> String {
        let tally = self.header_comparisons();
        let comparisons = match tally.uniform() {
            Some(token) => format!("each index_comparison {token}"),
            None => format!("index_comparison {}", tally.sentence()),
        };
        let first = self.headers.first().map(|h| h.height.text());
        let last = self.headers.last().map(|h| h.height.text());
        match (first, last) {
            (Some(first), Some(last)) => format!(
                "{} stored headers, {comparisons}, from seed {first} through I_17 at {last}.",
                self.headers.len()
            ),
            _ => format!("{} stored headers.", self.headers.len()),
        }
    }

    /// `anchored_total 14. skip_list [2, 9]. 14 anchored + 2 skips = 16 fetchable.`
    pub fn anchor_sentence(&self) -> String {
        let skips = self
            .skip_list
            .iter()
            .map(|g| g.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let mut out = format!(
            "anchored_total {}. skip_list [{skips}].",
            self.anchored_total.text()
        );
        if let Some(anchored) = self.anchored_total.int() {
            let skips = self.skip_list.len() as u64;
            let other = 16u64.saturating_sub(anchored + skips);
            if other == 0 {
                out.push_str(&format!(
                    " {anchored} anchored + {skips} skips = 16 fetchable."
                ));
            } else {
                out.push_str(&format!(
                    " {anchored} anchored + {skips} skips + {other} other = 16 fetchable."
                ));
            }
        }
        out.push_str(" The count is not a threshold. W_17 is not in it.");
        out
    }
}

/// One-line meaning of a section 14 token. The words are measurements.
pub fn explain(token: &str) -> &'static str {
    match token {
        "match" => "The recomputed or re-read value equals the value stored in the object.",
        "mismatch" => {
            "The recomputed or re-read value differs from the value stored in the object."
        }
        "not read" => {
            "The read was not issued or did not return. A measurement, not an application error."
        }
        "not defined" => "The rule that defines this field gives it no value for this object.",
        "n/a" => "The field does not apply here, for example the plant of a skipped path.",
        "present" => "The stage stores a path for this waypoint.",
        "skip" => "The stage stores an empty path for this waypoint (path_skip).",
        "anchored" => {
            "The path is present, its plant is match, and that header's index comparison is match."
        }
        "not anchored" => "The waypoint does not meet the anchored conditions in this record.",
        "not fetched by design" => "W_17 is posted and never asked. Its path is empty by design.",
        "earlier version" => {
            "The object is an earlier kind (Rule A5). The v2.0 matrix is not measured."
        }
        "production" => "Section 6 sizes. The record of a production object.",
        "conformance-small" => "SPEC 16.1 sizes. A conformance vector, not a production object.",
        "height_range" => "A stored header height is outside the range the shape rule allows.",
        "absent" => "The record does not carry this key.",
        _ => "A section 14 token. See the SPEC for its rule.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_of(file: &str) -> String {
        let text = match file {
            "kat-small" => include_str!("../../../../spec/conformance/v2.0/kat-small.json"),
            "test-3leg" => include_str!("../../../../spec/conformance/v2.0/test-3leg.json"),
            "anchor-window" => include_str!("../../../../spec/conformance/v2.0/anchor-window.json"),
            "anchor-w16-i17" => include_str!("../../../../spec/conformance/v2.0/anchor-w16-i17.json"),
            "earlier-run-b" => include_str!("../../../../spec/conformance/v2.0/earlier-run-b.json"),
            _ => unreachable!(),
        };
        let value: Value = serde_json::from_str(text).unwrap();
        value["record"].as_str().unwrap().to_owned()
    }

    fn view(file: &str) -> RecordView {
        match present(&record_of(file)).unwrap() {
            Presented::Record(view) => *view,
            Presented::Earlier { .. } => panic!("{file} is not a v2.0 record"),
        }
    }

    #[test]
    fn kat_small_fields() {
        let v = view("kat-small");
        assert_eq!(v.profile, "conformance-small");
        assert_eq!(v.shape, "match");
        assert!(v.shape_item.is_empty());
        assert_eq!(v.seed_value, "match");
        assert_eq!(v.headers.len(), 19);
        assert_eq!(v.headers[0].role, "seed beacon");
        assert_eq!(v.headers[1].role, "I_0");
        assert_eq!(v.headers[18].role, "I_17");
        assert_eq!(v.header_comparisons().uniform(), Some("match"));
        assert_eq!(v.checkpoint_results.sentence(), "every one is match");
        assert_eq!(v.checkpoint_results.total(), 18);
        assert_eq!(v.waypoint_results.total(), 17);
        assert_eq!(
            (v.a.int(), v.b.int(), v.n.int(), v.s.int(), v.delta_h.int()),
            (Some(1), Some(2), Some(1), Some(3), Some(3))
        );
        assert_eq!(v.fixed_start, [Val::Int(1), Val::Int(3), Val::Int(3)]);
        assert_eq!(v.anchored_total, Val::Int(2));
        assert_eq!(v.paths.len(), 16);
        assert_eq!(v.paths[0].d_g, Val::Int(3));
        assert_eq!(v.paths[2].d_g, Val::Token("not defined".into()));
        assert_eq!(v.w17_path, "not fetched by design");
        assert_eq!(v.n_over_s().unwrap(), "0.333");
        assert_eq!(
            v.trimmed(),
            TrimmedRecord {
                checkpoints: "18".into(),
                stages: "18".into(),
                span: "19 s".into(),
                rate: "0.333".into(),
            }
        );
        assert_eq!(v.plant_reads_measured(), 2);
        assert_eq!(
            v.anchor_sentence(),
            "anchored_total 2. skip_list [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]. 2 anchored + 14 skips = 16 fetchable. The count is not a threshold. W_17 is not in it."
        );
    }

    #[test]
    fn kat_small_ledger_adds_up() {
        let v = view("kat-small");
        let ledger = v.ledger().unwrap();
        assert_eq!(ledger.total, 18);
        assert_eq!(
            ledger.rows[0],
            LedgerRow {
                count: 1,
                label: "stage 1".into(),
                note: "bookend N, a = 1, b = 2".into()
            }
        );
        assert_eq!(ledger.rows.len(), 1 + 16 + 1);
        assert_eq!(ledger.rows.last().unwrap().label, "stage 0");
        assert!(ledger
            .outside_identity
            .starts_with("1 + 17 = 18 · 17 is stages 0, 2, 3,"));
        assert!(ledger
            .whole_identity
            .ends_with("= 17 · 17 is whole_hike_legs, stages 1–17"));
    }

    #[test]
    fn six_leg_indices_map_to_stage_and_j() {
        let v = view("kat-small");
        let rows = v.six_leg.unwrap();
        assert_eq!(
            rows.iter().map(|r| r.index).collect::<Vec<_>>(),
            [9, 10, 13, 12, 17, 16]
        );
        assert_eq!(
            (rows[0].stage, rows[0].j, rows[0].in_stage),
            (Some(9), Some(0), Some(1))
        );
        assert!(rows.iter().all(|r| r.result == "match"));
        assert_eq!(v.six_leg_runs, 1);
    }

    #[test]
    fn locate_matches_the_design_draw() {
        let counts = [
            177, 251, 134, 323, 92, 215, 167, 398, 117, 191, 230, 1, 280, 145, 204, 346, 108, 180,
        ];
        assert_eq!(locate(&counts, 3_499), Some((17, 120, 180)));
        assert_eq!(locate(&counts, 3_343), Some((16, 72, 108)));
        assert_eq!(locate(&counts, 816), Some((3, 254, 323)));
        assert_eq!(locate(&counts, 121), Some((0, 121, 177)));
        assert_eq!(locate(&counts, 2_111), Some((10, 46, 230)));
        assert_eq!(locate(&counts, 1_411), Some((7, 52, 398)));
        assert_eq!(locate(&counts, 3_559), None);
    }

    #[test]
    fn tokens_in_place_of_integers_are_shown_as_tokens() {
        let v = view("anchor-window");
        assert_eq!(v.shape, "mismatch");
        assert_eq!(v.shape_item, ["leg_count"]);
        assert_eq!(v.n.text(), "not defined");
        assert_eq!(v.whole_hike_span.seconds(), "not read");
        assert!(v.ledger().is_none());
        assert!(v.n_over_s().is_none());
        assert!(v.six_leg.is_none());
        assert_eq!(v.six_leg_token.as_deref(), Some("not defined"));
        let t = view("test-3leg");
        assert_eq!(t.w17_path, "not defined");
        assert_eq!(t.stage_counts[3], Val::Token("not defined".into()));
        let w = view("anchor-w16-i17");
        assert_eq!(w.anchors[15], "anchored");
    }

    #[test]
    fn earlier_version_has_no_matrix() {
        assert_eq!(
            present(&record_of("earlier-run-b")).unwrap(),
            Presented::Earlier {
                kind: "satoshi-hike-v1.35-object".into()
            }
        );
    }

    #[test]
    fn junk_is_not_a_record() {
        assert_eq!(present("not json"), Err(NotARecord));
        assert_eq!(present("[]"), Err(NotARecord));
    }

    /// An undecodable object still yields a record from the pinned verifier.
    #[test]
    fn undecodable_object_record_presents() {
        let m = hike::verify(
            b"not a hike object",
            hike::AuditDraw::PinnedSeed([0; 32]),
            |_| Err(()),
            |_| hike::ChainRead::NotRead,
        );
        let json = hike::verify_record_json("production", &m);
        let Presented::Record(v) = present(&json).unwrap() else {
            panic!()
        };
        assert_eq!(v.shape, "mismatch");
        assert_eq!(v.profile, "production");
    }

    /// No verdict vocabulary in anything the page derives (§14, §18 item 1).
    #[test]
    fn presentation_has_no_verdict_words() {
        let forbidden = [
            "pass", "fail", "valid", "invalid", "score", "verdict", "accept", "reject", "approved",
            "grade",
        ];
        let mut strings: Vec<String> = Vec::new();
        for file in ["kat-small", "test-3leg", "anchor-window", "anchor-w16-i17"] {
            let v = view(file);
            strings.push(v.anchor_sentence());
            strings.push(v.header_sentence());
            strings.push(v.checkpoint_results.sentence());
            if let Some(l) = v.ledger() {
                strings.push(l.outside_identity);
                strings.push(l.whole_identity);
                strings.extend(l.rows.into_iter().flat_map(|r| [r.label, r.note]));
            }
            let mut tokens: Vec<String> =
                vec![v.shape.clone(), v.seed_value.clone(), v.w17_path.clone()];
            tokens.extend(v.anchors.clone());
            tokens.extend(
                v.paths
                    .iter()
                    .flat_map(|p| [p.path_state.clone(), p.plant.clone()]),
            );
            for token in tokens {
                assert_ne!(
                    explain(&token),
                    explain("an unknown token"),
                    "no explanation for {token}"
                );
                strings.push(explain(&token).to_owned());
            }
        }
        for s in strings {
            let lower = s.to_lowercase();
            for word in forbidden {
                assert!(!lower.contains(word), "{word:?} in {s:?}");
            }
        }
    }
}
