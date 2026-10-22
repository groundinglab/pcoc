/* SPEC §6 formulas, §10 terrain, §11 leg, §12 waypoint.
 * Every step loads the whole slot and folds every word (§11.1, Rule I6).
 */
#include "pcoc.h"

#include <stdlib.h>
#include <string.h>

const char *pcoc_version(void) { return PCOC_SDK_VERSION; }

uint64_t pcoc_mix64(uint64_t x) {
  /* §10.1 */
  uint64_t y = x + PCOC_MIX_ADD;
  y = (y ^ (y >> 30)) * PCOC_MIX_MUL1;
  y = (y ^ (y >> 27)) * PCOC_MIX_MUL2;
  return y ^ (y >> 31);
}

void pcoc_le64(uint64_t x, uint8_t out[8]) {
  out[0] = (uint8_t)x;
  out[1] = (uint8_t)(x >> 8);
  out[2] = (uint8_t)(x >> 16);
  out[3] = (uint8_t)(x >> 24);
  out[4] = (uint8_t)(x >> 32);
  out[5] = (uint8_t)(x >> 40);
  out[6] = (uint8_t)(x >> 48);
  out[7] = (uint8_t)(x >> 56);
}

uint64_t pcoc_u64_le(const uint8_t b[8]) {
  return (uint64_t)b[0] | ((uint64_t)b[1] << 8) | ((uint64_t)b[2] << 16) |
         ((uint64_t)b[3] << 24) | ((uint64_t)b[4] << 32) | ((uint64_t)b[5] << 40) |
         ((uint64_t)b[6] << 48) | ((uint64_t)b[7] << 56);
}

void pcoc_profile_production(pcoc_profile *p) {
  p->slot_bytes = PCOC_SLOT_BYTES;
  p->sample_bytes = PCOC_SAMPLE_BYTES;
  p->terrain_bytes = PCOC_TERRAIN_BYTES;
  p->steps_per_leg = PCOC_STEPS_PER_LEG;
  p->conformance_small = 0;
}

int pcoc_profile_small(pcoc_profile *p, uint64_t slot_bytes, uint64_t sample_bytes,
                       uint64_t terrain_bytes, uint64_t steps_per_leg) {
  /* §16.1 bounds. */
  if (slot_bytes < 8 || slot_bytes > PCOC_SLOT_BYTES || (slot_bytes % 8) != 0) {
    return -1;
  }
  if (steps_per_leg < 1 || steps_per_leg > PCOC_STEPS_PER_LEG) {
    return -1;
  }
  if (sample_bytes < 1 || sample_bytes > PCOC_SAMPLE_BYTES) {
    return -1;
  }
  if (terrain_bytes > PCOC_TERRAIN_BYTES || terrain_bytes < sample_bytes ||
      (slot_bytes != 0 && terrain_bytes % slot_bytes != 0)) {
    return -1;
  }
  p->slot_bytes = slot_bytes;
  p->sample_bytes = sample_bytes;
  p->terrain_bytes = terrain_bytes;
  p->steps_per_leg = steps_per_leg;
  p->conformance_small = 1;
  return 0;
}

int pcoc_sizes_from_profile(const pcoc_profile *p, pcoc_sizes *s) {
  if (p->slot_bytes == 0 || p->slot_bytes % 8 != 0) {
    return -1;
  }
  if (p->terrain_bytes % p->slot_bytes != 0) {
    return -1;
  }
  if (p->sample_bytes == 0 || p->terrain_bytes < p->sample_bytes) {
    return -1;
  }
  s->slot_bytes = p->slot_bytes;
  s->words_per_slot = p->slot_bytes / 8; /* P6.23 */
  s->terrain_bytes = p->terrain_bytes;
  s->n_slots = p->terrain_bytes / p->slot_bytes; /* P6.24 */
  s->sample_bytes = p->sample_bytes;
  s->offset_modulus = p->terrain_bytes - p->sample_bytes + 1; /* P6.25 */
  s->steps_per_leg = p->steps_per_leg;
  return 0;
}

