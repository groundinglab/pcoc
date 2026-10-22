/* Permanent negatives, SPEC §16. Computed only by the generator.
 * This file is not on an SDK code path. Nothing here is called by the
 * verifier, the step, or the terrain fill.
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

/* slot_summary_fold: S XOR (XOR over w of (word XOR w)). §11.1 informative. */
static uint64_t fold_slot_summary(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t x = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    uint64_t word = pcoc_u64_le(slot + 8 * w);
    x ^= word ^ w;
  }
  uint64_t S_folded = S ^ x;
  uint8_t pre[20];
  memcpy(pre, PCOC_SEAL_DOMAIN, 12);
  pcoc_le64(S_folded, pre + 12);
  uint8_t E[32];
  pcoc_sha256(pre, 20, E);
  return pcoc_u64_le(E);
}

/* xor_accumulator_fold: acc = acc XOR mix64(word XOR w XOR S). */
static uint64_t fold_xor_acc(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    uint64_t word = pcoc_u64_le(slot + 8 * w);
    acc ^= pcoc_mix64(word ^ w ^ S);
  }
  uint8_t pre[20];
  memcpy(pre, PCOC_SEAL_DOMAIN, 12);
  pcoc_le64(acc, pre + 12);
  uint8_t E[32];
  pcoc_sha256(pre, 20, E);
  return pcoc_u64_le(E);
}

/* xor_w_fold: acc = (acc + mix64(word XOR w XOR S)) mod 2^64. */
static uint64_t fold_xor_w(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    uint64_t word = pcoc_u64_le(slot + 8 * w);
    acc += pcoc_mix64(word ^ w ^ S);
  }
  uint8_t pre[20];
  memcpy(pre, PCOC_SEAL_DOMAIN, 12);
  pcoc_le64(acc, pre + 12);
  uint8_t E[32];
  pcoc_sha256(pre, 20, E);
  return pcoc_u64_le(E);
}

/* s_plus_w_fold: acc = (acc + mix64(word XOR ((S + w) mod 2^64))) mod 2^64. */
static uint64_t fold_s_plus_w(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    uint64_t word = pcoc_u64_le(slot + 8 * w);
    acc += pcoc_mix64(word ^ (S + w));
  }
  uint8_t pre[20];
  memcpy(pre, PCOC_SEAL_DOMAIN, 12);
  pcoc_le64(acc, pre + 12);
  uint8_t E[32];
  pcoc_sha256(pre, 20, E);
  return pcoc_u64_le(E);
}

/* prefold_hoist: the per-word term word XOR w does not depend on S, so it
 * is mixed once per slot and the state is applied outside the loop. */
static uint64_t fold_prefold_hoist(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < sizes->words_per_slot; w++) {
    uint64_t word = pcoc_u64_le(slot + 8 * w);
    acc += pcoc_mix64(word ^ w); /* hoisted: no S inside mix64 */
  }
  acc ^= S;
  uint8_t pre[20];
  memcpy(pre, PCOC_SEAL_DOMAIN, 12);
  pcoc_le64(acc, pre + 12);
  uint8_t E[32];
  pcoc_sha256(pre, 20, E);
  return pcoc_u64_le(E);
}

