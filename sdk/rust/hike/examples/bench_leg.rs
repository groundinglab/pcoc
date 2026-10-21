//! One production terrain fill and one solo leg. Not a test.
//!
//! `cargo run -p hike --release --example bench_leg`

fn main() {
    println!(
        "sha_ni={}",
        std::is_x86_feature_detected!("sha")
    );
    let public_key = [0x11u8; 32];
    let beacon = [0x22u8; 32];
    let seed = hike::seed(&public_key, &beacon);
    let start = std::time::Instant::now();
    let terrain = hike::fill_terrain(&seed);
    let fill_s = start.elapsed().as_secs_f64();
    let start = std::time::Instant::now();
    let leg = hike::leg_result(&terrain, &seed);
    let leg_s = start.elapsed().as_secs_f64();
    println!("fill_s={fill_s:.4}");
    println!("leg_s={leg_s:.4}");
    println!("leg_result={leg:016x}");
    println!("terrain_bytes={}", terrain.len());
}
