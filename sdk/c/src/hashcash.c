/* SPEC §13.1 Hashcash, §14.6 draw, §15.5 thorough draw, Rule B2,
 * §16.1 anchor comparison, Rule A5.
 * The byte-source draws are the rig functions. The release draws read
 * the OS CSPRNG and take no byte source.
 */
#define _DEFAULT_SOURCE
#include "pcoc.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/random.h>

void pcoc_hashcash_challenge(const char *origin, size_t origin_len,
                             const uint8_t header_hash[32], uint8_t challenge[32]) {
  /* challenge = SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash) */
  size_t n = 10 + origin_len + 32;
  uint8_t *buf = (uint8_t *)malloc(n);
  if (!buf) {
    memset(challenge, 0, 32);
    return;
  }
  memcpy(buf, "bari-hc-v2", 10);
  if (origin_len) {
    memcpy(buf + 10, origin, origin_len);
  }
  memcpy(buf + 10 + origin_len, header_hash, 32);
  pcoc_sha256(buf, n, challenge);
  free(buf);
}

void pcoc_hashcash_digest(const uint8_t challenge[32], const uint8_t waypoint[32],
                          uint64_t nonce, uint8_t digest[32]) {
  uint8_t buf[72];
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

int pcoc_hashcash_solve(const uint8_t challenge[32], const uint8_t waypoint[32], int bits,
                        uint64_t max_nonce, uint64_t *nonce_out) {
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

static int os_read8(void *ctx, uint8_t out[8]) {
  (void)ctx;
  ssize_t n = getrandom(out, 8, 0);
  if (n == 8) {
    return 0;
  }
  FILE *f = fopen("/dev/urandom", "rb");
  if (!f) {
    return -1;
  }
  size_t got = fread(out, 1, 8, f);
  fclose(f);
  return got == 8 ? 0 : -1;
}

/* One §14.6 unbiased draw. Returns 0 and writes *q, or -1 if the source ends. */
static int unbiased(uint64_t leg_total, int (*read8)(void *, uint8_t *), void *ctx, uint64_t *q) {
  if (leg_total == 0 || !read8) {
    return -1;
  }
  uint64_t mod = (UINT64_MAX % leg_total + 1) % leg_total; /* 2^64 mod leg_total */
  int all = (mod == 0);
  uint64_t limit = all ? 0 : (0ull - mod);
  for (;;) {
    uint8_t b[8];
    if (read8(ctx, b) != 0) {
      return -1;
    }
    uint64_t u = pcoc_u64_le(b);
    if (!all && u >= limit) {
      continue;
    }
    *q = u % leg_total;
    return 0;
  }
}

static int six_from(uint64_t leg_total, int (*read8)(void *, uint8_t *), void *ctx,
                    uint64_t indices[6], int *status) {
  if (!status) {
    return -1;
  }
  if (leg_total < 6) {
    *status = 1;
    return 0;
  }
  int got = 0;
  while (got < 6) {
    uint64_t q;
    if (unbiased(leg_total, read8, ctx, &q) != 0) {
      *status = 2;
      return 0;
    }
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

int pcoc_six_leg_draw_rig(uint64_t leg_total, int (*read8)(void *ctx, uint8_t out[8]), void *ctx,
                          uint64_t indices[6], int *status) {
  return six_from(leg_total, read8, ctx, indices, status);
}

int pcoc_six_leg_draw(uint64_t leg_total, uint64_t indices[6], int *status) {
  return six_from(leg_total, os_read8, NULL, indices, status);
}

static int in_range(const uint64_t *xs, size_t n, uint64_t q) {
  for (size_t i = 0; i < n; i++) {
    if (xs[i] == q) {
      return 1;
    }
  }
  return 0;
}

static int thorough_from(uint64_t leg_total, int thorough_rounds, int (*read8)(void *, uint8_t *),
                         void *ctx, uint64_t *indices, size_t cap, size_t *n_out, int *status) {
  if (!status || !n_out || thorough_rounds < 1) {
    return -1;
  }
  *n_out = 0;
  if (leg_total < 6) {
    *status = 1;
    return 0;
  }
  size_t D = (size_t)6 * (size_t)thorough_rounds;
  if (cap < D) {
    return -1;
  }
  size_t n = 0;
  while (n < D) {
    uint64_t q;
    if (unbiased(leg_total, read8, ctx, &q) != 0) {
      *n_out = n - (n % 6); /* unfinished group is not a round */
      *status = 2;
      return 0;
    }
    size_t group = n - (n % 6);
    if (in_range(indices + group, n - group, q)) {
      continue;
    }
    int unused = 0;
    if (n < leg_total) {
      /* Some index in 0 .. leg_total-1 is not yet in phase. A repeat is
       * discarded while that is true (§15.5). */
      unused = 1;
    } else {
      /* phase may hold repeats; an index is unused when it never appears. */
      for (uint64_t i = 0; i < leg_total; i++) {
        if (!in_range(indices, n, i)) {
          unused = 1;
          break;
        }
      }
    }
    if (in_range(indices, n, q) && unused) {
      continue;
    }
    indices[n++] = q;
  }
  *n_out = n;
  *status = 0;
  return 0;
}

int pcoc_thorough_draw_rig(uint64_t leg_total, int thorough_rounds,
                           int (*read8)(void *ctx, uint8_t out[8]), void *ctx, uint64_t *indices,
                           size_t cap, size_t *n_out, int *status) {
  return thorough_from(leg_total, thorough_rounds, read8, ctx, indices, cap, n_out, status);
}

int pcoc_thorough_draw(uint64_t leg_total, int thorough_rounds, uint64_t *indices, size_t cap,
                       size_t *n_out, int *status) {
  return thorough_from(leg_total, thorough_rounds, os_read8, NULL, indices, cap, n_out, status);
}

int pcoc_leg_rate_floor(uint64_t hike_legs, int span_not_read, int64_t hike_span, uint64_t p,
                        uint64_t q, const char **outcome) {
  /* Rule B2 / §4 Comparison result / erratum E05. */
  if (!outcome) {
    return -1;
  }
  if (span_not_read) {
    *outcome = "held";
    return 0;
  }
  if (hike_span <= 0 || q == 0) {
    *outcome = "refuse";
    return 0;
  }
  __int128 left = (__int128)q * (__int128)hike_legs;
  __int128 right = (__int128)p * (__int128)hike_span;
  *outcome = (left >= right) ? "meets" : "refuse";
  return 0;
}

int pcoc_anchor_window(uint64_t H, uint64_t header_height, const char **outcome) {
  /* §16.1 Anchor comparison. Offsets 2, 3, 4 (§13.1). */
  if (!outcome) {
    return -1;
  }
  if (header_height > H) {
    uint64_t d = header_height - H;
    if (d == 2 || d == 3 || d == 4) {
      *outcome = "anchored";
      return 0;
    }
  }
  *outcome = "not anchored";
  return 0;
}

/* RFC 8785 string escape into dst. Returns length, or -1. */
static int json_escape(const uint8_t *s, size_t n, char *dst, size_t cap) {
  size_t o = 0;
  size_t i = 0;
  while (i < n) {
    uint8_t c = s[i];
    char tmp[16];
    const char *esc = NULL;
    size_t el = 0;
    uint32_t cp = 0;
    size_t adv = 1;
    if (c < 0x20 || c == '"' || c == '\\') {
      if (c == '"') {
        esc = "\\\"";
        el = 2;
      } else if (c == '\\') {
        esc = "\\\\";
        el = 2;
      } else {
        snprintf(tmp, sizeof(tmp), "\\u%04x", c);
        esc = tmp;
        el = 6;
      }
    } else if (c < 0x80) {
      tmp[0] = (char)c;
      esc = tmp;
      el = 1;
    } else {
      /* One UTF-8 code point, escaped as RFC 8785 (outside U+0020..U+007E). */
      if ((c & 0xE0) == 0xC0 && i + 1 < n) {
        cp = ((c & 0x1F) << 6) | (s[i + 1] & 0x3F);
        adv = 2;
      } else if ((c & 0xF0) == 0xE0 && i + 2 < n) {
        cp = ((c & 0x0F) << 12) | ((s[i + 1] & 0x3F) << 6) | (s[i + 2] & 0x3F);
        adv = 3;
      } else if ((c & 0xF8) == 0xF0 && i + 3 < n) {
        cp = ((c & 0x07) << 18) | ((s[i + 1] & 0x3F) << 12) | ((s[i + 2] & 0x3F) << 6) |
             (s[i + 3] & 0x3F);
        adv = 4;
      } else {
        return -1;
      }
      if (cp > 0xFFFF) {
        uint32_t u = cp - 0x10000;
        snprintf(tmp, sizeof(tmp), "\\u%04x\\u%04x", 0xD800 + (u >> 10), 0xDC00 + (u & 0x3FF));
        el = 12;
      } else {
        snprintf(tmp, sizeof(tmp), "\\u%04x", cp);
        el = 6;
      }
      esc = tmp;
    }
    if (o + el >= cap) {
      return -1;
    }
    memcpy(dst + o, esc, el);
    o += el;
    i += adv;
  }
  dst[o] = 0;
  return (int)o;
}

int pcoc_unknown_kind_record(const uint8_t *kind_text, size_t kind_len, char *buf, size_t cap) {
  char esc[2048];
  if (json_escape(kind_text ? kind_text : (const uint8_t *)"", kind_len, esc, sizeof(esc)) < 0) {
    return -1;
  }
  int n = snprintf(buf, cap, "{\"kind\":\"unknown kind\",\"kind_text\":\"%s\"}", esc);
  if (n < 0 || (size_t)n >= cap) {
    return -1;
  }
  return n;
}
