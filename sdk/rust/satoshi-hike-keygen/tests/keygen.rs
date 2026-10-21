//! The secret matches the published key, and a terminal does not receive it.
#![forbid(unsafe_code)]

use std::process::Command;

#[test]
fn piped_stdout_matches_the_pubkey_file_and_is_not_on_stderr() {
    let dir = std::env::temp_dir().join(format!(
        "pcoc-keygen-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let pub_out = dir.join("pubkey.txt");
    let bin = env!("CARGO_BIN_EXE_satoshi-hike-keygen");
    let run = Command::new(bin)
        .args(["--pub-out", pub_out.to_str().unwrap()])
        .output()
        .expect("spawn keygen");
    assert!(run.status.success(), "stderr {}", String::from_utf8_lossy(&run.stderr));
    let stdout = String::from_utf8(run.stdout).unwrap();
    let secret_hex = stdout.trim();
    assert_eq!(secret_hex.len(), 64);
    assert!(secret_hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let published = std::fs::read_to_string(&pub_out).unwrap();
    let public_hex = published
        .lines()
        .find_map(|line| line.strip_prefix("public_key="))
        .expect("public_key line");
    let npub = published
        .lines()
        .find_map(|line| line.strip_prefix("npub="))
        .expect("npub line");
    let secret = nip06::decode_hex32(secret_hex).unwrap();
    let derived = nip06::public_key_of_secret(&secret).unwrap();
    assert_eq!(nip06::encode_hex(&derived), public_hex);
    assert_eq!(nip06::npub_of(&derived), npub);
    let stderr = String::from_utf8(run.stderr).unwrap();
    assert!(!stderr.contains(secret_hex), "the secret was on stderr");
    assert!(!published.contains(secret_hex), "the secret was in the pubkey file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_terminal_stdout_is_refused_and_writes_nothing() {
    let dir = std::env::temp_dir().join(format!(
        "pcoc-keygen-tty-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let pub_out = dir.join("pubkey.txt");
    let bin = env!("CARGO_BIN_EXE_satoshi-hike-keygen");
    let script = r#"
import os, pty, subprocess, sys
bin, pub_out = sys.argv[1], sys.argv[2]
master, slave = pty.openpty()
proc = subprocess.Popen([bin, "--pub-out", pub_out], stdout=slave, stderr=subprocess.PIPE)
os.close(slave)
proc.wait()
try:
    data = os.read(master, 256)
except OSError:
    data = b""
os.close(master)
err = proc.stderr.read()
if data:
    sys.stderr.write(b"terminal received bytes\n")
    raise SystemExit(3)
sys.stderr.buffer.write(err)
raise SystemExit(proc.returncode)
"#;
    let run = Command::new("python3")
        .args(["-c", script, bin, pub_out.to_str().unwrap()])
        .output()
        .expect("python pty");
    assert_eq!(run.status.code(), Some(2), "{}", String::from_utf8_lossy(&run.stderr));
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("terminal"), "{stderr}");
    assert!(!pub_out.exists(), "a refused run wrote the pubkey file");
    let _ = std::fs::remove_dir_all(&dir);
}
