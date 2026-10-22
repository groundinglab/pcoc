/* Permanent negatives, SPEC §16 (erratum E01).
 * This file is the generator module named negatives. It is not linked
 * into libpcoc or the pcoc binary.
 */
#include "pcoc.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint64_t rotl64(uint64_t w, unsigned r) {
  r &= 63u;
  if (r == 0) {
    return w;
  }
  return (w << r) | (w >> (64u - r));
}

static uint64_t seal(uint64_t acc) {
  uint8_t pre[20];
  memcpy(pre, PCOC_SEAL_DOMAIN, 12);
  pcoc_le64(acc, pre + 12);
  uint8_t E[32];
  pcoc_sha256(pre, 20, E);
  return pcoc_u64_le(E);
}

static uint64_t word_at(const uint8_t *slot, uint64_t w) { return pcoc_u64_le(slot + 8 * w); }

static uint64_t fold_slot_summary(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t x = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    x ^= word_at(slot, w) ^ w;
  }
  return seal(S ^ x);
}

static uint64_t fold_xor_acc(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    acc ^= pcoc_mix64(word_at(slot, w) ^ w ^ S);
  }
  return seal(acc);
}

static uint64_t fold_xor_w(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    acc += pcoc_mix64(word_at(slot, w) ^ w ^ S);
  }
  return seal(acc);
}

static uint64_t fold_s_plus_w(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    acc += pcoc_mix64(word_at(slot, w) ^ (S + w));
  }
  return seal(acc);
}

static uint64_t mix_prefinal(uint64_t x) {
  uint64_t y = x + PCOC_MIX_ADD;
  y = (y ^ (y >> 30)) * PCOC_MIX_MUL1;
  y = (y ^ (y >> 27)) * PCOC_MIX_MUL2;
  return y;
}

static uint64_t fold_final_stage(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t x = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    x ^= mix_prefinal(word_at(slot, w) ^ S);
  }
  return seal(x ^ (x >> 31));
}

static uint64_t fold_partial_n(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S,
                               uint64_t nwords) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  uint64_t lim = nwords;
  if (lim > sizes->words_per_slot) {
    lim = sizes->words_per_slot;
  }
  for (uint64_t w = 0; w < lim; w++) {
    acc += pcoc_mix64(word_at(slot, w) ^ S);
  }
  return seal(acc);
}

typedef uint64_t (*step_fn)(const pcoc_sizes *, const uint8_t *, uint64_t);

static uint64_t run_leg(const pcoc_sizes *sizes, const uint8_t *terrain, const uint8_t prev[32],
                        step_fn fn) {
  uint64_t S = pcoc_u64_le(prev);
  for (uint64_t t = 0; t < sizes->steps_per_leg; t++) {
    S = fn(sizes, terrain, S);
  }
  return S;
}

/* pre(i) once per slot, then S_folded = (pre(i) + S) mod 2^64, then the seal. */
static uint64_t run_prefold(const pcoc_sizes *sizes, const uint8_t *terrain, const uint8_t prev[32]) {
  uint64_t *pre = calloc((size_t)sizes->n_slots, sizeof(uint64_t));
  if (!pre) {
    return 0;
  }
  for (uint64_t i = 0; i < sizes->n_slots; i++) {
    const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
    uint64_t acc = 0;
    for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
      acc += pcoc_mix64(word_at(slot, w) ^ w);
    }
    pre[i] = acc;
  }
  uint64_t S = pcoc_u64_le(prev);
  for (uint64_t t = 0; t < sizes->steps_per_leg; t++) {
    uint64_t i = S % sizes->n_slots;
    S = seal(pre[i] + S);
  }
  free(pre);
  return S;
}

