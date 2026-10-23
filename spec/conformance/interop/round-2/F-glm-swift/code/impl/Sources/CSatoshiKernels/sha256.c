/* SHA-256 per FIPS 180-4.
 * Scalar reference implementation, plus an x86 SHA-NI transform
 * (SPEC 2.2 section 11.1 requires SHA-NI where the host provides it;
 * hardware and software SHA-256 give identical bytes).
 * Runtime dispatch by CPUID. Differential-tested against the scalar path
 * over FIPS vectors and random multi-block buffers.
 */
#include "shk.h"

#include <string.h>
#include <stdlib.h>
#include <immintrin.h>

/* ---------------------------------------------------------------- scalar */

static const uint32_t K256[64] = {
    /* FIPS 180-4: fractional parts of the cube roots of the first 64 primes */
    0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u,
    0x3956c25bu, 0x59f111f1u, 0x923f82a4u, 0xab1c5ed5u,
    0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u,
    0x72be5d74u, 0x80deb1feu, 0x9bdc06a7u, 0xc19bf174u,
    0xe49b69c1u, 0xefbe4786u, 0x0fc19dc6u, 0x240ca1ccu,
    0x2de92c6fu, 0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau,
    0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u,
    0xc6e00bf3u, 0xd5a79147u, 0x06ca6351u, 0x14292967u,
    0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcu, 0x53380d13u,
    0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u,
    0xa2bfe8a1u, 0xa81a664bu, 0xc24b8b70u, 0xc76c51a3u,
    0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u,
    0x19a4c116u, 0x1e376c08u, 0x2748774cu, 0x34b0bcb5u,
    0x391c0cb3u, 0x4ed8aa4au, 0x5b9cca4fu, 0x682e6ff3u,
    0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u,
    0x90befffau, 0xa4506cebu, 0xbef9a3f7u, 0xc67178f2u,
};

static const uint32_t IV256[8] = {
    0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
    0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u,
};

static inline uint32_t ror32(uint32_t x, unsigned r) {
    return (x >> r) | (x << (32 - r));
}

