/* SPEC §13.6 Rule BR1, the Bari paid-waypoint check.
 * Block-hash comparisons are natural order on both sides (SPEC §9, P9.17).
 * This file does not reverse a hash.
 */
#include "pcoc.h"

#include <string.h>

enum { BR1_INTERVAL = 2016, BR1_TIMESPAN = 14 * 24 * 60 * 60 };

/* Limb 0 is the least significant. 256-bit width, arith_uint256 style. */
typedef struct {
  uint64_t w[4];
} u256;

static void u256_clear(u256 *a) { memset(a, 0, sizeof(*a)); }

static int u256_is_zero(const u256 *a) {
  return a->w[0] == 0 && a->w[1] == 0 && a->w[2] == 0 && a->w[3] == 0;
}

static void u256_from_le(u256 *o, const uint8_t in[32]) {
  for (int i = 0; i < 4; i++) {
    o->w[i] = pcoc_u64_le(in + (size_t)i * 8);
  }
}

static void u256_to_le(const u256 *a, uint8_t out[32]) {
  for (int i = 0; i < 4; i++) {
    pcoc_le64(a->w[i], out + (size_t)i * 8);
  }
}

static int u256_cmp(const u256 *a, const u256 *b) {
  for (int i = 3; i >= 0; i--) {
    if (a->w[i] < b->w[i]) return -1;
    if (a->w[i] > b->w[i]) return 1;
  }
  return 0;
}

static uint32_t u256_bitlen(const u256 *a) {
  for (int i = 3; i >= 0; i--) {
    if (a->w[i] != 0) {
      return (uint32_t)i * 64u + (64u - (uint32_t)__builtin_clzll(a->w[i]));
    }
  }
  return 0;
}

/* Whole-byte shift. Bits that leave the 256-bit width are dropped. */
static void u256_shl_bytes(u256 *o, const u256 *a, uint32_t nbytes) {
  u256 src = *a;
  if (nbytes >= 32u) {
    u256_clear(o);
    return;
  }
  uint8_t in[32], outb[32];
  u256_to_le(&src, in);
  memset(outb, 0, sizeof(outb));
  memcpy(outb + nbytes, in, 32u - nbytes);
  u256_from_le(o, outb);
}

static void u256_shr_bytes(u256 *o, const u256 *a, uint32_t nbytes) {
  u256 src = *a;
  if (nbytes >= 32u) {
    u256_clear(o);
    return;
  }
  uint8_t in[32], outb[32];
  u256_to_le(&src, in);
  memset(outb, 0, sizeof(outb));
  memcpy(outb, in + nbytes, 32u - nbytes);
  u256_from_le(o, outb);
}

/* Truncating 256-bit product with a u64. The high carry is dropped. */
static void u256_mul_u64(u256 *o, const u256 *a, uint64_t factor) {
  u256 src = *a;
  unsigned __int128 carry = 0;
  u256 out;
  for (int i = 0; i < 4; i++) {
    unsigned __int128 prod = (unsigned __int128)src.w[i] * (unsigned __int128)factor + carry;
    out.w[i] = (uint64_t)prod;
    carry = prod >> 64;
  }
  *o = out;
}

static void u256_div_u64(u256 *o, const u256 *a, uint64_t divisor) {
  u256 src = *a;
  unsigned __int128 rem = 0;
  u256 out;
  for (int i = 3; i >= 0; i--) {
    unsigned __int128 cur = (rem << 64) | (unsigned __int128)src.w[i];
    out.w[i] = (uint64_t)(cur / divisor);
    rem = cur % divisor;
  }
  *o = out;
}

/* Bitcoin arith_uint256::SetCompact. *neg and *over are negative and overflow. */
static void set_compact(uint32_t bits, u256 *value, int *neg, int *over) {
  int size = (int)(bits >> 24);
  uint32_t word = bits & 0x007fffffu;
  if (word == 0) {
    u256_clear(value);
  } else if (size <= 3) {
    unsigned sh = (unsigned)(8 * (3 - size));
    u256_clear(value);
    value->w[0] = (uint64_t)word >> sh;
  } else {
    u256 base;
    u256_clear(&base);
    base.w[0] = word;
    u256_shl_bytes(value, &base, (uint32_t)(size - 3));
  }
  *neg = word != 0 && (bits & 0x00800000u) != 0;
  *over = word != 0 && (size > 34 || (word > 0xffu && size > 33) || (word > 0xffffu && size > 32));
}

/* Bitcoin arith_uint256::GetCompact for a non-negative value. */
static uint32_t get_compact(const u256 *a) {
  if (u256_is_zero(a)) return 0;
  uint32_t size = (u256_bitlen(a) + 7u) / 8u;
  uint64_t compact;
  if (size <= 3u) {
    compact = a->w[0] << (8u * (3u - size));
  } else {
    u256 shifted;
    u256_shr_bytes(&shifted, a, size - 3u);
    compact = shifted.w[0];
  }
  if ((compact & 0x00800000ull) != 0) {
    compact >>= 8;
    size += 1u;
  }
  return ((uint32_t)compact & 0x007fffffu) | (size << 24);
}

static uint32_t u32_le(const uint8_t b[4]) {
  return (uint32_t)b[0] | ((uint32_t)b[1] << 8) | ((uint32_t)b[2] << 16) | ((uint32_t)b[3] << 24);
}

static void put_u32_le(uint8_t b[4], uint32_t x) {
  b[0] = (uint8_t)x;
  b[1] = (uint8_t)(x >> 8);
  b[2] = (uint8_t)(x >> 16);
  b[3] = (uint8_t)(x >> 24);
}

