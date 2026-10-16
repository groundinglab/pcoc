//! One config file. The process does not read `GL_VERIFY_*`.
//!
//! Every key is optional. An omitted key uses the default documented in
//! `deploy/DEPLOY.md`. Unknown keys are an error, so a retired flag cannot
//! be turned back on by leaving it in the file.

use crate::policy::{self, AdmissionPolicy, RateFloor};
use serde::Deserialize;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Transport cap on the uploaded object.
pub const DEFAULT_MAX_OBJECT_BYTES: usize = 2 * 1024 * 1024;

/// Slack so a multipart envelope around a max-size object reaches the picker.
pub const MULTIPART_ENVELOPE_SLACK: usize = 64 * 1024;

/// Spare bytes around the sign-in JSON.
///
/// The sign-in body is the six-field object: `origin`, `nonce`, `exp`,
/// `object`, `object_b64`, and `signature`. The transport limit is the base64
/// expansion of `max_object_bytes` plus this, and at least the raw object
/// plus [`MULTIPART_ENVELOPE_SLACK`].
pub const JSON_ENVELOPE_BYTES: usize = 256;

pub const DEFAULT_BIND_THREADS: usize = hike::DEFAULT_BIND_THREADS;
pub const DEFAULT_INDEX_TIMEOUT_SECS: u64 = 5;
pub const DEFAULT_ALLOWED_ORIGINS: &[&str] = &[];

pub const DEFAULT_BIND: &str = "127.0.0.1:8787";
pub const DEFAULT_INDEX_URL: &str = "https://index.groundinglab.io";
pub const DEFAULT_LOG_DIR: &str = "gate-log-v2";
pub const DEFAULT_JOB_RETENTION_SECS: u64 = 3600;
pub const DEFAULT_MAX_JOBS: usize = 256;
pub const DEFAULT_MAX_CONCURRENT: usize = 2;
pub const DEFAULT_RATE_PER_MIN: u32 = 12;
/// Shipped recommendation. The process reads this file when `policy` is omitted.
pub const DEFAULT_POLICY_FILE: &str = "policy/gl-admit-v4.toml";

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: SocketAddr,
    /// Exact CORS origins from the file. Empty (the default) allows loopback only.
    pub allowed_origins: Vec<String>,
    pub extra_origins: Vec<String>,
    pub max_object_bytes: usize,
    pub job_retention: Duration,
    pub max_jobs: usize,
    pub max_concurrent_verify: usize,
    pub rate_per_min: u32,
    pub client_ip_header: Option<String>,
    pub index_url: String,
    /// Decision log directory. The process creates it. A v1.35 chain here is
    /// not loaded.
    pub log_dir: PathBuf,
    /// The full policy table. The code default is `gl-admit-v4`.
    pub policy: AdmissionPolicy,
    /// Checkpoint binding threads. The process does not read `PCOC_BIND_THREADS`.
    pub bind_threads: usize,
    /// One-shot index read. A timeout, a 5xx, or a refused connection is not retried.
    pub index_timeout: Duration,
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct FileConfig {
    bind: String,
    index_url: String,
    log_dir: String,
    max_object_bytes: usize,
    job_retention_secs: u64,
    max_jobs: usize,
    max_concurrent: usize,
    rate_per_min: u32,
    client_ip_header: Option<String>,
    allowed_origins: Vec<String>,
    extra_origins: Vec<String>,
    bind_threads: usize,
    index_timeout_secs: u64,
    /// Path of a policy file, relative to this config file. When omitted, and
    /// [`Self::policy_table`] is also omitted, the process reads
    /// `policy/gl-admit-v4.toml` beside this file.
    policy: Option<String>,
    /// Inline policy table. The keys are those of `gl-admit-v4.toml`.
    /// Unknown keys are an error. Mutually exclusive with [`Self::policy`].
    #[serde(default, rename = "policy_table")]
    policy_table: Option<PolicyFile>,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND.to_owned(),
            index_url: DEFAULT_INDEX_URL.to_owned(),
            log_dir: DEFAULT_LOG_DIR.to_owned(),
            max_object_bytes: DEFAULT_MAX_OBJECT_BYTES,
            job_retention_secs: DEFAULT_JOB_RETENTION_SECS,
            max_jobs: DEFAULT_MAX_JOBS,
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            rate_per_min: DEFAULT_RATE_PER_MIN,
            client_ip_header: None,
            allowed_origins: DEFAULT_ALLOWED_ORIGINS
                .iter()
                .map(|origin| (*origin).to_owned())
                .collect(),
            extra_origins: Vec::new(),
            bind_threads: DEFAULT_BIND_THREADS,
            index_timeout_secs: DEFAULT_INDEX_TIMEOUT_SECS,
            policy: None,
            policy_table: None,
        }
    }
}

