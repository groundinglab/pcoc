/* SPEC §13.1 Hashcash digest, §14.6 draw, Rule B2, anchor window, Rule A5.
 * The digest function is in scope. The Bari paid-rule check is a later phase.
 */
#include "pcoc.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void pcoc_hashcash_challenge(const char *origin, size_t origin_len,
                             const uint8_t header_hash[32], uint8_t challenge[32]) {
  /* challenge = SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash) */
  size_t n = 10 + origin_len + 32;
  uint8_t *buf = (uint8_t *)malloc(n);
  memcpy(buf, "bari-hc-v2", 10);
  memcpy(buf + 10, origin, origin_len);
  memcpy(buf + 10 + origin_len, header_hash, 32);
  pcoc_sha256(buf, n, challenge);
  free(buf);
}

void pcoc_hashcash_digest(const uint8_t challenge[32], const uint8_t waypoint[32],
                          uint64_t nonce, uint8_t digest[32]) {
  /* SHA-256(challenge ‖ waypoint ‖ le64(nonce)) */
  uint8_t buf[32 + 32 + 8];
  memcpy(buf, challenge, 32);
  memcpy(buf + 32, waypoint, 32);
  pcoc_le64(nonce, buf + 64);
  pcoc_sha256(buf, 72, digest);
}

int pcoc_leading_zero_bits(const uint8_t digest[32]) {
  int bits = 0;
  for (int i = 0; i < 32; i++) {
    if (digest[i] == 0) {
      bits += 8;
      continue;
    }
    uint8_t b = digest[i];
    for (int k = 7; k >= 0; k--) {
      if ((b >> k) & 1) {
        return bits;
      }
      bits++;
    }
    return bits;
  }
  return bits;
}

int pcoc_hashcash_solve(const uint8_t challenge[32], const uint8_t waypoint[32],
                        int bits, uint64_t max_nonce, uint64_t *nonce_out) {
  uint8_t digest[32];
  for (uint64_t n = 0;; n++) {
    pcoc_hashcash_digest(challenge, waypoint, n, digest);
    if (pcoc_leading_zero_bits(digest) >= bits) {
      *nonce_out = n;
      return 0;
    }
    if (n == max_nonce) {
      return -1;
    }
  }
}

int pcoc_six_leg_draw(uint64_t leg_total, int (*read8)(void *ctx, uint8_t out[8]),
                      void *ctx, uint64_t indices[6], int *status) {
  /* §14.6. Fewer than 6 legs: not defined, draw not made.
   * Short byte source: not read. An 8-byte draw at or above the limit
   * is consumed and not reused. */
  if (!status) {
    return -1;
  }
  if (leg_total < 6) {
    *status = 1;
    return 0;
  }
  const uint64_t M_mod = (uint64_t)(-(unsigned long)0); /* unused */
  (void)M_mod;
  /* limit = 2^64 - (2^64 mod leg_total). 2^64 mod n = (2^64 - n * q)
   * and 2^64 mod n == (-n) mod n == 0 when we compute
   * (UINT64_MAX % leg_total + 1) % leg_total, because 2^64 ≡ 0. */
  uint64_t mod = (UINT64_MAX % leg_total + 1) % leg_total; /* 2^64 mod leg_total */
  uint64_t limit = (mod == 0) ? 0 : (0ull - mod);          /* 2^64 - mod, or 0 when mod is 0 which means limit = 2^64 */
  /* When leg_total is a power of two, mod == 0 and limit = 2^64, so every
   * u is accepted. Represent that as limit_is_all. */
  int limit_is_all = (mod == 0);
  int got = 0;
  while (got < 6) {
    uint8_t b[8];
    if (!read8 || read8(ctx, b) != 0) {
      *status = 2;
      return 0;
    }
    uint64_t u = pcoc_u64_le(b);
    if (!limit_is_all && u >= limit) {
      continue; /* consumed, not reused */
    }
    uint64_t q = u % leg_total;
    int seen = 0;
    for (int i = 0; i < got; i++) {
      if (indices[i] == q) {
        seen = 1;
        break;
      }
    }
    if (seen) {
      continue;
    }
    indices[got++] = q;
  }
  *status = 0;
  return 0;
}

