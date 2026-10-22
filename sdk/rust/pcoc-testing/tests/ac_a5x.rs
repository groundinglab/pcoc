//! AC-A5x. Verify each round-2 object copy and this tree's copy.
//! The draw is the injected label `pcoc-pack-2.2/draw`, so the records match.

use pcoc::profile::Profile;
use pcoc::verify::Verified;
use pcoc_testing::{verify_from_bytes, CasesIndex, LabelChain};
use std::path::{Path, PathBuf};

fn find_up(start: &Path, rel: &str) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(rel);
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn profile_for(name: &str) -> Profile {
    match name {
        "O1-S1.cbor" => Profile::conformance_small(32, 64, 65_536, 100).unwrap(),
        "O2-S2.cbor" => Profile::conformance_small(64, 256, 1_048_576, 1_000).unwrap(),
        "O3-PROD.cbor" => Profile::production(),
        other => panic!("unknown object {other}"),
    }
}

fn verify(bytes: &[u8], profile: &Profile) -> String {
    let mut index = CasesIndex::new();
    let mut chain = LabelChain::new("pcoc-pack-2.2/draw");
    let mut next = || chain.next8();
    match verify_from_bytes(bytes, profile, &mut index, &mut next) {
        Verified::Record(s) => s,
        other => panic!("expected a record, got {other:?}"),
    }
}

fn slice_between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let i = text.find(start).unwrap_or_else(|| panic!("missing {start}"));
    let rest = &text[i + start.len()..];
    let j = rest.find(end).unwrap_or_else(|| panic!("missing {end} after {start}"));
    &rest[..j]
}

fn assert_measured(name: &str, record: &str) {
    assert!(record.contains("\"shape\":\"match\""), "{name} shape");
    assert!(record.contains("\"shape_item\":[]"), "{name} shape_item");
    let headers = slice_between(record, "\"headers\":", ",\"seed_value\":");
    assert_eq!(
        headers.matches("\"index_comparison\":\"match\"").count(),
        19,
        "{name} headers"
    );
    assert!(!headers.contains("\"not read\""), "{name} header not read");
    assert!(!headers.contains("\"mismatch\""), "{name} header mismatch");
    let mut rest = headers;
    let mut times = 0;
    while let Some(i) = rest.find("\"T\":") {
        let after = &rest[i + 4..];
        let digit = after.starts_with(|c: char| c.is_ascii_digit());
        assert!(digit, "{name} T is not a number: {}", &after[..after.len().min(24)]);
        times += 1;
        rest = &after[1..];
    }
    assert_eq!(times, 19, "{name} T count");
    assert!(record.contains("\"seed_value\":\"match\""), "{name} seed");
    for (start, end) in [("\"checkpoints\":", ",\"waypoints\":"), ("\"waypoints\":", ",\"paths\":")] {
        let body = slice_between(record, start, end);
        let mut i = 0;
        let mut n = 0;
        while let Some(p) = body[i..].find("\"result\":") {
            let v = &body[i + p + 9..];
            assert!(v.starts_with("\"match\""), "{name} {start} {v}");
            n += 1;
            i += p + 9;
        }
        assert!(n > 0, "{name} {start}");
    }
    let six = slice_between(record, "\"six_leg\":", ",\"six_leg_runs\":");
    assert_eq!(six.matches("\"match\"").count(), 6, "{name} six_leg {six}");
    assert!(!six.contains("mismatch"), "{name} six_leg");
}

#[test]
fn reference_objects_verify() {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut dirs = Vec::new();
    dirs.push(("this-tree".to_string(), here.join("../objects")));
    if find_up(&here, "ref-round2").is_some() {
        let job = find_up(&here, "ref-round2").unwrap();
        for (label, rel) in [
            ("A-sonnet", "A-sonnet/impl/objects"),
            ("B-sol", "B-sol/objects"),
            ("C-grok", "C-grok/objects"),
            ("D-gemini", "D-gemini/objects"),
            ("E-kimi", "E-kimi/objects"),
            ("F-glm", "F-glm/impl/objects"),
        ] {
            let dir = job.join(rel);
            assert!(dir.is_dir(), "missing {}", dir.display());
            dirs.push((label.to_string(), dir));
        }
    }
    let names = ["O1-S1.cbor", "O2-S2.cbor", "O3-PROD.cbor"];
    for name in names {
        let profile = profile_for(name);
        let mut first: Option<(String, String)> = None;
        for (label, dir) in &dirs {
            let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{label} {name}: {e}"));
            let record = verify(&bytes, &profile);
            assert_measured(&format!("{label} {name}"), &record);
            match &first {
                None => first = Some((label.clone(), record)),
                Some((other, prior)) => {
                    assert_eq!(&record, prior, "{label} {name} record differs from {other}");
                }
            }
        }
    }
}
