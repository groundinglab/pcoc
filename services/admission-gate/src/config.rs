//! One config file. The process does not read `GL_VERIFY_*`.
//!
//! Every key but `served_origins` and `policy` is optional. An omitted key
//! uses the default documented in `deploy/DEPLOY.md`. `served_origins` has no
//! default: a file without it is a startup error. `policy` is a path, relative
//! to this file, of a SPEC §15.4 policy file. There is no inline table and no
//! build-tree fallback. Unknown keys are an error, so a retired flag cannot
//! be turned back on by leaving it in the file.

use crate::policy::AdmissionPolicy;
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

/// Checkpoint-binding threads. The process does not read `PCOC_BIND_THREADS`.
pub const DEFAULT_BIND_THREADS: usize = 6;
pub const DEFAULT_INDEX_TIMEOUT_SECS: u64 = 5;
pub const DEFAULT_ALLOWED_ORIGINS: &[&str] = &[];

pub const DEFAULT_BIND: &str = "127.0.0.1:8787";
pub const DEFAULT_INDEX_URL: &str = "https://index.groundinglab.io";
pub const DEFAULT_LOG_DIR: &str = "gate-log-v2";
pub const DEFAULT_JOB_RETENTION_SECS: u64 = 3600;
pub const DEFAULT_MAX_JOBS: usize = 256;
pub const DEFAULT_MAX_CONCURRENT: usize = 2;
pub const DEFAULT_RATE_PER_MIN: u32 = 12;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: SocketAddr,
    /// Site origins this gate serves for sign-in (SPEC §15.1 **Origin**):
    /// the signed body's `origin` must equal one of these exactly. Required,
    /// no default. Separate from the CORS lists below.
    pub served_origins: Vec<String>,
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
    /// The full policy table. The shipped file is `gl-admit-v10`.
    pub policy: AdmissionPolicy,
    /// Checkpoint binding threads. The process does not read `PCOC_BIND_THREADS`.
    pub bind_threads: usize,
    /// One-shot index read. A timeout, a 5xx, or a refused connection is not retried.
    pub index_timeout: Duration,
    pub config_path: Option<PathBuf>,
    /// `test` or `live`. Empty means `live`. A live plane does not start
    /// unless [`Self::allow_2_2_live`] is set.
    pub plane: String,
    /// Opt in to running this 2.2 build where a live v2.0 service would be
    /// replaced. Defaults off. Nothing in the repo sets it.
    pub allow_2_2_live: bool,
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
    /// Required. `None` (the key is absent) is a startup error.
    served_origins: Option<Vec<String>>,
    allowed_origins: Vec<String>,
    extra_origins: Vec<String>,
    bind_threads: usize,
    index_timeout_secs: u64,
    /// Path of a SPEC §15.4 policy file, relative to this config file.
    /// Required. There is no inline table and no build-tree fallback.
    policy: Option<String>,
    /// `test` starts without the live-replacement opt-in. Empty means `live`.
    #[serde(default)]
    plane: String,
    /// Explicit opt-in. Default false. Do not set this to replace the live
    /// v2.0 admission service.
    #[serde(default)]
    allow_2_2_live: bool,
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
            served_origins: None,
            allowed_origins: DEFAULT_ALLOWED_ORIGINS
                .iter()
                .map(|origin| (*origin).to_owned())
                .collect(),
            extra_origins: Vec::new(),
            bind_threads: DEFAULT_BIND_THREADS,
            index_timeout_secs: DEFAULT_INDEX_TIMEOUT_SECS,
            policy: None,
            plane: String::new(),
            allow_2_2_live: false,
        }
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
            "admission-service policy: gl-admit-v1 float min_leg_rate is retired; active {} compares q·hike_legs ≥ p·hike_span with p/q = {}/{}",
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
        let served_origins = parse_served_origins(file.served_origins.clone())?;
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
            served_origins,
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
            plane: if file.plane.is_empty() {
                "live".to_owned()
            } else {
                file.plane
            },
            allow_2_2_live: file.allow_2_2_live,
        })
    }

    pub fn policy(&self) -> AdmissionPolicy {
        self.policy.clone()
    }

    /// A config for tests: empty temp log, limiter off, the given index
    /// origin, and [`TEST_SERVED_ORIGIN`] as the one served origin. The
    /// policy is [`AdmissionPolicy::default`], which matches gl-admit-v10,
    /// except `born_height` is 0 so fixtures such as O3 stay inside the
    /// boundary. A test that needs another boundary sets
    /// `policy.born_height` on the value it owns. The plane is `test`.
    /// A deploy never uses this, and it does not read a policy file from
    /// the build tree. Unit tests compile this function. Integration tests
    /// compile it through the `test-util` feature. A release build does not.
    #[cfg(any(test, feature = "test-util"))]
    pub fn for_tests(log_dir: PathBuf, index_url: &str) -> Self {
        let policy = AdmissionPolicy {
            born_height: 0,
            ..AdmissionPolicy::default()
        };
        Self {
            bind: DEFAULT_BIND.parse().expect("default bind"),
            served_origins: vec![TEST_SERVED_ORIGIN.to_owned()],
            allowed_origins: Vec::new(),
            extra_origins: Vec::new(),
            max_object_bytes: DEFAULT_MAX_OBJECT_BYTES,
            job_retention: Duration::from_secs(DEFAULT_JOB_RETENTION_SECS),
            max_jobs: DEFAULT_MAX_JOBS,
            max_concurrent_verify: DEFAULT_MAX_CONCURRENT,
            rate_per_min: 0,
            client_ip_header: None,
            index_url: index_url.to_owned(),
            log_dir,
            policy,
            bind_threads: DEFAULT_BIND_THREADS,
            index_timeout: Duration::from_secs(1),
            config_path: None,
            plane: "test".to_owned(),
            allow_2_2_live: false,
        }
    }
}

