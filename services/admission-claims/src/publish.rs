//! Artifacts on disk, then the relay, then R2 when it is configured.
//!
//! Dir mode writes `artifact_dir` and does not PUT. A failed send leaves the
//! outbox row. A later attempt sends the stored event bytes, so the id does
//! not change.

use crate::award::badge_html;
use crate::clock::Clock;
use crate::config::Config;
use crate::nostr::Event;
use crate::nostr_json;
use crate::s3::R2Client;
use crate::store::{BadgeRow, DefinitionRow, NameRow, OutboxRow, Store};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

pub type RelayFut = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

#[derive(Clone)]
pub struct RelaySender {
    inner: Arc<dyn Fn(String) -> RelayFut + Send + Sync>,
}

impl RelaySender {
    pub fn new<F, Fut>(send: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        Self {
            inner: Arc::new(move |body| Box::pin(send(body))),
        }
    }

    pub fn websocket(url: String, timeout: Duration) -> Self {
        Self::new(move |body| {
            let url = url.clone();
            let timeout = timeout;
            async move { send_to_relay(&url, &body, timeout).await }
        })
    }

    /// Our relay decides the result. After it accepts, each public relay gets
    /// the same bytes in the background; each outcome is logged, none retried.
    pub fn with_public(primary: RelaySender, public: Vec<String>, timeout: Duration) -> Self {
        Self::new(move |body: String| {
            let primary = primary.clone();
            let public = public.clone();
            async move {
                primary.send(body.clone()).await?;
                for url in public {
                    let body = body.clone();
                    tokio::spawn(async move {
                        match send_to_relay(&url, &body, timeout).await {
                            Ok(()) => tracing::warn!(relay = %url, "public relay OK"),
                            Err(err) => tracing::warn!(relay = %url, error = %err, "public relay failed"),
                        }
                    });
                }
                Ok(())
            }
        })
    }

    pub async fn send(&self, body: String) -> Result<(), String> {
        (self.inner)(body).await
    }
}

#[derive(Clone, Debug)]
pub struct Artifact {
    pub key: String,
    pub body: Vec<u8>,
    pub content_type: &'static str,
}

#[derive(Serialize)]
struct IndexFile {
    feed: &'static str,
    items: Vec<IndexItem>,
}

#[derive(Serialize)]
struct IndexItem {
    #[serde(rename = "type")]
    kind: &'static str,
    id: String,
    pubkey: String,
    name: Option<String>,
    created_at: i64,
}

pub fn render(
    definitions: &[DefinitionRow],
    badges: &[BadgeRow],
    names: &[NameRow],
) -> Result<Vec<Artifact>, String> {
    let mut items = Vec::new();
    let mut files = Vec::new();
    for definition in definitions {
        let event = Event::from_json(&definition.event_json)
            .map_err(|_| "stored definition is not an event".to_owned())?;
        items.push(IndexItem {
            kind: "definition",
            id: event.id.clone(),
            pubkey: event.pubkey.clone(),
            name: None,
            created_at: event.created_at,
        });
        files.push(json_artifact(
            format!("claims/{}.json", event.id),
            &definition.event_json,
        ));
    }
    for badge in badges {
        let event = Event::from_json(&badge.event_json)
            .map_err(|_| "stored award is not an event".to_owned())?;
        let npub = crate::nostr::npub_from_hex(&badge.pubkey)?;
        items.push(IndexItem {
            kind: "badge",
            id: event.id.clone(),
            pubkey: badge.pubkey.clone(),
            name: None,
            created_at: event.created_at,
        });
        files.push(json_artifact(
            format!("claims/{}.json", event.id),
            &badge.event_json,
        ));
        files.push(Artifact {
            key: format!("badge/{npub}.html"),
            body: badge_html(&event, badge.admitted_unix_s).into_bytes(),
            content_type: "text/html; charset=utf-8",
        });
    }
    for row in names {
        items.push(IndexItem {
            kind: "name",
            id: row.event_id.clone(),
            pubkey: row.pubkey.clone(),
            name: Some(row.name.clone()),
            created_at: row.created_unix_s,
        });
        files.push(json_artifact(format!("claims/{}.json", row.event_id), &row.event_json));
    }
    items.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    let index = serde_json::to_vec(&IndexFile {
        feed: "groundinglab-claims-v1",
        items,
    })
    .map_err(|err| err.to_string())?;
    let nostr = nostr_json::from_index(&index)?;
    files.push(Artifact {
        key: "claims/index.json".into(),
        body: index,
        content_type: "application/json",
    });
    files.push(Artifact {
        key: ".well-known/nostr.json".into(),
        body: nostr,
        content_type: "application/json",
    });
    Ok(files)
}