static void sha256_blocks_scalar(uint32_t st[8], const uint8_t *p, size_t nblocks) {
    uint32_t w[64];
    for (size_t nb = 0; nb < nblocks; nb++, p += 64) {
        for (int i = 0; i < 16; i++) {
            const uint8_t *q = p + 4 * i;
            w[i] = ((uint32_t)q[0] << 24) | ((uint32_t)q[1] << 16) |
                   ((uint32_t)q[2] << 8) | (uint32_t)q[3];
        }
        for (int i = 16; i < 64; i++) {
            uint32_t s0 = ror32(w[i - 15], 7) ^ ror32(w[i - 15], 18) ^ (w[i - 15] >> 3);
            uint32_t s1 = ror32(w[i - 2], 17) ^ ror32(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        uint32_t a = st[0], b = st[1], c = st[2], d = st[3];
        uint32_t e = st[4], f = st[5], g = st[6], h = st[7];
        for (int i = 0; i < 64; i++) {
            uint32_t S1 = ror32(e, 6) ^ ror32(e, 11) ^ ror32(e, 25);
            uint32_t ch = (e & f) ^ (~e & g);
            uint32_t t1 = h + S1 + ch + K256[i] + w[i];
            uint32_t S0 = ror32(a, 2) ^ ror32(a, 13) ^ ror32(a, 22);
            uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
            uint32_t t2 = S0 + maj;
            h = g; g = f; f = e; e = d + t1;
            d = c; c = b; b = a; a = t1 + t2;
        }
        st[0] += a; st[1] += b; st[2] += c; st[3] += d;
        st[4] += e; st[5] += f; st[6] += g; st[7] += h;
    }
}

/* --------------------------------------------------------------- SHA-NI */

#if defined(__x86_64__)

/* Round constants packed per 4-round group straight from K256:
 * low 64-bit element = (K[4g+1] << 32) | K[4g], high = (K[4g+3] << 32) | K[4g+2].
 * Built from the same table as the scalar path, so the two cannot diverge. */
#define KQ(g)                                                                  \
    _mm_set_epi64x(                                                            \
        (int64_t)(((uint64_t)K256[4 * (g) + 3] << 32) | K256[4 * (g) + 2]),    \
        (int64_t)(((uint64_t)K256[4 * (g) + 1] << 32) | K256[4 * (g)]))

__attribute__((target("sha,sse4.1,ssse3")))
static void sha256_blocks_ni(uint32_t state[8], const uint8_t *data, size_t nblocks) {
    __m128i STATE0, STATE1, TMP;
    const __m128i MASK =
        _mm_set_epi64x(0x0c0d0e0f08090a0bULL, 0x0405060700010203ULL);

    TMP = _mm_loadu_si128((const __m128i *)&state[0]);     /* A B C D */
    STATE1 = _mm_loadu_si128((const __m128i *)&state[4]); /* E F G H */
    TMP = _mm_shuffle_epi32(TMP, 0xB1);                   /* B A D C */
    STATE1 = _mm_shuffle_epi32(STATE1, 0x1B);             /* H G F E */
    STATE0 = _mm_alignr_epi8(TMP, STATE1, 8);             /* ABEF */
    STATE1 = _mm_blend_epi16(STATE1, TMP, 0xF0);          /* CDGH */

    while (nblocks--) {
        __m128i MSG, MSG0, MSG1, MSG2, MSG3, ABEF_SAVE, CDGH_SAVE;
        ABEF_SAVE = STATE0;
        CDGH_SAVE = STATE1;

        /* Rounds 0-3 */
        MSG0 = _mm_shuffle_epi8(_mm_loadu_si128((const __m128i *)data), MASK);
        MSG = _mm_add_epi32(MSG0, KQ(0));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

        /* Rounds 4-7 */
        MSG1 = _mm_shuffle_epi8(_mm_loadu_si128((const __m128i *)(data + 16)), MASK);
        MSG = _mm_add_epi32(MSG1, KQ(1));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG0 = _mm_sha256msg1_epu32(MSG0, MSG1);

        /* Rounds 8-11 */
        MSG2 = _mm_shuffle_epi8(_mm_loadu_si128((const __m128i *)(data + 32)), MASK);
        MSG = _mm_add_epi32(MSG2, KQ(2));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG1 = _mm_sha256msg1_epu32(MSG1, MSG2);

        /* Rounds 12-15 */
        MSG3 = _mm_shuffle_epi8(_mm_loadu_si128((const __m128i *)(data + 48)), MASK);
        MSG = _mm_add_epi32(MSG3, KQ(3));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG3, MSG2, 4);
        MSG0 = _mm_add_epi32(MSG0, TMP);
        MSG0 = _mm_sha256msg2_epu32(MSG0, MSG3);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG2 = _mm_sha256msg1_epu32(MSG2, MSG3);

        /* Rounds 16-19 */
        MSG = _mm_add_epi32(MSG0, KQ(4));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG0, MSG3, 4);
        MSG1 = _mm_add_epi32(MSG1, TMP);
        MSG1 = _mm_sha256msg2_epu32(MSG1, MSG0);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG3 = _mm_sha256msg1_epu32(MSG3, MSG0);

        /* Rounds 20-23 */
        MSG = _mm_add_epi32(MSG1, KQ(5));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG1, MSG0, 4);
        MSG2 = _mm_add_epi32(MSG2, TMP);
        MSG2 = _mm_sha256msg2_epu32(MSG2, MSG1);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG0 = _mm_sha256msg1_epu32(MSG0, MSG1);

        /* Rounds 24-27 */
        MSG = _mm_add_epi32(MSG2, KQ(6));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG2, MSG1, 4);
        MSG3 = _mm_add_epi32(MSG3, TMP);
        MSG3 = _mm_sha256msg2_epu32(MSG3, MSG2);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG1 = _mm_sha256msg1_epu32(MSG1, MSG2);

        /* Rounds 28-31 */
        MSG = _mm_add_epi32(MSG3, KQ(7));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG3, MSG2, 4);
        MSG0 = _mm_add_epi32(MSG0, TMP);
        MSG0 = _mm_sha256msg2_epu32(MSG0, MSG3);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG2 = _mm_sha256msg1_epu32(MSG2, MSG3);

        /* Rounds 32-35 */
        MSG = _mm_add_epi32(MSG0, KQ(8));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG0, MSG3, 4);
        MSG1 = _mm_add_epi32(MSG1, TMP);
        MSG1 = _mm_sha256msg2_epu32(MSG1, MSG0);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG3 = _mm_sha256msg1_epu32(MSG3, MSG0);

        /* Rounds 36-39 */
        MSG = _mm_add_epi32(MSG1, KQ(9));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG1, MSG0, 4);
        MSG2 = _mm_add_epi32(MSG2, TMP);
        MSG2 = _mm_sha256msg2_epu32(MSG2, MSG1);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG0 = _mm_sha256msg1_epu32(MSG0, MSG1);

        /* Rounds 40-43 */
        MSG = _mm_add_epi32(MSG2, KQ(10));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG2, MSG1, 4);
        MSG3 = _mm_add_epi32(MSG3, TMP);
        MSG3 = _mm_sha256msg2_epu32(MSG3, MSG2);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG1 = _mm_sha256msg1_epu32(MSG1, MSG2);

        /* Rounds 44-47 */
        MSG = _mm_add_epi32(MSG3, KQ(11));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG3, MSG2, 4);
        MSG0 = _mm_add_epi32(MSG0, TMP);
        MSG0 = _mm_sha256msg2_epu32(MSG0, MSG3);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG2 = _mm_sha256msg1_epu32(MSG2, MSG3);

        /* Rounds 48-51 */
        MSG = _mm_add_epi32(MSG0, KQ(12));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG0, MSG3, 4);
        MSG1 = _mm_add_epi32(MSG1, TMP);
        MSG1 = _mm_sha256msg2_epu32(MSG1, MSG0);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
        MSG3 = _mm_sha256msg1_epu32(MSG3, MSG0);

        /* Rounds 52-55 */
        MSG = _mm_add_epi32(MSG1, KQ(13));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG1, MSG0, 4);
        MSG2 = _mm_add_epi32(MSG2, TMP);
        MSG2 = _mm_sha256msg2_epu32(MSG2, MSG1);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

        /* Rounds 56-59 */
        MSG = _mm_add_epi32(MSG2, KQ(14));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        TMP = _mm_alignr_epi8(MSG2, MSG1, 4);
        MSG3 = _mm_add_epi32(MSG3, TMP);
        MSG3 = _mm_sha256msg2_epu32(MSG3, MSG2);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

        /* Rounds 60-63 */
        MSG = _mm_add_epi32(MSG3, KQ(15));
        STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
        MSG = _mm_shuffle_epi32(MSG, 0x0E);
        STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

        STATE0 = _mm_add_epi32(STATE0, ABEF_SAVE);
        STATE1 = _mm_add_epi32(STATE1, CDGH_SAVE);
        data += 64;
    }

    TMP = _mm_shuffle_epi32(STATE0, 0x1B);       /* FEBA */
    STATE1 = _mm_shuffle_epi32(STATE1, 0xB1);    /* DCHG */
    STATE0 = _mm_blend_epi16(TMP, STATE1, 0xF0); /* DCBA */
    STATE1 = _mm_alignr_epi8(STATE1, TMP, 8);    /* ABEF */
    _mm_storeu_si128((__m128i *)&state[0], STATE0);
    _mm_storeu_si128((__m128i *)&state[4], STATE1);
}