/// Startup refusal outside the test plane. A live plane replaces nothing
/// unless `allow_2_2_live` is set. The message names the live v2.0 service.
pub fn refuse_live_replacement(plane: &str, allow_2_2_live: bool) -> Result<(), String> {
    let plane = if plane.is_empty() { "live" } else { plane };
    if plane != "test" && !allow_2_2_live {
        return Err(
            "refusing to start: this build must not replace the live v2.0 admission service (set plane = \"test\", or allow_2_2_live = true to opt in)"
                .to_owned(),
        );
    }
    Ok(())
}

/// Served origin of [`Config::for_tests`]. The signed test vectors use it.
/// It is not a default: a config file must name its own `served_origins`.
pub const TEST_SERVED_ORIGIN: &str = "https://groundinglab.io";

/// `served_origins`: required, at least one, each an exact `http(s)://`
/// origin with no path and no trailing slash. Kept byte for byte.
fn parse_served_origins(raw: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let Some(raw) = raw else {
        return Err(
            "served_origins is required: list the exact site origins this gate serves for sign-in, e.g. served_origins = [\"https://site.example\"]; there is no default"
                .to_owned(),
        );
    };
    if raw.is_empty() {
        return Err("served_origins must name at least one site origin".to_owned());
    }
    for origin in &raw {
        let rest = origin
            .strip_prefix("https://")
            .or_else(|| origin.strip_prefix("http://"));
        let ok = rest.is_some_and(|host| {
            !host.is_empty() && !host.contains(['/', '?', '#', ' ']) && origin.trim() == origin
        });
        if !ok {
            return Err(format!(
                "served_origins entry {origin:?} must be an exact http(s) origin with no path or trailing slash"
            ));
        }
    }
    Ok(raw)
}

fn normalize_origins(origins: Vec<String>) -> Vec<String> {
    origins
        .into_iter()
        .map(|origin| origin.trim().trim_end_matches('/').to_owned())
        .filter(|origin| !origin.is_empty())
        .collect()
}