int pcoc_leg_rate_floor(uint64_t hike_legs, int span_not_read, int64_t hike_span,
                        uint64_t p, uint64_t q, const char **outcome) {
  /* Rule B2. Integer comparison q · hike_legs ≥ p · hike_span.
   * hike_span not read → held. hike_span ≤ 0 → refuse. A numeric miss
   * → short. A meet → meet.
   * CASES.md says "the SPEC's outcome word". The SPEC words are
   * `refuse` (span ≤ 0, or a numeric rate that falls short) and the
   * comparison itself has no token for a meet. Section 15 outcomes are
   * provisional, accept, refuse, revoke, held. A rate that meets the
   * floor is not named. This core spells:
   *   held    — hike_span is not read
   *   refuse  — hike_span ≤ 0, or the integer comparison fails
   *   meet    — q·hike_legs ≥ p·hike_span with hike_span > 0
   * "meet" is the reading of a comparison that holds. Recorded in DONE.md. */
  if (span_not_read) {
    *outcome = "held";
    return 0;
  }
  if (hike_span <= 0) {
    *outcome = "refuse";
    return 0;
  }
  /* Compare in 128-bit so the product does not wrap. */
  __int128 left = (__int128)q * (__int128)hike_legs;
  __int128 right = (__int128)p * (__int128)hike_span;
  if (left >= right) {
    *outcome = "meet";
  } else {
    *outcome = "refuse";
  }
  return 0;
}

int pcoc_anchor_window(uint64_t H, uint64_t header_height, const char **outcome) {
  /* §13.1 offsets 2, 3, 4 after stored stage height H.
   * The full anchored token also needs a stored interrupt and plant match.
   * This function is the window comparison alone: anchored when
   * header_height is H+2, H+3, or H+4, otherwise not anchored. */
  uint64_t d = header_height - H;
  if (header_height > H && (d == 2 || d == 3 || d == 4)) {
    *outcome = "anchored";
  } else {
    *outcome = "not anchored";
  }
  return 0;
}

static int json_escape_into(const char *s, size_t n, char *buf, size_t cap) {
  size_t o = 0;
  for (size_t i = 0; i < n; i++) {
    unsigned char c = (unsigned char)s[i];
    const char *esc = NULL;
    char tmp[8];
    if (c == '"' || c == '\\') {
      tmp[0] = '\\';
      tmp[1] = (char)c;
      tmp[2] = 0;
      esc = tmp;
    } else if (c < 0x20) {
      snprintf(tmp, sizeof(tmp), "\\u%04x", c);
      esc = tmp;
    }
    if (esc) {
      size_t el = strlen(esc);
      if (o + el >= cap) {
        return -1;
      }
      memcpy(buf + o, esc, el);
      o += el;
    } else {
      if (o + 1 >= cap) {
        return -1;
      }
      buf[o++] = (char)c;
    }
  }
  if (o >= cap) {
    return -1;
  }
  buf[o] = 0;
  return (int)o;
}

int pcoc_unknown_kind_record(const char *kind_text, size_t kind_len, char *buf, size_t cap) {
  /* Rule A5. {"kind":"unknown kind","kind_text":"<the kind text>"} */
  char esc[512];
  if (json_escape_into(kind_text ? kind_text : "", kind_len, esc, sizeof(esc)) < 0) {
    return -1;
  }
  int n = snprintf(buf, cap, "{\"kind\":\"unknown kind\",\"kind_text\":\"%s\"}", esc);
  if (n < 0 || (size_t)n >= cap) {
    return -1;
  }
  return n;
}
