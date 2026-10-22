//! CASES.md generator command.
#![deny(unsafe_code)]

fn main() {
    let mut args = std::env::args().skip(1);
    let mut out = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => {
                let dir = args.next().unwrap_or_else(|| {
                    eprintln!("--out needs a directory");
                    std::process::exit(2);
                });
                out = Some(std::path::PathBuf::from(dir));
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let out = out.unwrap_or_else(|| {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../candidates")
    });
    if let Err(err) = pcoc_gen::write_candidates(&out) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
