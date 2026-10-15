//! Hourly snapshot. Integrity check, then age encryption of the snapshot and
//! the issuer key. Ciphertext is what leaves the box.

use crate::config::{Config, Role};
use crate::key::check_mode;
use crate::s3::{read_token, R2Client};
use crate::timeutil::utc_basic;
use rusqlite::Connection;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

pub struct Sealed {
    pub stamp: String,
    pub snapshot_age: PathBuf,
    pub key_age: PathBuf,
}

/// Snapshot, check, encrypt, and keep the ciphertext under `backup_dir`.
/// The plaintext snapshot is removed after encryption.
pub fn seal(cfg: &Config) -> Result<Sealed, String> {
    if cfg.role != Role::Writer {
        return Err("backup runs on the writer".into());
    }
    check_mode(&cfg.issuer_key_file)?;
    let recipient = read_recipient(&cfg.backup_age_recipient_file)?;
    fs::create_dir_all(&cfg.backup_dir)
        .map_err(|err| format!("could not create backup dir: {err}"))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock is before the unix epoch".to_owned())?
        .as_secs() as i64;
    let stamp = utc_basic(now);
    let plain = cfg.backup_dir.join(format!("claims-{stamp}.sqlite"));
    snapshot_db(&cfg.db_path, &plain)?;
    let plain_bytes = fs::read(&plain).map_err(|err| format!("could not read snapshot: {err}"))?;
    let key_bytes = Zeroizing::new(
        fs::read(&cfg.issuer_key_file)
            .map_err(|err| format!("could not read issuer key file: {err}"))?,
    );
    let snapshot_cipher = age_encrypt(&recipient, &plain_bytes)?;
    let key_cipher = age_encrypt(&recipient, &key_bytes)?;
    drop(key_bytes);
    fs::remove_file(&plain).map_err(|err| format!("could not remove plaintext snapshot: {err}"))?;
    let snapshot_age = cfg.backup_dir.join(format!("claims-{stamp}.sqlite.age"));
    let key_age = cfg.backup_dir.join(format!("issuer-{stamp}.key.age"));
    fs::write(&snapshot_age, snapshot_cipher)
        .map_err(|err| format!("could not write snapshot ciphertext: {err}"))?;
    fs::write(&key_age, key_cipher)
        .map_err(|err| format!("could not write key ciphertext: {err}"))?;
    prune(&cfg.backup_dir, cfg.backup_keep)?;
    Ok(Sealed {
        stamp,
        snapshot_age,
        key_age,
    })
}

/// Seal, then PUT both ciphertext objects to `groundinglab-claims-backup`.
pub async fn run(cfg: &Config) -> Result<Sealed, String> {
    let sealed = seal(cfg)?;
    if cfg.r2_account_id.is_empty() {
        return Err("backup needs r2_account_id for groundinglab-claims-backup".into());
    }
    let client = R2Client::new(
        &cfg.r2_account_id,
        &cfg.backup_r2_bucket,
        &cfg.backup_r2_token_file,
        cfg.r2_timeout,
    )?;
    let _ = read_token(&cfg.backup_r2_token_file)?;
    let snap = fs::read(&sealed.snapshot_age)
        .map_err(|err| format!("could not read snapshot ciphertext: {err}"))?;
    let key =
        fs::read(&sealed.key_age).map_err(|err| format!("could not read key ciphertext: {err}"))?;
    let snap_key = format!("claims-{}.sqlite.age", sealed.stamp);
    let key_key = format!("issuer-{}.key.age", sealed.stamp);
    client
        .put(&snap_key, &snap, "application/octet-stream")
        .await?;
    client
        .put(&key_key, &key, "application/octet-stream")
        .await?;
    tracing::info!(interval_secs = cfg.backup_interval_secs, bucket = %cfg.backup_r2_bucket, "backup uploaded");
    Ok(sealed)
}

fn read_recipient(path: &Path) -> Result<String, String> {
    let raw =
        fs::read_to_string(path).map_err(|err| format!("could not read age recipient: {err}"))?;
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'));
    let Some(line) = line else {
        return Err("age recipient file is empty".into());
    };
    if !line.starts_with("age1") || line.len() > 200 {
        return Err("age recipient file is not an age1 recipient".into());
    }
    Ok(line.to_owned())
}

fn snapshot_db(src_path: &Path, dest: &Path) -> Result<(), String> {
    let src =
        Connection::open(src_path).map_err(|err| format!("could not open database: {err}"))?;
    let mut dst =
        Connection::open(dest).map_err(|err| format!("could not create snapshot: {err}"))?;
    {
        let backup = rusqlite::backup::Backup::new(&src, &mut dst)
            .map_err(|err| format!("could not start snapshot: {err}"))?;
        backup
            .step(-1)
            .map_err(|err| format!("snapshot failed: {err}"))?;
    }
    let check: String = dst
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|err| format!("integrity_check failed: {err}"))?;
    if check != "ok" {
        let _ = fs::remove_file(dest);
        return Err(format!("integrity_check returned {check}"));
    }
    Ok(())
}

pub fn age_encrypt(recipient_line: &str, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let recipient: age::x25519::Recipient = recipient_line
        .trim()
        .parse()
        .map_err(|_| "age recipient is not usable".to_owned())?;
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|_| "age recipient was rejected".to_owned())?;
    let mut out = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut out)
        .map_err(|_| "age encrypt failed".to_owned())?;
    writer
        .write_all(plaintext)
        .map_err(|_| "age encrypt failed".to_owned())?;
    writer
        .finish()
        .map_err(|_| "age encrypt failed".to_owned())?;
    Ok(out)
}

fn prune(dir: &Path, keep: usize) -> Result<(), String> {
    let mut snaps = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| format!("could not read backup dir: {err}"))? {
        let entry = entry.map_err(|err| format!("could not read backup dir: {err}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with("claims-") && name.ends_with(".sqlite.age") {
            snaps.push(name.to_owned());
        }
    }
    snaps.sort();
    if snaps.len() <= keep {
        return Ok(());
    }
    for name in snaps.into_iter().rev().skip(keep) {
        let stamp = name
            .trim_start_matches("claims-")
            .trim_end_matches(".sqlite.age");
        let _ = fs::remove_file(dir.join(&name));
        let _ = fs::remove_file(dir.join(format!("issuer-{stamp}.key.age")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn snapshot_round_trips_under_age() {
        let dir = std::env::temp_dir().join(format!("claims-backup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("claims.sqlite");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE badge (id INTEGER PRIMARY KEY); INSERT INTO badge (id) VALUES (7);",
        )
        .unwrap();
        drop(conn);
        let snap = dir.join("snap.sqlite");
        snapshot_db(&db_path, &snap).unwrap();
        let opened = Connection::open(&snap).unwrap();
        let count: i64 = opened
            .query_row("SELECT COUNT(*) FROM badge", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public().to_string();
        let cipher = age_encrypt(&recipient, b"badge-snapshot").unwrap();
        assert!(!cipher.windows(14).any(|window| window == b"badge-snapshot"));
        let decrypted = decrypt(&identity, &cipher);
        assert_eq!(decrypted, b"badge-snapshot");
        let _ = fs::remove_dir_all(&dir);
    }

    fn decrypt(identity: &age::x25519::Identity, cipher: &[u8]) -> Vec<u8> {
        let decryptor = age::Decryptor::new(cipher).unwrap();
        let mut reader = decryptor
            .decrypt(std::iter::once(identity as &dyn age::Identity))
            .unwrap();
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        out
    }
}
