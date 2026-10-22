/* FIPS 180-4 SHA-256.
 * §11.1: where the host has SHA-256 instructions, every SHA-256 uses them.
 * The SHA-NI compress is compiled for that ISA only
 * (__attribute__((target("sha,sse4.1")))) and selected at runtime by cpuid.
 * The rest of this file is the baseline ISA. Software runs only when the
 * host has no SHA-NI. A SHA-NI self-check against the §7.4 "abc" digest
 * aborts if the instructions do not match; it does not fall back.
 */
#include "pcoc.h"

#include <cpuid.h>
#include <immintrin.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t rotr32(uint32_t x, int n) {
  return (x >> n) | (x << (32 - n));
}

static uint32_t load_be32(const uint8_t *p) {
  return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) |
         (uint32_t)p[3];
}

static void store_be32(uint8_t *p, uint32_t v) {
  p[0] = (uint8_t)(v >> 24);
  p[1] = (uint8_t)(v >> 16);
  p[2] = (uint8_t)(v >> 8);
  p[3] = (uint8_t)v;
}

static const uint32_t K256[64] = {
    0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u, 0x3956c25bu, 0x59f111f1u,
    0x923f82a4u, 0xab1c5ed5u, 0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u,
    0x72be5d74u, 0x80deb1feu, 0x9bdc06a7u, 0xc19bf174u, 0xe49b69c1u, 0xefbe4786u,
    0x0fc19dc6u, 0x240ca1ccu, 0x2de92c6fu, 0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau,
    0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u, 0xc6e00bf3u, 0xd5a79147u,
    0x06ca6351u, 0x14292967u, 0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcu, 0x53380d13u,
    0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u, 0xa2bfe8a1u, 0xa81a664bu,
    0xc24b8b70u, 0xc76c51a3u, 0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u,
    0x19a4c116u, 0x1e376c08u, 0x2748774cu, 0x34b0bcb5u, 0x391c0cb3u, 0x4ed8aa4au,
    0x5b9cca4fu, 0x682e6ff3u, 0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u,
    0x90befffau, 0xa4506cebu, 0xbef9a3f7u, 0xc67178f2u};

static void compress_sw(uint32_t s[8], const uint8_t block[64]) {
  uint32_t w[64];
  int i;
  for (i = 0; i < 16; i++) {
    w[i] = load_be32(block + 4 * i);
  }
  for (i = 16; i < 64; i++) {
    uint32_t s0 = rotr32(w[i - 15], 7) ^ rotr32(w[i - 15], 18) ^ (w[i - 15] >> 3);
    uint32_t s1 = rotr32(w[i - 2], 17) ^ rotr32(w[i - 2], 19) ^ (w[i - 2] >> 10);
    w[i] = w[i - 16] + s0 + w[i - 7] + s1;
  }
  uint32_t a = s[0], b = s[1], c = s[2], d = s[3];
  uint32_t e = s[4], f = s[5], g = s[6], h = s[7];
  for (i = 0; i < 64; i++) {
    uint32_t S1 = rotr32(e, 6) ^ rotr32(e, 11) ^ rotr32(e, 25);
    uint32_t ch = (e & f) ^ ((~e) & g);
    uint32_t t1 = h + S1 + ch + K256[i] + w[i];
    uint32_t S0 = rotr32(a, 2) ^ rotr32(a, 13) ^ rotr32(a, 22);
    uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
    uint32_t t2 = S0 + maj;
    h = g;
    g = f;
    f = e;
    e = d + t1;
    d = c;
    c = b;
    b = a;
    a = t1 + t2;
  }
  s[0] += a;
  s[1] += b;
  s[2] += c;
  s[3] += d;
  s[4] += e;
  s[5] += f;
  s[6] += g;
  s[7] += h;
}