void pcoc_seed(const uint8_t public_key[32], const uint8_t seed_beacon[32],
               uint8_t preimage[64], uint8_t D[32], uint64_t P[4]) {
  /* §10 */
  memcpy(preimage, public_key, 32);
  memcpy(preimage + 32, seed_beacon, 32);
  pcoc_sha256(preimage, 64, D);
  for (int k = 0; k < 4; k++) {
    P[k] = pcoc_u64_le(D + 8 * k);
  }
}

static uint64_t rotl64(uint64_t w, unsigned r);

uint64_t pcoc_nl4_base(uint64_t i, uint64_t j, const uint64_t P[4]) {
  /* §10.4 base word, before parent mixing and slot_mix. */
  uint64_t c = (i << 32) ^ j ^ (uint64_t)PCOC_WORD_XOR;
  uint64_t t = pcoc_mix64(P[j % 4] ^ c);
  t = pcoc_mix64(t ^ rotl64(P[(j + 1) % 4], 17));
  t = pcoc_mix64(t ^ rotl64(P[(j + 2) % 4], 31));
  t = pcoc_mix64(t ^ rotl64(P[(j + 3) % 4], 47));
  return t;
}

static uint64_t rotl64(uint64_t w, unsigned r) {
  r &= 63u;
  if (r == 0) {
    return w;
  }
  return (w << r) | (w >> (64u - r));
}

static int floor_log2_u64(uint64_t n) {
  int b = 0;
  while ((1ull << (b + 1)) <= n && b < 63) {
    b++;
  }
  return b;
}

static uint32_t reverse32(uint32_t v) {
  /* §10.3: bit k of the result equals bit 31-k of v. */
  uint32_t r = 0;
  for (int k = 0; k < 32; k++) {
    if ((v >> (31 - k)) & 1u) {
      r |= 1u << k;
    }
  }
  return r;
}

static void parent_indices(uint64_t i, uint64_t *bitrev, uint64_t *mul) {
  int bits = floor_log2_u64(i);
  if (bits < 1) {
    bits = 1;
  }
  uint32_t rev = reverse32((uint32_t)i);
  *bitrev = ((uint64_t)rev >> (32 - bits)) % i;
  uint64_t h = i * PCOC_MIX_ADD;
  *mul = (h >> 32) % i;
}