/* Reuse the earlier read's S_folded when a slot is visited again. */
static uint64_t run_cached(const pcoc_sizes *sizes, const uint8_t *terrain, const uint8_t prev[32],
                           int *revisited) {
  uint64_t *saved = calloc((size_t)sizes->n_slots, sizeof(uint64_t));
  uint8_t *seen = calloc((size_t)sizes->n_slots, 1);
  if (!saved || !seen) {
    free(saved);
    free(seen);
    return 0;
  }
  uint64_t S = pcoc_u64_le(prev);
  int again = 0;
  for (uint64_t t = 0; t < sizes->steps_per_leg; t++) {
    uint64_t i = S % sizes->n_slots;
    uint64_t acc;
    if (seen[i]) {
      acc = saved[i];
      again = 1;
    } else {
      const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
      acc = 0;
      for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
        acc += pcoc_mix64(word_at(slot, w) ^ S);
      }
      saved[i] = acc;
      seen[i] = 1;
    }
    S = seal(acc);
  }
  if (revisited) {
    *revisited = again;
  }
  free(saved);
  free(seen);
  return S;
}

static int floor_log2_u64(uint64_t n) {
  int b = 0;
  while ((1ull << (b + 1)) <= n && b < 63) {
    b++;
  }
  return b;
}

static uint32_t reverse32(uint32_t v) {
  uint32_t r = 0;
  for (int k = 0; k < 32; k++) {
    if ((v >> (31 - k)) & 1u) {
      r |= 1u << k;
    }
  }
  return r;
}

static void parents_and_store(const pcoc_sizes *sizes, uint64_t i, uint64_t *w, uint8_t *terrain) {
  uint64_t wps = sizes->words_per_slot;
  uint64_t sb = sizes->slot_bytes;
  if (i > 0) {
    int bits = floor_log2_u64(i);
    if (bits < 1) {
      bits = 1;
    }
    uint64_t bitrev = ((uint64_t)reverse32((uint32_t)i) >> (32 - bits)) % i;
    uint64_t mul = ((i * PCOC_MIX_ADD) >> 32) % i;
    uint64_t parents[3] = {i - 1, bitrev, mul};
    for (int k = 1; k <= 3; k++) {
      const uint8_t *pslot = terrain + (size_t)(parents[k - 1] * sb);
      uint64_t kmul = (uint64_t)k * PCOC_PARENT_MUL;
      for (uint64_t j = 0; j < wps; j++) {
        uint64_t pw = word_at(pslot, j);
        unsigned rot = (unsigned)((j + (uint64_t)k) % 64);
        w[j] = pcoc_mix64((w[j] ^ rotl64(pw, rot)) ^ kmul);
      }
    }
    uint64_t slot_mix = i * PCOC_SLOT_MUL;
    for (uint64_t j = 0; j < wps; j++) {
      w[j] = pcoc_mix64(w[j] ^ slot_mix);
    }
  }
  uint8_t *slot = terrain + (size_t)(i * sb);
  for (uint64_t j = 0; j < wps; j++) {
    pcoc_le64(w[j], slot + 8 * j);
  }
}

static void fill_one_lane(const pcoc_sizes *sizes, const uint64_t P[4], uint8_t *terrain) {
  memset(terrain, 0, (size_t)sizes->terrain_bytes);
  uint64_t w[2048];
  for (uint64_t i = 0; i < sizes->n_slots; i++) {
    for (uint64_t j = 0; j < sizes->words_per_slot; j++) {
      uint64_t c = (i << 32) ^ j ^ (uint64_t)PCOC_WORD_XOR;
      w[j] = pcoc_mix64(P[j % 4] ^ c);
    }
    parents_and_store(sizes, i, w, terrain);
  }
}

