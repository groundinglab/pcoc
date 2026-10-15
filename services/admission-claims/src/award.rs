//! Kind 30009 once, kind 8 per npub. The award records admission only.

use crate::config::Config;
use crate::logproof::Admission;
use crate::nostr::{self, Event, KIND_BADGE_AWARD, KIND_BADGE_DEFINITION};
use crate::store::BadgeRow;
use crate::timeutil::utc_words;

/// The 30009 tags the config asks for. A stored definition with other tags is re-signed.
pub fn definition_tags(cfg: &Config) -> Vec<Vec<String>> {
    let mut tags = vec![
        vec!["d".into(), cfg.badge_d.clone()],
        vec!["name".into(), cfg.badge_name.clone()],
        vec!["description".into(), cfg.badge_description.clone()],
        vec!["image".into(), cfg.badge_image_url.clone(), "1024x1024".into()],
    ];
    if !cfg.badge_thumb_url.is_empty() {
        tags.push(vec!["thumb".into(), cfg.badge_thumb_url.clone(), "256x256".into()]);
    }
    tags
}

pub fn definition_event(cfg: &Config, secret: &[u8; 32], created_at: i64) -> Result<Event, String> {
    nostr::sign_event(secret, created_at, KIND_BADGE_DEFINITION, definition_tags(cfg), "")
}

pub fn award_event(
    cfg: &Config,
    secret: &[u8; 32],
    admission: &Admission,
    created_at: i64,
) -> Result<Event, String> {
    let issuer = nostr::pubkey_hex(secret)?;
    let a = format!("30009:{issuer}:{}", cfg.badge_d);
    let r = nostr::r_tag(&admission.accept_day, admission.accept_seq);
    let tags = vec![
        vec!["a".into(), a],
        vec!["p".into(), admission.pubkey.clone()],
        vec!["policy".into(), admission.policy_id.clone()],
        vec!["r".into(), r],
    ];
    nostr::sign_event(secret, created_at, KIND_BADGE_AWARD, tags, "")
}

pub fn badge_row(event: &Event, admission: &Admission) -> BadgeRow {
    BadgeRow {
        pubkey: admission.pubkey.clone(),
        event_id: event.id.clone(),
        event_json: event.to_json(),
        policy_id: admission.policy_id.clone(),
        log_day: admission.accept_day.clone(),
        log_seq: admission.accept_seq,
        admitted_unix_s: admission.admitted_unix_s,
        object_sha256: admission.object_sha256.clone(),
        created_unix_s: event.created_at,
    }
}

/// Badge page. It names the issuer, the recipient, both times, the policy,
/// the log reference, the event id, and the raw event. It loads no script.
pub fn badge_html(event: &Event, admitted_unix_s: i64) -> String {
    let issuer = escape(&event.pubkey);
    let recipient_hex = event
        .tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some("p"))
        .and_then(|tag| tag.get(1))
        .map(String::as_str)
        .unwrap_or("");
    let recipient_npub = nostr::npub_from_hex(recipient_hex).unwrap_or_default();
    let policy = tag_value(event, "policy");
    let log_ref = tag_value(event, "r");
    let raw = escape(&event.to_json());
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>groundinglab admitted</title>\n</head>\n<body>\n<h1>groundinglab admitted</h1>\n<dl>\n<dt>issuer</dt><dd>{issuer}</dd>\n<dt>recipient</dt><dd>{npub}</dd>\n<dt>recipient public key</dt><dd>{recipient}</dd>\n<dt>admission time</dt><dd>{admitted}</dd>\n<dt>issue time</dt><dd>{issued}</dd>\n<dt>policy id</dt><dd>{policy}</dd>\n<dt>log reference</dt><dd>{log_ref}</dd>\n<dt>event id</dt><dd>{id}</dd>\n</dl>\n<h2>signed event</h2>\n<pre>{raw}</pre>\n</body>\n</html>\n",
        issuer = issuer,
        npub = escape(&recipient_npub),
        recipient = escape(recipient_hex),
        admitted = escape(&format!("{} ({admitted_unix_s})", utc_words(admitted_unix_s))),
        issued = escape(&format!("{} ({})", utc_words(event.created_at), event.created_at)),
        policy = escape(policy),
        log_ref = escape(log_ref),
        id = escape(&event.id),
        raw = raw,
    )
}

fn tag_value<'a>(event: &'a Event, name: &str) -> &'a str {
    event
        .tags
        .iter()
        .find(|tag| tag.first().map(String::as_str) == Some(name))
        .and_then(|tag| tag.get(1))
        .map(String::as_str)
        .unwrap_or("")
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Kind 1742, regular: the name points to the npub in the cited gate line.
pub fn name_event(
    cfg: &Config,
    secret: &[u8; 32],
    name: &str,
    admission: &Admission,
    created_at: i64,
) -> Result<Event, String> {
    let tags = vec![
        vec!["d".into(), name.to_owned()],
        vec!["p".into(), admission.pubkey.clone()],
        vec!["policy".into(), admission.policy_id.clone()],
        vec!["r".into(), nostr::r_tag(&admission.accept_day, admission.accept_seq)],
    ];
    nostr::sign_event(secret, created_at, cfg.name_event_kind, tags, "")
}
