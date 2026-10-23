#include <immintrin.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* OpenSSL's one-shot ABI; declared locally so only libcrypto runtime is needed. */
extern unsigned char *SHA256(const unsigned char *data, size_t count,
                             unsigned char *md_buf);

#define MASK64 UINT64_MAX
#define MIX_ADD UINT64_C(0x9E3779B97F4A7C15)
#define MIX_MUL1 UINT64_C(0xBF58476D1CE4E5B9)
#define MIX_MUL2 UINT64_C(0x94D049BB133111EB)
#define WORD_XOR UINT64_C(0xC0FFEE)
#define PARENT_MUL UINT64_C(0xD1B54A32D192ED03)
#define SLOT_MUL UINT64_C(0xA24BAED4963EE407)

#if defined(__GNUC__) && !defined(__clang__)
#define SCALAR_NOVEC __attribute__((optimize("no-tree-vectorize")))
#else
#define SCALAR_NOVEC
#endif

static const uint8_t SEAL_DOMAIN[12] = {
    'p','c','o','c','-','w','a','l','k','-','v','1'
};

static inline uint64_t load64le(const uint8_t *p) {
    uint64_t x;
    memcpy(&x, p, sizeof(x));
#if __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
    x = __builtin_bswap64(x);
#endif
    return x;
}

static inline void store64le(uint8_t *p, uint64_t x) {
#if __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
    x = __builtin_bswap64(x);
#endif
    memcpy(p, &x, sizeof(x));
}

static inline uint64_t rotl64(uint64_t x, unsigned r) {
    return r ? (x << r) | (x >> (64 - r)) : x;
}

static inline uint64_t mix64_scalar(uint64_t x) {
    uint64_t y = x + MIX_ADD;
    y = (y ^ (y >> 30)) * MIX_MUL1;
    y = (y ^ (y >> 27)) * MIX_MUL2;
    return y ^ (y >> 31);
}

static inline uint64_t mix64_prefinal(uint64_t x) {
    uint64_t y = x + MIX_ADD;
    y = (y ^ (y >> 30)) * MIX_MUL1;
    return (y ^ (y >> 27)) * MIX_MUL2;
}

#if defined(__AVX512F__) && defined(__AVX512DQ__)
static inline __m512i mix64_vec(__m512i x) {
    const __m512i add = _mm512_set1_epi64((long long)MIX_ADD);
    const __m512i mul1 = _mm512_set1_epi64((long long)MIX_MUL1);
    const __m512i mul2 = _mm512_set1_epi64((long long)MIX_MUL2);
    __m512i y = _mm512_add_epi64(x, add);
    y = _mm512_mullo_epi64(_mm512_xor_si512(y, _mm512_srli_epi64(y, 30)), mul1);
    y = _mm512_mullo_epi64(_mm512_xor_si512(y, _mm512_srli_epi64(y, 27)), mul2);
    return _mm512_xor_si512(y, _mm512_srli_epi64(y, 31));
}

static inline __m512i rotl_vec(__m512i x, __m512i r) {
    return _mm512_rolv_epi64(x, r);
}

static inline uint64_t reduce_add64(__m512i v) {
    uint64_t a[8];
    _mm512_storeu_si512((void *)a, v);
    return a[0] + a[1] + a[2] + a[3] + a[4] + a[5] + a[6] + a[7];
}
#endif

static uint32_t reverse32(uint32_t v) {
    v = ((v & 0x55555555U) << 1) | ((v >> 1) & 0x55555555U);
    v = ((v & 0x33333333U) << 2) | ((v >> 2) & 0x33333333U);
    v = ((v & 0x0F0F0F0FU) << 4) | ((v >> 4) & 0x0F0F0F0FU);
    v = ((v & 0x00FF00FFU) << 8) | ((v >> 8) & 0x00FF00FFU);
    return (v << 16) | (v >> 16);
}

static inline uint64_t bitrev_parent(uint64_t i) {
    unsigned bits = 63U - (unsigned)__builtin_clzll(i);
    if (bits < 1) bits = 1;
    return ((uint64_t)(reverse32((uint32_t)i) >> (32 - bits))) % i;
}

static inline uint64_t mul_parent(uint64_t i) {
    return ((i * MIX_ADD) >> 32) % i;
}

