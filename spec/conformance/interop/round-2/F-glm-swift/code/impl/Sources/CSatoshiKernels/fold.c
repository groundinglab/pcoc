/* Section 11.1 slot fold.
 * acc = (acc + mix64(word(w) XOR S)) mod 2^64 over every word of the slot,
 * acc starting at 0. Wrapping addition is associative and commutative, so
 * the unrolled/SIMD paths keep independent lane accumulators and combine at
 * the end; the u64 result is identical to the sequential loop. Every word
 * of the slot is loaded and mixed on every call.
 */
#include "shk.h"
#include <string.h>
#include <immintrin.h>

#define MIX_ADD  0x9E3779B97F4A7C15ULL
#define MIX_MUL1 0xBF58476D1CE4E5B9ULL
#define MIX_MUL2 0x94D049BB133111EBULL

static inline uint64_t load64(const uint8_t *p) {
    uint64_t v;
    memcpy(&v, p, 8);
    return v;
}

static inline uint64_t mix64(uint64_t x) {
    uint64_t y = x + MIX_ADD;
    y = (y ^ (y >> 30)) * MIX_MUL1;
    y = (y ^ (y >> 27)) * MIX_MUL2;
    return y ^ (y >> 31);
}

uint64_t shk_fold_scalar(const uint8_t *slot, size_t words, uint64_t S) {
    uint64_t a0 = 0, a1 = 0, a2 = 0, a3 = 0;
    size_t i = 0;
    for (; i + 4 <= words; i += 4) {
        a0 += mix64(load64(slot + (i + 0) * 8) ^ S);
        a1 += mix64(load64(slot + (i + 1) * 8) ^ S);
        a2 += mix64(load64(slot + (i + 2) * 8) ^ S);
        a3 += mix64(load64(slot + (i + 3) * 8) ^ S);
    }
    uint64_t acc = (a0 + a1) + (a2 + a3);
    for (; i < words; i++) acc += mix64(load64(slot + i * 8) ^ S);
    return acc;
}

/* ---------------- AVX-2 (64x64 low multiply via vpmuludq) ---------------- */

__attribute__((target("avx2")))
static inline __m256i mul64lo_avx2(__m256i a, __m256i b) {
    /* low 64 bits of a*b, 4 lanes at once:
     * lo = a0*b0 + ((a0*b1 + a1*b0) << 32) per lane */
    __m256i b_lo = _mm256_shuffle_epi32(b, _MM_SHUFFLE(1, 0, 1, 0));
    __m256i b_hi = _mm256_shuffle_epi32(b, _MM_SHUFFLE(3, 3, 3, 1));
    __m256i a_hi = _mm256_shuffle_epi32(a, _MM_SHUFFLE(3, 3, 3, 1));
    __m256i t0 = _mm256_mul_epu32(a, b_lo);
    __m256i t1 = _mm256_mul_epu32(a, b_hi);
    __m256i t2 = _mm256_mul_epu32(a_hi, b_lo);
    __m256i cross = _mm256_add_epi64(t1, t2);
    cross = _mm256_slli_epi64(cross, 32);
    return _mm256_add_epi64(t0, cross);
}

__attribute__((target("avx2")))
static inline __m256i mix64_avx2(__m256i x) {
    __m256i y = _mm256_add_epi64(x, _mm256_set1_epi64x((int64_t)MIX_ADD));
    y = _mm256_xor_si256(y, _mm256_srli_epi64(y, 30));
    y = mul64lo_avx2(y, _mm256_set1_epi64x((int64_t)MIX_MUL1));
    y = _mm256_xor_si256(y, _mm256_srli_epi64(y, 27));
    y = mul64lo_avx2(y, _mm256_set1_epi64x((int64_t)MIX_MUL2));
    y = _mm256_xor_si256(y, _mm256_srli_epi64(y, 31));
    return y;
}