static void fill_all4(const pcoc_sizes *sizes, const uint64_t P[4], uint8_t *terrain) {
  memset(terrain, 0, (size_t)sizes->terrain_bytes);
  uint64_t w[2048];
  for (uint64_t i = 0; i < sizes->n_slots; i++) {
    for (uint64_t j = 0; j < sizes->words_per_slot; j++) {
      uint64_t c = (i << 32) ^ j ^ (uint64_t)PCOC_WORD_XOR;
      uint64_t x = P[j % 4] ^ rotl64(P[(j + 1) % 4], 17) ^ rotl64(P[(j + 2) % 4], 31) ^
                   rotl64(P[(j + 3) % 4], 47) ^ c;
      w[j] = pcoc_mix64(x);
    }
    parents_and_store(sizes, i, w, terrain);
  }
}

static void le_hex(uint64_t v, char out[17]) {
  uint8_t b[8];
  pcoc_le64(v, b);
  static const char *H = "0123456789abcdef";
  for (int i = 0; i < 8; i++) {
    out[2 * i] = H[b[i] >> 4];
    out[2 * i + 1] = H[b[i] & 0xf];
  }
  out[16] = 0;
}

static void dig_hex(const uint8_t *p, char out[65]) {
  static const char *H = "0123456789abcdef";
  for (int i = 0; i < 32; i++) {
    out[2 * i] = H[p[i] >> 4];
    out[2 * i + 1] = H[p[i] & 0xf];
  }
  out[64] = 0;
}

/* Leg negative names, in the order the text lists them. */
static const char *LEG_NAMES[] = {
    "slot_summary_fold", "xor_accumulator_fold", "xor_w_fold",     "s_plus_w_fold",
    "prefold_hoist",     "final_stage_hoist",    "partial_slot_read", "partial_slot_read_2047",
    "f3_shared_terrain", "cached_leg"};

int pcoc_neg_leg_value(const char *name, const pcoc_sizes *sizes, const uint8_t *terrain_a,
                       const uint8_t *terrain_b, const uint8_t prev[32], uint64_t *out) {
  if (strcmp(name, "slot_summary_fold") == 0) {
    *out = run_leg(sizes, terrain_a, prev, fold_slot_summary);
  } else if (strcmp(name, "xor_accumulator_fold") == 0) {
    *out = run_leg(sizes, terrain_a, prev, fold_xor_acc);
  } else if (strcmp(name, "xor_w_fold") == 0) {
    *out = run_leg(sizes, terrain_a, prev, fold_xor_w);
  } else if (strcmp(name, "s_plus_w_fold") == 0) {
    *out = run_leg(sizes, terrain_a, prev, fold_s_plus_w);
  } else if (strcmp(name, "prefold_hoist") == 0) {
    *out = run_prefold(sizes, terrain_a, prev);
  } else if (strcmp(name, "final_stage_hoist") == 0) {
    *out = run_leg(sizes, terrain_a, prev, fold_final_stage);
  } else if (strcmp(name, "partial_slot_read") == 0) {
    uint64_t S = pcoc_u64_le(prev);
    for (uint64_t t = 0; t < sizes->steps_per_leg; t++) {
      S = fold_partial_n(sizes, terrain_a, S, 1);
    }
    *out = S;
  } else if (strcmp(name, "partial_slot_read_2047") == 0) {
    uint64_t lim = sizes->words_per_slot - 1;
    uint64_t S = pcoc_u64_le(prev);
    for (uint64_t t = 0; t < sizes->steps_per_leg; t++) {
      S = fold_partial_n(sizes, terrain_a, S, lim);
    }
    *out = S;
  } else if (strcmp(name, "f3_shared_terrain") == 0) {
    *out = pcoc_leg_result(sizes, terrain_b, prev);
  } else if (strcmp(name, "cached_leg") == 0) {
    int again = 0;
    *out = run_cached(sizes, terrain_a, prev, &again);
    if (!again) {
      fprintf(stderr, "negatives: cached_leg leg does not revisit a slot\n");
      return -1;
    }
  } else {
    return -1;
  }
  return 0;
}

