//! What About shows (DESIGN-v2 section 7.7, US-9). The pin facts come from
//! `build.rs`, which reads the workspace manifest and the lock file.

pub const SPEC: &str = "Satoshi Hike v2.0";
pub const SPEC_HARD_CUT: &str = "2026-10-08";
pub const SPEC_SIGN_IN_LOCK: &str = "§15.1 locked 2026-10-10";
pub const APP_ID: &str = "app.pcoc.Hike";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const HIKE_VERSION: &str = env!("PCOC_HIKE_VERSION");
/// `tag`, `rev`, or `path`.
pub const PIN_KIND: &str = env!("PCOC_REF_PIN_KIND");
pub const PIN: &str = env!("PCOC_REF_PIN");
/// The commit Cargo.lock resolved a git pin to. `local` for a path or patched pin.
pub const PIN_COMMIT: &str = env!("PCOC_REF_COMMIT");
pub const OBJECT_KIND: &str = hike::KIND;

pub const BITCOIN: &str = "mainnet";

/// `crate hike 0.1.0`.
pub fn crate_line() -> String {
    format!("hike {HIKE_VERSION}")
}

/// `git tag ref-v2.0.0 …`, a commit pin, or the in-tree reference.
pub fn pin_line() -> String {
    match PIN_KIND {
        "tag" => format!("git tag {PIN} of the reference workspace, not branch main"),
        "path" => "in-tree reference workspace, not branch main".to_owned(),
        _ => format!(
            "git rev {} of the reference workspace, not branch main",
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
    hike::effective_bits()
}

pub fn dns_seeds() -> String {
    hike::MAINNET_DNS_SEEDS.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_is_never_a_branch_and_names_a_commit() {
        assert_eq!(PIN_KIND, "tag");
        assert_eq!(PIN, "ref-rust-v2.0.3");
        assert_ne!(PIN_COMMIT, "unresolved");
        assert!(pin_line().contains("not branch main"));
        assert_eq!(OBJECT_KIND, "satoshi-hike-v2.0-object");
        assert_eq!(hike::MAINNET_DNS_SEEDS.len(), 7);
    }
}
