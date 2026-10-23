#ifndef SHK_H
#define SHK_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---------------- CPU features (runtime dispatch) ---------------- */

int shk_cpu_has_sha_ni(void);
int shk_cpu_has_avx2(void);
int shk_cpu_has_avx512dq(void);
void shk_cpu_brand(char *buf, size_t buflen);

/* ---------------- SHA-256 (FIPS 180-4) ----------------
 * One-shot over a buffer. Uses x86 SHA-NI when the host provides it
 * (SPEC 2.2 section 11.1 normative requirement), otherwise software.
 * mode: 0 = auto, 1 = force software, 2 = force SHA-NI (if unavailable
 * falls back to software). mode exists only for benchmarking.
 */
void shk_sha256(const uint8_t *data, size_t len, uint8_t out[32]);
void shk_sha256_mode(const uint8_t *data, size_t len, uint8_t out[32], int mode);

/* Multi-part one-shot: concatenates parts[0..n-1] (raw concatenation,
 * section 3) and hashes. Convenience for small preimages; not for
 * very large inputs.
 */
void shk_sha256_parts(const uint8_t *const *parts, const size_t *lens,
                      size_t n, uint8_t out[32]);

/* The section 11.1 seal:
 *   E = SHA-256(SEAL_DOMAIN(12 bytes "pcoc-walk-v1") || le64(acc))
 *   returns u64_le(E[0:8]) as the next state S'.
 * The preimage is 20 bytes, one SHA-256 block.
 */
uint64_t shk_seal_step(uint64_t acc);

/* The same digest as bytes (32), for tests and the section 7.4 check. */
void shk_seal_digest(uint64_t acc, uint8_t out[32]);

/* ---------------- slot fold (section 11.1) ----------------
 * acc = (sum over w of mix64(word(w) XOR S)) mod 2^64, wrapping addition,
 * acc starts at 0. Every word of the slot is loaded and mixed.
 * Wrapping addition is associative and commutative, so the SIMD paths use
 * independent lane accumulators and give the identical u64 result.
 */
uint64_t shk_fold_scalar(const uint8_t *slot, size_t words, uint64_t S);
uint64_t shk_fold_avx2(const uint8_t *slot, size_t words, uint64_t S);
uint64_t shk_fold_avx512(const uint8_t *slot, size_t words, uint64_t S);
uint64_t shk_fold(const uint8_t *slot, size_t words, uint64_t S); /* best available */

/* ---------------- terrain fill (section 10.4, honest NL4 dag2) ----------------
 * Fills terrain[0 .. n_slots*words_per_slot*8) exactly per section 10.4.
 * P[0..3] are the four lanes of the seed (section 10).
 */
void shk_terrain_fill_scalar(uint8_t *terrain, uint64_t n_slots,
                             size_t words_per_slot, const uint64_t P[4]);
void shk_terrain_fill_avx512(uint8_t *terrain, uint64_t n_slots,
                             size_t words_per_slot, const uint64_t P[4]);
void shk_terrain_fill(uint8_t *terrain, uint64_t n_slots,
                      size_t words_per_slot, const uint64_t P[4]);

/* ---------------- leg (section 11.2) ----------------
 * Runs `steps` steps starting from S0 over the terrain.
 * opt: -1 auto (best SIMD fold), 0 scalar fold, 1 AVX-2, 2 AVX-512.
 * SHA-256 in the seal always uses the host's best (SHA-NI where present).
 */
uint64_t shk_leg_run(const uint8_t *terrain, uint64_t n_slots,
                     size_t words_per_slot, uint64_t steps,
                     uint64_t S0, int opt);

/* One step, with the measurements the fold vector needs:
 * slot_index = S mod n_slots; acc = fold of the whole slot; returns next S'.
 */
uint64_t shk_step_measure(const uint8_t *terrain, uint64_t n_slots,
                          size_t words_per_slot, uint64_t S,
                          uint64_t *slot_index_out, uint64_t *acc_out, int opt);

/* OS CSPRNG read (section 14.6 byte source). 0 on success. */
int shk_os_getrandom(uint8_t *buf, size_t n);

#ifdef __cplusplus
}
#endif
#endif
