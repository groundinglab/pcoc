//! SPEC 2.3 / SDK 2.3.1 pin for the Windows producer.
//!
//! The numbers come from the in-tree SDK (`pcoc` path dependency) and from
//! `sdk/VERSION`. This module does not invent a second pin.

/// SPEC the produce path implements. Matches the Rust SDK 2.3 pack.
pub const SPEC: &str = "2.3";

/// Section 6 stage count. Numbered 0–17.
pub const STAGES: usize = pcoc::constants::STAGES;

/// Object kind this producer writes and this verifier measures.
pub const OBJECT_KIND: &str = pcoc::constants::KIND;

/// SDK version string compiled into `pcoc`. Equals `sdk/VERSION`.
pub const SDK_VERSION: &str = pcoc::VERSION;

/// `sdk/VERSION` as included next to the crate, trimmed.
pub fn sdk_version_file() -> &'static str {
    include_str!("../../../../sdk/VERSION").trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_is_spec_2_3_stages_18_kind_and_sdk_2_3_1() {
        assert_eq!(SPEC, "2.3");
        assert_eq!(STAGES, 18);
        assert_eq!(pcoc::constants::STAGES, 18);
        assert_eq!(OBJECT_KIND, "v2.2-pcoc-object");
        assert_eq!(pcoc::constants::KIND, "v2.2-pcoc-object");
        assert_eq!(SDK_VERSION, "2.3.1");
        assert_eq!(sdk_version_file(), "2.3.1");
        assert_eq!(pcoc::version(), "2.3.1");
    }
}
