//! Pinned round-2 candidate files and objects.
//! The agreed SHA-256 values are literals. Callers regenerate the files and
//! verify the objects from this checkout. Nothing is read from beside the repository.

use pcoc::hash::sha256;
use pcoc::hexutil::hex_encode;
use pcoc::profile::Profile;
use pcoc::verify::{HeightIndex, Verified};
use pcoc_testing::{cases_document, verify_from_bytes, LabelChain};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// SHA-256 of each committed `sdk/rust/candidates/*.txt`, lowercase hex.
pub const CANDIDATES: &[(&str, &str)] = &[
    (
        "anchor.txt",
        "fe58c0b64263d11daa7dcbae7c6daa3e0b0daecb1084b44d47f6f2570da3811a",
    ),
    (
        "draw.txt",
        "45821e7ed5d9bbb6b03c686dfc467c623203027a6adf5160414c4e4074a85e0c",
    ),
    (
        "fold.txt",
        "2f14b5ef6953e00673c4c7f124f0057cb5498f17d354057639d944e441ffd271",
    ),
    (
        "hashcash.txt",
        "92f010b46c15483877343f635164c485500e55527721305bedd0a34def7ca9d5",
    ),
    (
        "kat-small.txt",
        "f999097217af06b00a8b879dfc9422263f63cc80b1462d41ff64d09ea866b192",
    ),
    (
        "negatives.txt",
        "a236c2966ae46b3e2a9bd83f4414fb4cf8f166106ce0553443aa4bc2e91462d4",
    ),
    (
        "nl4.txt",
        "f3fc7424ecd76dbe8ca7976d2fc780bb2a448206e6f116defa4b7cb67fb67a0b",
    ),
    (
        "prod.txt",
        "0eb0a567aa73350b80822c86ea3b3ec71ce8f3078bb59012019662cb5850c995",
    ),
    (
        "rate.txt",
        "153fbec4acbd3678953f4420d4e5af2f405e866ea16361cc50ab8142cfeef34f",
    ),
    (
        "seed.txt",
        "10179736fe1394a3275ad010ad87d5db03214ab3993d94d7cd27ec5c43802002",
    ),
    (
        "terrain.txt",
        "f5664d41b9bdc811adc2d7b321f518d33b13808746cf7016837948269019e348",
    ),
    (
        "test-3leg.txt",
        "621277600af9a31e40f244ac6010dbe059e602b3b48e9b92a8ad0c0b6d8f7eea",
    ),
    (
        "unknown-kind.txt",
        "351fe1fa59d6acd69039ca08704720dba7505155f0d5b26a7bb927321fbcccc2",
    ),
    (
        "wes.txt",
        "91c24b4c0e64b5e75bbe5422341cf01e09b25a12833196ac9ed8b898b5f6d768",
    ),
];

/// SHA-256 of each committed `sdk/rust/objects/*.cbor`, lowercase hex.
pub const OBJECTS: &[(&str, &str)] = &[
    (
        "O1-S1.cbor",
        "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246",
    ),
    (
        "O2-S2.cbor",
        "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0",
    ),
    (
        "O3-PROD.cbor",
        "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7",
    ),
];

/// PROD stage 0 leg 0 `legs_le_hex`.
pub const PROD_LEG_0: &str = "88a04ac5beacb8a7";

fn read_bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn file_names(dir: &Path) -> Vec<String> {
    let rd = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    let mut names = Vec::new();
    for ent in rd {
        let ent = ent.unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        let typ = ent
            .file_type()
            .unwrap_or_else(|e| panic!("type {}: {e}", dir.display()));
        if !typ.is_file() {
            continue;
        }
        let name = ent.file_name();
        let name = name
            .to_str()
            .unwrap_or_else(|| panic!("filename in {}", dir.display()));
        names.push(name.to_string());
    }
    names.sort();
    names
}

fn pinned_names(table: &[(&str, &str)]) -> Vec<String> {
    let mut names: Vec<String> = table.iter().map(|(name, _)| (*name).to_string()).collect();
    names.sort();
    names
}

