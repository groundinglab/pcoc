//! One TOML file. Unknown keys are a startup error. Omitted keys use the defaults.
//! The file holds paths, never a key, a token, or an age identity.

use serde::Deserialize;
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_BIND: &str = "127.0.0.1:8793";
pub const DEFAULT_CLAIMS_ORIGIN: &str = "https://claims.groundinglab.io";
pub const DEFAULT_GATE_BASE: &str = "http://127.0.0.1:8787";
pub const DEFAULT_DB_PATH: &str = "/var/lib/admission-claims/claims.sqlite";
pub const DEFAULT_ISSUER_KEY_FILE: &str = "/var/lib/admission-claims/issuer.key";
pub const DEFAULT_BADGE_D: &str = "groundinglab-admitted";
pub const DEFAULT_BADGE_NAME: &str = "groundinglab admitted";
pub const DEFAULT_BADGE_DESCRIPTION: &str =
    "Admitted by groundinglab.io with a PCOC: a self-issued identifier backed by a resource commitment.";
pub const DEFAULT_BADGE_IMAGE: &str = "https://groundinglab.io/media/badge/groundinglab-admitted-1024.png";
pub const DEFAULT_BADGE_THUMB: &str = "https://groundinglab.io/media/badge/groundinglab-admitted-256.png";
pub const DEFAULT_NAME_KIND: u32 = 1742;
pub const DEFAULT_BLOCKED: &str = "/etc/admission-claims/blocked-names.txt";
pub const DEFAULT_RELAY: &str = "wss://relay.groundinglab.io";
pub const DEFAULT_R2_BUCKET: &str = "groundinglab-claims";
pub const DEFAULT_R2_TOKEN: &str = "/var/lib/admission-claims/r2.token";
pub const DEFAULT_PUBLIC_SITE: &str = "https://groundinglab.io";
pub const DEFAULT_ARTIFACT_DIR: &str = "/var/lib/admission-claims/artifacts";
pub const DEFAULT_BACKUP_DIR: &str = "/var/lib/admission-claims/backups";
pub const DEFAULT_BACKUP_BUCKET: &str = "groundinglab-claims-backup";
pub const DEFAULT_BACKUP_TOKEN: &str = "/var/lib/admission-claims/backup.token";
pub const DEFAULT_AGE_RECIPIENT: &str = "/var/lib/admission-claims/backup.age.pub";

/// The two demonstration object hashes in `apps/www/demo-objects.js`.
pub const DEFAULT_DEMO_OBJECTS: &[&str] = &[
    "42a34e98196b4a1b11c3a5ed3357be7358d98c98176b414efa99f23fb4f7fe06",
    "6d5c8e5c0f8180312fdb5a3c9a9113c1dea1467cd27c90ca005e06b71d1528f7",
];

const DEFAULT_ORIGINS: &[&str] = &["https://groundinglab.io", "https://www.groundinglab.io"];
const DEFAULT_RETRY: &[u64] = &[1, 2, 4, 8, 16, 60];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Writer,
    Mirror,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub role: Role,
    pub bind: SocketAddr,
    pub claims_origin: String,
    pub allowed_origins: Vec<String>,
    pub gate_base: String,
    pub gate_timeout: Duration,
    pub max_log_bytes: usize,
    pub claim_window_secs: i64,
    pub line_future_skew_secs: i64,
    pub sig_skew_secs: i64,
    pub db_path: PathBuf,
    pub issuer_key_file: PathBuf,
    pub issuer_pubkey_hex: String,
    pub badge_d: String,
    pub badge_name: String,
    pub badge_description: String,
    pub badge_image_url: String,
    pub badge_thumb_url: String,
    pub name_event_kind: u32,
    pub blocked_names_file: PathBuf,
    pub demo_object_sha256: Vec<String>,
    pub claim_per_ip_per_min: u32,
    pub claim_per_pubkey_per_min: u32,
    pub availability_per_ip_per_min: u32,
    pub client_ip_header: String,
    pub relay_url: String,
    /// Best effort copies after `relay_url` accepts. They never hold the outbox.
    pub public_relays: Vec<String>,
    pub relay_timeout: Duration,
    pub relay_retry_secs: Vec<u64>,
    pub r2_account_id: String,
    pub r2_bucket: String,
    pub r2_token_file: PathBuf,
    pub r2_timeout: Duration,
    pub public_site_origin: String,
    pub artifact_dir: PathBuf,
    pub backup_dir: PathBuf,
    pub backup_keep: usize,
    pub backup_r2_bucket: String,
    pub backup_r2_token_file: PathBuf,
    pub backup_age_recipient_file: PathBuf,
    pub backup_interval_secs: u64,
    pub log_filter: String,
    pub config_path: Option<PathBuf>,
}