static void lanes_from_seed(const uint8_t seed[32], uint64_t p[4], int variant) {
    for (int k = 0; k < 4; ++k) p[k] = load64le(seed + 8 * k);
    if (variant == 3) {
        uint64_t s = p[0] ^ p[1] ^ p[2] ^ p[3];
        p[0] = p[1] = p[2] = p[3] = s;
    }
}

static inline uint64_t base_word(uint64_t i, uint64_t j, const uint64_t p[4], int variant) {
    uint64_t c = (i << 32) ^ j ^ WORD_XOR;
    if (variant == 1) return mix64_scalar(p[j & 3] ^ c);
    if (variant == 2) {
        return mix64_scalar(p[j & 3] ^ rotl64(p[(j + 1) & 3], 17) ^
                            rotl64(p[(j + 2) & 3], 31) ^
                            rotl64(p[(j + 3) & 3], 47) ^ c);
    }
    uint64_t t = mix64_scalar(p[j & 3] ^ c);
    t = mix64_scalar(t ^ rotl64(p[(j + 1) & 3], 17));
    t = mix64_scalar(t ^ rotl64(p[(j + 2) & 3], 31));
    return mix64_scalar(t ^ rotl64(p[(j + 3) & 3], 47));
}

static SCALAR_NOVEC int fill_scalar(uint8_t *terrain, uint64_t terrain_bytes,
                                    uint64_t slot_bytes, const uint8_t seed[32],
                                    int variant) {
    if (!terrain || !seed || slot_bytes < 8 || (slot_bytes & 7) ||
        terrain_bytes % slot_bytes) return -1;
    const uint64_t words = slot_bytes / 8;
    const uint64_t slots = terrain_bytes / slot_bytes;
    uint64_t p[4];
    lanes_from_seed(seed, p, variant);
    for (uint64_t i = 0; i < slots; ++i) {
        uint8_t *dst = terrain + i * slot_bytes;
        uint64_t parents[3] = {0, 0, 0};
        if (i) {
            parents[0] = i - 1;
            parents[1] = bitrev_parent(i);
            parents[2] = mul_parent(i);
        }
        const uint64_t slot_mix = i * SLOT_MUL;
#if defined(__clang__)
#pragma clang loop vectorize(disable) interleave(disable)
#endif
        for (uint64_t j = 0; j < words; ++j) {
            uint64_t w = base_word(i, j, p, variant);
            if (i) {
                for (unsigned k = 1; k <= 3; ++k) {
                    uint64_t pw = load64le(terrain + parents[k - 1] * slot_bytes + j * 8);
                    w = mix64_scalar((w ^ rotl64(pw, (unsigned)((j + k) & 63))) ^
                                     (k * PARENT_MUL));
                }
                w = mix64_scalar(w ^ slot_mix);
            }
            store64le(dst + j * 8, w);
        }
    }
    return 0;
}

