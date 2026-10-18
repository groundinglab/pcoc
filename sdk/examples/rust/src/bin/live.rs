//! Verify a real hike object against the public index, then apply the
//! section 15 refusals the way a gate does. Usage: live OBJECT_FILE
fn main() {
    let path = std::env::args().nth(1).expect("usage: live OBJECT_FILE");
    let object: Vec<u8> = std::fs::read(path).expect("read object");

    let agent = hike::http_agent();
    let index = "https://index.groundinglab.io"; // or your own pcoc-index
    let m: hike::Measurements = hike::verify(
        &object,
        hike::AuditDraw::OsCsprng,
        |h| hike::read_index_beacon(&agent, index, h).map_err(|_| ()),
        |loc| match hike::index_document(&agent, index, u64::from(loc.block_height)) {
            Ok(doc) => hike::proxy_matches(&doc, loc).into(),
            Err(_) => hike::ChainRead::NotRead,
        },
    );
    print!("{}", hike::verify_record_json("production", &m));

    // Gate: section 15 invariant refusals only. Thresholds (for example the
    // leg-rate floor) are the relying party's policy, applied on top.
    if hike::admit(&m) {
        eprintln!("clear: no section 15 refusal and no held read");
    } else {
        let facts: Vec<&'static str> = hike::failure_facts(&m);
        if facts.is_empty() {
            eprintln!("held: a read was not taken (for example the six-leg draw); retry later");
        } else {
            eprintln!("refused: {facts:?}");
        }
    }
}