__attribute__((target("avx2")))
uint64_t shk_fold_avx2(const uint8_t *slot, size_t words, uint64_t S) {
    __m256i vS = _mm256_set1_epi64x((int64_t)S);
    __m256i acc0 = _mm256_setzero_si256();
    __m256i acc1 = _mm256_setzero_si256();
    size_t i = 0;
    for (; i + 8 <= words; i += 8) {
        __m256i x0 = _mm256_loadu_si256((const __m256i *)(slot + (i + 0) * 8));
        __m256i x1 = _mm256_loadu_si256((const __m256i *)(slot + (i + 4) * 8));
        acc0 = _mm256_add_epi64(acc0, mix64_avx2(_mm256_xor_si256(x0, vS)));
        acc1 = _mm256_add_epi64(acc1, mix64_avx2(_mm256_xor_si256(x1, vS)));
    }
    for (; i + 4 <= words; i += 4) {
        __m256i x = _mm256_loadu_si256((const __m256i *)(slot + i * 8));
        acc0 = _mm256_add_epi64(acc0, mix64_avx2(_mm256_xor_si256(x, vS)));
    }
    __m256i tot = _mm256_add_epi64(acc0, acc1);
    uint64_t lanes[4];
    _mm256_storeu_si256((__m256i *)lanes, tot);
    uint64_t acc = lanes[0] + lanes[1] + lanes[2] + lanes[3];
    for (; i < words; i++) acc += mix64(load64(slot + i * 8) ^ S);
    return acc;
}

/* ---------------- AVX-512 (vpmullq needs avx512dq) ---------------- */

__attribute__((target("avx512f,avx512dq,avx512vl")))
static inline __m512i mix64_avx512(__m512i x) {
    __m512i y = _mm512_add_epi64(x, _mm512_set1_epi64((int64_t)MIX_ADD));
    y = _mm512_xor_si512(y, _mm512_srli_epi64(y, 30));
    y = _mm512_mullo_epi64(y, _mm512_set1_epi64((int64_t)MIX_MUL1));
    y = _mm512_xor_si512(y, _mm512_srli_epi64(y, 27));
    y = _mm512_mullo_epi64(y, _mm512_set1_epi64((int64_t)MIX_MUL2));
    y = _mm512_xor_si512(y, _mm512_srli_epi64(y, 31));
    return y;
}

__attribute__((target("avx512f,avx512dq,avx512vl")))
uint64_t shk_fold_avx512(const uint8_t *slot, size_t words, uint64_t S) {
    const __m512i vS = _mm512_set1_epi64((int64_t)S);
    __m512i acc0 = _mm512_setzero_si512();
    __m512i acc1 = _mm512_setzero_si512();
    __m512i acc2 = _mm512_setzero_si512();
    __m512i acc3 = _mm512_setzero_si512();
    size_t i = 0;
    for (; i + 32 <= words; i += 32) {
        __m512i x0 = _mm512_loadu_si512((const void *)(slot + (i + 0) * 8));
        __m512i x1 = _mm512_loadu_si512((const void *)(slot + (i + 8) * 8));
        __m512i x2 = _mm512_loadu_si512((const void *)(slot + (i + 16) * 8));
        __m512i x3 = _mm512_loadu_si512((const void *)(slot + (i + 24) * 8));
        acc0 = _mm512_add_epi64(acc0, mix64_avx512(_mm512_xor_si512(x0, vS)));
        acc1 = _mm512_add_epi64(acc1, mix64_avx512(_mm512_xor_si512(x1, vS)));
        acc2 = _mm512_add_epi64(acc2, mix64_avx512(_mm512_xor_si512(x2, vS)));
        acc3 = _mm512_add_epi64(acc3, mix64_avx512(_mm512_xor_si512(x3, vS)));
    }
    for (; i + 8 <= words; i += 8) {
        __m512i x = _mm512_loadu_si512((const void *)(slot + i * 8));
        acc0 = _mm512_add_epi64(acc0, mix64_avx512(_mm512_xor_si512(x, vS)));
    }
    __m512i tot01 = _mm512_add_epi64(acc0, acc1);
    __m512i tot23 = _mm512_add_epi64(acc2, acc3);
    __m512i tot = _mm512_add_epi64(tot01, tot23);
    uint64_t lanes[8];
    _mm512_storeu_si512(lanes, tot);
    uint64_t acc = lanes[0] + lanes[1] + lanes[2] + lanes[3] +
                   lanes[4] + lanes[5] + lanes[6] + lanes[7];
    for (; i < words; i++) acc += mix64(load64(slot + i * 8) ^ S);
    return acc;
}

uint64_t shk_fold(const uint8_t *slot, size_t words, uint64_t S) {
    if (shk_cpu_has_avx512dq()) return shk_fold_avx512(slot, words, S);
    if (shk_cpu_has_avx2()) return shk_fold_avx2(slot, words, S);
    return shk_fold_scalar(slot, words, S);
}