#if defined(__AVX512F__) && defined(__AVX512DQ__)
static int fill_avx512(uint8_t *terrain, uint64_t terrain_bytes, uint64_t slot_bytes,
                       const uint8_t seed[32], int variant) {
    if (variant != 0 && variant != 3)
        return fill_scalar(terrain, terrain_bytes, slot_bytes, seed, variant);
    if (!terrain || !seed || slot_bytes < 8 || (slot_bytes & 7) ||
        terrain_bytes % slot_bytes) return -1;
    const uint64_t words = slot_bytes / 8;
    const uint64_t slots = terrain_bytes / slot_bytes;
    uint64_t p[4];
    lanes_from_seed(seed, p, variant);
    const __m512i lane0 = _mm512_set_epi64((long long)p[3], (long long)p[2],
        (long long)p[1], (long long)p[0], (long long)p[3], (long long)p[2],
        (long long)p[1], (long long)p[0]);
    const __m512i lane1 = _mm512_set_epi64((long long)p[0], (long long)p[3],
        (long long)p[2], (long long)p[1], (long long)p[0], (long long)p[3],
        (long long)p[2], (long long)p[1]);
    const __m512i lane2 = _mm512_set_epi64((long long)p[1], (long long)p[0],
        (long long)p[3], (long long)p[2], (long long)p[1], (long long)p[0],
        (long long)p[3], (long long)p[2]);
    const __m512i lane3 = _mm512_set_epi64((long long)p[2], (long long)p[1],
        (long long)p[0], (long long)p[3], (long long)p[2], (long long)p[1],
        (long long)p[0], (long long)p[3]);
    const __m512i r17 = _mm512_set1_epi64(17), r31 = _mm512_set1_epi64(31);
    const __m512i r47 = _mm512_set1_epi64(47);
    for (uint64_t i = 0; i < slots; ++i) {
        uint8_t *dst = terrain + i * slot_bytes;
        uint64_t parents[3] = {0, 0, 0};
        if (i) {
            parents[0] = i - 1;
            parents[1] = bitrev_parent(i);
            parents[2] = mul_parent(i);
        }
        const __m512i smix = _mm512_set1_epi64((long long)(i * SLOT_MUL));
        uint64_t j = 0;
        for (; j + 8 <= words; j += 8) {
            __m512i js = _mm512_set_epi64((long long)(j+7), (long long)(j+6),
                (long long)(j+5), (long long)(j+4), (long long)(j+3),
                (long long)(j+2), (long long)(j+1), (long long)j);
            __m512i c = _mm512_xor_si512(
                _mm512_set1_epi64((long long)((i << 32) ^ WORD_XOR)), js);
            __m512i w = mix64_vec(_mm512_xor_si512(lane0, c));
            w = mix64_vec(_mm512_xor_si512(w, rotl_vec(lane1, r17)));
            w = mix64_vec(_mm512_xor_si512(w, rotl_vec(lane2, r31)));
            w = mix64_vec(_mm512_xor_si512(w, rotl_vec(lane3, r47)));
            if (i) {
                for (unsigned k = 1; k <= 3; ++k) {
                    __m512i pw = _mm512_loadu_si512((const void *)
                        (terrain + parents[k-1] * slot_bytes + j * 8));
                    __m512i rv = _mm512_and_si512(
                        _mm512_add_epi64(js, _mm512_set1_epi64(k)),
                        _mm512_set1_epi64(63));
                    __m512i x = _mm512_xor_si512(w, rotl_vec(pw, rv));
                    x = _mm512_xor_si512(x,
                        _mm512_set1_epi64((long long)(k * PARENT_MUL)));
                    w = mix64_vec(x);
                }
                w = mix64_vec(_mm512_xor_si512(w, smix));
            }
            _mm512_storeu_si512((void *)(dst + j * 8), w);
        }
        for (; j < words; ++j) {
            uint64_t w = base_word(i, j, p, variant);
            if (i) {
                for (unsigned k = 1; k <= 3; ++k) {
                    uint64_t pw = load64le(terrain + parents[k-1] * slot_bytes + j*8);
                    w = mix64_scalar((w ^ rotl64(pw, (unsigned)((j+k)&63))) ^
                                     (k * PARENT_MUL));
                }
                w = mix64_scalar(w ^ (i * SLOT_MUL));
            }
            store64le(dst + j * 8, w);
        }
    }
    return 0;
}
#endif

int sh_fill(uint8_t *terrain, uint64_t terrain_bytes, uint64_t slot_bytes,
            const uint8_t seed[32], int variant, int optimized) {
#if defined(__AVX512F__) && defined(__AVX512DQ__)
    if (optimized) return fill_avx512(terrain, terrain_bytes, slot_bytes, seed, variant);
#else
    (void)optimized;
#endif
    return fill_scalar(terrain, terrain_bytes, slot_bytes, seed, variant);
}

static inline uint64_t seal(uint64_t folded) {
    uint8_t input[20], digest[32];
    memcpy(input, SEAL_DOMAIN, 12);
    store64le(input + 12, folded);
    SHA256(input, sizeof(input), digest);
    return load64le(digest);
}

static SCALAR_NOVEC uint64_t fold_honest_scalar(const uint8_t *slot, uint64_t words,
                                                 uint64_t s) {
    uint64_t acc = 0;
#if defined(__clang__)
#pragma clang loop vectorize(disable) interleave(disable)
#endif
    for (uint64_t w = 0; w < words; ++w)
        acc += mix64_scalar(load64le(slot + w * 8) ^ s);
    return acc;
}

#if defined(__AVX512F__) && defined(__AVX512DQ__)
static uint64_t fold_honest_avx512(const uint8_t *slot, uint64_t words, uint64_t s) {
    __m512i acc = _mm512_setzero_si512();
    const __m512i state = _mm512_set1_epi64((long long)s);
    uint64_t w = 0;
    for (; w + 8 <= words; w += 8) {
        __m512i x = _mm512_loadu_si512((const void *)(slot + w * 8));
        acc = _mm512_add_epi64(acc, mix64_vec(_mm512_xor_si512(x, state)));
    }
    uint64_t sum = reduce_add64(acc);
    for (; w < words; ++w) sum += mix64_scalar(load64le(slot + w * 8) ^ s);
    return sum;
}
#endif