/* One 64-byte block. State words are host-endian SHA-256 working state. */
__attribute__((target("sha,sse4.1"), noinline)) static void compress_ni(uint32_t state[8],
                                                                       const uint8_t block[64]) {
  __m128i STATE0, STATE1, MSG, TMP;
  __m128i MSG0, MSG1, MSG2, MSG3;
  __m128i ABEF_SAVE, CDGH_SAVE;
  const __m128i MASK = _mm_set_epi64x(0x0c0d0e0f08090a0bULL, 0x0405060700010203ULL);

  TMP = _mm_loadu_si128((const __m128i *)&state[0]);
  STATE1 = _mm_loadu_si128((const __m128i *)&state[4]);
  TMP = _mm_shuffle_epi32(TMP, 0xB1);
  STATE1 = _mm_shuffle_epi32(STATE1, 0x1B);
  STATE0 = _mm_alignr_epi8(TMP, STATE1, 8);
  STATE1 = _mm_blend_epi16(STATE1, TMP, 0xF0);

  ABEF_SAVE = STATE0;
  CDGH_SAVE = STATE1;

  MSG = _mm_loadu_si128((const __m128i *)(block + 0));
  MSG0 = _mm_shuffle_epi8(MSG, MASK);
  MSG = _mm_add_epi32(MSG0, _mm_set_epi64x(0xE9B5DBA5B5C0FBCFULL, 0x71374491428A2F98ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

  MSG = _mm_loadu_si128((const __m128i *)(block + 16));
  MSG1 = _mm_shuffle_epi8(MSG, MASK);
  MSG = _mm_add_epi32(MSG1, _mm_set_epi64x(0xAB1C5ED5923F82A4ULL, 0x59F111F13956C25BULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG0 = _mm_sha256msg1_epu32(MSG0, MSG1);

  MSG = _mm_loadu_si128((const __m128i *)(block + 32));
  MSG2 = _mm_shuffle_epi8(MSG, MASK);
  MSG = _mm_add_epi32(MSG2, _mm_set_epi64x(0x550C7DC3243185BEULL, 0x12835B01D807AA98ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG1 = _mm_sha256msg1_epu32(MSG1, MSG2);

  MSG = _mm_loadu_si128((const __m128i *)(block + 48));
  MSG3 = _mm_shuffle_epi8(MSG, MASK);
  MSG = _mm_add_epi32(MSG3, _mm_set_epi64x(0xC19BF1749BDC06A7ULL, 0x80DEB1FE72BE5D74ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG3, MSG2, 4);
  MSG0 = _mm_add_epi32(MSG0, TMP);
  MSG0 = _mm_sha256msg2_epu32(MSG0, MSG3);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG2 = _mm_sha256msg1_epu32(MSG2, MSG3);

  MSG = _mm_add_epi32(MSG0, _mm_set_epi64x(0x240CA1CC0FC19DC6ULL, 0xEFBE4786E49B69C1ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG0, MSG3, 4);
  MSG1 = _mm_add_epi32(MSG1, TMP);
  MSG1 = _mm_sha256msg2_epu32(MSG1, MSG0);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG3 = _mm_sha256msg1_epu32(MSG3, MSG0);

  MSG = _mm_add_epi32(MSG1, _mm_set_epi64x(0x76F988DA5CB0A9DCULL, 0x4A7484AA2DE92C6FULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG1, MSG0, 4);
  MSG2 = _mm_add_epi32(MSG2, TMP);
  MSG2 = _mm_sha256msg2_epu32(MSG2, MSG1);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG0 = _mm_sha256msg1_epu32(MSG0, MSG1);

  MSG = _mm_add_epi32(MSG2, _mm_set_epi64x(0xBF597FC7B00327C8ULL, 0xA831C66D983E5152ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG2, MSG1, 4);
  MSG3 = _mm_add_epi32(MSG3, TMP);
  MSG3 = _mm_sha256msg2_epu32(MSG3, MSG2);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG1 = _mm_sha256msg1_epu32(MSG1, MSG2);

  MSG = _mm_add_epi32(MSG3, _mm_set_epi64x(0x1429296706CA6351ULL, 0xD5A79147C6E00BF3ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG3, MSG2, 4);
  MSG0 = _mm_add_epi32(MSG0, TMP);
  MSG0 = _mm_sha256msg2_epu32(MSG0, MSG3);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG2 = _mm_sha256msg1_epu32(MSG2, MSG3);

  MSG = _mm_add_epi32(MSG0, _mm_set_epi64x(0x53380D134D2C6DFCULL, 0x2E1B213827B70A85ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG0, MSG3, 4);
  MSG1 = _mm_add_epi32(MSG1, TMP);
  MSG1 = _mm_sha256msg2_epu32(MSG1, MSG0);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG3 = _mm_sha256msg1_epu32(MSG3, MSG0);

  MSG = _mm_add_epi32(MSG1, _mm_set_epi64x(0x92722C8581C2C92EULL, 0x766A0ABB650A7354ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG1, MSG0, 4);
  MSG2 = _mm_add_epi32(MSG2, TMP);
  MSG2 = _mm_sha256msg2_epu32(MSG2, MSG1);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG0 = _mm_sha256msg1_epu32(MSG0, MSG1);

  MSG = _mm_add_epi32(MSG2, _mm_set_epi64x(0xC76C51A3C24B8B70ULL, 0xA81A664BA2BFE8A1ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG2, MSG1, 4);
  MSG3 = _mm_add_epi32(MSG3, TMP);
  MSG3 = _mm_sha256msg2_epu32(MSG3, MSG2);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG1 = _mm_sha256msg1_epu32(MSG1, MSG2);

  MSG = _mm_add_epi32(MSG3, _mm_set_epi64x(0x106AA070F40E3585ULL, 0xD6990624D192E819ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG3, MSG2, 4);
  MSG0 = _mm_add_epi32(MSG0, TMP);
  MSG0 = _mm_sha256msg2_epu32(MSG0, MSG3);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG2 = _mm_sha256msg1_epu32(MSG2, MSG3);

  MSG = _mm_add_epi32(MSG0, _mm_set_epi64x(0x34B0BCB52748774CULL, 0x1E376C0819A4C116ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG0, MSG3, 4);
  MSG1 = _mm_add_epi32(MSG1, TMP);
  MSG1 = _mm_sha256msg2_epu32(MSG1, MSG0);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
  MSG3 = _mm_sha256msg1_epu32(MSG3, MSG0);

  MSG = _mm_add_epi32(MSG1, _mm_set_epi64x(0x682E6FF35B9CCA4FULL, 0x4ED8AA4A391C0CB3ULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG1, MSG0, 4);
  MSG2 = _mm_add_epi32(MSG2, TMP);
  MSG2 = _mm_sha256msg2_epu32(MSG2, MSG1);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

  MSG = _mm_add_epi32(MSG2, _mm_set_epi64x(0x8CC7020884C87814ULL, 0x78A5636F748F82EEULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  TMP = _mm_alignr_epi8(MSG2, MSG1, 4);
  MSG3 = _mm_add_epi32(MSG3, TMP);
  MSG3 = _mm_sha256msg2_epu32(MSG3, MSG2);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

  MSG = _mm_add_epi32(MSG3, _mm_set_epi64x(0xC67178F2BEF9A3F7ULL, 0xA4506CEB90BEFFFAULL));
  STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
  MSG = _mm_shuffle_epi32(MSG, 0x0E);
  STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

  STATE0 = _mm_add_epi32(STATE0, ABEF_SAVE);
  STATE1 = _mm_add_epi32(STATE1, CDGH_SAVE);

  TMP = _mm_shuffle_epi32(STATE0, 0x1B);
  STATE1 = _mm_shuffle_epi32(STATE1, 0xB1);
  STATE0 = _mm_blend_epi16(TMP, STATE1, 0xF0);
  STATE1 = _mm_alignr_epi8(STATE1, TMP, 8);

  _mm_storeu_si128((__m128i *)&state[0], STATE0);
  _mm_storeu_si128((__m128i *)&state[4], STATE1);
}

typedef void (*compress_fn)(uint32_t state[8], const uint8_t block[64]);

static compress_fn g_compress;
static const char *g_backend = "software";
static int g_ready;

static int host_has_sha_ni(void) {
  unsigned int eax = 0, ebx = 0, ecx = 0, edx = 0;
  if (!__get_cpuid(0, &eax, &ebx, &ecx, &edx)) {
    return 0;
  }
  if (eax < 7) {
    return 0;
  }
  if (!__get_cpuid_count(7, 0, &eax, &ebx, &ecx, &edx)) {
    return 0;
  }
  return (ebx & (1u << 29)) != 0;
}

static void hash_with(compress_fn fn, const uint8_t *data, size_t len, uint8_t out[32]) {
  uint32_t s[8] = {0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
                   0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u};
  uint8_t block[64];
  size_t off = 0;
  while (off + 64 <= len) {
    fn(s, data + off);
    off += 64;
  }
  size_t rem = len - off;
  memset(block, 0, 64);
  if (rem) {
    memcpy(block, data + off, rem);
  }
  block[rem] = 0x80;
  if (rem >= 56) {
    fn(s, block);
    memset(block, 0, 64);
  }
  uint64_t bits = (uint64_t)len * 8u;
  store_be32(block + 56, (uint32_t)(bits >> 32));
  store_be32(block + 60, (uint32_t)bits);
  fn(s, block);
  for (int i = 0; i < 8; i++) {
    store_be32(out + 4 * i, s[i]);
  }
}

static const uint8_t ABC_DIGEST[32] = {
    0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
    0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad};

static void sha_init(void) {
  if (g_ready) {
    return;
  }
  if (host_has_sha_ni()) {
    uint8_t out[32];
    hash_with(compress_ni, (const uint8_t *)"abc", 3, out);
    if (memcmp(out, ABC_DIGEST, 32) != 0) {
      fprintf(stderr, "sha-ni compress does not match the section 7.4 abc digest\n");
      abort();
    }
    g_compress = compress_ni;
    g_backend = "sha-ni";
  } else {
    g_compress = compress_sw;
    g_backend = "software";
  }
  g_ready = 1;
}

const char *pcoc_sha256_backend(void) {
  sha_init();
  return g_backend;
}

void pcoc_sha256(const uint8_t *data, size_t len, uint8_t out[32]) {
  sha_init();
  hash_with(g_compress, data, len, out);
}

void pcoc_sha256d_80(const uint8_t header[80], uint8_t out[32]) {
  uint8_t mid[32];
  pcoc_sha256(header, 80, mid);
  pcoc_sha256(mid, 32, out);
}
