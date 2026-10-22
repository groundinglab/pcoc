/* Satoshi Hike 2.2 — C SDK.
 * Public functions name the SPEC section they implement (Rule I2).
 * Production sizes are section 6. conformance-small may shrink
 * SAMPLE_BYTES, SLOT_BYTES, TERRAIN_BYTES, and STEPS_PER_LEG (§16.1).
 */
#ifndef PCOC_H
#define PCOC_H

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

#ifdef __cplusplus
extern "C" {
#endif

#define PCOC_SDK_VERSION "2.2.0" /* Rule I5; equals sdk/VERSION */
#define PCOC_SPEC_TAG "spec-2.2.0"

/* §6 constants (production). */
#define PCOC_SLOT_BYTES 16384u
#define PCOC_WORDS_PER_SLOT 2048u
#define PCOC_TERRAIN_BYTES 268435456u
#define PCOC_N_SLOTS 16384u
#define PCOC_SAMPLE_BYTES 1024u
#define PCOC_OFFSET_MODULUS 268434433u
#define PCOC_STEPS_PER_LEG 2000000u
#define PCOC_STAGES 18u
#define PCOC_HASH_BYTES 32u
#define PCOC_LEG_RESULT_BYTES 8u
#define PCOC_PUBLIC_KEY_BYTES 32u
#define PCOC_BEACON_BYTES 32u
#define PCOC_KIND "v2.2-pcoc-object"
#define PCOC_FILL "dag2"
#define PCOC_SEAL_DOMAIN "pcoc-walk-v1"
#define PCOC_SEAL_DOMAIN_LEN 12u
#define PCOC_MIX_ADD 0x9E3779B97F4A7C15ull
#define PCOC_MIX_MUL1 0xBF58476D1CE4E5B9ull
#define PCOC_MIX_MUL2 0x94D049BB133111EBull
#define PCOC_WORD_XOR 0xC0FFEEull
#define PCOC_PARENT_MUL 0xD1B54A32D192ED03ull
#define PCOC_SLOT_MUL 0xA24BAED4963EE407ull
#define PCOC_HASHCASH_BITS 21u /* §13.1 */
#define PCOC_ANCHOR_OFF_0 2u
#define PCOC_ANCHOR_OFF_1 3u
#define PCOC_ANCHOR_OFF_2 4u

/* Fixture hikes store at most 8 legs in a stage. A longer array is a
 * shape measurement the decoder still counts; the stored bytes kept here
 * cover the CASES profiles (1 and 3). */
#define PCOC_MAX_LEGS_PER_STAGE 8u

#define PCOC_CBOR_OK 0
#define PCOC_CBOR_MALFORMED 1
#define PCOC_CBOR_DECODE_FAILURE 2

typedef struct pcoc_profile {
  uint64_t slot_bytes;
  uint64_t sample_bytes;
  uint64_t terrain_bytes;
  uint64_t steps_per_leg;
  int conformance_small; /* 0 = production, 1 = conformance-small */
} pcoc_profile;

typedef struct pcoc_sizes {
  uint64_t slot_bytes;
  uint64_t words_per_slot;
  uint64_t terrain_bytes;
  uint64_t n_slots;
  uint64_t sample_bytes;
  uint64_t offset_modulus;
  uint64_t steps_per_leg;
} pcoc_sizes;

/* §10.1 mix64. */
uint64_t pcoc_mix64(uint64_t x);

/* §7 encodings. */
void pcoc_le64(uint64_t x, uint8_t out[8]);
uint64_t pcoc_u64_le(const uint8_t b[8]);

/* FIPS 180-4 SHA-256. Hardware SHA-NI when cpuid says the host has it (§11.1). */
void pcoc_sha256(const uint8_t *data, size_t len, uint8_t out[32]);
/* Double SHA-256 of an 80-byte header: natural header_hash (§13.1). */
void pcoc_sha256d_80(const uint8_t header[80], uint8_t out[32]);
/* "sha-ni" or "software". */
const char *pcoc_sha256_backend(void);

const char *pcoc_version(void); /* Rule I5: "2.2.0" */

/* build_info / selftest. spec_tag and pack_sha256 come from sdk/SPEC_PIN
 * when path is non-NULL and the file can be read; otherwise the compiled
 * tag and an empty pin. */
typedef struct pcoc_build_info {
  char version[16];
  char spec_tag[32];
  char pack_sha256[65];
  char sha256_backend[16];
  char kernel[16];
  int sha_ni;
} pcoc_build_info;

void pcoc_build_info_get(pcoc_build_info *info, const char *spec_pin_path);
/* Prints the selftest lines. Returns 0. */
int pcoc_selftest(FILE *out, const char *spec_pin_path);

void pcoc_profile_production(pcoc_profile *p);
/* Returns 0 when the four integers meet §16.1, -1 otherwise. */
int pcoc_profile_small(pcoc_profile *p, uint64_t slot_bytes, uint64_t sample_bytes,
                       uint64_t terrain_bytes, uint64_t steps_per_leg);
int pcoc_sizes_from_profile(const pcoc_profile *p, pcoc_sizes *s);

/* §10 seed. preimage is 64 bytes public_key ‖ seed_beacon. D is 32 bytes.
 * P0..P3 are the four little-endian lanes. */
void pcoc_seed(const uint8_t public_key[32], const uint8_t seed_beacon[32],
               uint8_t preimage[64], uint8_t D[32], uint64_t P[4]);

/* §10.4 dag2 fill. terrain is sizes->terrain_bytes, caller-owned.
 * Filled once per seed. The step does not write it. */
void pcoc_terrain_fill(const pcoc_sizes *sizes, const uint64_t P[4], uint8_t *terrain);

/* §10.4 NL4 base word at slot i, word j, before parents and slot_mix. */
uint64_t pcoc_nl4_base(uint64_t i, uint64_t j, const uint64_t P[4]);

/* §11.1 one step. Reads and folds every word of the selected slot.
 * The scalar fold is the only fold kernel in this phase. */
uint64_t pcoc_step(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S);
uint64_t pcoc_step_parts(const pcoc_sizes *sizes, const uint8_t *terrain, uint64_t S,
                         uint64_t *slot_index, uint64_t *acc);

/* §11.2 leg result. */
uint64_t pcoc_leg_result(const pcoc_sizes *sizes, const uint8_t *terrain,
                         const uint8_t previous[32]);

/* §11.3 terrain sample into out (sample_bytes). */
void pcoc_terrain_sample(const pcoc_sizes *sizes, const uint8_t *terrain,
                         const uint8_t previous[32], uint8_t *out);

/* §11.4 checkpoint. stage_limb is the seed on stage 0, W_g on stage g ≥ 1. */
void pcoc_checkpoint(const pcoc_sizes *sizes, const uint8_t *terrain,
                     const uint8_t previous[32], const uint8_t stage_limb[32],
                     uint64_t leg_result, uint8_t out[32]);

/* §12.2 fold of n checkpoints (each 32 bytes). n = 0 leaves 0^32. */
void pcoc_checkpoint_fold(const uint8_t *checkpoints, size_t n, uint8_t fold[32]);

/* §12.2 waypoint. */
void pcoc_waypoint(const pcoc_sizes *sizes, const uint8_t *terrain,
                   const uint8_t interrupt[32], const uint8_t prev_waypoint[32],
                   const uint8_t *checkpoints, size_t n, uint8_t out[32]);

/* §13.1 Hashcash. */
void pcoc_hashcash_challenge(const char *origin, size_t origin_len,
                             const uint8_t header_hash[32], uint8_t challenge[32]);
void pcoc_hashcash_digest(const uint8_t challenge[32], const uint8_t waypoint[32],
                          uint64_t nonce, uint8_t digest[32]);
int pcoc_leading_zero_bits(const uint8_t digest[32]);
int pcoc_hashcash_solve(const uint8_t challenge[32], const uint8_t waypoint[32], int bits,
                        uint64_t max_nonce, uint64_t *nonce_out);

/* §14.6 six-leg draw from the OS CSPRNG. No byte-source parameter.
 * *status: 0 indices filled, 1 not defined (leg_total < 6), 2 not read. */
int pcoc_six_leg_draw(uint64_t leg_total, uint64_t indices[6], int *status);

/* §14.6 rig variant: read8 returns 0 and writes 8 bytes, or -1 when exhausted. */
int pcoc_six_leg_draw_rig(uint64_t leg_total, int (*read8)(void *ctx, uint8_t out[8]), void *ctx,
                          uint64_t indices[6], int *status);

/* §15.5 thorough draw from the OS CSPRNG. indices must hold D = 6 * rounds.
 * *n_out is the number of indices appended (complete groups only when the
 * source ran dry: the caller sees *status). *status: 0 D indices, 1 not
 * defined (leg_total < 6), 2 not read (source exhausted before D). */
int pcoc_thorough_draw(uint64_t leg_total, int thorough_rounds, uint64_t *indices, size_t cap,
                       size_t *n_out, int *status);

/* §15.5 rig variant. Same status codes. The byte source is not hashed again. */
int pcoc_thorough_draw_rig(uint64_t leg_total, int thorough_rounds,
                           int (*read8)(void *ctx, uint8_t out[8]), void *ctx, uint64_t *indices,
                           size_t cap, size_t *n_out, int *status);

/* Rule B2. *outcome is "meets", "refuse", or "held".
 * held is hike_span not read (span_not_read = 1).
 * refuse is hike_span ≤ 0 or the integer comparison does not hold.
 * meets is q·hike_legs ≥ p·hike_span with hike_span > 0 (§15.4, §4). */
int pcoc_leg_rate_floor(uint64_t hike_legs, int span_not_read, int64_t hike_span, uint64_t p,
                        uint64_t q, const char **outcome);

/* §16.1 integer anchor comparison. Window only: the caller has already
 * treated the header as a stored interrupt whose plant comparison is match.
 * *outcome is "anchored" or "not anchored". */
int pcoc_anchor_window(uint64_t H, uint64_t header_height, const char **outcome);

/* Rule A5 record bytes, no newline. RFC 8785 spelling of kind_text. */
int pcoc_unknown_kind_record(const uint8_t *kind_text, size_t kind_len, char *buf, size_t cap);

/* ---- object (§8, Rule A3) ---- */

typedef struct pcoc_stage {
  uint8_t beacon[32];
  uint64_t beacon_height;
  uint64_t leg_count;
  uint8_t waypoint[32];
  int has_path; /* 0 on stage 0; 1 when the key is present */
  uint8_t path[256];
  size_t path_len;
  uint8_t checkpoints[PCOC_MAX_LEGS_PER_STAGE][32];
  uint8_t leg_results[PCOC_MAX_LEGS_PER_STAGE][8];
  uint64_t n_checkpoints;
  uint64_t n_leg_results;
  int checkpoint_width_bad;
  int leg_result_width_bad;
  int field_type_bad;
  int extra_key;
  int beacon_len_bad;
  int waypoint_len_bad;
} pcoc_stage;

typedef struct pcoc_object {
  char kind[256];
  size_t kind_len;
  int kind_is_text;
  char fill[32];
  size_t fill_len;
  uint64_t slot_bytes;
  uint64_t n_slots;
  uint64_t sample_bytes;
  uint64_t steps_per_leg;
  int has_slot_bytes, has_n_slots, has_sample_bytes, has_steps_per_leg;
  int has_kind, has_fill, has_public_key, has_seed_beacon;
  int has_stages, has_last_interrupt, has_last_interrupt_height;
  int public_key_len_bad, seed_beacon_len_bad, last_interrupt_len_bad;
  int field_type_bad;
  int extra_key;
  int duplicate_key;
  uint8_t public_key[32];
  uint8_t seed_beacon[32];
  pcoc_stage stages[PCOC_STAGES];
  size_t n_stages;
  uint8_t last_interrupt[32];
  size_t last_interrupt_len;
  uint64_t last_interrupt_height;
  int malformed;
  int cbor_class; /* PCOC_CBOR_* */
} pcoc_object;

/* Scripted-rig hike (§12). beacons[0] is the seed beacon; beacons[1..18]
 * are I_0 … I_17. heights[0..17] are the stage beacon heights; heights[18]
 * is last_interrupt_height. legs[g] ≥ 1. Paths on stages 1–17 are empty.
 * Returns 0, or -1. */
int pcoc_hike_run(const pcoc_sizes *sizes, const uint8_t *terrain, const uint8_t public_key[32],
                  const uint8_t seed_beacon[32], const uint8_t beacons[19][32],
                  const uint64_t heights[19], const int legs[18], pcoc_object *obj);

/* Encode a well-formed object in core deterministic CBOR (§7.3).
 * Returns the byte length, or -1. */
int pcoc_object_encode(const pcoc_object *obj, uint8_t *buf, size_t cap);

/* Decode. Returns 0 (PCOC_CBOR_OK), -1 malformed, -2 decode failure (§7.3). */
int pcoc_object_decode(const uint8_t *buf, size_t len, pcoc_object *obj, size_t *consumed);

/* ---- verify (§14) ---- */

typedef struct pcoc_index_doc {
  int present;
  int has_header;
  int header_hash_ok;
  uint8_t header_hash_display[32];
  int header_height_ok;
  uint64_t header_height;
  int header_time_ok;
  uint64_t header_time;
  int has_network;
  char network[32];
  size_t network_len;
  int n_plants;
  struct {
    int root_ok, txid_ok, block_hash_ok, vout_ok, block_height_ok;
    uint8_t root[32];
    uint8_t txid[32];
    uint8_t block_hash[32];
    uint64_t vout;
    uint64_t block_height;
  } plants[4];
} pcoc_index_doc;

typedef int (*pcoc_index_get)(void *ctx, uint64_t height, pcoc_index_doc *doc);

typedef struct pcoc_verify_input {
  const pcoc_profile *profile;
  const uint8_t *object_bytes;
  size_t object_len;
  pcoc_index_get index_get;
  void *index_ctx;
  /* Rig only. NULL reads the OS CSPRNG (§14.6). A non-NULL pointer is the
   * injected byte source and is not hashed again. */
  const uint8_t *draw_bytes;
  size_t draw_len;
  const uint8_t *terrain; /* optional prebuilt terrain of this seed; NULL builds it */
} pcoc_verify_input;

/* Write the verify record (no trailing newline) or the Rule A5 record.
 * Returns the length, -1 on an internal error, -2 on a §7.3 decode failure
 * (no record). A malformed CBOR value returns a record, not -2. */
int pcoc_verify(const pcoc_verify_input *in, char *buf, size_t cap);

#ifdef __cplusplus
}
#endif

#endif