static uint64_t fold_variant(const uint8_t *slot, uint64_t words, uint64_t s, int mode) {
    uint64_t acc = 0;
    if (mode == 1) {
        for (uint64_t w = 0; w < words; ++w) acc ^= load64le(slot + w*8) ^ w;
        return s ^ acc;
    }
    if (mode == 2) {
        for (uint64_t w = 0; w < words; ++w)
            acc ^= mix64_scalar(load64le(slot + w*8) ^ w ^ s);
        return acc;
    }
    if (mode == 3) {
        for (uint64_t w = 0; w < words; ++w)
            acc += mix64_scalar(load64le(slot + w*8) ^ w ^ s);
        return acc;
    }
    if (mode == 4) {
        for (uint64_t w = 0; w < words; ++w)
            acc += mix64_scalar(load64le(slot + w*8) ^ (s + w));
        return acc;
    }
    if (mode == 5) return mix64_scalar(load64le(slot) ^ s);
    if (mode == 6) {
        for (uint64_t w = 0; w + 1 < words; ++w)
            acc += mix64_scalar(load64le(slot + w*8) ^ s);
        return acc;
    }
    if (mode == 7) {
        for (uint64_t w = 0; w < words; ++w)
            acc += mix64_scalar(load64le(slot + w*8) ^ w);
        return acc + s;
    }
    if (mode == 8) {
        for (uint64_t w = 0; w < words; ++w)
            acc ^= mix64_prefinal(load64le(slot + w*8) ^ s);
        return acc ^ (acc >> 31);
    }
    return fold_honest_scalar(slot, words, s);
}

int sh_leg(const uint8_t *terrain, uint64_t terrain_bytes, uint64_t slot_bytes,
           uint64_t steps, const uint8_t previous[32], int mode, int optimized,
           uint64_t *result, uint64_t trace_s[4], uint64_t trace_i[4],
           uint64_t trace_acc[4], uint64_t trace_next[4], int *revisited) {
    if (!terrain || !previous || !result || slot_bytes < 8 || (slot_bytes & 7) ||
        terrain_bytes % slot_bytes) return -1;
    const uint64_t words = slot_bytes / 8;
    const uint64_t slots = terrain_bytes / slot_bytes;
    uint64_t s = load64le(previous);
    uint8_t *seen = NULL;
    uint64_t *cache = NULL;
    if (mode == 9) {
        seen = (uint8_t *)calloc((size_t)slots, 1);
        cache = (uint64_t *)calloc((size_t)slots, sizeof(uint64_t));
        if (!seen || !cache) { free(seen); free(cache); return -2; }
    }
    int repeats = 0;
    for (uint64_t t = 0; t < steps; ++t) {
        uint64_t idx = s % slots;
        const uint8_t *slot = terrain + idx * slot_bytes;
        uint64_t folded;
        if (mode == 9 && seen[idx]) {
            folded = cache[idx];
            repeats = 1;
        } else if (mode != 0) {
            folded = fold_variant(slot, words, s, mode);
        } else {
#if defined(__AVX512F__) && defined(__AVX512DQ__)
            folded = optimized ? fold_honest_avx512(slot, words, s)
                               : fold_honest_scalar(slot, words, s);
#else
            (void)optimized;
            folded = fold_honest_scalar(slot, words, s);
#endif
        }
        if (mode == 9 && !seen[idx]) {
            seen[idx] = 1;
            cache[idx] = folded;
        }
        uint64_t next = seal(folded);
        if (t < 4) {
            trace_s[t] = s;
            trace_i[t] = idx;
            trace_acc[t] = folded;
            trace_next[t] = next;
        }
        s = next;
    }
    free(seen);
    free(cache);
    *result = s;
    if (revisited) *revisited = repeats;
    return 0;
}

uint64_t sh_mix64(uint64_t x) { return mix64_scalar(x); }

const char *sh_backend(void) {
#if defined(__AVX512F__) && defined(__AVX512DQ__)
    return "avx512dq+openssl";
#else
    return "scalar+openssl";
#endif
}
