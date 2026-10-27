//! W1 harness for keys, prefs, endpoints, and the omitted claims step.
//! Produce, measurement, history, and the sign-in door also have module tests.

use crate::claims::{self, ClaimsOmitted};
use crate::endpoints::{ADMISSION, BARI, INDEX, SIGN_IN, SIGN_IN_ORIGIN, VERIFY};
use crate::history::Prefs;
use crate::keys::{KeyMaterial, PublicKey, SecretKey};
use crate::pin::{self, OBJECT_KIND, SDK_VERSION, SPEC, STAGES};

#[test]
fn w1_ac2_pin_matches_the_in_tree_sdk() {
    assert_eq!(SPEC, "2.3");
    assert_eq!(STAGES, 18);
    assert_eq!(OBJECT_KIND, "v2.2-pcoc-object");
    assert_eq!(SDK_VERSION, "2.3.1");
    assert_eq!(pin::sdk_version_file(), SDK_VERSION);
    assert_eq!(crate::produce::pinned_stages(), 18);
}

#[test]
fn w1_ac3_optional_private_key_derives_and_prefs_drop_a_planted_secret() {
    let generated = SecretKey::generate();
    let nsec = generated.to_nsec();
    let hex = generated.to_hex();
    let from_nsec = KeyMaterial::parse(nsec.as_str()).unwrap();
    let from_hex = KeyMaterial::parse(hex.as_str()).unwrap();
    assert_eq!(from_nsec.public_key(), generated.public_key());
    assert_eq!(from_hex.public_key(), generated.public_key());
    let rendered = format!("{from_hex:?}");
    assert!(rendered.contains("redacted"), "{rendered}");
    assert!(!rendered.contains(hex.as_str()), "{rendered}");
    assert!(!rendered.contains(crate::keys::NSEC_PREFIX), "{rendered}");

    let public = PublicKey::parse(&generated.public_key().npub()).unwrap();
    assert_eq!(public, generated.public_key());

    let planted = format!(
        "{{\"nsec\":\"{nsec}\",\"secret_hex\":\"{hex}\",\"threads\":4,\"bari_url\":\"https://bari.groundinglab.io\"}}",
        nsec = nsec.as_str(),
        hex = hex.as_str()
    );
    let loaded = Prefs::load(&planted);
    let saved = loaded.to_json();
    assert!(!saved.contains("nsec"), "{saved}");
    assert!(!saved.contains(nsec.as_str()), "{saved}");
    assert!(!saved.contains(hex.as_str()), "{saved}");
    assert!(!saved.contains("secret_hex"), "{saved}");
    // deny_unknown_fields refuses the whole document, so a planted secret
    // does not become a key store. Defaults have no secret and no override.
    assert_eq!(loaded, Prefs::default());
    assert_eq!(loaded.effective_bari(), BARI);

    let clean = Prefs {
        threads: 4,
        ..Prefs::default()
    };
    let text = clean.to_json();
    assert!(!text.contains("nsec"));
    assert!(!text.contains(hex.as_str()));
    assert_eq!(Prefs::load(&text).threads, 4);
}

#[test]
fn w1_ac6_and_ac8_doors_match_linux_and_claims_are_omitted() {
    assert_eq!(SIGN_IN_ORIGIN, "https://groundinglab.io");
    assert_eq!(SIGN_IN, "https://admission.groundinglab.io/v1/sign-in");
    assert_eq!(ADMISSION, "https://admission.groundinglab.io");
    assert_eq!(BARI, "https://bari.groundinglab.io");
    assert_eq!(INDEX, "https://index.groundinglab.io");
    assert_eq!(VERIFY, "https://verify.groundinglab.io");
    assert_eq!(crate::signin::ORIGIN, SIGN_IN_ORIGIN);
    assert_eq!(crate::signin::DOOR, SIGN_IN);
    assert!(!crate::signin::is_result_route(
        crate::signin::sign_in_post_target()
    ));

    assert_eq!(claims::auto_claims(), Err(ClaimsOmitted::AskA1));
    let claims_source = include_str!("claims.rs");
    assert!(!claims_source.contains("https://claims"));
    assert!(!claims_source.contains("/badge"));
    assert!(!claims_source.contains("/name"));
    let tree = include_str!("lib.rs");
    assert!(!tree.contains("mod dm"));
    assert!(!tree.contains("nip04"));
}