impl Config {
    /// Directory publisher: artifacts stay on disk. Set when `r2_account_id` is empty.
    pub fn dir_mode(&self) -> bool {
        self.r2_account_id.is_empty()
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path)
            .map_err(|err| format!("could not read config {}: {err}", path.display()))?;
        let file: FileConfig = toml::from_str(&raw)
            .map_err(|err| format!("config {} is not usable: {err}", path.display()))?;
        Self::from_file(file, Some(path.to_path_buf()))
    }

    fn from_file(file: FileConfig, source: Option<PathBuf>) -> Result<Self, String> {
        let role = match file.role.as_str() {
            "writer" => Role::Writer,
            "mirror" => Role::Mirror,
            other => return Err(format!("role {other:?} is not writer or mirror")),
        };
        let bind: SocketAddr = file
            .bind
            .parse()
            .map_err(|err| format!("bind {:?}: {err}", file.bind))?;
        if !bind.ip().is_loopback() {
            return Err(format!("bind {bind} is not a loopback address"));
        }
        let claims_origin = no_slash("claims_origin", &file.claims_origin)?;
        require_origin(&claims_origin)?;
        let gate_base = no_slash("gate_base", &file.gate_base)?;
        require_origin(&gate_base)?;
        let public_site_origin = no_slash("public_site_origin", &file.public_site_origin)?;
        require_origin(&public_site_origin)?;
        if file.gate_timeout_secs == 0 || file.r2_timeout_secs == 0 || file.relay_timeout_secs == 0
        {
            return Err("timeouts must be at least 1 second".into());
        }
        if file.max_log_bytes == 0 {
            return Err("max_log_bytes must be at least 1".into());
        }
        if file.claim_window_secs == 0 {
            return Err("claim_window_secs must be at least 1".into());
        }
        if file.sig_skew_secs == 0 {
            return Err("sig_skew_secs must be at least 1".into());
        }
        if file.relay_retry_secs.is_empty() {
            return Err("relay_retry_secs needs at least one value".into());
        }
        if file.badge_d.is_empty() || file.badge_name.is_empty() {
            return Err("badge_d and badge_name must be set".into());
        }
        if file.name_event_kind == 0 {
            return Err("name_event_kind must be at least 1".into());
        }
        if file.blocked_names_file.trim().is_empty() {
            return Err("blocked_names_file is empty".into());
        }
        if file.backup_keep == 0 {
            return Err("backup_keep must be at least 1".into());
        }
        if file.backup_interval_secs == 0 {
            return Err("backup_interval_secs must be at least 1".into());
        }
        if file.log_filter.trim().is_empty() {
            return Err("log_filter is empty".into());
        }
        let client_ip_header = file.client_ip_header.trim().to_ascii_lowercase();
        if client_ip_header.is_empty() {
            return Err("client_ip_header is empty".into());
        }
        let allowed_origins = normalize_origins(file.allowed_origins)?;
        let demo = normalize_demo(file.demo_object_sha256)?;
        let issuer_pubkey_hex = file.issuer_pubkey_hex.trim().to_ascii_lowercase();
        if !issuer_pubkey_hex.is_empty() && !is_pubkey(&issuer_pubkey_hex) {
            return Err("issuer_pubkey_hex is not 64 hex characters".into());
        }
        if role == Role::Writer {
            if issuer_pubkey_hex.is_empty() {
                return Err("writer requires issuer_pubkey_hex".into());
            }
            if file.issuer_key_file.trim().is_empty() {
                return Err("writer requires issuer_key_file".into());
            }
            if file.relay_url.trim().is_empty() {
                return Err("writer requires relay_url".into());
            }
            if !file.r2_account_id.trim().is_empty() && file.r2_token_file.trim().is_empty() {
                return Err("r2_token_file is empty".into());
            }
        } else if !issuer_pubkey_hex.is_empty() {
            return Err("mirror requires an empty issuer_pubkey_hex".into());
        }
        if file
            .public_relays
            .iter()
            .any(|url| !url.starts_with("wss://") || url.trim() != url)
        {
            return Err("public_relays entries must start with wss://".into());
        }
        if !file.relay_url.trim().is_empty()
            && !file.relay_url.starts_with("wss://")
            && !file.relay_url.starts_with("ws://")
        {
            return Err("relay_url must start with ws:// or wss://".into());
        }
        let _loopback: IpAddr = bind.ip();
        Ok(Self {
            role,
            bind,
            claims_origin,
            allowed_origins,
            gate_base,
            gate_timeout: Duration::from_secs(file.gate_timeout_secs),
            max_log_bytes: file.max_log_bytes,
            claim_window_secs: file.claim_window_secs,
            line_future_skew_secs: file.line_future_skew_secs,
            sig_skew_secs: file.sig_skew_secs,
            db_path: PathBuf::from(file.db_path),
            issuer_key_file: PathBuf::from(file.issuer_key_file),
            issuer_pubkey_hex,
            badge_d: file.badge_d,
            badge_name: file.badge_name,
            badge_description: file.badge_description,
            badge_image_url: file.badge_image_url,
            badge_thumb_url: file.badge_thumb_url.trim().to_owned(),
            name_event_kind: file.name_event_kind,
            blocked_names_file: PathBuf::from(file.blocked_names_file),
            demo_object_sha256: demo,
            claim_per_ip_per_min: file.claim_per_ip_per_min,
            claim_per_pubkey_per_min: file.claim_per_pubkey_per_min,
            availability_per_ip_per_min: file.availability_per_ip_per_min,
            client_ip_header,
            relay_url: file.relay_url,
            public_relays: file.public_relays,
            relay_timeout: Duration::from_secs(file.relay_timeout_secs),
            relay_retry_secs: file.relay_retry_secs,
            r2_account_id: file.r2_account_id.trim().to_owned(),
            r2_bucket: file.r2_bucket,
            r2_token_file: PathBuf::from(file.r2_token_file),
            r2_timeout: Duration::from_secs(file.r2_timeout_secs),
            public_site_origin,
            artifact_dir: PathBuf::from(file.artifact_dir),
            backup_dir: PathBuf::from(file.backup_dir),
            backup_keep: file.backup_keep,
            backup_r2_bucket: file.backup_r2_bucket,
            backup_r2_token_file: PathBuf::from(file.backup_r2_token_file),
            backup_age_recipient_file: PathBuf::from(file.backup_age_recipient_file),
            backup_interval_secs: file.backup_interval_secs,
            log_filter: file.log_filter,
            config_path: source,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct FileConfig {
    role: String,
    bind: String,
    claims_origin: String,
    allowed_origins: Vec<String>,
    gate_base: String,
    gate_timeout_secs: u64,
    max_log_bytes: usize,
    claim_window_secs: i64,
    line_future_skew_secs: i64,
    sig_skew_secs: i64,
    db_path: String,
    issuer_key_file: String,
    issuer_pubkey_hex: String,
    badge_d: String,
    badge_name: String,
    badge_description: String,
    badge_image_url: String,
    badge_thumb_url: String,
    name_event_kind: u32,
    blocked_names_file: String,
    demo_object_sha256: Vec<String>,
    claim_per_ip_per_min: u32,
    claim_per_pubkey_per_min: u32,
    availability_per_ip_per_min: u32,
    client_ip_header: String,
    relay_url: String,
    public_relays: Vec<String>,
    relay_timeout_secs: u64,
    relay_retry_secs: Vec<u64>,
    r2_account_id: String,
    r2_bucket: String,
    r2_token_file: String,
    r2_timeout_secs: u64,
    public_site_origin: String,
    artifact_dir: String,
    backup_dir: String,
    backup_keep: usize,
    backup_r2_bucket: String,
    backup_r2_token_file: String,
    backup_age_recipient_file: String,
    backup_interval_secs: u64,
    log_filter: String,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            role: "writer".into(),
            bind: DEFAULT_BIND.into(),
            claims_origin: DEFAULT_CLAIMS_ORIGIN.into(),
            allowed_origins: DEFAULT_ORIGINS.iter().map(|s| (*s).to_owned()).collect(),
            gate_base: DEFAULT_GATE_BASE.into(),
            gate_timeout_secs: 5,
            max_log_bytes: 33_554_432,
            claim_window_secs: 300,
            line_future_skew_secs: 5,
            sig_skew_secs: 60,
            db_path: DEFAULT_DB_PATH.into(),
            issuer_key_file: DEFAULT_ISSUER_KEY_FILE.into(),
            issuer_pubkey_hex: String::new(),
            badge_d: DEFAULT_BADGE_D.into(),
            badge_name: DEFAULT_BADGE_NAME.into(),
            badge_description: DEFAULT_BADGE_DESCRIPTION.into(),
            badge_image_url: DEFAULT_BADGE_IMAGE.into(),
            badge_thumb_url: DEFAULT_BADGE_THUMB.into(),
            name_event_kind: DEFAULT_NAME_KIND,
            blocked_names_file: DEFAULT_BLOCKED.into(),
            demo_object_sha256: DEFAULT_DEMO_OBJECTS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            claim_per_ip_per_min: 10,
            claim_per_pubkey_per_min: 5,
            availability_per_ip_per_min: 60,
            client_ip_header: "cf-connecting-ip".into(),
            relay_url: DEFAULT_RELAY.into(),
            public_relays: Vec::new(),
            relay_timeout_secs: 10,
            relay_retry_secs: DEFAULT_RETRY.to_vec(),
            r2_account_id: String::new(),
            r2_bucket: DEFAULT_R2_BUCKET.into(),
            r2_token_file: DEFAULT_R2_TOKEN.into(),
            r2_timeout_secs: 10,
            public_site_origin: DEFAULT_PUBLIC_SITE.into(),
            artifact_dir: DEFAULT_ARTIFACT_DIR.into(),
            backup_dir: DEFAULT_BACKUP_DIR.into(),
            backup_keep: 48,
            backup_r2_bucket: DEFAULT_BACKUP_BUCKET.into(),
            backup_r2_token_file: DEFAULT_BACKUP_TOKEN.into(),
            backup_age_recipient_file: DEFAULT_AGE_RECIPIENT.into(),
            backup_interval_secs: 3600,
            log_filter: "info".into(),
        }
    }
}

fn no_slash(name: &str, raw: &str) -> Result<String, String> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(format!("{name} is empty"));
    }
    if text.ends_with('/') {
        return Err(format!("{name} must not end with /"));
    }
    Ok(text.to_owned())
}

