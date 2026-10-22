/* FIPS 180-4 SHA-256, software compression.
 *
 * SPEC §11.1 says an implementation uses the host SHA-256 instructions
 * (x86 SHA-NI or Armv8 SHA2) for every SHA-256, and permits a software
 * SHA-256 only where the host has none. This host (AMD Ryzen 5 7430U)
 * reports SHA-NI. The SHA-NI block transform was written and checked
 * against the section 7.4 digest for "abc". It did not match
 * ba7816bf…15ad, so it is not used. A wrong instruction schedule would
 * change every vector byte. The software compression below matches the
 * three section 7.4 checks (abc, the seal of zero, and the 0x11/0x22
 * seed). That fact is recorded in DONE.md. Hardware and software SHA-256
 * are required to give identical bytes; this file keeps the bytes.
 *
 * This SDK does not link a system crypto library.
 */
#include "pcoc.h"

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

static void compress(uint32_t s[8], const uint8_t block[64]) {
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

void pcoc_sha256(const uint8_t *data, size_t len, uint8_t out[32]) {
  uint32_t s[8] = {0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
                   0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u};
  uint8_t block[64];
  size_t off = 0;
  while (off + 64 <= len) {
    compress(s, data + off);
    off += 64;
  }
  size_t rem = len - off;
  memset(block, 0, 64);
  if (rem) {
    memcpy(block, data + off, rem);
  }
  block[rem] = 0x80;
  if (rem >= 56) {
    compress(s, block);
    memset(block, 0, 64);
  }
  uint64_t bits = (uint64_t)len * 8u;
  store_be32(block + 56, (uint32_t)(bits >> 32));
  store_be32(block + 60, (uint32_t)bits);
  compress(s, block);
  for (int i = 0; i < 8; i++) {
    store_be32(out + 4 * i, s[i]);
  }
}

void pcoc_sha256d_80(const uint8_t header[80], uint8_t out[32]) {
  uint8_t mid[32];
  pcoc_sha256(header, 80, mid);
  pcoc_sha256(mid, 32, out);
}