int pcoc_negatives_write(const pcoc_sizes *sizes, const uint64_t PA[4], const uint64_t PB[4],
                         const uint8_t *terrain_a, const uint8_t prev[32], char *out, size_t cap) {
  uint64_t honest = pcoc_leg_result(sizes, terrain_a, prev);
  uint8_t honest_dig[32];
  pcoc_sha256(terrain_a, (size_t)sizes->terrain_bytes, honest_dig);

  uint8_t *alt = malloc((size_t)sizes->terrain_bytes);
  if (!alt) {
    return -1;
  }
  pcoc_terrain_fill(sizes, PB, alt);

  uint64_t legs[10];
  for (int i = 0; i < 10; i++) {
    if (pcoc_neg_leg_value(LEG_NAMES[i], sizes, terrain_a, alt, prev, &legs[i]) != 0) {
      free(alt);
      return -1;
    }
    if (legs[i] == honest) {
      fprintf(stderr, "negatives: %s equals honest\n", LEG_NAMES[i]);
      free(alt);
      return -1;
    }
    for (int j = 0; j < i; j++) {
      if (legs[i] == legs[j]) {
        fprintf(stderr, "negatives: %s duplicates %s\n", LEG_NAMES[i], LEG_NAMES[j]);
        free(alt);
        return -1;
      }
    }
  }

  uint8_t digs[3][32];
  fill_one_lane(sizes, PA, alt);
  pcoc_sha256(alt, (size_t)sizes->terrain_bytes, digs[0]);
  fill_all4(sizes, PA, alt);
  pcoc_sha256(alt, (size_t)sizes->terrain_bytes, digs[1]);
  uint64_t squeezed[4];
  uint64_t pin = PA[0] ^ PA[1] ^ PA[2] ^ PA[3];
  squeezed[0] = squeezed[1] = squeezed[2] = squeezed[3] = pin;
  pcoc_terrain_fill(sizes, squeezed, alt);
  pcoc_sha256(alt, (size_t)sizes->terrain_bytes, digs[2]);
  free(alt);

  const char *tnames[3] = {"one_lane_flip", "all4_kernel", "squeezed_seed_terrain"};
  for (int i = 0; i < 3; i++) {
    if (memcmp(digs[i], honest_dig, 32) == 0) {
      fprintf(stderr, "negatives: %s equals honest\n", tnames[i]);
      return -1;
    }
    for (int j = 0; j < i; j++) {
      if (memcmp(digs[i], digs[j], 32) == 0) {
        fprintf(stderr, "negatives: %s duplicates %s\n", tnames[i], tnames[j]);
        return -1;
      }
    }
  }

  size_t used = 0;
#define ADD(...)                                                                               \
  do {                                                                                         \
    int _n = snprintf(out + used, cap - used, __VA_ARGS__);                                    \
    if (_n < 0 || (size_t)_n >= cap - used) {                                                  \
      return -1;                                                                               \
    }                                                                                          \
    used += (size_t)_n;                                                                        \
  } while (0)

  char hx[80];
  le_hex(honest, hx);
  ADD("negatives.honest.legs_le_hex\t%s\n", hx);
  dig_hex(honest_dig, hx);
  ADD("negatives.honest.terrain_sha256\t%s\n", hx);
  for (int i = 0; i < 10; i++) {
    le_hex(legs[i], hx);
    int match = legs[i] == honest;
    ADD("negatives.%s.legs_le_hex\t%s\n", LEG_NAMES[i], hx);
    ADD("negatives.%s.matches_honest\t%s\n", LEG_NAMES[i], match ? "true" : "false");
  }
  for (int i = 0; i < 3; i++) {
    dig_hex(digs[i], hx);
    int match = memcmp(digs[i], honest_dig, 32) == 0;
    ADD("negatives.%s.terrain_sha256\t%s\n", tnames[i], hx);
    ADD("negatives.%s.matches_honest\t%s\n", tnames[i], match ? "true" : "false");
  }
#undef ADD
  return 0;
}
