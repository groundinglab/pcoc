//! Fresh test keys for tonight's hikes.
//!
//! The secret hex is written to stdout and nowhere else. A terminal on stdout
//! is refused, so the secret is not echoed. Pipe stdout into `age`.
#![forbid(unsafe_code)]

use std::io::{IsTerminal, Write};
use std::path::Path;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(pub_out) = pub_out(&args) else {
        usage();
        return 2;
    };
    if std::io::stdout().is_terminal() {
        eprintln!("satoshi-hike-keygen: stdout is a terminal; refusing to print the secret");
        return 2;
    }
    let key = match nip06::fresh_test_key() {
        Ok(key) => key,
        Err(err) => {
            eprintln!("satoshi-hike-keygen: {err}");
            return 1;
        }
    };
    let text = format!(
        "public_key={}\nnpub={}\n",
        key.public_key_hex(),
        nip06::npub_of(&key.public_key())
    );
    if let Err(err) = write_pub(Path::new(&pub_out), &text) {
        eprintln!("satoshi-hike-keygen: {err}");
        return 1;
    }
    let hex = nip06::encode_hex(key.secret_bytes());
    let mut stdout = std::io::stdout();
    if writeln!(stdout, "{hex}").is_err() || stdout.flush().is_err() {
        eprintln!("satoshi-hike-keygen: could not write stdout");
        return 1;
    }
    0
}

fn pub_out(args: &[String]) -> Option<String> {
    if args.len() == 2 && args[0] == "--pub-out" && !args[1].is_empty() && !args[1].starts_with('-')
    {
        return Some(args[1].clone());
    }
    if args.len() == 1 {
        if let Some(value) = args[0].strip_prefix("--pub-out=") {
            if !value.is_empty() {
                return Some(value.to_owned());
            }
        }
    }
    None
}

fn write_pub(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err(format!("{} is not a directory", parent.display()));
        }
    }
    std::fs::write(path, text).map_err(|err| format!("{}: {err}", path.display()))
}

fn usage() {
    eprintln!(
        "usage: satoshi-hike-keygen --pub-out FILE\n       FILE receives the x-only public key hex and the npub.\n       The secret hex is printed on stdout only. A terminal stdout is refused.\n       Pipe stdout into age. The producer takes --pubkey-hex and does not read this secret."
    );
}
