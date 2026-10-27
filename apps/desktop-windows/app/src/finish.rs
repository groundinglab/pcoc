//! After a finished object is on disk: measure, one sign-in POST, one NIP-04 send.
//! A later failure does not delete the file. Claims are the fail-closed stub.

use crate::success::{measurement_sentence, success_block, FinishParts, CLAIM_LINE};
use pcoc::profile::Profile;
use pcoc_windows_core::claims;
use pcoc_windows_core::dm;
use pcoc_windows_core::keys::SecretKey;
use pcoc_windows_core::signin::{self, ClientClass};
use pcoc_windows_core::verify::{self, MeasureError};
use std::path::Path;
use zeroize::Zeroize;

pub struct FinishInput<'a> {
    pub saved_path: &'a Path,
    pub bytes: &'a [u8],
    pub profile: &'a Profile,
    pub secret: &'a SecretKey,
    pub sign_in_link: &'a str,
    pub door: &'a str,
    pub relays: &'a [&'a str],
}

/// Measure, post sign-in once when the link parses, and send the direct message.
/// Does not remove `saved_path`.
pub fn complete(input: &FinishInput<'_>) -> String {
    let _ = claims::auto_claims();
    let saved = Ok(input.saved_path.display().to_string());
    let measurement = match verify::measure_bytes(input.bytes, input.profile) {
        Ok(measured) => Ok(measurement_sentence(&measured.record_json)),
        Err(MeasureError::CouldNotOpen(err)) => Err(format!("Could not open the file: {err}")),
        Err(MeasureError::Unreadable) => {
            Err("The saved bytes did not decode into a record.".to_owned())
        }
    };
    let sign_in = sign_in_sentence(input);
    let direct_message = dm_sentence(input);
    let text = success_block(&FinishParts {
        saved,
        measurement,
        sign_in,
        direct_message,
    });
    debug_assert!(text.contains(CLAIM_LINE));
    text
}

fn sign_in_sentence(input: &FinishInput<'_>) -> String {
    let link = input
        .sign_in_link
        .trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    if link.is_empty() {
        return "Sign-in was not sent. Paste the sign-in link from the website on this screen."
            .to_owned();
    }
    let challenge = match signin::parse_link(link) {
        Ok(challenge) => challenge,
        Err(err) => return err.sentence().to_owned(),
    };
    let object_key = match dm::recipient(input.bytes) {
        Ok(key) => key,
        Err(_) => {
            return "Sign-in was not sent. This file has no public key to sign.".to_owned();
        }
    };
    let mut aux = [0u8; 32];
    if getrandom::getrandom(&mut aux).is_err() {
        return "Sign-in was not sent. The signature randomness failed.".to_owned();
    }
    let submission = match signin::sign(&challenge, input.bytes, object_key, input.secret, &aux) {
        Ok(submission) => submission,
        Err(err) => {
            aux.zeroize();
            return err.sentence().to_owned();
        }
    };
    aux.zeroize();
    match signin::post_once(input.door, &submission.body) {
        ClientClass::NotDelivered => "Fail. The gate did not answer.".to_owned(),
        class => class
            .sentence()
            .unwrap_or("Fail. The gate did not answer.")
            .to_owned(),
    }
}

fn dm_sentence(input: &FinishInput<'_>) -> String {
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    match dm::prepare(input.bytes, input.secret, created_at) {
        Ok(prepared) => match dm::publish(&prepared, input.relays) {
            Ok(sent) => sent.sentence(),
            Err(err) => err.to_string(),
        },
        Err(err) => err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn finish_sources_do_not_name_claim_paths_or_the_result_route() {
        let blob = concat!(
            include_str!("finish.rs"),
            include_str!("success.rs"),
            include_str!("lib.rs")
        );
        let badge = concat!("/", "badge");
        let name_path = concat!("/", "name");
        assert!(!blob.contains(badge), "{blob}");
        assert!(!blob.contains(name_path));
        let marker = concat!("/v1/sign-in/", "result");
        assert!(!blob.contains(marker));
    }
}
