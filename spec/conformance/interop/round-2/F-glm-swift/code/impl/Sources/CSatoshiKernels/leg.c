/* Section 11 leg driver: STEPS_PER_LEG steps, each one full-slot fold
 * (section 11.1) followed by the SHA-256 seal. The step is inherently
 * serial (S' selects the next slot), so the loop is a single thread.
 */
#include "shk.h"

typedef uint64_t (*fold_fn)(const uint8_t *, size_t, uint64_t);

static fold_fn pick_fold(int opt) {
    switch (opt) {
        case 0: return shk_fold_scalar;
        case 1: return shk_cpu_has_avx2() ? shk_fold_avx2 : shk_fold_scalar;
        case 2: return shk_cpu_has_avx512dq() ? shk_fold_avx512 : shk_fold_scalar;
        default: return shk_fold;
    }
}

uint64_t shk_leg_run(const uint8_t *terrain, uint64_t n_slots,
                     size_t words_per_slot, uint64_t steps,
                     uint64_t S0, int opt) {
    fold_fn fold = pick_fold(opt);
    const size_t slot_bytes = words_per_slot * 8;
    uint64_t S = S0;
    for (uint64_t t = 0; t < steps; t++) {
        const uint64_t idx = S % n_slots;
        const uint8_t *slot = terrain + idx * slot_bytes;
        const uint64_t acc = fold(slot, words_per_slot, S);
        S = shk_seal_step(acc);
    }
    return S;
}

uint64_t shk_step_measure(const uint8_t *terrain, uint64_t n_slots,
                          size_t words_per_slot, uint64_t S,
                          uint64_t *slot_index_out, uint64_t *acc_out, int opt) {
    fold_fn fold = pick_fold(opt);
    const size_t slot_bytes = words_per_slot * 8;
    const uint64_t idx = S % n_slots;
    const uint64_t acc = fold(terrain + idx * slot_bytes, words_per_slot, S);
    if (slot_index_out) *slot_index_out = idx;
    if (acc_out) *acc_out = acc;
    return shk_seal_step(acc);
}
