//! Rule A4.7 on the 546d24f reading. The head bounds JSON counts to ±(2^53−1),
//! so this wide sum is not a head record.

#[path = "common/support.rs"]
mod support;

/// `u64::MAX + 5` is 18446744073709551620. An object cannot store `2^64`
/// checkpoints, so the sum stays on the oracle inputs. main @ 546d24f L1201.
#[test]
fn a4_7_n_above_u64() {
    let mut rows = Vec::with_capacity(16);
    for g in 1..=16 {
        let (path_state, plant) = if g == 1 || g == 3 {
            ("present", "match")
        } else {
            ("skip", "n/a")
        };
        rows.push(support::PlantInput {
            g,
            path_state,
            plant,
            height_p: 123_456,
            height_i_prev: 1_000,
            t_i_prev: Some(1_700_000_000),
            block_hash: "ab".repeat(32),
        });
    }
    let mut counts = [1u128; 18];
    counts[1] = u64::MAX as u128;
    counts[2] = 5;
    let (book, _, _) = support::bookends(
        &rows,
        &counts,
        support::PbRead {
            index_comparison: "match",
            header_time: Some(1_700_001_250),
        },
    );
    assert_eq!(book.a, support::Field::Value(1));
    assert_eq!(book.b, support::Field::Value(3));
    assert_eq!(book.n, support::Field::Value(18_446_744_073_709_551_620));
}