/// The keys of `policy/gl-admit-v4.toml`. Unknown keys are an error.
///
/// `min_leg_rate` is not a key. Gate 0.4.1 read that float. A log line at
/// startup records the move from that float to this integer floor.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyFile {
    policy_id: String,
    leg_rate_floor: RateFloorFile,
    min_anchored_paths: u64,
    require_first_anchored: u64,
    require_last_anchored: u64,
    thorough_six_leg_runs: u64,
    thorough_early_stop: bool,
    provisional: String,
    held_on_not_read: bool,
    ban_key_on_fake_leg: bool,
    draw_commitment: String,
    /// Optional. The shipped file omits these. The code default fills them.
    #[serde(default)]
    reverify_max_attempts: Option<u64>,
    #[serde(default)]
    max_held_blocks: Option<u64>,
    #[serde(default)]
    pending_per_source: Option<u64>,
    #[serde(default)]
    pending_global: Option<u64>,
    #[serde(default)]
    verify_cap_per_door_session: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RateFloorFile {
    p: u64,
    q: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self::from_file(FileConfig::default(), None).expect("default config")
    }
}

impl Config {
    pub fn max_body_bytes(&self) -> usize {
        let raw = self.max_object_bytes;
        // 4 * ceil(raw / 3), the base64 length of a max-size object.
        let b64 = raw.saturating_add(2) / 3 * 4;
        let json = b64.saturating_add(JSON_ENVELOPE_BYTES);
        let multipart = raw.saturating_add(MULTIPART_ENVELOPE_SLACK);
        json.max(multipart)
    }