fn resolve_policy(file: &FileConfig, source: Option<&Path>) -> Result<AdmissionPolicy, String> {
    let Some(named) = file.policy.as_deref() else {
        return Err("policy path is required".to_owned());
    };
    if named.is_empty() {
        return Err("policy path is required".to_owned());
    }
    let Some(source) = source.and_then(|path| path.parent()) else {
        return Err("policy path is required".to_owned());
    };
    let path = source.join(named);
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("could not read policy {}: {err}", path.display()))?;
    crate::levers::load(&raw, file.max_concurrent)
        .map_err(|err| format!("policy {}: {err}", path.display()))
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

    /// Documented defaults. Built here, not loaded through a missing policy path.
    fn defaults() -> Config {
        Config {
            bind: "127.0.0.1:8787".parse().unwrap(),
            served_origins: vec!["https://site.example".to_owned()],
            allowed_origins: Vec::new(),
            extra_origins: Vec::new(),
            max_object_bytes: DEFAULT_MAX_OBJECT_BYTES,
            job_retention: Duration::from_secs(DEFAULT_JOB_RETENTION_SECS),
            max_jobs: DEFAULT_MAX_JOBS,
            max_concurrent_verify: DEFAULT_MAX_CONCURRENT,
            rate_per_min: DEFAULT_RATE_PER_MIN,
            client_ip_header: None,
            index_url: DEFAULT_INDEX_URL.to_owned(),
            log_dir: PathBuf::from(DEFAULT_LOG_DIR),
            policy: AdmissionPolicy::default(),
            bind_threads: DEFAULT_BIND_THREADS,
            index_timeout: Duration::from_secs(DEFAULT_INDEX_TIMEOUT_SECS),
            config_path: None,
            plane: "live".to_owned(),
            allow_2_2_live: false,
        }
    }

    #[test]
    fn defaults_match_the_documented_values() {
        let cfg = defaults();
        assert_eq!(cfg.bind, "127.0.0.1:8787".parse().unwrap());
        assert_eq!(cfg.index_url, DEFAULT_INDEX_URL);
        assert_eq!(cfg.log_dir, PathBuf::from(DEFAULT_LOG_DIR));
        assert_eq!(cfg.max_object_bytes, DEFAULT_MAX_OBJECT_BYTES);
        assert_eq!(cfg.max_concurrent_verify, 2);
        assert_eq!(cfg.rate_per_min, 12);
        assert_eq!(cfg.policy.policy_id, "gl-admit-v10");
        assert_eq!(cfg.policy.leg_rate_floor.p, 1);
        assert_eq!(cfg.policy.leg_rate_floor.q, 5);
        assert_eq!(cfg.policy.born_height, 972685);
        assert_eq!(cfg.policy.min_anchored_paths, 6);
        assert_eq!(cfg.policy.require_first_anchored, None);
        assert_eq!(cfg.policy.require_last_anchored, None);
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
        assert_eq!(cfg.allowed_origins, Vec::<String>::new());
        assert!(cfg.client_ip_header.is_none());
        let raw = 260_000usize;
        let b64 = raw.saturating_add(2) / 3 * 4;
        assert!(cfg.max_body_bytes() >= b64 + JSON_ENVELOPE_BYTES || raw > cfg.max_object_bytes);
        let sized = Config {
            max_object_bytes: raw,
            ..defaults()
        };
        assert!(sized.max_body_bytes() >= b64 + JSON_ENVELOPE_BYTES);
        assert!(sized.max_body_bytes() >= raw + MULTIPART_ENVELOPE_SLACK);
    }

    #[test]
    fn an_empty_file_is_an_error_and_a_missing_policy_path_is_a_startup_error() {
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
        let error = Config::load(&path).unwrap_err();
        assert!(error.contains("served_origins is required"), "{error}");
        fs::write(&path, "served_origins = [\"https://site.example\"]\n").unwrap();
        let error = Config::load(&path).unwrap_err();
        assert!(error.contains("policy path is required"), "{error}");
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
        assert_eq!(cfg.bind, defaults().bind);
        assert_eq!(cfg.index_url, DEFAULT_INDEX_URL);
        assert_eq!(cfg.log_dir, PathBuf::from(DEFAULT_LOG_DIR));
        assert_eq!(cfg.max_object_bytes, DEFAULT_MAX_OBJECT_BYTES);
        assert_eq!(cfg.rate_per_min, DEFAULT_RATE_PER_MIN);
        assert_eq!(cfg.policy, defaults().policy);
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
        assert_eq!(
            cfg.served_origins,
            vec!["https://groundinglab.io".to_owned()]
        );
        assert_eq!(cfg.plane, "live");
        assert!(!cfg.allow_2_2_live);
    }

    /// Defect 26 (gate review r2, finding 26). `served_origins` is required,
    /// has no default, and is separate from the CORS lists.
    #[test]
    fn served_origins_is_required_and_separate_from_cors() {
        let error = load_text("allowed_origins = [\"https://site.example\"]\n").unwrap_err();
        assert!(error.contains("served_origins is required"), "{error}");
        let error = load_text("served_origins = []\n").unwrap_err();
        assert!(error.contains("at least one"), "{error}");
        for bad in [
            "https://site.example/",
            "site.example",
            "https://site.example/path",
        ] {
            let error = load_text(&format!("served_origins = [{bad:?}]\n")).unwrap_err();
            assert!(error.contains("exact http(s) origin"), "{bad}: {error}");
        }
        let cfg = load_with_policy("served_origins = [\"https://site.example\"]\n").unwrap();
        assert_eq!(cfg.served_origins, vec!["https://site.example".to_owned()]);
        assert!(cfg.allowed_origins.is_empty(), "served is not CORS");
    }

    #[test]
    fn a_configured_fraction_is_applied() {
        let cfg = load_policy_file(
            r#"
            served_origins = ["https://site.example"]
            policy = "custom.toml"
            "#,
            r#"
            spec = "2.2"
            policy_id = "gl-admit-v10"
            [levers]
            min_anchored_paths = 8
            leg_rate_floor = { p = 7, q = 20 }
            held_retry_seconds = 30
            thorough_rounds = 2
            challenge_lifetime_seconds = 60
            challenge_clock_skew_seconds = 2
            result_retention_seconds = 90
            reverify_max_attempts = 2
            max_held_blocks = 4
            provisional = "off"
            fast_accept_bound = 1
            async_catch_budget = 3
            [policy_rules]
            expected_network = "main"
            draw_commitment = "sha256(draw || salt), revealed with the result"
            born_height = 0
            "#,
            "custom.toml",
        )
        .unwrap();
        assert_eq!(cfg.policy.policy_id, "gl-admit-v10");
        assert_eq!(
            (cfg.policy.leg_rate_floor.p, cfg.policy.leg_rate_floor.q),
            (7, 20)
        );
        assert_eq!(cfg.policy.min_anchored_paths, 8);
        assert_eq!(cfg.policy.thorough_six_leg_runs, 2);
        assert!(!cfg.policy.provisional_on());
        assert!(serde_json::to_value(&cfg.policy)
            .expect("policy json")
            .get("submission_deadline_blocks")
            .is_none());
        assert_eq!(cfg.policy.held_retry_seconds, 30);
        assert_eq!(cfg.policy.challenge_lifetime_seconds, 60);
        assert!(cfg.policy.thorough_early_stop);
        assert!(cfg.policy.require_first_anchored.is_none());
        assert_eq!(cfg.policy.born_height, 0);
        assert!(!cfg.policy.sticky_refuse);
    }

    fn load_text(body: &str) -> Result<Config, String> {
        let dir = temp_dir("text");
        let path = dir.join("gate.toml");
        fs::write(&path, body).unwrap();
        let loaded = Config::load(&path);
        let _ = fs::remove_dir_all(&dir);
        loaded
    }

    fn load_with_policy(body: &str) -> Result<Config, String> {
        let shipped = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../policy/groundinglab-admission/gl-admit-v10.toml");
        let raw = fs::read_to_string(shipped).unwrap();
        let mut gate = body.to_owned();
        if !gate.contains("policy") {
            gate.push_str("policy = \"policy.toml\"\n");
        }
        load_policy_file(&gate, &raw, "policy.toml")
    }

    fn load_policy_file(gate: &str, policy: &str, policy_name: &str) -> Result<Config, String> {
        let dir = temp_dir("policy");
        fs::write(dir.join(policy_name), policy).unwrap();
        let path = dir.join("gate.toml");
        fs::write(&path, gate).unwrap();
        let loaded = Config::load(&path);
        let _ = fs::remove_dir_all(&dir);
        loaded
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-gate-cfg-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_empty_origin_list_does_not_restore_the_public_sites() {
        let cfg =
            load_with_policy("served_origins = [\"https://site.example\"]\nallowed_origins = []\n")
                .unwrap();
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
        let error = load_policy_file(
            "served_origins = [\"https://site.example\"]\npolicy = \"bad.toml\"\n",
            "spec = \"2.2\"\npolicy_id = \"gl-admit-v10\"\n[levers]\nsurprise = 1\n",
            "bad.toml",
        )
        .unwrap_err();
        assert!(error.contains("surprise"), "{error}");
    }

    #[test]
    fn a_live_plane_refuses_to_replace_the_live_service() {
        let error = refuse_live_replacement("live", false).unwrap_err();
        assert!(error.contains("live v2.0 admission service"), "{error}");
        assert!(refuse_live_replacement("test", false).is_ok());
        assert!(refuse_live_replacement("live", true).is_ok());
        assert!(refuse_live_replacement("", false).is_err());
    }

    #[test]
    fn index_url_rejects_a_trailing_slash() {
        assert!(parse_index_url("https://index.groundinglab.io").is_ok());
        assert!(parse_index_url("https://index.groundinglab.io/").is_err());
        assert!(parse_index_url("off").is_err());
    }
}
