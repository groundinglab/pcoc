#ifndef HIKE_H
#define HIKE_H

/*
 * Satoshi Hike v2.1 reference, built from the specification text.
 * Terrain, fold, and hike keep no slot digest, word table, partial read,
 * or leg-result cache. Each step mixes every word of the selected slot.
 */

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

#define HIKE_HASH 32

enum {
    HIKE_OK = 0,
    HIKE_ERR = 1,
    HIKE_MISMATCH = 2
};

/* Section 6 production constants. conformance-small may shrink four of them. */
enum {
    HIKE_SLOT_BYTES = 16384,
    HIKE_TERRAIN_BYTES = 268435456,
    HIKE_SAMPLE_BYTES = 1024,
    HIKE_STEPS_PER_LEG = 2000000,
    HIKE_N_SLOTS = 16384,
    HIKE_WORDS_PER_SLOT = 2048,
    HIKE_STAGES = 18
};

typedef struct HikeProfile {
    uint32_t slot_bytes;
    uint32_t terrain_bytes;
    uint32_t sample_bytes;
    uint64_t steps_per_leg;
} HikeProfile;

typedef struct HikeLegRec {
    uint8_t previous[HIKE_HASH];
    uint8_t result_le[8];
    uint8_t checkpoint[HIKE_HASH];
} HikeLegRec;

typedef struct HikeStageRec {
    uint8_t limb[HIKE_HASH];
    uint32_t leg_count;
    HikeLegRec *legs;
    int has_interrupt;
    uint8_t interrupt[HIKE_HASH];
    int has_waypoint;
    uint8_t waypoint[HIKE_HASH];
} HikeStageRec;

typedef struct HikeRecord {
    HikeProfile profile;
    uint8_t public_key[HIKE_HASH];
    uint8_t seed_beacon[HIKE_HASH];
    uint8_t seed[HIKE_HASH];
    uint32_t nstages;
    HikeStageRec *stages;
} HikeRecord;

typedef struct HikeStageIn {
    uint32_t leg_count;
    int has_interrupt;
    uint8_t interrupt[HIKE_HASH];
} HikeStageIn;

int hike_profile_ok(const HikeProfile *p);

/* 1 when SHA-256 compression uses SHA-NI. 0 on a host with no such instructions. */
int hike_sha256_hw(void);
/* 1 when SHA-NI was reported but disagreed with the software compress. */
int hike_sha256_hw_fault(void);

void hike_sha256(const uint8_t *data, size_t n, uint8_t out[32]);
void hike_sha256_cat(const uint8_t *const *parts, const size_t *lens, size_t nparts,
                     uint8_t out[32]);
/* One-block seal. The digest is E, not the seed D. SPEC §11.1, 20-byte input. */
void hike_sha256_seal(uint64_t s_folded, uint8_t out[32]);

void hike_seed(const uint8_t public_key[32], const uint8_t seed_beacon[32],
               uint8_t seed[32]);
void hike_lanes(const uint8_t seed[32], uint64_t lane[4]);

/* NL4 base word, before parents and slot_mix. SPEC §10.4. */
uint64_t hike_base_word(const uint64_t lane[4], uint32_t slot_index, uint32_t word_index);

int hike_terrain_fill(const uint8_t seed[32], uint32_t slot_bytes,
                      uint32_t terrain_bytes, uint8_t *terrain);

/* Wrapping-add accumulator of one slot. Reads every word. SPEC §11.1. */
uint64_t hike_fold_slot(const uint8_t *slot, uint32_t words, uint64_t state);

#define HIKE_KIND "satoshi-hike-v2.1-object"
#define HIKE_KIND_V20 "satoshi-hike-v2.0-object"
#define HIKE_KIND_V135 "satoshi-hike-v1.35-object"

/*
 * 0 current kind. 1 earlier kind: reason earlier_version, and the caller
 * must not fill terrain. -1 is any other text.
 */
int hike_kind_check(const char *kind, const char **reason);

/* BIP113 median. n = min(11, height+1). missing[i] != 0 means that time was not read. */
int hike_mtp_n(int64_t height);
int hike_mtp(const int64_t *times, const int *missing, int n, int *not_read, int64_t *out);

/*
 * Gate receipt rule. tip_present == 0 is a missing tip (held).
 * *reason is submission_deadline only on refuse.
 * Returns "accept", "refuse", or "held". Does not touch terrain.
 */
const char *hike_deadline(int tip_present, int64_t tip_height, int64_t last_beacon_height,
                          int64_t k, const char **reason);

/*
 * Thorough-round index draw, section 15.5, using injected u64 draws.
 * leg_total < 6 sets *defined = 0 and does not read draws.
 * A short draw list sets *not_read = 1.
 * indices holds up to 18 global indices in draw order.
 */
int hike_thorough_rounds(uint64_t leg_total, const uint64_t *draws, size_t n_draws,
                         uint32_t indices[18], uint32_t *n_indices, int *defined, int *not_read);

uint64_t hike_step(const uint8_t *terrain, const HikeProfile *p, uint64_t state);

uint64_t hike_leg(const uint8_t *terrain, const HikeProfile *p,
                  const uint8_t previous[32]);

void hike_sample(const uint8_t *terrain, const HikeProfile *p,
                 const uint8_t previous[32], uint8_t *sample);

void hike_checkpoint(const uint8_t previous[32], const uint8_t limb[32],
                     const uint8_t *sample, uint32_t sample_bytes,
                     uint64_t leg_result, uint8_t out[32]);

void hike_fold_checkpoints(const uint8_t *checkpoints, uint32_t n, uint8_t fold[32]);

void hike_waypoint(const uint8_t *terrain, const HikeProfile *p,
                   const uint8_t interrupt[32], const uint8_t limb[32],
                   const uint8_t *checkpoints, uint32_t n_ck, uint8_t out[32]);

int hike_produce(const uint8_t public_key[32], const uint8_t seed_beacon[32],
                 const HikeProfile *profile, const HikeStageIn *schedule,
                 uint32_t nstages, HikeRecord *out);

void hike_record_free(HikeRecord *record);

/*
 * Re-walk chosen global leg indices. only == NULL re-walks every leg.
 * Terrain is filled once for the call, then each chosen leg is walked in full.
 * Returns HIKE_MISMATCH and sets *why to "seed", "limb", "chain", "leg",
 * "checkpoint", or "waypoint".
 */
int hike_verify(const HikeRecord *record, const uint64_t *only, size_t n_only,
                uint64_t *fail_at, const char **why);

/* Rule B2. *meets is 1 only when span > 0 and q*legs >= p*span. */
int hike_rate_meets(uint64_t hike_legs, int64_t hike_span, uint64_t p, uint64_t q,
                    int *meets);

/* Distinct indices in 1..16. Rule B4 meets when the count is at least 6. */
uint32_t hike_anchor_count(const uint32_t *indices, uint32_t n);
int hike_anchors_meet(const uint32_t *indices, uint32_t n);

int hike_record_write(FILE *fp, const HikeRecord *record);
int hike_record_read(FILE *fp, HikeRecord *record);

void hike_store_le64(uint8_t out[8], uint64_t x);
uint64_t hike_load_le64(const uint8_t in[8]);
void hike_hex_encode(const uint8_t *bytes, size_t n, char *out);
int hike_hex_decode(const char *text, uint8_t *out, size_t n);
void hike_u64_hex(uint64_t x, char out[17]);

#endif
