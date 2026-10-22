//! The few real preferences. The file has no field for an nsec, so a secret
//! cannot be written here. Endpoints live in [`crate::endpoints`].

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const THREADS_MIN: u32 = 1;
pub const THREADS_MAX: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// `None` until the user picks. Dark is the default for a new install.
    pub theme: Option<Theme>,
    /// On for a first launch, then remembered.
    pub learn_mode: bool,
    pub data_folder: PathBuf,
    pub threads: u32,
    /// `host:port`. Stored for a later header source. This build does not dial it.
    pub gossip_peer: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: None,
            learn_mode: true,
            data_folder: home().join("PCOC").join("hikes"),
            threads: 6,
            gossip_peer: String::new(),
        }
    }
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// `$XDG_CONFIG_HOME/pcoc-hike/config.json`. Inside the Flatpak sandbox
/// `XDG_CONFIG_HOME` is the app's own directory.
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".config"));
    base.join("pcoc-hike").join("config.json")
}

impl Config {
    pub fn effective_theme(&self) -> Theme {
        self.theme.unwrap_or(Theme::Dark)
    }

    pub fn threads(&self) -> usize {
        self.threads.clamp(THREADS_MIN, THREADS_MAX) as usize
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Config>(&text).ok())
            .map(|mut c| {
                c.threads = c.threads.clamp(THREADS_MIN, THREADS_MAX);
                c
            })
            .unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("config serializes");
        text.push('\n');
        text
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, self.to_json())?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::SecretKey;

    #[test]
    fn defaults_match_the_design() {
        let c = Config::default();
        assert_eq!(c.effective_theme(), Theme::Dark);
        assert!(c.learn_mode);
        assert_eq!(c.threads(), 6);
        assert!(c.data_folder.ends_with("PCOC/hikes"));
        assert!(c.gossip_peer.is_empty());
    }

    #[test]
    fn learn_toggle_round_trip_keeps_the_other_settings() {
        let dir = std::env::temp_dir().join(format!("pcoc-learn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let c = Config {
            threads: 4,
            learn_mode: false,
            ..Config::default()
        };
        c.save(&path).unwrap();
        let mut loaded = Config::load(&path);
        assert!(!loaded.learn_mode);
        assert_eq!(loaded.threads(), 4);
        loaded.learn_mode = true;
        loaded.save(&path).unwrap();
        let again = Config::load(&path);
        assert!(again.learn_mode);
        assert_eq!(again.threads(), 4);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("nsec"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_config_never_holds_a_secret() {
        let dir = std::env::temp_dir().join(format!("pcoc-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let secret = SecretKey::from_nsec(crate::nip19_vector::NIP19_NSEC).unwrap();
        let nsec = secret.to_nsec();
        let config_path = dir.join("config.json");
        Config::default().save(&config_path).unwrap();
        let text = std::fs::read_to_string(&config_path).unwrap();
        assert!(!text.contains("nsec"), "{text}");
        assert!(!text.contains(nsec.as_str()));
        assert!(!text.contains("npub"));

        let planted = dir.join("planted.json");
        std::fs::write(
            &planted,
            format!(
                "{{\"nsec\":\"{}\",\"data_folder\":\"/tmp/pcoc-hikes\",\"threads\":4}}",
                nsec.as_str()
            ),
        )
        .unwrap();
        let loaded = Config::load(&planted);
        assert_eq!(loaded.data_folder, PathBuf::from("/tmp/pcoc-hikes"));
        assert_eq!(loaded.threads(), 4);
        let saved = loaded.to_json();
        assert!(!saved.contains("nsec"));
        assert!(!saved.contains(nsec.as_str()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn junk_and_out_of_range_fall_back() {
        let dir = std::env::temp_dir().join(format!("pcoc-config-junk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.json");
        std::fs::write(&p, "{\"threads\": 99, \"theme\": \"light\"}").unwrap();
        let c = Config::load(&p);
        assert_eq!(c.threads(), 32);
        assert_eq!(c.effective_theme(), Theme::Light);
        assert!(c.learn_mode);
        std::fs::write(&p, "not json").unwrap();
        assert_eq!(Config::load(&p), Config::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