    /// Read `path`. The file is the only configuration. Environment variables
    /// are not consulted.
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path)
            .map_err(|err| format!("could not read config {}: {err}", path.display()))?;
        let file: FileConfig = toml::from_str(&raw)
            .map_err(|err| format!("config {} is not usable: {err}", path.display()))?;
        Self::from_file(file, Some(path.to_path_buf()))
    }

    /// v1 → v4. The float `min_leg_rate` and the named `leg_rate_floor`
    /// (`strict` / `default` / `lenient`) are not read. The active floor is
    /// the integer `p/q` in the policy table.
    fn note_v1_migration(policy: &AdmissionPolicy) {
        eprintln!(
            "admission-service policy: gl-admit-v1 float min_leg_rate is retired; active {} compares q·N ≥ p·S with p/q = {}/{}",
            policy.policy_id, policy.leg_rate_floor.p, policy.leg_rate_floor.q
        );
    }

    fn from_file(file: FileConfig, source: Option<PathBuf>) -> Result<Self, String> {
        if file.max_object_bytes == 0 {
            return Err("max_object_bytes must be at least 1".to_owned());
        }
        if file.job_retention_secs == 0 {
            return Err("job_retention_secs must be at least 1".to_owned());
        }
        if file.max_jobs == 0 {
            return Err("max_jobs must be at least 1".to_owned());
        }
        if file.max_concurrent == 0 {
            return Err("max_concurrent must be at least 1".to_owned());
        }
        if file.bind_threads == 0 {
            return Err("bind_threads must be at least 1".to_owned());
        }
        if file.index_timeout_secs == 0 {
            return Err("index_timeout_secs must be at least 1".to_owned());
        }
        let policy = resolve_policy(&file, source.as_deref())?;
        Self::note_v1_migration(&policy);
        let bind = file
            .bind
            .parse()
            .map_err(|err| format!("bind {:?}: {err}", file.bind))?;
        let index_url = parse_index_url(&file.index_url)?;
        let client_ip_header = file.client_ip_header.and_then(|raw| {
            let trimmed = raw.trim().to_ascii_lowercase();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        let extra_origins = normalize_origins(file.extra_origins);
        let allowed_origins = normalize_origins(file.allowed_origins);
        Ok(Self {
            bind,
            allowed_origins,
            extra_origins,
            max_object_bytes: file.max_object_bytes,
            job_retention: Duration::from_secs(file.job_retention_secs),
            max_jobs: file.max_jobs,
            max_concurrent_verify: file.max_concurrent,
            rate_per_min: file.rate_per_min,
            client_ip_header,
            index_url,
            log_dir: PathBuf::from(file.log_dir),
            policy,
            bind_threads: file.bind_threads,
            index_timeout: Duration::from_secs(file.index_timeout_secs),
            config_path: source,
        })
    }

    pub fn policy(&self) -> AdmissionPolicy {
        self.policy.clone()
    }

    /// A config for tests: empty temp log, limiter off, the given index origin.
    pub fn for_tests(log_dir: PathBuf, index_url: &str) -> Self {
        Self {
            rate_per_min: 0,
            index_url: index_url.to_owned(),
            log_dir,
            index_timeout: Duration::from_secs(1),
            ..Self::default()
        }
    }
}

fn normalize_origins(origins: Vec<String>) -> Vec<String> {
    origins
        .into_iter()
        .map(|origin| origin.trim().trim_end_matches('/').to_owned())
        .filter(|origin| !origin.is_empty())
        .collect()
}

fn resolve_policy(file: &FileConfig, source: Option<&Path>) -> Result<AdmissionPolicy, String> {
    if file.policy.is_some() && file.policy_table.is_some() {
        return Err(
            "policy path and an inline [policy_table] are both set; use one".to_owned(),
        );
    }
    let table = if let Some(inline) = &file.policy_table {
        inline.clone()
    } else {
        let path = policy_path(file.policy.as_deref(), source)?;
        let raw = fs::read_to_string(&path)
            .map_err(|err| format!("could not read policy {}: {err}", path.display()))?;
        toml::from_str(&raw).map_err(|err| format!("policy {} is not usable: {err}", path.display()))?
    };
    policy_from_file(table, file.max_concurrent)
}

fn policy_path(named: Option<&str>, source: Option<&Path>) -> Result<PathBuf, String> {
    let relative = named.unwrap_or(DEFAULT_POLICY_FILE);
    let shipped = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    if let Some(source) = source {
        if let Some(parent) = source.parent() {
            let beside = parent.join(relative);
            // A deploy names the file. An omitted key uses the shipped table
            // when the config file's directory has no copy of its own.
            if named.is_some() || beside.is_file() {
                return Ok(beside);
            }
            if shipped.is_file() {
                return Ok(shipped);
            }
            return Ok(beside);
        }
    }
    Ok(shipped)
}

fn policy_from_file(file: PolicyFile, max_concurrent: usize) -> Result<AdmissionPolicy, String> {
    let policy_id = policy::parse_policy_id(&file.policy_id)?;
    if file.leg_rate_floor.p == 0 || file.leg_rate_floor.q == 0 {
        return Err("leg_rate_floor p and q must each be at least 1".to_owned());
    }
    if file.min_anchored_paths > policy::ANCHORED_SPAN {
        return Err(format!(
            "min_anchored_paths must be at most {}",
            policy::ANCHORED_SPAN
        ));
    }
    if file.require_first_anchored == 0 || file.require_first_anchored > policy::ANCHORED_SPAN {
        return Err("require_first_anchored must be a waypoint index from 1 through 16".to_owned());
    }
    if file.require_last_anchored == 0 || file.require_last_anchored > policy::ANCHORED_SPAN {
        return Err("require_last_anchored must be a waypoint index from 1 through 16".to_owned());
    }
    if file.require_first_anchored > file.require_last_anchored {
        return Err("require_first_anchored must be at most require_last_anchored".to_owned());
    }
    if file.thorough_six_leg_runs == 0 {
        return Err("thorough_six_leg_runs must be at least 1".to_owned());
    }
    if file.provisional != "on" && file.provisional != "off" {
        return Err("provisional must be \"on\" or \"off\"".to_owned());
    }
    let reverify_max_attempts = file.reverify_max_attempts.unwrap_or(3);
    let max_held_blocks = file.max_held_blocks.unwrap_or(6);
    if reverify_max_attempts == 0 || max_held_blocks == 0 {
        return Err("reverify_max_attempts and max_held_blocks must each be at least 1".to_owned());
    }
    let pending_per_source = file
        .pending_per_source
        .unwrap_or(policy::DEFAULT_PENDING_PER_SOURCE);
    let pending_global = file.pending_global.unwrap_or(
        (max_concurrent as u64).saturating_mul(policy::DEFAULT_PENDING_GLOBAL_FACTOR),
    );
    if pending_per_source == 0 || pending_global == 0 {
        return Err("pending caps must each be at least 1".to_owned());
    }
    let verify_cap_per_door_session = file.verify_cap_per_door_session.unwrap_or(3);
    if verify_cap_per_door_session == 0 {
        return Err("verify_cap_per_door_session must be at least 1".to_owned());
    }
    Ok(AdmissionPolicy {
        policy_id,
        leg_rate_floor: RateFloor {
            p: file.leg_rate_floor.p,
            q: file.leg_rate_floor.q,
        },
        min_anchored_paths: file.min_anchored_paths,
        require_first_anchored: file.require_first_anchored,
        require_last_anchored: file.require_last_anchored,
        thorough_six_leg_runs: file.thorough_six_leg_runs,
        thorough_early_stop: file.thorough_early_stop,
        provisional: file.provisional,
        held_on_not_read: file.held_on_not_read,
        ban_key_on_fake_leg: file.ban_key_on_fake_leg,
        draw_commitment: file.draw_commitment,
        reverify_max_attempts,
        max_held_blocks,
        pending_per_source,
        pending_global,
        verify_cap_per_door_session,
    })
}

/// Absolute `http` or `https` URL, no trailing slash.
pub fn parse_index_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() || url.ends_with('/') {
        return Err(format!(
            "index_url {raw:?} must be an absolute http(s) origin with no trailing slash"
        ));
    }
    let Some((scheme, rest)) = url.split_once("://") else {
        return Err(format!(
            "index_url {raw:?} must be an absolute http(s) origin with no trailing slash"
        ));
    };
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return Err(format!(
            "index_url {raw:?} must be an absolute http(s) origin with no trailing slash"
        ));
    }
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = &rest[..host_end];
    if host.is_empty() || host.contains(' ') {
        return Err(format!(
            "index_url {raw:?} must be an absolute http(s) origin with no trailing slash"
        ));
    }
    Ok(url.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_documented_values() {
        let cfg = Config::default();
        assert_eq!(cfg.bind, "127.0.0.1:8787".parse().unwrap());
        assert_eq!(cfg.index_url, DEFAULT_INDEX_URL);
        assert_eq!(cfg.log_dir, PathBuf::from(DEFAULT_LOG_DIR));
        assert_eq!(cfg.max_object_bytes, DEFAULT_MAX_OBJECT_BYTES);
        assert_eq!(cfg.max_concurrent_verify, 2);
        assert_eq!(cfg.rate_per_min, 12);
        assert_eq!(cfg.policy.policy_id, "gl-admit-v4");
        assert_eq!(cfg.policy.leg_rate_floor.p, 1);
        assert_eq!(cfg.policy.leg_rate_floor.q, 5);
        assert_eq!(cfg.policy.min_anchored_paths, 6);
        assert_eq!(cfg.policy.require_first_anchored, 1);
        assert_eq!(cfg.policy.require_last_anchored, 16);
        assert_eq!(cfg.policy.thorough_six_leg_runs, 3);
        assert!(cfg.policy.thorough_early_stop);
        assert!(cfg.policy.provisional_on());
        assert!(cfg.policy.held_on_not_read);
        assert!(cfg.policy.ban_key_on_fake_leg);
        assert_eq!(cfg.policy.reverify_max_attempts, 3);
        assert_eq!(cfg.policy.max_held_blocks, 6);
        assert_eq!(cfg.policy.pending_per_source, 1);
        assert_eq!(cfg.policy.pending_global, 4);
        assert_eq!(cfg.bind_threads, 6);
        assert_eq!(cfg.index_timeout, Duration::from_secs(5));
        assert_eq!(
            cfg.allowed_origins,
            Vec::<String>::new()
        );
        assert!(cfg.client_ip_header.is_none());
        let raw = 260_000usize;
        let b64 = raw.saturating_add(2) / 3 * 4;
        assert!(cfg.max_body_bytes() >= b64 + JSON_ENVELOPE_BYTES || raw > cfg.max_object_bytes);
        let sized = Config {
            max_object_bytes: raw,
            ..Config::default()
        };
        assert!(sized.max_body_bytes() >= b64 + JSON_ENVELOPE_BYTES);
        assert!(sized.max_body_bytes() >= raw + MULTIPART_ENVELOPE_SLACK);
    }

    #[test]
    fn an_empty_file_uses_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-gate-cfg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gate.toml");
        fs::write(&path, "").unwrap();
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.bind, Config::default().bind);
        assert_eq!(cfg.policy.leg_rate_floor.q, 5);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_retired_flag_is_rejected() {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-gate-cfg-flag-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gate.toml");
        fs::write(&path, "friendly_list = false\n").unwrap();
        let error = Config::load(&path).unwrap_err();
        assert!(error.contains("friendly_list"), "{error}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_deploy_example_loads_the_documented_defaults() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("deploy/gate.toml");
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.bind, Config::default().bind);
        assert_eq!(cfg.index_url, DEFAULT_INDEX_URL);
        assert_eq!(cfg.log_dir, PathBuf::from(DEFAULT_LOG_DIR));
        assert_eq!(cfg.max_object_bytes, DEFAULT_MAX_OBJECT_BYTES);
        assert_eq!(cfg.rate_per_min, DEFAULT_RATE_PER_MIN);
        assert_eq!(cfg.policy, Config::default().policy);
        assert_eq!(cfg.bind_threads, 6);
        assert_eq!(
            cfg.allowed_origins,
            vec![
                "https://groundinglab.io".to_owned(),
                "https://www.groundinglab.io".to_owned()
            ]
        );
        assert!(cfg.client_ip_header.is_none());
        assert!(cfg.extra_origins.is_empty());
    }

    #[test]
    fn a_configured_fraction_is_applied() {
        let cfg = load_text(
            r#"
            [policy_table]
            policy_id = "gl-admit-v9"
            leg_rate_floor = { p = 7, q = 20 }
            min_anchored_paths = 8
            require_first_anchored = 1
            require_last_anchored = 14
            thorough_six_leg_runs = 2
            thorough_early_stop = false
            provisional = "off"
            held_on_not_read = false
            ban_key_on_fake_leg = false
            draw_commitment = "sha256(draw || salt), revealed with the result"
            "#,
        )
        .unwrap();
        assert_eq!(cfg.policy.policy_id, "gl-admit-v9");
        assert_eq!((cfg.policy.leg_rate_floor.p, cfg.policy.leg_rate_floor.q), (7, 20));
        assert_eq!(cfg.policy.min_anchored_paths, 8);
        assert_eq!(cfg.policy.require_last_anchored, 14);
        assert_eq!(cfg.policy.thorough_six_leg_runs, 2);
        assert!(!cfg.policy.thorough_early_stop);
        assert!(!cfg.policy.provisional_on());
        assert!(!cfg.policy.ban_key_on_fake_leg);
    }

    fn load_text(body: &str) -> Result<Config, String> {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-gate-cfg-text-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gate.toml");
        fs::write(&path, body).unwrap();
        let loaded = Config::load(&path);
        let _ = fs::remove_dir_all(&dir);
        loaded
    }

    #[test]
    fn an_empty_origin_list_does_not_restore_the_public_sites() {
        let cfg = load_text("allowed_origins = []\n").unwrap();
        assert!(cfg.allowed_origins.is_empty());
    }

    #[test]
    fn a_retired_floor_key_is_rejected() {
        let error = load_text("leg_rate_floor = \"strict\"\n").unwrap_err();
        assert!(error.contains("leg_rate_floor"), "{error}");
        let error = load_text("min_leg_rate = 0.25\n").unwrap_err();
        assert!(error.contains("min_leg_rate"), "{error}");
        let error = load_text("min_waypoints = 6\n").unwrap_err();
        assert!(error.contains("min_waypoints"), "{error}");
    }

    #[test]
    fn an_unknown_policy_key_is_rejected() {
        let error = load_text(
            r#"
            [policy_table]
            policy_id = "gl-admit-v4"
            leg_rate_floor = { p = 1, q = 5 }
            min_anchored_paths = 6
            require_first_anchored = 1
            require_last_anchored = 16
            thorough_six_leg_runs = 3
            thorough_early_stop = true
            provisional = "on"
            held_on_not_read = true
            ban_key_on_fake_leg = true
            draw_commitment = "x"
            surprise = 1
            "#,
        )
        .unwrap_err();
        assert!(error.contains("surprise"), "{error}");
    }

    #[test]
    fn index_url_rejects_a_trailing_slash() {
        assert!(parse_index_url("https://index.groundinglab.io").is_ok());
        assert!(parse_index_url("https://index.groundinglab.io/").is_err());
        assert!(parse_index_url("off").is_err());
    }
}