/* partial_slot_read: fold only the first half of the words. */
static uint64_t fold_partial(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S,
                             uint64_t nwords) {
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  uint64_t lim = nwords;
  if (lim > sizes->words_per_slot) {
    lim = sizes->words_per_slot;
  }
  for (uint64_t w = 0; w < lim; w++) {
    uint64_t word = pcoc_u64_le(slot + 8 * w);
    acc += pcoc_mix64(word ^ S);
  }
  uint8_t pre[20];
  memcpy(pre, PCOC_SEAL_DOMAIN, 12);
  pcoc_le64(acc, pre + 12);
  uint8_t E[32];
  pcoc_sha256(pre, 20, E);
  return pcoc_u64_le(E);
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

/* one_lane_flip: NL4 base uses only lane j mod 4 (the informative one-lane map),
 * then the honest parent mix and slot_mix. */
static void fill_one_lane(const pcoc_sizes *sizes, const uint64_t P[4], uint8_t *terrain) {
  uint64_t nslots = sizes->n_slots;
  uint64_t wps = sizes->words_per_slot;
  uint64_t sb = sizes->slot_bytes;
  memset(terrain, 0, (size_t)sizes->terrain_bytes);
  uint64_t w[2048];
  for (uint64_t i = 0; i < nslots; i++) {
    for (uint64_t j = 0; j < wps; j++) {
      uint64_t c = (i << 32) ^ j ^ (uint64_t)PCOC_WORD_XOR;
      w[j] = pcoc_mix64(P[j % 4] ^ c);
    }
    if (i > 0) {
      /* Reuse the honest parent rule by calling the public fill's parent
       * arithmetic inline, matching §10.3. */
      int bits = 0;
      while ((1ull << (bits + 1)) <= i && bits < 63) {
        bits++;
      }
      if (bits < 1) {
        bits = 1;
      }
      uint32_t rev = 0, v = (uint32_t)i;
      for (int k = 0; k < 32; k++) {
        if ((v >> (31 - k)) & 1u) {
          rev |= 1u << k;
        }
      }
      uint64_t bitrev = ((uint64_t)rev >> (32 - bits)) % i;
      uint64_t mul = ((i * PCOC_MIX_ADD) >> 32) % i;
      uint64_t parents[3] = {i - 1, bitrev, mul};
      for (int k = 1; k <= 3; k++) {
        const uint8_t *pslot = terrain + (size_t)(parents[k - 1] * sb);
        uint64_t kmul = (uint64_t)k * PCOC_PARENT_MUL;
        for (uint64_t j = 0; j < wps; j++) {
          uint64_t pw = pcoc_u64_le(pslot + 8 * j);
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
}

/* all4_kernel: the informative linear map, one mix64 of all four lanes. */
static void fill_all4(const pcoc_sizes *sizes, const uint64_t P[4], uint8_t *terrain) {
  uint64_t nslots = sizes->n_slots;
  uint64_t wps = sizes->words_per_slot;
  uint64_t sb = sizes->slot_bytes;
  memset(terrain, 0, (size_t)sizes->terrain_bytes);
  uint64_t w[2048];
  for (uint64_t i = 0; i < nslots; i++) {
    for (uint64_t j = 0; j < wps; j++) {
      uint64_t c = (i << 32) ^ j ^ (uint64_t)PCOC_WORD_XOR;
      uint64_t x = P[j % 4] ^ rotl64(P[(j + 1) % 4], 17) ^ rotl64(P[(j + 2) % 4], 31) ^
                   rotl64(P[(j + 3) % 4], 47) ^ c;
      w[j] = pcoc_mix64(x);
    }
    if (i > 0) {
      int bits = 0;
      while ((1ull << (bits + 1)) <= i && bits < 63) {
        bits++;
      }
      if (bits < 1) {
        bits = 1;
      }
      uint32_t rev = 0, v = (uint32_t)i;
      for (int k = 0; k < 32; k++) {
        if ((v >> (31 - k)) & 1u) {
          rev |= 1u << k;
        }
      }
      uint64_t bitrev = ((uint64_t)rev >> (32 - bits)) % i;
      uint64_t mul = ((i * PCOC_MIX_ADD) >> 32) % i;
      uint64_t parents[3] = {i - 1, bitrev, mul};
      for (int k = 1; k <= 3; k++) {
        const uint8_t *pslot = terrain + (size_t)(parents[k - 1] * sb);
        uint64_t kmul = (uint64_t)k * PCOC_PARENT_MUL;
        for (uint64_t j = 0; j < wps; j++) {
          uint64_t pw = pcoc_u64_le(pslot + 8 * j);
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
}

/* squeezed seed: one 64-bit pin stands in for the seed. All four lanes
 * are that pin. §10.2 says do not compute one. This negative does. */
static void squeezed_lanes(const uint64_t P[4], uint64_t out[4]) {
  uint64_t pin = P[0] ^ P[1] ^ P[2] ^ P[3];
  out[0] = out[1] = out[2] = out[3] = pin;
}

static void digest_terrain(const uint8_t *terrain, size_t n, uint8_t out[32]) {
  pcoc_sha256(terrain, n, out);
}

void pcoc_negatives_s1(const pcoc_sizes *sizes, const uint64_t P[4], const uint8_t *honest,
                       const uint8_t previous[32], uint64_t honest_leg, char *out, size_t cap) {
  /* Writes the negatives.txt body for the leg-level and terrain-level
   * negatives on S1. matches_honest is false for each. */
  size_t used = 0;
#define ADD(...)                                                                               \
  do {                                                                                         \
    int _n = snprintf(out + used, cap - used, __VA_ARGS__);                                    \
    if (_n < 0 || (size_t)_n >= cap - used) {                                                  \
      return;                                                                                  \
    }                                                                                          \
    used += (size_t)_n;                                                                        \
  } while (0)

  uint8_t le[8];
  char hex[17];

  struct {
    const char *name;
    step_fn fn;
    int partial_n;
  } folds[] = {
      {"slot_summary_fold", fold_slot_summary, 0},
      {"xor_accumulator_fold", fold_xor_acc, 0},
      {"xor_w_fold", fold_xor_w, 0},
      {"s_plus_w_fold", fold_s_plus_w, 0},
      {"prefold_hoist", fold_prefold_hoist, 0},
  };
  for (unsigned i = 0; i < sizeof(folds) / sizeof(folds[0]); i++) {
    uint64_t lr = run_leg(sizes, honest, previous, folds[i].fn);
    pcoc_le64(lr, le);
    for (int b = 0; b < 8; b++) {
      sprintf(hex + 2 * b, "%02x", le[b]);
    }
    int match = (lr == honest_leg);
    ADD("negatives.S1.pk_A.%s.legs_le_hex\t%s\n", folds[i].name, hex);
    ADD("negatives.S1.pk_A.%s.matches_honest\t%s\n", folds[i].name, match ? "true" : "false");
  }

  uint64_t part = 0;
  uint64_t S = pcoc_u64_le(previous);
  for (uint64_t t = 0; t < sizes->steps_per_leg; t++) {
    S = fold_partial(sizes, honest, S, sizes->words_per_slot / 2);
  }
  part = S;
  pcoc_le64(part, le);
  for (int b = 0; b < 8; b++) {
    sprintf(hex + 2 * b, "%02x", le[b]);
  }
  ADD("negatives.S1.pk_A.partial_slot_read.legs_le_hex\t%s\n", hex);
  ADD("negatives.S1.pk_A.partial_slot_read.matches_honest\t%s\n",
      part == honest_leg ? "true" : "false");

  /* partial_slot_read_2047: production WORDS_PER_SLOT is 2048, so the
   * shortcut reads 2047 words. On S1, WORDS_PER_SLOT is 8, and the SPEC
   * defines this negative at the production word count. The generator
   * reports the production-semantics read: 2047 of 2048, which on a
   * profile whose slot is shorter than 2048 words reads every word the
   * profile has except it still stops at min(2047, words-1) so it is a
   * short read on every profile. */
  uint64_t n2047 = 2047;
  S = pcoc_u64_le(previous);
  for (uint64_t t = 0; t < sizes->steps_per_leg; t++) {
    uint64_t lim = n2047;
    if (lim >= sizes->words_per_slot) {
      lim = sizes->words_per_slot - 1; /* still a short read */
    }
    S = fold_partial(sizes, honest, S, lim);
  }
  pcoc_le64(S, le);
  for (int b = 0; b < 8; b++) {
    sprintf(hex + 2 * b, "%02x", le[b]);
  }
  ADD("negatives.S1.pk_A.partial_slot_read_2047.legs_le_hex\t%s\n", hex);
  ADD("negatives.S1.pk_A.partial_slot_read_2047.matches_honest\t%s\n",
      S == honest_leg ? "true" : "false");

  uint8_t honest_dig[32];
  digest_terrain(honest, (size_t)sizes->terrain_bytes, honest_dig);

  uint8_t *alt = malloc((size_t)sizes->terrain_bytes);
  fill_one_lane(sizes, P, alt);
  uint8_t dig[32];
  digest_terrain(alt, (size_t)sizes->terrain_bytes, dig);
  int same = memcmp(dig, honest_dig, 32) == 0;
  ADD("negatives.S1.pk_A.one_lane_flip.terrain_sha256\t");
  for (int b = 0; b < 32; b++) {
    ADD("%02x", dig[b]);
  }
  ADD("\nnegatives.S1.pk_A.one_lane_flip.matches_honest\t%s\n", same ? "true" : "false");

  fill_all4(sizes, P, alt);
  digest_terrain(alt, (size_t)sizes->terrain_bytes, dig);
  same = memcmp(dig, honest_dig, 32) == 0;
  ADD("negatives.S1.pk_A.all4_kernel.terrain_sha256\t");
  for (int b = 0; b < 32; b++) {
    ADD("%02x", dig[b]);
  }
  ADD("\nnegatives.S1.pk_A.all4_kernel.matches_honest\t%s\n", same ? "true" : "false");

  /* f3_shared_terrain: a second seed reuses the first terrain. The digest
   * of the "second" PCOC is the honest digest, which is not the terrain
   * that seed would fill. matches_honest is false against the terrain
   * that seed's own lanes would produce. We fill the squeezed-different
   * check separately. Here the shared digest is the honest one reported
   * as the second key's terrain, and matches_honest compares it with the
   * terrain that key would have filled on its own. */
  uint64_t P2[4] = {P[0] ^ 1, P[1], P[2], P[3]};
  uint8_t *own = malloc((size_t)sizes->terrain_bytes);
  pcoc_terrain_fill(sizes, P2, own);
  uint8_t own_dig[32];
  digest_terrain(own, (size_t)sizes->terrain_bytes, own_dig);
  same = memcmp(honest_dig, own_dig, 32) == 0;
  ADD("negatives.S1.pk_A.f3_shared_terrain.terrain_sha256\t");
  for (int b = 0; b < 32; b++) {
    ADD("%02x", honest_dig[b]);
  }
  ADD("\nnegatives.S1.pk_A.f3_shared_terrain.matches_honest\t%s\n", same ? "true" : "false");

  /* cached_leg: the leg result is reused from a stored value instead of
   * recomputing. The cached value is the honest result XOR 1, a stand-in
   * that does not equal the recomputed leg. */
  uint64_t cached = honest_leg ^ 1ull;
  pcoc_le64(cached, le);
  for (int b = 0; b < 8; b++) {
    sprintf(hex + 2 * b, "%02x", le[b]);
  }
  ADD("negatives.S1.pk_A.cached_leg.legs_le_hex\t%s\n", hex);
  ADD("negatives.S1.pk_A.cached_leg.matches_honest\t%s\n",
      cached == honest_leg ? "true" : "false");

  uint64_t squeezed[4];
  squeezed_lanes(P, squeezed);
  pcoc_terrain_fill(sizes, squeezed, alt);
  digest_terrain(alt, (size_t)sizes->terrain_bytes, dig);
  same = memcmp(dig, honest_dig, 32) == 0;
  ADD("negatives.S1.pk_A.squeezed_seed_terrain.terrain_sha256\t");
  for (int b = 0; b < 32; b++) {
    ADD("%02x", dig[b]);
  }
  ADD("\nnegatives.S1.pk_A.squeezed_seed_terrain.matches_honest\t%s\n", same ? "true" : "false");

  free(alt);
  free(own);
#undef ADD
}
