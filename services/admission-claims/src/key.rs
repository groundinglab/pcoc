//! Issuer key. The secret is never formatted. The file must not be readable
//! by group or other. The derived public key must match the pin.

use crate::config::{Config, Role, DEFAULT_ISSUER_KEY_FILE};
use crate::nostr::{parse_secret, pubkey_hex};
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use zeroize::Zeroizing;

pub struct IssuerKey {
    secret: Zeroizing<[u8; 32]>,
    pubkey_hex: String,
}

impl fmt::Debug for IssuerKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IssuerKey([redacted])")
    }
}

impl IssuerKey {
    pub fn pubkey_hex(&self) -> &str {
        &self.pubkey_hex
    }

    pub fn secret(&self) -> &[u8; 32] {
        &self.secret
    }

    pub fn load(path: &Path, pin: &str) -> Result<Self, String> {
        check_mode(path)?;
        let mut file =
            File::open(path).map_err(|err| format!("could not open issuer key file: {err}"))?;
        let mut buf = Zeroizing::new(Vec::new());
        file.read_to_end(&mut buf)
            .map_err(|err| format!("could not read issuer key file: {err}"))?;
        if buf.len() > 256 {
            return Err("issuer key file is too long".into());
        }
        let text = String::from_utf8(buf.to_vec())
            .map_err(|_| "issuer key file is not text".to_owned())?;
        let secret = parse_secret(&text)
            .ok_or_else(|| "issuer key file is not 64 hex characters or an nsec".to_owned())?;
        // `text` held the secret. Drop it before any further work.
        drop(text);
        let pubkey = pubkey_hex(&secret)?;
        if pubkey != pin {
            return Err("issuer key does not match issuer_pubkey_hex".into());
        }
        Ok(Self {
            secret,
            pubkey_hex: pubkey,
        })
    }
}

/// Group and other must have no permission bits. `0600` and a narrower owner
/// mode pass. `0640` and `0644` do not.
pub fn check_mode(path: &Path) -> Result<(), String> {
    let meta =
        fs::metadata(path).map_err(|err| format!("could not stat issuer key file: {err}"))?;
    if !meta.is_file() {
        return Err("issuer key file is not a regular file".into());
    }
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(format!(
            "issuer key file mode is {mode:04o}; group and other must have no access"
        ));
    }
    Ok(())
}

/// A mirror exits when a key file is present at the configured path or at the
/// default path. A writer loads the configured file.
pub fn load_for_role(cfg: &Config) -> Result<Option<IssuerKey>, String> {
    match cfg.role {
        Role::Mirror => {
            if key_present(&cfg.issuer_key_file) || key_present(Path::new(DEFAULT_ISSUER_KEY_FILE))
            {
                return Err("mirror refuses to start while an issuer key file is present".into());
            }
            Ok(None)
        }
        Role::Writer => {
            let key = IssuerKey::load(&cfg.issuer_key_file, &cfg.issuer_pubkey_hex)?;
            Ok(Some(key))
        }
    }
}

fn key_present(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    fn write_mode(path: &Path, body: &str, mode: u32) {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(mode)
            .open(path)
            .unwrap();
        file.write_all(body.as_bytes()).unwrap();
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(mode);
        fs::set_permissions(path, perms).unwrap();
    }

    #[test]
    fn wide_mode_and_a_wrong_pin_refuse_to_load() {
        let dir = std::env::temp_dir().join(format!("claims-key-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("issuer.key");
        let scalar = "0000000000000000000000000000000000000000000000000000000000000001";
        write_mode(&path, scalar, 0o644);
        let err = IssuerKey::load(&path, "ab").unwrap_err();
        assert!(err.contains("mode"), "{err}");
        assert!(!err.contains(scalar));
        write_mode(&path, scalar, 0o600);
        let pin = pubkey_hex(&parse_secret(scalar).unwrap()).unwrap();
        let err = IssuerKey::load(&path, &"ab".repeat(32)).unwrap_err();
        assert!(err.contains("does not match"), "{err}");
        assert!(!err.contains(scalar));
        let loaded = IssuerKey::load(&path, &pin).unwrap();
        assert_eq!(loaded.pubkey_hex(), pin);
        assert_eq!(format!("{loaded:?}"), "IssuerKey([redacted])");
        assert!(!format!("{loaded:?}").contains(scalar));
        let _ = fs::remove_dir_all(&dir);
    }
}