/* NL4 base word, then parents and slot_mix, then store. §10.4. */
void pcoc_terrain_fill(const pcoc_sizes *sizes, const uint64_t P[4], uint8_t *terrain) {
  uint64_t nslots = sizes->n_slots;
  uint64_t wps = sizes->words_per_slot;
  uint64_t sb = sizes->slot_bytes;
  memset(terrain, 0, (size_t)sizes->terrain_bytes);

  /* One slot of working words. WORDS_PER_SLOT is at most 2048. */
  uint64_t w[2048];

  for (uint64_t i = 0; i < nslots; i++) {
    for (uint64_t j = 0; j < wps; j++) {
      w[j] = pcoc_nl4_base(i, j, P);
    }
    if (i > 0) {
      uint64_t bitrev, mul;
      parent_indices(i, &bitrev, &mul);
      uint64_t parents[3] = {i - 1, bitrev, mul};
      for (int k = 1; k <= 3; k++) {
        uint64_t p = parents[k - 1];
        const uint8_t *pslot = terrain + (size_t)(p * sb);
        uint64_t kmul = (uint64_t)k * PCOC_PARENT_MUL;
        for (uint64_t j = 0; j < wps; j++) {
          uint64_t pw = pcoc_u64_le(pslot + (size_t)(8 * j));
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
      pcoc_le64(w[j], slot + (size_t)(8 * j));
    }
  }
}

uint64_t pcoc_step_parts(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S,
                         uint64_t *slot_index, uint64_t *acc_out) {
  /* §11.1. Load every byte of the selected slot and fold every word with S.
   * acc starts at 0. The per-word input is word(w) XOR S. No w term. */
  uint64_t i = S % sizes->n_slots;
  const uint8_t *slot = terrain + (size_t)(i * sizes->slot_bytes);
  uint64_t acc = 0;
  uint64_t wps = sizes->words_per_slot;
  for (uint64_t w = 0; w < wps; w++) {
    uint64_t word = pcoc_u64_le(slot + (size_t)(8 * w));
    acc = acc + pcoc_mix64(word ^ S);
  }
  if (slot_index) {
    *slot_index = i;
  }
  if (acc_out) {
    *acc_out = acc;
  }
  uint8_t pre[20];
  memcpy(pre, PCOC_SEAL_DOMAIN, PCOC_SEAL_DOMAIN_LEN);
  pcoc_le64(acc, pre + PCOC_SEAL_DOMAIN_LEN);
  uint8_t E[32];
  pcoc_sha256(pre, 20, E);
  return pcoc_u64_le(E);
}

uint64_t pcoc_step(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S) {
  return pcoc_step_parts(sizes, terrain, S, NULL, NULL);
}

uint64_t pcoc_leg_result(const pcoc_sizes *sizes, const uint8_t *terrain,
                         const uint8_t previous[32]) {
  /* §11.2 */
  uint64_t S = pcoc_u64_le(previous);
  uint64_t steps = sizes->steps_per_leg;
  for (uint64_t t = 0; t < steps; t++) {
    S = pcoc_step(sizes, terrain, S);
  }
  return S;
}

void pcoc_terrain_sample(const pcoc_sizes *sizes, const uint8_t *terrain,
                         const uint8_t previous[32], uint8_t *out) {
  /* §11.3 */
  uint64_t offset = pcoc_u64_le(previous) % sizes->offset_modulus;
  memcpy(out, terrain + (size_t)offset, (size_t)sizes->sample_bytes);
}

void pcoc_checkpoint(const pcoc_sizes *sizes, const uint8_t *terrain,
                     const uint8_t previous[32], const uint8_t stage_limb[32],
                     uint64_t leg_result, uint8_t out[32]) {
  /* §11.4. Preimage is 32 + 32 + SAMPLE_BYTES + 8. */
  size_t n = 32 + 32 + (size_t)sizes->sample_bytes + 8;
  uint8_t pre[32 + 32 + 1024 + 8];
  memcpy(pre, previous, 32);
  memcpy(pre + 32, stage_limb, 32);
  pcoc_terrain_sample(sizes, terrain, previous, pre + 64);
  pcoc_le64(leg_result, pre + 64 + (size_t)sizes->sample_bytes);
  pcoc_sha256(pre, n, out);
}

void pcoc_checkpoint_fold(const uint8_t *checkpoints, size_t n, uint8_t fold[32]) {
  /* §12.2. F_0 = 0^32. F_{j+1} = SHA-256(F_j ‖ C[j]). n = 0 leaves F_0. */
  memset(fold, 0, 32);
  uint8_t buf[64];
  for (size_t j = 0; j < n; j++) {
    memcpy(buf, fold, 32);
    memcpy(buf + 32, checkpoints + 32 * j, 32);
    pcoc_sha256(buf, 64, fold);
  }
}

void pcoc_waypoint(const pcoc_sizes *sizes, const uint8_t *terrain,
                   const uint8_t interrupt[32], const uint8_t prev_waypoint[32],
                   const uint8_t *checkpoints, size_t n, uint8_t out[32]) {
  /* §12.2. W = SHA-256(B ‖ slot_sample ‖ W ‖ fold). */
  uint8_t fold[32];
  pcoc_checkpoint_fold(checkpoints, n, fold);
  uint64_t slot_index = pcoc_u64_le(interrupt) % sizes->n_slots;
  const uint8_t *slot_sample = terrain + (size_t)(slot_index * sizes->slot_bytes);
  size_t pre_len = 32 + (size_t)sizes->slot_bytes + 32 + 32;
  /* Production preimage is 16480. conformance-small is smaller.
   * Allocate the production maximum on the heap so a small profile does
   * not pay 16 KiB of stack, and a production call still has the room. */
  uint8_t stack_pre[32 + 256 + 32 + 32];
  uint8_t *pre = stack_pre;
  uint8_t *heap = NULL;
  if (pre_len > sizeof(stack_pre)) {
    heap = (uint8_t *)malloc(pre_len);
    pre = heap;
  }
  memcpy(pre, interrupt, 32);
  memcpy(pre + 32, slot_sample, (size_t)sizes->slot_bytes);
  memcpy(pre + 32 + (size_t)sizes->slot_bytes, prev_waypoint, 32);
  memcpy(pre + 32 + (size_t)sizes->slot_bytes + 32, fold, 32);
  pcoc_sha256(pre, pre_len, out);
  free(heap);
}
