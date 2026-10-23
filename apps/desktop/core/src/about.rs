//! What About shows (DESIGN-v2 section 7.7, US-9). The pin facts come from
//! `build.rs`, which reads the workspace manifest and the lock file.

pub const SPEC: &str = "SDK 2.2";
pub const SPEC_HARD_CUT: &str = "2026-10-08";
pub const SPEC_SIGN_IN_LOCK: &str = "§15.1 locked 2026-10-10";
pub const APP_ID: &str = "app.pcoc.Hike";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SDK_VERSION: &str = env!("PCOC_SDK_VERSION");
/// `tag`, `rev`, or `path`.
pub const PIN_KIND: &str = env!("PCOC_REF_PIN_KIND");
pub const PIN: &str = env!("PCOC_REF_PIN");
/// The commit Cargo.lock resolved a git pin to. `local` for a path or patched pin.
pub const PIN_COMMIT: &str = env!("PCOC_REF_COMMIT");
pub const OBJECT_KIND: &str = pcoc::constants::KIND;

pub const BITCOIN: &str = "mainnet";

/// `pcoc 2.3.0`: the Rust SDK crate and the version Cargo.lock resolved.
pub fn crate_line() -> String {
    format!("pcoc {SDK_VERSION}")
}

/// The About row "PCOC SDK": the SDK version this build uses.
pub fn sdk_line() -> &'static str {
    SDK_VERSION
}

/// The About row "SDK source", in words a reader can use.
pub fn source_line() -> String {
    match PIN_KIND {
        "path" => "built in, from this repository".to_owned(),
        "tag" => format!("git tag {PIN} of the PCOC Rust SDK"),
        _ => format!("git commit {} of the PCOC Rust SDK", short_rev(PIN)),
    }
}

/// "Resolved commit" means something only for a git pin. A path build has none.
pub fn shows_resolved_commit() -> bool {
    PIN_KIND != "path"
}

/// The note under the About rows. It states facts `build.rs` reports; the
/// build does not refuse any pin.
pub fn pin_note() -> String {
    match PIN_KIND {
        "path" => "This build includes the PCOC Rust SDK from this repository. The Go and Java SDKs are not included.".to_owned(),
        "tag" => format!(
            "This build includes the PCOC Rust SDK at tag {PIN}. Upgrading that pin is a separate change. The Go and Java SDKs are not included."
        ),
        _ => "This build includes the PCOC Rust SDK at a pinned commit rather than a tag. Upgrading the pin is a separate change. The Go and Java SDKs are not included.".to_owned(),
    }
}

/// A commit pin, or the in-tree SDK at `sdk/rust` (developer wording).
pub fn pin_line() -> String {
    match PIN_KIND {
        "tag" => format!("git tag {PIN} of the Rust SDK, not branch main"),
        "path" => format!("in-tree Rust SDK at {PIN}"),
        _ => format!(
            "git rev {} of the Rust SDK, not branch main",
            short_rev(PIN)
        ),
    }
}

pub fn commit_line() -> String {
    short_rev(PIN_COMMIT).to_owned()
}

fn short_rev(rev: &str) -> &str {
    &rev[..rev.len().min(7)]
}

/// Leading zero bits of the planting puzzle (P13.5).
pub fn hashcash_bits() -> u32 {
    pcoc::constants::HASHCASH_DIFFICULTY
}

/// Live produce does not dial. Phase C owns the header source.
pub fn dns_seeds() -> String {
    crate::produce::live::LIVE_HEADER_SOURCE.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_is_the_in_tree_sdk() {
        assert_eq!(PIN_KIND, "path");
        assert_eq!(PIN, "../../sdk/rust/pcoc");
        assert_eq!(PIN_COMMIT, "local");
        assert_eq!(pin_line(), "in-tree Rust SDK at ../../sdk/rust/pcoc");
        assert_eq!(OBJECT_KIND, "v2.2-pcoc-object");
        assert_eq!(SPEC, "SDK 2.2");
        assert_eq!(hashcash_bits(), 21);
        assert_eq!(dns_seeds(), "live header source is not in this build");
    }

    #[test]
    fn about_rows_for_a_path_pin_are_plain() {
        assert_eq!(source_line(), "built in, from this repository");
        assert!(!shows_resolved_commit());
        let note = pin_note();
        assert!(
            note.contains("PCOC Rust SDK from this repository"),
            "{note}"
        );
        assert!(
            !note.contains("reference") && !note.contains("branch main"),
            "{note}"
        );
    }

    #[test]
    fn reports_the_sdk_version_from_sdk_version_file() {
        let want = include_str!("../../../../sdk/VERSION").trim();
        assert_eq!(SDK_VERSION, want);
        assert_eq!(want, "2.3.0");
        assert_eq!(crate_line(), format!("pcoc {want}"));
        assert_eq!(sdk_line(), want);
    }
}