static int hex_nibble(unsigned char c) {
  if (c >= '0' && c <= '9') return (int)(c - '0');
  if (c >= 'a' && c <= 'f') return (int)(c - 'a' + 10);
  return -1;
}

/* SPEC §9. A negative compact, an overflow compact, or a zero target is not proof of work. */
static int pow_meets(const uint8_t header[80]) {
  u256 target;
  int neg = 0, over = 0;
  set_compact(u32_le(header + 72), &target, &neg, &over);
  if (neg || over || u256_is_zero(&target)) return 0;
  uint8_t hash[32];
  pcoc_sha256d_80(header, hash);
  u256 got;
  u256_from_le(&got, hash);
  return u256_cmp(&got, &target) <= 0;
}

int pcoc_br1_parse_nonce(const char *raw, uint64_t *nonce) {
  if (!raw || !nonce || raw[0] == '\0') return -1;
  if (raw[0] == '0' && raw[1] != '\0') return -1;
  uint64_t v = 0;
  for (const unsigned char *p = (const unsigned char *)raw; *p; p++) {
    if (*p < '0' || *p > '9') return -1;
    uint64_t d = (uint64_t)(*p - '0');
    if (v > (UINT64_MAX - d) / 10ull) return -1;
    v = v * 10ull + d;
  }
  *nonce = v;
  return 0;
}

int pcoc_br1_parse_fee_header(const char *raw, uint8_t out[80]) {
  if (!raw || !out || strlen(raw) != 160) return -1;
  for (int i = 0; i < 80; i++) {
    int hi = hex_nibble((unsigned char)raw[2 * i]);
    int lo = hex_nibble((unsigned char)raw[2 * i + 1]);
    if (hi < 0 || lo < 0) return -1;
    out[i] = (uint8_t)((hi << 4) | lo);
  }
  return 0;
}

void pcoc_br1_assemble_header(const uint8_t previous[32], uint32_t n_time, uint32_t bits,
                              uint32_t nonce, uint8_t out[80]) {
  memset(out, 0, 80);
  out[0] = 1; /* version u32le = 1 */
  memcpy(out + 4, previous, 32);
  put_u32_le(out + 68, n_time);
  put_u32_le(out + 72, bits);
  put_u32_le(out + 76, nonce);
}

uint32_t pcoc_br1_mainnet_next_bits(uint32_t height, uint32_t tip_bits, uint32_t tip_time,
                                    uint32_t period_first_time) {
  if (((height + 1u) % (uint32_t)BR1_INTERVAL) != 0u) return tip_bits;
  int64_t span = (int64_t)tip_time - (int64_t)period_first_time;
  int64_t min_span = (int64_t)BR1_TIMESPAN / 4;
  int64_t max_span = (int64_t)BR1_TIMESPAN * 4;
  if (span < min_span) span = min_span;
  if (span > max_span) span = max_span;
  u256 target;
  int neg = 0, over = 0;
  set_compact(tip_bits, &target, &neg, &over);
  u256_mul_u64(&target, &target, (uint64_t)span);
  u256_div_u64(&target, &target, (uint64_t)BR1_TIMESPAN);
  u256 limit;
  set_compact(PCOC_BR1_POW_LIMIT_BITS, &limit, &neg, &over);
  if (u256_cmp(&target, &limit) > 0) target = limit;
  return get_compact(&target);
}

uint32_t pcoc_br1_required_bits(const pcoc_br1_tip *tip, int plane) {
  if (plane == PCOC_BR1_PLANE_LIVE && tip) {
    return pcoc_br1_mainnet_next_bits(tip->height, tip->bits, tip->time, tip->period_first_time);
  }
  return PCOC_BR1_SYNTH_NBITS;
}

int pcoc_br1_window(const uint8_t *header, size_t header_len, const pcoc_br1_tip *tip, int plane) {
  if (!header || header_len != 80 || !tip) return PCOC_BR1_OUTSIDE;
  uint8_t hash[32];
  pcoc_sha256d_80(header, hash);
  if (memcmp(hash, tip->hash, 32) == 0) return PCOC_BR1_CURRENT;
  if (memcmp(hash, tip->previous, 32) == 0) return PCOC_BR1_BEFORE;
  if (memcmp(header + 4, tip->hash, 32) == 0 && u32_le(header + 72) == pcoc_br1_required_bits(tip, plane) &&
      pow_meets(header)) {
    return PCOC_BR1_AFTER;
  }
  return PCOC_BR1_OUTSIDE;
}

uint16_t pcoc_br1_status(const uint8_t *body, size_t body_len, const char *nonce_decimal,
                         const char *fee_header_hex, const char *bari_origin, const pcoc_br1_tip *tip,
                         int plane) {
  uint8_t header[80];
  uint64_t nonce = 0;
  if (pcoc_br1_parse_fee_header(fee_header_hex, header) != 0) return 402;
  if (pcoc_br1_parse_nonce(nonce_decimal, &nonce) != 0) return 402;
  if (!body || body_len != 32 || !tip || !bari_origin) return 402;
  if (pcoc_br1_window(header, 80, tip, plane) == PCOC_BR1_OUTSIDE) return 402;
  uint8_t hash[32], challenge[32], digest[32];
  pcoc_sha256d_80(header, hash);
  pcoc_hashcash_challenge(bari_origin, strlen(bari_origin), hash, challenge);
  pcoc_hashcash_digest(challenge, body, nonce, digest);
  if (pcoc_leading_zero_bits(digest) < (int)PCOC_HASHCASH_BITS) return 402;
  return 204;
}