/// Committed candidate files, their SHA-256 pins, the PROD leg pin, and the object pins.
pub fn assert_committed_pins(root: &Path) {
    assert_eq!(CANDIDATES.len(), 14, "candidate pin count");
    assert_eq!(OBJECTS.len(), 3, "object pin count");
    let cand = root.join("sdk/rust/candidates");
    assert_eq!(
        file_names(&cand),
        pinned_names(CANDIDATES),
        "candidate files"
    );
    for (name, pin) in CANDIDATES {
        let got = hex_encode(&sha256(&read_bytes(&cand.join(name))));
        assert_eq!(got.as_str(), *pin, "candidate pin {name}");
    }
    let prod = String::from_utf8(read_bytes(&cand.join("prod.txt"))).expect("prod.txt utf-8");
    let line = format!("prod.PROD.stage.0.leg.0.legs_le_hex\t{PROD_LEG_0}\n");
    assert!(
        prod.contains(&line),
        "PROD leg pin prod.PROD.stage.0.leg.0.legs_le_hex"
    );

    let obj = root.join("sdk/rust/objects");
    assert_eq!(file_names(&obj), pinned_names(OBJECTS), "object files");
    for (name, pin) in OBJECTS {
        let got = hex_encode(&sha256(&read_bytes(&obj.join(name))));
        assert_eq!(got.as_str(), *pin, "object pin {name}");
    }
}

/// Two generations, byte-identical to each other and to the committed files.
/// `write` is one generator run into the directory it is given.
pub fn assert_regenerated_candidates(root: &Path, mut write: impl FnMut(&Path)) {
    let committed = root.join("sdk/rust/candidates");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let scratch =
        std::env::temp_dir().join(format!("pcoc-candidates-{}-{nanos}", std::process::id()));
    let again = scratch.join("again");
    let _ = std::fs::remove_dir_all(&scratch);
    let guard = Scratch(scratch.clone());
    write(&scratch);
    write(&again);
    for (name, _) in CANDIDATES {
        let got = read_bytes(&scratch.join(name));
        let twice = read_bytes(&again.join(name));
        let prior = read_bytes(&committed.join(name));
        assert_eq!(got, twice, "regenerated twice {name}");
        assert_eq!(got, prior, "regenerated {name}");
    }
    drop(guard);
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn profile_for(name: &str) -> Profile {
    match name {
        "O1-S1.cbor" => Profile::conformance_small(32, 64, 65_536, 100).unwrap(),
        "O2-S2.cbor" => Profile::conformance_small(64, 256, 1_048_576, 1_000).unwrap(),
        "O3-PROD.cbor" => Profile::production(),
        other => panic!("unknown object {other}"),
    }
}

/// CASES synthetic index on 127.0.0.1. One `GET /index/{height}` per read.
struct LoopbackIndex {
    addr: std::net::SocketAddr,
    #[allow(dead_code)]
    join: std::thread::JoinHandle<()>,
}

impl LoopbackIndex {
    fn spawn() -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("loopback index bind: {e}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|e| panic!("loopback index addr: {e}"));
        let join = std::thread::spawn(move || serve(listener));
        Self { addr, join }
    }
}

fn serve(listener: TcpListener) {
    for conn in listener.incoming() {
        let Ok(mut conn) = conn else { continue };
        let _ = conn.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = handle(&mut conn);
    }
}

fn handle(conn: &mut TcpStream) {
    let Some(req) = read_until_headers(conn) else {
        return;
    };
    let path = req.split_whitespace().nth(1).unwrap_or("");
    let body = path
        .strip_prefix("/index/")
        .and_then(|h| h.parse::<u64>().ok())
        .filter(|h| (989..=1018).contains(h))
        .map(cases_document);
    if let Some(body) = body {
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = conn.write_all(head.as_bytes());
        let _ = conn.write_all(&body);
    } else {
        let head = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let _ = conn.write_all(head.as_bytes());
    }
    let _ = conn.shutdown(Shutdown::Both);
}

fn read_until_headers(conn: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 256];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        if buf.len() > 4096 {
            return None;
        }
        let n = conn.read(&mut tmp).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    String::from_utf8(buf).ok()
}

