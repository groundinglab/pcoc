//! One config file. The process does not read `GL_VERIFY_*`.
//!
//! Every key is optional. An omitted key uses the default documented in
//! `deploy/DEPLOY.md`. Unknown keys are an error, so a retired flag cannot
//! be turned back on by leaving it in the file.

use crate::policy::{self, AdmissionPolicy};
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
/// Locked default floor. The file names `strict`, `default`, or `lenient`.
pub const DEFAULT_LEG_RATE_FLOOR: &str = "default";

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
    /// `strict`, `default`, or `lenient`. Used when a provisional line resumes.
    pub leg_rate_floor: String,
    pub leg_rate_floor_p: u64,
    pub leg_rate_floor_q: u64,
    /// `gl-admit-vN`. Bound to [`Self::min_leg_rate`] and [`Self::min_waypoints`].
    /// A change of either number requires a new id.
    pub policy_id: String,
    /// Minimum measured leg rate, leg/s. Default 0.25.
    pub min_leg_rate: f64,
    /// Minimum waypoints fetched and validated, of 18. Default 6.
    pub min_waypoints: u64,
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
    leg_rate_floor: String,
    bind_threads: usize,
    index_timeout_secs: u64,
    /// Immutable id for the floor below. A value change requires a new id.
    policy_id: String,
    /// Minimum measured leg rate, leg/s.
    min_leg_rate: f64,
    /// Minimum waypoints fetched and validated, of 18.
    min_waypoints: u64,
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
            leg_rate_floor: DEFAULT_LEG_RATE_FLOOR.to_owned(),
            bind_threads: DEFAULT_BIND_THREADS,
            index_timeout_secs: DEFAULT_INDEX_TIMEOUT_SECS,
            policy_id: policy::DEFAULT_POLICY_ID.to_owned(),
            min_leg_rate: policy::DEFAULT_MIN_LEG_RATE,
            min_waypoints: policy::DEFAULT_MIN_WAYPOINTS,
        }
    }
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
        let (floor_name, floor_p, floor_q) = parse_leg_rate_floor(&file.leg_rate_floor)?;
        let policy_id = policy::parse_policy_id(&file.policy_id)?;
        if !(file.min_leg_rate.is_finite() && file.min_leg_rate > 0.0) {
            return Err("min_leg_rate must be a positive finite number".to_owned());
        }
        if file.min_waypoints > policy::WAYPOINTS_TOTAL {
            return Err(format!(
                "min_waypoints must be at most {}",
                policy::WAYPOINTS_TOTAL
            ));
        }
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
            leg_rate_floor: floor_name,
            leg_rate_floor_p: floor_p,
            leg_rate_floor_q: floor_q,
            policy_id,
            min_leg_rate: file.min_leg_rate,
            min_waypoints: file.min_waypoints,
            bind_threads: file.bind_threads,
            index_timeout: Duration::from_secs(file.index_timeout_secs),
            config_path: source,
        })
    }

    pub fn policy(&self) -> AdmissionPolicy {
        AdmissionPolicy {
            policy_id: self.policy_id.clone(),
            min_leg_rate: self.min_leg_rate,
            min_waypoints: self.min_waypoints,
        }
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

/// `strict` is 7/20, `default` is 1/4, `lenient` is 1/8.
pub fn parse_leg_rate_floor(raw: &str) -> Result<(String, u64, u64), String> {
    let name = raw.trim();
    let pair = match name {
        "strict" => hike::LEG_RATE_FLOOR_STRICT,
        "default" => hike::LEG_RATE_FLOOR_DEFAULT,
        "lenient" => hike::LEG_RATE_FLOOR_LENIENT,
        _ => {
            return Err(format!(
                "leg_rate_floor {raw:?} must be strict, default, or lenient"
            ))
        }
    };
    Ok((name.to_owned(), pair.0, pair.1))
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
        assert_eq!(cfg.leg_rate_floor, "default");
        assert_eq!(cfg.leg_rate_floor_p, 1);
        assert_eq!(cfg.leg_rate_floor_q, 4);
        assert_eq!(cfg.policy_id, "gl-admit-v1");
        assert_eq!(cfg.min_leg_rate, 0.25);
        assert_eq!(cfg.min_waypoints, 6);
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
        assert_eq!(cfg.leg_rate_floor_q, 4);
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
        assert_eq!(cfg.leg_rate_floor, "default");
        assert_eq!(cfg.leg_rate_floor_p, 1);
        assert_eq!(cfg.leg_rate_floor_q, 4);
        assert_eq!(cfg.policy_id, "gl-admit-v1");
        assert_eq!(cfg.min_leg_rate, 0.25);
        assert_eq!(cfg.min_waypoints, 6);
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
    fn named_floors_resolve_to_the_locked_fractions() {
        assert_eq!(
            parse_leg_rate_floor("strict").unwrap(),
            ("strict".to_owned(), 7, 20)
        );
        assert_eq!(
            parse_leg_rate_floor("default").unwrap(),
            ("default".to_owned(), 1, 4)
        );
        assert_eq!(
            parse_leg_rate_floor("lenient").unwrap(),
            ("lenient".to_owned(), 1, 8)
        );
        assert!(parse_leg_rate_floor("1/4").is_err());
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
    fn a_named_floor_in_the_file_is_the_floor() {
        let cfg = load_text("leg_rate_floor = \"strict\"\n").unwrap();
        assert_eq!(cfg.leg_rate_floor_p, 7);
        assert_eq!(cfg.leg_rate_floor_q, 20);
        let cfg = load_text("leg_rate_floor = \"lenient\"\n").unwrap();
        assert_eq!((cfg.leg_rate_floor_p, cfg.leg_rate_floor_q), (1, 8));
    }

    #[test]
    fn index_url_rejects_a_trailing_slash() {
        assert!(parse_index_url("https://index.groundinglab.io").is_ok());
        assert!(parse_index_url("https://index.groundinglab.io/").is_err());
        assert!(parse_index_url("off").is_err());
    }
}