fn json_artifact(key: String, event_json: &str) -> Artifact {
    Artifact {
        key,
        body: event_json.as_bytes().to_vec(),
        content_type: "application/json",
    }
}

pub fn write_artifacts(dir: &Path, files: &[Artifact]) -> Result<(), String> {
    for file in files {
        let path = dir.join(&file.key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
        }
        fs::write(&path, &file.body)
            .map_err(|err| format!("could not write {}: {err}", path.display()))?;
    }
    Ok(())
}

pub fn artifact_index_matches(dir: &Path, files: &[Artifact]) -> bool {
    match_key(dir, files, "claims/index.json") && match_key(dir, files, ".well-known/nostr.json")
}

fn match_key(dir: &Path, files: &[Artifact], key: &str) -> bool {
    let Some(file) = files.iter().find(|file| file.key == key) else {
        return false;
    };
    fs::read(dir.join(key)).ok().as_ref() == Some(&file.body)
}

/// Write every artifact, enqueue new outbox rows, and try anything that is due.
/// Relay failures stay in the outbox. Dir mode enqueues no `r2:` target.
pub async fn publish_stored(
    store: &Store,
    cfg: &Config,
    relay: &RelaySender,
    r2: Option<&R2Client>,
    now: i64,
) -> Result<(), String> {
    let definitions = store.definitions()?;
    let badges = store.badges()?;
    let names = store.names()?;
    let files = render(&definitions, &badges, &names)?;
    write_artifacts(&cfg.artifact_dir, &files)?;
    for definition in &definitions {
        store.enqueue(&definition.event_id, "relay", &definition.event_json, now)?;
        if r2.is_some() {
            store.enqueue(
                &definition.event_id,
                &format!("r2:claims/{}.json", definition.event_id),
                &definition.event_json,
                now,
            )?;
        }
    }
    for badge in &badges {
        store.enqueue(&badge.event_id, "relay", &badge.event_json, now)?;
        if r2.is_some() {
            let npub = crate::nostr::npub_from_hex(&badge.pubkey)?;
            store.enqueue(
                &badge.event_id,
                &format!("r2:claims/{}.json", badge.event_id),
                &badge.event_json,
                now,
            )?;
            let html = files
                .iter()
                .find(|file| file.key == format!("badge/{npub}.html"))
                .map(|file| String::from_utf8_lossy(&file.body).into_owned())
                .unwrap_or_default();
            store.enqueue(
                &badge.event_id,
                &format!("r2:badge/{npub}.html"),
                &html,
                now,
            )?;
        }
    }
    for row in &names {
        store.enqueue(&row.event_id, "relay", &row.event_json, now)?;
        if r2.is_some() {
            store.enqueue(
                &row.event_id,
                &format!("r2:claims/{}.json", row.event_id),
                &row.event_json,
                now,
            )?;
        }
    }
    if r2.is_some() {
        let index = file_text(&files, "claims/index.json");
        let nostr = file_text(&files, ".well-known/nostr.json");
        store.enqueue("index", "r2:claims/index.json", &index, now)?;
        store.enqueue("nostr-json", "r2:.well-known/nostr.json", &nostr, now)?;
    }
    send_due(store, relay, r2, &cfg.relay_retry_secs, now).await;
    Ok(())
}

