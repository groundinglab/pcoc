//! AC-A9 library half. The CLI half is `pcoc-cli` test `ac_a9`.

#[path = "../../support/repo.rs"]
mod repo;

use repo::repo_root;

#[test]
fn version_constant_equals_version_file() {
    let root = repo_root();
    let file = std::fs::read_to_string(root.join("sdk/VERSION")).unwrap();
    assert_eq!(file.trim(), pcoc::VERSION);
    assert_eq!(pcoc::version(), "2.3.0");
    let pin_text = std::fs::read_to_string(root.join("sdk/SPEC_PIN")).unwrap();
    let pin = pcoc::pin::read_pin(&pin_text).unwrap();
    let info = pcoc::build_info();
    assert_eq!(info.version, "2.3.0");
    assert_eq!(info.spec_tag, pin.tag);
    assert_eq!(info.spec_tag, "spec-2.3.0");
    assert_eq!(info.pack_pin, pin.vector_sha256);
}