impl HeightIndex for LoopbackIndex {
    fn read_height(&mut self, height: u64) -> Option<Vec<u8>> {
        let mut stream = TcpStream::connect(self.addr)
            .unwrap_or_else(|e| panic!("loopback index connect {height}: {e}"));
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap_or_else(|e| panic!("loopback index timeout {height}: {e}"));
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap_or_else(|e| panic!("loopback index timeout {height}: {e}"));
        let req =
            format!("GET /index/{height} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        stream
            .write_all(req.as_bytes())
            .unwrap_or_else(|e| panic!("loopback index write {height}: {e}"));
        let mut buf = Vec::new();
        stream
            .read_to_end(&mut buf)
            .unwrap_or_else(|e| panic!("loopback index read {height}: {e}"));
        let text =
            String::from_utf8(buf).unwrap_or_else(|e| panic!("loopback index utf-8 {height}: {e}"));
        let (head, body) = text
            .split_once("\r\n\r\n")
            .unwrap_or_else(|| panic!("loopback index headers {height}"));
        let status = head.lines().next().unwrap_or("");
        if status.contains(" 404 ") {
            return None;
        }
        if !status.contains(" 200 ") {
            panic!("loopback index status {height}: {status}");
        }
        Some(body.as_bytes().to_vec())
    }
}

fn verify_object(
    index: &mut dyn HeightIndex,
    name: &str,
    bytes: &[u8],
    profile: &Profile,
) -> String {
    let mut chain = LabelChain::new("pcoc-pack-2.2/draw");
    let mut next = || chain.next8();
    match verify_from_bytes(bytes, profile, index, &mut next) {
        Verified::Record(s) => s,
        other => panic!("{name}: expected a record, got {other:?}"),
    }
}

fn slice_between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let i = text
        .find(start)
        .unwrap_or_else(|| panic!("missing {start}"));
    let rest = &text[i + start.len()..];
    let j = rest
        .find(end)
        .unwrap_or_else(|| panic!("missing {end} after {start}"));
    &rest[..j]
}

fn assert_measured(name: &str, record: &str) {
    assert!(record.contains("\"shape\":\"match\""), "{name} shape");
    assert!(record.contains("\"shape_item\":[]"), "{name} shape_item");
    let headers = slice_between(record, "\"headers\":", ",\"seed_value\":");
    assert_eq!(
        headers.matches("\"index_comparison\":\"match\"").count(),
        19,
        "{name} headers"
    );
    assert!(!headers.contains("\"not read\""), "{name} header not read");
    assert!(!headers.contains("\"mismatch\""), "{name} header mismatch");
    let mut rest = headers;
    let mut times = 0;
    while let Some(i) = rest.find("\"T\":") {
        let after = &rest[i + 4..];
        let digit = after.starts_with(|c: char| c.is_ascii_digit());
        assert!(
            digit,
            "{name} T is not a number: {}",
            &after[..after.len().min(24)]
        );
        times += 1;
        rest = &after[1..];
    }
    assert_eq!(times, 19, "{name} T count");
    assert!(record.contains("\"seed_value\":\"match\""), "{name} seed");
    for (start, end) in [
        ("\"checkpoints\":", ",\"waypoints\":"),
        ("\"waypoints\":", ",\"paths\":"),
    ] {
        let body = slice_between(record, start, end);
        let mut i = 0;
        let mut n = 0;
        while let Some(p) = body[i..].find("\"result\":") {
            let v = &body[i + p + 9..];
            assert!(v.starts_with("\"match\""), "{name} {start} {v}");
            n += 1;
            i += p + 9;
        }
        assert!(n > 0, "{name} {start}");
    }
    let six = slice_between(record, "\"six_leg\":", ",\"six_leg_runs\":");
    assert_eq!(six.matches("\"match\"").count(), 6, "{name} six_leg {six}");
    assert!(!six.contains("mismatch"), "{name} six_leg");
}

/// SHA-256 pins already checked. Verify each committed object over the loopback index.
pub fn assert_objects_verify(root: &Path) {
    let mut index = LoopbackIndex::spawn();
    let sample = index.read_height(1000).expect("loopback index 1000");
    assert_eq!(sample, cases_document(1000), "loopback index document");
    let dir = root.join("sdk/rust/objects");
    for (name, _) in OBJECTS {
        let bytes = read_bytes(&dir.join(name));
        let profile = profile_for(name);
        let record = verify_object(&mut index, name, &bytes, &profile);
        assert_measured(name, &record);
    }
}