async fn send_due(
    store: &Store,
    relay: &RelaySender,
    r2: Option<&R2Client>,
    retry: &[u64],
    now: i64,
) {
    let due = match store.due(now) {
        Ok(rows) => rows,
        Err(err) => {
            tracing::warn!(error = %err, "outbox read failed");
            return;
        }
    };
    for row in due {
        let result = dispatch(&row, relay, r2).await;
        match result {
            Ok(()) => {
                if let Err(err) = store.ack(&row.event_id, &row.target) {
                    tracing::warn!(error = %err, "outbox ack failed");
                }
            }
            Err(err) => {
                let attempts = row.attempts.saturating_add(1);
                let delay = retry_delay(retry, attempts);
                tracing::warn!(target = %row.target, attempts, "publish not acknowledged");
                let _ = err;
                if let Err(update) = store.fail(
                    &row.event_id,
                    &row.target,
                    attempts,
                    now.saturating_add(delay),
                ) {
                    tracing::warn!(error = %update, "outbox delay failed");
                }
            }
        }
    }
}

async fn dispatch(
    row: &OutboxRow,
    relay: &RelaySender,
    r2: Option<&R2Client>,
) -> Result<(), String> {
    if row.target == "relay" {
        return relay.send(row.body.clone()).await;
    }
    if let Some(key) = row.target.strip_prefix("r2:") {
        let Some(client) = r2 else {
            return Err("r2 is not configured".into());
        };
        let content_type = if key.ends_with(".html") {
            "text/html; charset=utf-8"
        } else {
            "application/json"
        };
        return client.put(key, row.body.as_bytes(), content_type).await;
    }
    Err(format!("unknown outbox target {}", row.target))
}

fn retry_delay(retry: &[u64], attempts: i64) -> i64 {
    if retry.is_empty() {
        return 60;
    }
    let index = (attempts.saturating_sub(1).max(0) as usize).min(retry.len() - 1);
    retry[index] as i64
}

fn file_text(files: &[Artifact], key: &str) -> String {
    files
        .iter()
        .find(|file| file.key == key)
        .map(|file| String::from_utf8_lossy(&file.body).into_owned())
        .unwrap_or_default()
}

pub async fn send_to_relay(url: &str, event_json: &str, timeout: Duration) -> Result<(), String> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    let (mut socket, _) = tokio::time::timeout(timeout, tokio_tungstenite::connect_async(url))
        .await
        .map_err(|_| "relay timed out".to_owned())?
        .map_err(|_| "relay connect failed".to_owned())?;
    let event: Value =
        serde_json::from_str(event_json).map_err(|_| "stored event is not json".to_owned())?;
    let id = event
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let payload = serde_json::to_string(&serde_json::json!(["EVENT", event]))
        .map_err(|err| err.to_string())?;
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|_| "relay send failed".to_owned())?;
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return Err("relay ack timed out".into());
        }
        let message = tokio::time::timeout(left, socket.next())
            .await
            .map_err(|_| "relay ack timed out".to_owned())?
            .ok_or_else(|| "relay closed".to_owned())?
            .map_err(|_| "relay read failed".to_owned())?;
        let text = match message {
            Message::Text(text) => text.to_string(),
            Message::Ping(data) => {
                let _ = socket.send(Message::Pong(data)).await;
                continue;
            }
            Message::Close(_) => return Err("relay closed".into()),
            _ => continue,
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(items) = value.as_array() else {
            continue;
        };
        if items.first().and_then(Value::as_str) != Some("OK") {
            continue;
        }
        if items.get(1).and_then(Value::as_str) != Some(id.as_str()) {
            continue;
        }
        if items.get(2).and_then(Value::as_bool) == Some(true) {
            return Ok(());
        }
        return Err("relay rejected the event".into());
    }
}

/// The clock parameter keeps publish decisions on the injected clock.
pub fn now_s(clock: &dyn Clock) -> i64 {
    clock.unix_s()
}
