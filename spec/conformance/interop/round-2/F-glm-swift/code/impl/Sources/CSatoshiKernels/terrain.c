/* Section 10.4 dag2 terrain fill (honest NL4 word map only).
 * Scalar reference plus an AVX-512 path; both produce identical bytes
 * (differential-tested over the full 256 MiB production terrain).
 * The Negatives module (generator-only) implements the forbidden fill
 * variants itself in Swift; they do not appear here.
 */
#include "shk.h"
#include <string.h>
#include <stdlib.h>
#include <immintrin.h>

#define MIX_ADD     0x9E3779B97F4A7C15ULL
#define WORD_XOR    0xC0FFEEULL
#define PARENT_MUL  0xD1B54A32D192ED03ULL
#define SLOT_MUL    0xA24BAED4963EE407ULL

static inline uint64_t mix64(uint64_t x) {
    uint64_t y = x + MIX_ADD;
    y = (y ^ (y >> 30)) * 0xBF58476D1CE4E5B9ULL;
    y = (y ^ (y >> 27)) * 0x94D049BB133111EBULL;
    return y ^ (y >> 31);
}

static inline uint64_t rotl64(uint64_t x, unsigned r) {
    return r == 0 ? x : (x << r) | (x >> (64 - r));
}

static inline uint32_t reverse32(uint32_t v) {
    v = ((v & 0x55555555u) << 1) | ((v >> 1) & 0x55555555u);
    v = ((v & 0x33333333u) << 2) | ((v >> 2) & 0x33333333u);
    v = ((v & 0x0F0F0F0Fu) << 4) | ((v >> 4) & 0x0F0F0F0Fu);
    v = ((v & 0x00FF00FFu) << 8) | ((v >> 8) & 0x00FF00FFu);
    return (v << 16) | (v >> 16);
}

static inline uint64_t load64(const uint8_t *p) {
    uint64_t v;
    memcpy(&v, p, 8);
    return v;
}

static inline void store64(uint8_t *p, uint64_t v) { memcpy(p, &v, 8); }

void shk_terrain_fill_scalar(uint8_t *terrain, uint64_t n_slots,
                             size_t words_per_slot, const uint64_t P[4]) {
    const size_t slot_bytes = words_per_slot * 8;
    uint64_t *w = (uint64_t *)malloc(slot_bytes);
    if (!w) abort();
    for (uint64_t i = 0; i < n_slots; i++) {
        const uint64_t ish = i << 32;
        for (size_t j = 0; j < words_per_slot; j++) {
            uint64_t c = ish ^ (uint64_t)j ^ WORD_XOR;
            uint64_t t = mix64(P[j & 3] ^ c);
            t = mix64(t ^ rotl64(P[(j + 1) & 3], 17));
            t = mix64(t ^ rotl64(P[(j + 2) & 3], 31));
            t = mix64(t ^ rotl64(P[(j + 3) & 3], 47));
            w[j] = t;
        }
        if (i > 0) {
            const unsigned bits = 63 - (unsigned)__builtin_clzll(i);
            const unsigned b = bits == 0 ? 1 : bits; /* max(floor_log2(i), 1) */
            uint64_t parents[3];
            parents[0] = i - 1;
            parents[1] = ((uint64_t)reverse32((uint32_t)i) >> (32 - b)) % i;
            parents[2] = ((i * MIX_ADD) >> 32) % i;
            for (int k = 1; k <= 3; k++) {
                const uint64_t *pw =
                    (const uint64_t *)(terrain + parents[k - 1] * slot_bytes);
                const uint64_t kc = (uint64_t)k * PARENT_MUL;
                for (size_t j = 0; j < words_per_slot; j++) {
                    const unsigned r = (unsigned)((j + (size_t)k) & 63);
                    const uint64_t x = (w[j] ^ rotl64(load64((const uint8_t *)(pw + j)), r)) ^ kc;
                    w[j] = mix64(x);
                }
            }
            const uint64_t sm = i * SLOT_MUL;
            for (size_t j = 0; j < words_per_slot; j++) w[j] = mix64(w[j] ^ sm);
        }
        memcpy(terrain + i * slot_bytes, w, slot_bytes);
    }
    free(w);
}

/* ------------------------------- AVX-512 ------------------------------- */

__attribute__((target("avx512f,avx512dq,avx512vl")))
static inline __m512i mix64v(__m512i x) {
    __m512i y = _mm512_add_epi64(x, _mm512_set1_epi64((int64_t)MIX_ADD));
    y = _mm512_xor_si512(y, _mm512_srli_epi64(y, 30));
    y = _mm512_mullo_epi64(y, _mm512_set1_epi64((int64_t)0xBF58476D1CE4E5B9ULL));
    y = _mm512_xor_si512(y, _mm512_srli_epi64(y, 27));
    y = _mm512_mullo_epi64(y, _mm512_set1_epi64((int64_t)0x94D049BB133111EBULL));
    y = _mm512_xor_si512(y, _mm512_srli_epi64(y, 31));
    return y;
}

