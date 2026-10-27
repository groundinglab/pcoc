//! One success block. Measurement stays a measurement. Claims stay on the website.

pub const CLAIM_LINE: &str = "Claim on the groundinglab website. This app does not submit a claim.";

pub struct FinishParts {
    pub saved: Result<String, String>,
    pub measurement: Result<String, String>,
    pub sign_in: String,
    pub direct_message: String,
}

pub fn success_block(parts: &FinishParts) -> String {
    let saved = match &parts.saved {
        Ok(path) => format!("Saved: {path}"),
        Err(err) => format!("Not saved. {err}"),
    };
    let measurement = match &parts.measurement {
        Ok(sentence) => sentence.clone(),
        Err(err) => format!("Measurement failed. {err} This is not a gate decision."),
    };
    format!(
        "{saved}\n{measurement}\n{}\n{}\n{CLAIM_LINE}",
        parts.sign_in, parts.direct_message
    )
}

/// Plain words for a section 14 record. No gate admit or refuse.
pub fn measurement_sentence(record_json: &str) -> String {
    let shape = serde_json::from_str::<serde_json::Value>(record_json)
        .ok()
        .and_then(|value| value.get("shape").map(|item| item.to_string()))
        .unwrap_or_else(|| "\"unreadable\"".to_owned());
    format!("Measurement record. Shape: {shape}. This is a measurement, not a gate decision.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_block_names_each_step_and_does_not_invent_a_claim() {
        let text = success_block(&FinishParts {
            saved: Ok("C:/Users/pcoc/PCOC/hikes/ab.cbor".into()),
            measurement: Ok(measurement_sentence(r#"{"shape":"record"}"#)),
            sign_in: "Sign-in submitted".into(),
            direct_message: "Not sent. No relay accepted the message.".into(),
        });
        assert!(text.contains("Saved:"));
        assert!(text.contains("Measurement record"));
        assert!(text.contains("not a gate decision"));
        assert!(text.contains("Sign-in submitted"));
        assert!(text.contains("Not sent"));
        assert!(text.contains(CLAIM_LINE));
        assert!(!text.to_ascii_lowercase().contains("admitted"));
        assert!(!text.to_ascii_lowercase().contains("refused"));
        assert!(!text.contains("badge was claimed"));
        let failed = success_block(&FinishParts {
            saved: Ok("kept.cbor".into()),
            measurement: Err("unreadable".into()),
            sign_in: "Fail. The gate did not answer.".into(),
            direct_message: "Not sent. No relay accepted the message.".into(),
        });
        assert!(failed.contains("Measurement failed"));
        assert!(failed.contains("Saved:"));
        assert!(failed.contains(CLAIM_LINE));
    }
}