fn require_origin(raw: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(raw).map_err(|err| format!("{raw} is not a URL: {err}"))?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(format!("{raw} is not http or https"));
    }
    if url.host_str().is_none() {
        return Err(format!("{raw} has no host"));
    }
    if url.path() != "/" && url.path() != "" {
        // `Url::parse` of an origin without a path yields `/`.
        if url.path() != "/" {
            return Err(format!("{raw} must be an origin with no path"));
        }
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(format!("{raw} must not carry a query or a fragment"));
    }
    Ok(())
}

fn normalize_origins(raw: Vec<String>) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for item in raw {
        let text = no_slash("allowed_origins", &item)?;
        require_origin(&text)?;
        out.push(text);
    }
    Ok(out)
}

fn normalize_demo(raw: Vec<String>) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for item in raw {
        let text = item.trim().to_ascii_lowercase();
        if !is_pubkey(&text) {
            return Err("demo_object_sha256 entry is not 64 hex characters".to_owned());
        }
        out.push(text);
    }
    Ok(out)
}

fn is_pubkey(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_key_is_rejected_and_dir_mode_is_the_empty_account() {
        let err = toml::from_str::<FileConfig>("nope = true").unwrap_err();
        assert!(err.to_string().contains("nope") || err.to_string().contains("unknown"));
        let file = FileConfig {
            issuer_pubkey_hex: "ab".repeat(32),
            r2_account_id: String::new(),
            ..FileConfig::default()
        };
        let cfg = Config::from_file(file, None).unwrap();
        assert!(cfg.dir_mode());
        assert_eq!(cfg.bind, "127.0.0.1:8793".parse().unwrap());
        assert_eq!(cfg.demo_object_sha256.len(), 2);
        assert_eq!(cfg.name_event_kind, 1742);
        assert_eq!(cfg.backup_r2_bucket, "groundinglab-claims-backup");
        let wide = FileConfig {
            bind: "0.0.0.0:8793".into(),
            issuer_pubkey_hex: "ab".repeat(32),
            ..FileConfig::default()
        };
        assert!(Config::from_file(wide, None).is_err());
    }
}