__attribute__((target("avx512f,avx512dq,avx512vl")))
void shk_terrain_fill_avx512(uint8_t *terrain, uint64_t n_slots,
                             size_t words_per_slot, const uint64_t P[4]) {
    const size_t slot_bytes = words_per_slot * 8;
    const __m512i vid = _mm512_setr_epi64(0, 1, 2, 3, 4, 5, 6, 7);
    const __m512i mask63 = _mm512_set1_epi64(63);

    /* Lane vectors: lane j of a zmm starting at a word index divisible by 8
     * uses P[(j0+d)&3]; j0 % 8 == 0 implies the per-lane lane pattern is
     * [0,1,2,3,0,1,2,3] shifted by d. */
    __m512i VL[4], R17, R31, R47;
    for (int d = 0; d < 4; d++) {
        uint64_t tmp[8];
        for (int lane = 0; lane < 8; lane++) tmp[lane] = P[(lane + d) & 3];
        VL[d] = _mm512_loadu_si512(tmp);
    }
    R17 = _mm512_rolv_epi64(VL[1], _mm512_set1_epi64(17));
    R31 = _mm512_rolv_epi64(VL[2], _mm512_set1_epi64(31));
    R47 = _mm512_rolv_epi64(VL[3], _mm512_set1_epi64(47));
    const __m512i vwordxor = _mm512_set1_epi64((int64_t)WORD_XOR);
    const __m512i kpar[3] = {
        _mm512_set1_epi64((int64_t)PARENT_MUL),
        _mm512_set1_epi64((int64_t)(2 * PARENT_MUL)),
        _mm512_set1_epi64((int64_t)(3 * PARENT_MUL)),
    };

    uint64_t *w = (uint64_t *)malloc(slot_bytes);
    if (!w) abort();

    for (uint64_t i = 0; i < n_slots; i++) {
        const __m512i vish = _mm512_set1_epi64((int64_t)(i << 32));
        size_t j = 0;
        for (; j + 8 <= words_per_slot; j += 8) {
            __m512i c = _mm512_xor_si512(
                _mm512_xor_si512(_mm512_add_epi64(vid, vish), vwordxor),
                _mm512_set1_epi64((int64_t)j));
            __m512i t = mix64v(_mm512_xor_si512(VL[0], c));
            t = mix64v(_mm512_xor_si512(t, R17));
            t = mix64v(_mm512_xor_si512(t, R31));
            t = mix64v(_mm512_xor_si512(t, R47));
            _mm512_storeu_si512(w + j, t);
        }
        for (; j < words_per_slot; j++) {
            uint64_t c = (i << 32) ^ (uint64_t)j ^ WORD_XOR;
            uint64_t t = mix64(P[j & 3] ^ c);
            t = mix64(t ^ rotl64(P[(j + 1) & 3], 17));
            t = mix64(t ^ rotl64(P[(j + 2) & 3], 31));
            t = mix64(t ^ rotl64(P[(j + 3) & 3], 47));
            w[j] = t;
        }
        if (i > 0) {
            const unsigned bits = 63 - (unsigned)__builtin_clzll(i);
            const unsigned b = bits == 0 ? 1 : bits;
            uint64_t parents[3];
            parents[0] = i - 1;
            parents[1] = ((uint64_t)reverse32((uint32_t)i) >> (32 - b)) % i;
            parents[2] = ((i * MIX_ADD) >> 32) % i;
            for (int k = 1; k <= 3; k++) {
                const uint64_t *pw =
                    (const uint64_t *)(terrain + parents[k - 1] * slot_bytes);
                size_t q = 0;
                for (; q + 8 <= words_per_slot; q += 8) {
                    __m512i pwv = _mm512_loadu_si512((const void *)(pw + q));
                    __m512i wv = _mm512_loadu_si512((const void *)(w + q));
                    /* rot amounts: (q + lane + k) & 63 */
                    __m512i rot = _mm512_add_epi64(
                        _mm512_add_epi64(vid, _mm512_set1_epi64((int64_t)(q + k))),
                        _mm512_setzero_si512());
                    rot = _mm512_and_si512(rot, mask63);
                    __m512i x = _mm512_xor_si512(wv, _mm512_rolv_epi64(pwv, rot));
                    x = _mm512_xor_si512(x, kpar[k - 1]);
                    _mm512_storeu_si512(w + q, mix64v(x));
                }
                for (; q < words_per_slot; q++) {
                    const unsigned r = (unsigned)((q + (size_t)k) & 63);
                    const uint64_t x =
                        (w[q] ^ rotl64(load64((const uint8_t *)(pw + q)), r)) ^
                        (uint64_t)k * PARENT_MUL;
                    w[q] = mix64(x);
                }
            }
            const __m512i vsm = _mm512_set1_epi64((int64_t)(i * SLOT_MUL));
            size_t q = 0;
            for (; q + 8 <= words_per_slot; q += 8) {
                __m512i wv = _mm512_loadu_si512((const void *)(w + q));
                _mm512_storeu_si512(w + q, mix64v(_mm512_xor_si512(wv, vsm)));
            }
            for (; q < words_per_slot; q++)
                w[q] = mix64(w[q] ^ (i * SLOT_MUL));
        }
        memcpy(terrain + i * slot_bytes, w, slot_bytes);
    }
    free(w);
}

void shk_terrain_fill(uint8_t *terrain, uint64_t n_slots,
                      size_t words_per_slot, const uint64_t P[4]) {
    if (shk_cpu_has_avx512dq())
        shk_terrain_fill_avx512(terrain, n_slots, words_per_slot, P);
    else
        shk_terrain_fill_scalar(terrain, n_slots, words_per_slot, P);
}