#endif /* __x86_64__ */

/* ------------------------------------------------------------- dispatch */

static int sha_mode_auto(void) {
    static int cached = -1;
    if (cached < 0) cached = shk_cpu_has_sha_ni() ? 2 : 1;
    return cached;
}

static void sha256_emit(const uint32_t st[8], uint8_t out[32]) {
    for (int i = 0; i < 8; i++) {
        out[4 * i + 0] = (uint8_t)(st[i] >> 24);
        out[4 * i + 1] = (uint8_t)(st[i] >> 16);
        out[4 * i + 2] = (uint8_t)(st[i] >> 8);
        out[4 * i + 3] = (uint8_t)(st[i]);
    }
}

void shk_sha256_mode(const uint8_t *data, size_t len, uint8_t out[32], int mode) {
    int use_ni;
    if (mode == 0) use_ni = sha_mode_auto() == 2;
    else if (mode == 2) use_ni = shk_cpu_has_sha_ni();
    else use_ni = 0;

    uint32_t st[8];
    for (int i = 0; i < 8; i++) st[i] = IV256[i];
    size_t full = len & ~(size_t)63;
    if (full) {
#if defined(__x86_64__)
        if (use_ni) sha256_blocks_ni(st, data, full / 64);
        else
#endif
        sha256_blocks_scalar(st, data, full / 64);
    }
    uint8_t tail[128];
    size_t rem = len - full;
    if (rem) memcpy(tail, data + full, rem);
    tail[rem] = 0x80;
    size_t tailsz = (rem < 56) ? 64 : 128;
    memset(tail + rem + 1, 0, tailsz - rem - 9);
    uint64_t bits = (uint64_t)len * 8;
    for (int i = 0; i < 8; i++) tail[tailsz - 1 - i] = (uint8_t)(bits >> (8 * i));
#if defined(__x86_64__)
    if (use_ni) sha256_blocks_ni(st, tail, tailsz / 64);
    else
#endif
    sha256_blocks_scalar(st, tail, tailsz / 64);
    sha256_emit(st, out);
}

void shk_sha256(const uint8_t *data, size_t len, uint8_t out[32]) {
    shk_sha256_mode(data, len, out, 0);
}

void shk_sha256_parts(const uint8_t *const *parts, const size_t *lens,
                      size_t n, uint8_t out[32]) {
    size_t total = 0;
    for (size_t i = 0; i < n; i++) total += lens[i];
    uint8_t *buf = (uint8_t *)malloc(total ? total : 1);
    size_t off = 0;
    for (size_t i = 0; i < n; i++) {
        memcpy(buf + off, parts[i], lens[i]);
        off += lens[i];
    }
    shk_sha256(buf, total, out);
    free(buf);
}

/* --------------------------------------------------------------- seal */

static const uint8_t SEAL_DOMAIN[12] = {
    'p', 'c', 'o', 'c', '-', 'w', 'a', 'l', 'k', '-', 'v', '1' /* "pcoc-walk-v1" */
};

void shk_seal_digest(uint64_t acc, uint8_t out[32]) {
    /* message = 12 domain bytes + 8 LE bytes = 20 bytes; single padded block. */
    uint8_t blk[64];
    memcpy(blk, SEAL_DOMAIN, 12);
    for (int i = 0; i < 8; i++) blk[12 + i] = (uint8_t)(acc >> (8 * i));
    blk[20] = 0x80;
    memset(blk + 21, 0, 35);
    uint64_t bits = 160;
    for (int i = 0; i < 8; i++) blk[63 - i] = (uint8_t)(bits >> (8 * i));
    uint32_t st[8];
    for (int i = 0; i < 8; i++) st[i] = IV256[i];
#if defined(__x86_64__)
    if (sha_mode_auto() == 2) sha256_blocks_ni(st, blk, 1);
    else
#endif
    sha256_blocks_scalar(st, blk, 1);
    sha256_emit(st, out);
}

uint64_t shk_seal_step(uint64_t acc) {
    uint8_t d[32];
    shk_seal_digest(acc, d);
    uint64_t v = 0;
    for (int i = 0; i < 8; i++) v |= (uint64_t)d[i] << (8 * i);
    return v;
}
