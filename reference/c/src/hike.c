#include "hike.h"

#include <stdlib.h>
#include <string.h>

/* SPEC §6. */
static const uint64_t MIX_ADD = 0x9E3779B97F4A7C15ULL;
static const uint64_t MIX_MUL1 = 0xBF58476D1CE4E5B9ULL;
static const uint64_t MIX_MUL2 = 0x94D049BB133111EBULL;
static const uint64_t WORD_XOR = 0xC0FFEEULL;
static const uint64_t PARENT_MUL = 0xD1B54A32D192ED03ULL;
static const uint64_t SLOT_MUL = 0xA24BAED4963EE407ULL;

void hike_store_le64(uint8_t out[8], uint64_t x)
{
    unsigned i;
    for (i = 0; i < 8; i++) {
        out[i] = (uint8_t)(x >> (8 * i));
    }
}

uint64_t hike_load_le64(const uint8_t in[8])
{
    uint64_t x = 0;
    unsigned i;
    for (i = 0; i < 8; i++) {
        x |= (uint64_t)in[i] << (8 * i);
    }
    return x;
}

void hike_hex_encode(const uint8_t *bytes, size_t n, char *out)
{
    static const char hex[] = "0123456789abcdef";
    size_t i;
    for (i = 0; i < n; i++) {
        out[i * 2] = hex[bytes[i] >> 4];
        out[i * 2 + 1] = hex[bytes[i] & 0x0f];
    }
    out[n * 2] = 0;
}

int hike_hex_decode(const char *text, uint8_t *out, size_t n)
{
    size_t i;
    if (text == NULL) {
        return -1;
    }
    for (i = 0; i < n; i++) {
        char c0 = text[i * 2];
        char c1 = text[i * 2 + 1];
        int d0, d1;
        if (c0 >= '0' && c0 <= '9') {
            d0 = c0 - '0';
        } else if (c0 >= 'a' && c0 <= 'f') {
            d0 = c0 - 'a' + 10;
        } else if (c0 >= 'A' && c0 <= 'F') {
            d0 = c0 - 'A' + 10;
        } else {
            return -1;
        }
        if (c1 >= '0' && c1 <= '9') {
            d1 = c1 - '0';
        } else if (c1 >= 'a' && c1 <= 'f') {
            d1 = c1 - 'a' + 10;
        } else if (c1 >= 'A' && c1 <= 'F') {
            d1 = c1 - 'A' + 10;
        } else {
            return -1;
        }
        out[i] = (uint8_t)((d0 << 4) | d1);
    }
    if (text[n * 2] != 0) {
        return -1;
    }
    return 0;
}

void hike_u64_hex(uint64_t x, char out[17])
{
    uint8_t be[8];
    int i;
    for (i = 7; i >= 0; i--) {
        be[7 - i] = (uint8_t)(x >> (8 * i));
    }
    hike_hex_encode(be, 8, out);
}

static uint64_t rotl64(uint64_t w, unsigned r)
{
    if (r == 0) {
        return w;
    }
    return (w << r) | (w >> (64u - r));
}

/* SPEC §10.1. MIX_ADD is part of the function. */
static uint64_t mix64(uint64_t x)
{
    uint64_t y = x + MIX_ADD;
    y = (y ^ (y >> 30)) * MIX_MUL1;
    y = (y ^ (y >> 27)) * MIX_MUL2;
    return y ^ (y >> 31);
}

static uint32_t reverse32(uint32_t v)
{
    v = ((v >> 1) & 0x55555555u) | ((v & 0x55555555u) << 1);
    v = ((v >> 2) & 0x33333333u) | ((v & 0x33333333u) << 2);
    v = ((v >> 4) & 0x0f0f0f0fu) | ((v & 0x0f0f0f0fu) << 4);
    v = ((v >> 8) & 0x00ff00ffu) | ((v & 0x00ff00ffu) << 8);
    v = (v >> 16) | (v << 16);
    return v;
}

static int floor_log2_u32(uint32_t n)
{
    return 31 - __builtin_clz(n);
}

/* SPEC §10.3. */
static uint32_t bitrev_parent(uint32_t i)
{
    int bits = floor_log2_u32(i);
    uint32_t shifted;
    if (bits < 1) {
        bits = 1;
    }
    shifted = reverse32(i) >> (32 - bits);
    return shifted % i;
}

static uint32_t mul_parent(uint32_t i)
{
    uint64_t h = (uint64_t)i * MIX_ADD;
    return (uint32_t)((h >> 32) % i);
}

int hike_profile_ok(const HikeProfile *p)
{
    if (p == NULL) {
        return 0;
    }
    if (p->slot_bytes < 8 || (p->slot_bytes % 8) != 0 || p->slot_bytes > HIKE_SLOT_BYTES) {
        return 0;
    }
    if (p->steps_per_leg < 1 || p->steps_per_leg > HIKE_STEPS_PER_LEG) {
        return 0;
    }
    if (p->sample_bytes < 1 || p->sample_bytes > HIKE_SAMPLE_BYTES) {
        return 0;
    }
    if (p->terrain_bytes < p->sample_bytes || p->terrain_bytes > HIKE_TERRAIN_BYTES) {
        return 0;
    }
    if ((p->terrain_bytes % p->slot_bytes) != 0) {
        return 0;
    }
    return 1;
}

void hike_seed(const uint8_t public_key[32], const uint8_t seed_beacon[32], uint8_t seed[32])
{
    uint8_t preimage[64];
    memcpy(preimage, public_key, 32);
    memcpy(preimage + 32, seed_beacon, 32);
    hike_sha256(preimage, 64, seed);
}

void hike_lanes(const uint8_t seed[32], uint64_t lane[4])
{
    unsigned k;
    for (k = 0; k < 4; k++) {
        lane[k] = hike_load_le64(seed + 8 * k);
    }
}

uint64_t hike_base_word(const uint64_t lane[4], uint32_t slot_index, uint32_t word_index)
{
    uint64_t c = ((uint64_t)slot_index << 32) ^ (uint64_t)word_index ^ WORD_XOR;
    uint64_t t = mix64(lane[word_index & 3u] ^ c);
    t = mix64(t ^ rotl64(lane[(word_index + 1u) & 3u], 17));
    t = mix64(t ^ rotl64(lane[(word_index + 2u) & 3u], 31));
    t = mix64(t ^ rotl64(lane[(word_index + 3u) & 3u], 47));
    return t;
}

int hike_terrain_fill(const uint8_t seed[32], uint32_t slot_bytes, uint32_t terrain_bytes,
                      uint8_t *terrain)
{
    uint32_t words;
    uint32_t nslots;
    uint32_t i, j;
    uint64_t lane[4];
    uint64_t *w;

    if (terrain == NULL || slot_bytes < 8 || (slot_bytes % 8) != 0 ||
        slot_bytes > HIKE_SLOT_BYTES || terrain_bytes == 0 ||
        terrain_bytes > HIKE_TERRAIN_BYTES || (terrain_bytes % slot_bytes) != 0) {
        return HIKE_ERR;
    }
    words = slot_bytes / 8;
    nslots = terrain_bytes / slot_bytes;
    hike_lanes(seed, lane);
    w = (uint64_t *)malloc((size_t)words * sizeof(uint64_t));
    if (w == NULL) {
        return HIKE_ERR;
    }

    for (i = 0; i < nslots; i++) {
        for (j = 0; j < words; j++) {
            w[j] = hike_base_word(lane, i, j);
        }
        if (i > 0) {
            uint32_t parents[3];
            int k;
            uint64_t slot_mix;
            parents[0] = i - 1;
            parents[1] = bitrev_parent(i);
            parents[2] = mul_parent(i);
            for (k = 1; k <= 3; k++) {
                uint32_t parent = parents[k - 1];
                uint64_t kmul = (uint64_t)k * PARENT_MUL;
                const uint8_t *ps = terrain + (size_t)parent * slot_bytes;
                for (j = 0; j < words; j++) {
                    uint64_t pw = hike_load_le64(ps + (size_t)j * 8);
                    unsigned rot = (unsigned)((j + (uint32_t)k) % 64u);
                    w[j] = mix64((w[j] ^ rotl64(pw, rot)) ^ kmul);
                }
            }
            slot_mix = (uint64_t)i * SLOT_MUL;
            for (j = 0; j < words; j++) {
                w[j] = mix64(w[j] ^ slot_mix);
            }
        }
        {
            uint8_t *slot = terrain + (size_t)i * slot_bytes;
            for (j = 0; j < words; j++) {
                hike_store_le64(slot + (size_t)j * 8, w[j]);
            }
        }
    }
    free(w);
    return HIKE_OK;
}

uint64_t hike_fold_slot(const uint8_t *slot, uint32_t words, uint64_t state)
{
    uint64_t acc = 0;
    uint32_t w;
    /* Every word. acc starts at 0. Wrapping addition. SPEC §11.1.
     * The per-word input is word XOR S. There is no w term. */
    for (w = 0; w < words; w++) {
        uint64_t word = hike_load_le64(slot + (size_t)w * 8);
        acc += mix64(word ^ state);
    }
    return acc;
}

uint64_t hike_step(const uint8_t *terrain, const HikeProfile *p, uint64_t state)
{
    uint32_t words = p->slot_bytes / 8;
    uint32_t nslots = p->terrain_bytes / p->slot_bytes;
    uint32_t index = (uint32_t)(state % (uint64_t)nslots);
    const uint8_t *slot = terrain + (size_t)index * p->slot_bytes;
    uint64_t folded = hike_fold_slot(slot, words, state);
    uint8_t digest[32];
    hike_sha256_seal(folded, digest);
    return hike_load_le64(digest);
}

uint64_t hike_leg(const uint8_t *terrain, const HikeProfile *p, const uint8_t previous[32])
{
    uint64_t state = hike_load_le64(previous);
    uint64_t t;
    for (t = 0; t < p->steps_per_leg; t++) {
        state = hike_step(terrain, p, state);
    }
    return state;
}

void hike_sample(const uint8_t *terrain, const HikeProfile *p, const uint8_t previous[32],
                 uint8_t *sample)
{
    uint64_t modulus = (uint64_t)p->terrain_bytes - p->sample_bytes + 1;
    uint64_t offset = hike_load_le64(previous) % modulus;
    memcpy(sample, terrain + offset, p->sample_bytes);
}

void hike_checkpoint(const uint8_t previous[32], const uint8_t limb[32],
                     const uint8_t *sample, uint32_t sample_bytes, uint64_t leg_result,
                     uint8_t out[32])
{
    uint8_t result_le[8];
    const uint8_t *parts[4];
    size_t lens[4];
    hike_store_le64(result_le, leg_result);
    parts[0] = previous;
    lens[0] = 32;
    parts[1] = limb;
    lens[1] = 32;
    parts[2] = sample;
    lens[2] = sample_bytes;
    parts[3] = result_le;
    lens[3] = 8;
    hike_sha256_cat(parts, lens, 4, out);
}

void hike_fold_checkpoints(const uint8_t *checkpoints, uint32_t n, uint8_t fold[32])
{
    uint32_t j;
    memset(fold, 0, 32);
    for (j = 0; j < n; j++) {
        const uint8_t *parts[2];
        size_t lens[2];
        uint8_t next[32];
        parts[0] = fold;
        lens[0] = 32;
        parts[1] = checkpoints + (size_t)j * 32;
        lens[1] = 32;
        hike_sha256_cat(parts, lens, 2, next);
        memcpy(fold, next, 32);
    }
}

void hike_waypoint(const uint8_t *terrain, const HikeProfile *p, const uint8_t interrupt[32],
                   const uint8_t limb[32], const uint8_t *checkpoints, uint32_t n_ck,
                   uint8_t out[32])
{
    uint8_t fold[32];
    uint32_t nslots = p->terrain_bytes / p->slot_bytes;
    uint64_t slot_index = hike_load_le64(interrupt) % (uint64_t)nslots;
    const uint8_t *slot = terrain + (size_t)slot_index * p->slot_bytes;
    const uint8_t *parts[4];
    size_t lens[4];
    hike_fold_checkpoints(checkpoints, n_ck, fold);
    parts[0] = interrupt;
    lens[0] = 32;
    parts[1] = slot;
    lens[1] = p->slot_bytes;
    parts[2] = limb;
    lens[2] = 32;
    parts[3] = fold;
    lens[3] = 32;
    hike_sha256_cat(parts, lens, 4, out);
}

void hike_record_free(HikeRecord *record)
{
    uint32_t g;
    if (record == NULL || record->stages == NULL) {
        return;
    }
    for (g = 0; g < record->nstages; g++) {
        free(record->stages[g].legs);
        record->stages[g].legs = NULL;
    }
    free(record->stages);
    record->stages = NULL;
    record->nstages = 0;
}

int hike_produce(const uint8_t public_key[32], const uint8_t seed_beacon[32],
                 const HikeProfile *profile, const HikeStageIn *schedule, uint32_t nstages,
                 HikeRecord *out)
{
    uint8_t *terrain = NULL;
    uint8_t *sample = NULL;
    uint8_t waypoint[32];
    uint32_t g;
    int rc = HIKE_ERR;

    if (out == NULL || profile == NULL || schedule == NULL || nstages == 0 ||
        !hike_profile_ok(profile)) {
        return HIKE_ERR;
    }
    memset(out, 0, sizeof *out);
    out->profile = *profile;
    memcpy(out->public_key, public_key, 32);
    memcpy(out->seed_beacon, seed_beacon, 32);
    hike_seed(public_key, seed_beacon, out->seed);

    terrain = (uint8_t *)malloc(profile->terrain_bytes);
    sample = (uint8_t *)malloc(profile->sample_bytes);
    out->stages = (HikeStageRec *)calloc(nstages, sizeof(HikeStageRec));
    if (terrain == NULL || sample == NULL || out->stages == NULL) {
        goto done;
    }
    if (hike_terrain_fill(out->seed, profile->slot_bytes, profile->terrain_bytes, terrain) !=
        HIKE_OK) {
        goto done;
    }
    memcpy(waypoint, out->seed, 32);

    for (g = 0; g < nstages; g++) {
        HikeStageRec *stage = &out->stages[g];
        uint32_t nlegs = schedule[g].leg_count;
        uint32_t j;
        uint8_t *ck = NULL;
        uint8_t previous[32];

        if (nlegs < 1) {
            goto done;
        }
        if (g + 1 < nstages && !schedule[g].has_interrupt) {
            goto done;
        }
        stage->leg_count = nlegs;
        memcpy(stage->limb, waypoint, 32);
        stage->legs = (HikeLegRec *)calloc(nlegs, sizeof(HikeLegRec));
        out->nstages = g + 1;
        ck = (uint8_t *)malloc((size_t)nlegs * 32);
        if (stage->legs == NULL || ck == NULL) {
            free(ck);
            goto done;
        }
        memcpy(previous, stage->limb, 32);
        for (j = 0; j < nlegs; j++) {
            uint64_t result = hike_leg(terrain, profile, previous);
            hike_sample(terrain, profile, previous, sample);
            hike_checkpoint(previous, stage->limb, sample, profile->sample_bytes, result,
                            stage->legs[j].checkpoint);
            memcpy(stage->legs[j].previous, previous, 32);
            hike_store_le64(stage->legs[j].result_le, result);
            memcpy(ck + (size_t)j * 32, stage->legs[j].checkpoint, 32);
            memcpy(previous, stage->legs[j].checkpoint, 32);
        }
        if (schedule[g].has_interrupt) {
            stage->has_interrupt = 1;
            memcpy(stage->interrupt, schedule[g].interrupt, 32);
            hike_waypoint(terrain, profile, stage->interrupt, stage->limb, ck, nlegs,
                          stage->waypoint);
            stage->has_waypoint = 1;
            memcpy(waypoint, stage->waypoint, 32);
        }
        free(ck);
    }
    rc = HIKE_OK;
done:
    free(terrain);
    free(sample);
    if (rc != HIKE_OK) {
        hike_record_free(out);
    }
    return rc;
}

static int locate_leg(const HikeRecord *record, uint64_t index, uint32_t *stage_out,
                      uint32_t *leg_out)
{
    uint64_t cursor = 0;
    uint32_t g;
    for (g = 0; g < record->nstages; g++) {
        uint64_t n = record->stages[g].leg_count;
        if (index < cursor + n) {
            *stage_out = g;
            *leg_out = (uint32_t)(index - cursor);
            return 0;
        }
        cursor += n;
    }
    return -1;
}

static int verify_one(const uint8_t *terrain, const HikeRecord *record, uint64_t index,
                      const char **why)
{
    uint32_t g, j;
    const HikeStageRec *stage;
    uint8_t previous[32];
    uint8_t *sample;
    uint8_t digest[32];
    uint64_t walked;
    uint64_t stored;

    if (locate_leg(record, index, &g, &j) != 0) {
        *why = "index";
        return HIKE_ERR;
    }
    stage = &record->stages[g];
    if (j == 0) {
        memcpy(previous, stage->limb, 32);
    } else {
        memcpy(previous, stage->legs[j - 1].checkpoint, 32);
    }
    if (memcmp(previous, stage->legs[j].previous, 32) != 0) {
        *why = "chain";
        return HIKE_MISMATCH;
    }
    walked = hike_leg(terrain, &record->profile, previous);
    stored = hike_load_le64(stage->legs[j].result_le);
    if (walked != stored) {
        *why = "leg";
        return HIKE_MISMATCH;
    }
    sample = (uint8_t *)malloc(record->profile.sample_bytes);
    if (sample == NULL) {
        *why = "memory";
        return HIKE_ERR;
    }
    hike_sample(terrain, &record->profile, previous, sample);
    hike_checkpoint(previous, stage->limb, sample, record->profile.sample_bytes, stored, digest);
    free(sample);
    if (memcmp(digest, stage->legs[j].checkpoint, 32) != 0) {
        *why = "checkpoint";
        return HIKE_MISMATCH;
    }
    return HIKE_OK;
}

int hike_verify(const HikeRecord *record, const uint64_t *only, size_t n_only,
                uint64_t *fail_at, const char **why)
{
    uint8_t *terrain;
    size_t n;
    size_t k;
    uint64_t total = 0;
    uint32_t g;
    int rc;
    const char *local_why = "ok";

    if (record == NULL || record->stages == NULL || record->nstages == 0 ||
        !hike_profile_ok(&record->profile)) {
        if (why) {
            *why = "record";
        }
        return HIKE_ERR;
    }
    for (g = 0; g < record->nstages; g++) {
        total += record->stages[g].leg_count;
    }
    n = (only == NULL) ? (size_t)total : n_only;
    terrain = (uint8_t *)malloc(record->profile.terrain_bytes);
    if (terrain == NULL) {
        if (why) {
            *why = "memory";
        }
        return HIKE_ERR;
    }
    /* Once per check, not once per leg. SPEC §10.4. */
    if (hike_terrain_fill(record->seed, record->profile.slot_bytes, record->profile.terrain_bytes,
                          terrain) != HIKE_OK) {
        free(terrain);
        if (why) {
            *why = "terrain";
        }
        return HIKE_ERR;
    }
    {
        uint8_t seed_check[32];
        hike_seed(record->public_key, record->seed_beacon, seed_check);
        if (memcmp(seed_check, record->seed, 32) != 0) {
            free(terrain);
            if (fail_at) {
                *fail_at = 0;
            }
            if (why) {
                *why = "seed";
            }
            return HIKE_MISMATCH;
        }
    }
    /* Stage 0 opens at the seed. Each later stage opens at the previous waypoint. */
    if (memcmp(record->stages[0].limb, record->seed, 32) != 0) {
        free(terrain);
        if (fail_at) {
            *fail_at = 0;
        }
        if (why) {
            *why = "limb";
        }
        return HIKE_MISMATCH;
    }
    for (g = 1; g < record->nstages; g++) {
        if (!record->stages[g - 1].has_waypoint ||
            memcmp(record->stages[g].limb, record->stages[g - 1].waypoint, 32) != 0) {
            free(terrain);
            if (fail_at) {
                *fail_at = 0;
            }
            if (why) {
                *why = "limb";
            }
            return HIKE_MISMATCH;
        }
    }
    for (k = 0; k < n; k++) {
        uint64_t index = (only == NULL) ? (uint64_t)k : only[k];
        rc = verify_one(terrain, record, index, &local_why);
        if (rc != HIKE_OK) {
            free(terrain);
            if (fail_at) {
                *fail_at = index;
            }
            if (why) {
                *why = local_why;
            }
            return rc;
        }
    }
    /* §12.2 waypoint from the stored checkpoints. The last stage has no waypoint. */
    {
        uint64_t cursor = 0;
        for (g = 0; g < record->nstages; g++) {
            const HikeStageRec *stage = &record->stages[g];
            uint8_t *ck;
            uint8_t got[32];
            uint32_t j;
            int last = (g + 1 == record->nstages);
            if (last && !stage->has_interrupt && !stage->has_waypoint) {
                cursor += stage->leg_count;
                continue;
            }
            if (!stage->has_interrupt || !stage->has_waypoint) {
                free(terrain);
                if (fail_at) {
                    *fail_at = cursor;
                }
                if (why) {
                    *why = "waypoint";
                }
                return HIKE_MISMATCH;
            }
            ck = (uint8_t *)malloc((size_t)stage->leg_count * 32);
            if (ck == NULL) {
                free(terrain);
                if (why) {
                    *why = "memory";
                }
                return HIKE_ERR;
            }
            for (j = 0; j < stage->leg_count; j++) {
                memcpy(ck + (size_t)j * 32, stage->legs[j].checkpoint, 32);
            }
            hike_waypoint(terrain, &record->profile, stage->interrupt, stage->limb, ck,
                          stage->leg_count, got);
            free(ck);
            if (memcmp(got, stage->waypoint, 32) != 0) {
                free(terrain);
                if (fail_at) {
                    *fail_at = cursor;
                }
                if (why) {
                    *why = "waypoint";
                }
                return HIKE_MISMATCH;
            }
            cursor += stage->leg_count;
        }
    }
    free(terrain);
    if (why) {
        *why = "ok";
    }
    return HIKE_OK;
}

int hike_rate_meets(uint64_t hike_legs, int64_t hike_span, uint64_t p, uint64_t q, int *meets)
{
    __int128 left;
    __int128 right;
    if (meets == NULL || q == 0) {
        return HIKE_ERR;
    }
    /* SPEC Rule B2. No comparison when the span is not a positive integer. */
    if (hike_span <= 0) {
        *meets = 0;
        return HIKE_OK;
    }
    left = (__int128)q * (__int128)hike_legs;
    right = (__int128)p * (__int128)hike_span;
    *meets = left >= right;
    return HIKE_OK;
}

uint32_t hike_anchor_count(const uint32_t *indices, uint32_t n)
{
    uint8_t seen[17];
    uint32_t i;
    uint32_t count = 0;
    memset(seen, 0, sizeof seen);
    if (indices == NULL) {
        return 0;
    }
    for (i = 0; i < n; i++) {
        uint32_t g = indices[i];
        if (g >= 1 && g <= 16 && !seen[g]) {
            seen[g] = 1;
            count++;
        }
    }
    return count;
}

int hike_anchors_meet(const uint32_t *indices, uint32_t n)
{
    return hike_anchor_count(indices, n) >= 6;
}

int hike_kind_check(const char *kind, const char **reason)
{
    if (reason) {
        *reason = NULL;
    }
    if (kind == NULL) {
        return -1;
    }
    if (strcmp(kind, HIKE_KIND) == 0) {
        return 0;
    }
    /* Earlier kinds stop here. No terrain is filled. */
    if (strcmp(kind, HIKE_KIND_V20) == 0 || strcmp(kind, HIKE_KIND_V135) == 0) {
        if (reason) {
            *reason = "earlier_version";
        }
        return 1;
    }
    return -1;
}

int hike_mtp_n(int64_t height)
{
    if (height < 0) {
        return 0;
    }
    if (height + 1 < 11) {
        return (int)(height + 1);
    }
    return 11;
}

int hike_mtp(const int64_t *times, const int *missing, int n, int *not_read, int64_t *out)
{
    int64_t sorted[11];
    int i, j;
    if (not_read == NULL || out == NULL || n < 1 || n > 11 || times == NULL) {
        return HIKE_ERR;
    }
    for (i = 0; i < n; i++) {
        if (missing != NULL && missing[i]) {
            *not_read = 1;
            return HIKE_OK;
        }
        sorted[i] = times[i];
    }
    for (i = 1; i < n; i++) {
        int64_t v = sorted[i];
        j = i;
        while (j > 0 && sorted[j - 1] > v) {
            sorted[j] = sorted[j - 1];
            j--;
        }
        sorted[j] = v;
    }
    *not_read = 0;
    *out = sorted[n / 2];
    return HIKE_OK;
}

const char *hike_deadline(int tip_present, int64_t tip_height, int64_t last_beacon_height, int64_t k,
                          const char **reason)
{
    if (reason) {
        *reason = NULL;
    }
    if (!tip_present) {
        return "held";
    }
    if (tip_height < last_beacon_height) {
        return "held";
    }
    if (tip_height - last_beacon_height > k) {
        if (reason) {
            *reason = "submission_deadline";
        }
        return "refuse";
    }
    /* Rule B10. A gap that is not greater than K is not refused by this rule. */
    return "not refused by this rule";
}

int hike_thorough_rounds(uint64_t leg_total, const uint64_t *draws, size_t n_draws,
                         uint32_t indices[18], uint32_t *n_indices, int *defined, int *not_read)
{
    uint8_t *seen;
    size_t di = 0;
    uint32_t n = 0;
    int unlimited;
    uint64_t limit = 0;

    if (n_indices == NULL || defined == NULL || not_read == NULL) {
        return HIKE_ERR;
    }
    *n_indices = 0;
    *defined = 1;
    *not_read = 0;
    if (leg_total < 6) {
        *defined = 0;
        return HIKE_OK;
    }
    if (leg_total > 1000000ull || indices == NULL) {
        return HIKE_ERR;
    }
    /* M = 2^64. limit = M - (M mod leg_total). A power of two accepts every u. */
    unlimited = (leg_total & (leg_total - 1)) == 0;
    if (!unlimited) {
        unsigned __int128 mod = ((unsigned __int128)1 << 64) % leg_total;
        limit = (uint64_t)(((unsigned __int128)1 << 64) - mod);
    }
    seen = (uint8_t *)calloc((size_t)leg_total, 1);
    if (seen == NULL) {
        return HIKE_ERR;
    }
    while (n < 18) {
        uint64_t u;
        uint32_t q;
        uint32_t group;
        uint32_t k;
        int in_group = 0;
        int unused = 0;
        if (draws == NULL || di >= n_draws) {
            free(seen);
            *not_read = 1;
            *n_indices = n;
            return HIKE_OK;
        }
        u = draws[di++];
        if (!unlimited && u >= limit) {
            continue;
        }
        q = (uint32_t)(u % leg_total);
        group = (n / 6u) * 6u;
        for (k = group; k < n; k++) {
            if (indices[k] == q) {
                in_group = 1;
                break;
            }
        }
        if (in_group) {
            continue;
        }
        for (k = 0; k < leg_total; k++) {
            if (!seen[k]) {
                unused = 1;
                break;
            }
        }
        if (seen[q] && unused) {
            continue;
        }
        indices[n++] = q;
        seen[q] = 1;
    }
    free(seen);
    *n_indices = n;
    return HIKE_OK;
}

static int write_hex_line(FILE *fp, const char *key, const uint8_t *bytes, size_t n)
{
    char *text = (char *)malloc(n * 2 + 1);
    int rc;
    if (text == NULL) {
        return -1;
    }
    hike_hex_encode(bytes, n, text);
    rc = fprintf(fp, "%s %s\n", key, text);
    free(text);
    return rc < 0 ? -1 : 0;
}

int hike_record_write(FILE *fp, const HikeRecord *record)
{
    uint32_t g, j;
    if (fp == NULL || record == NULL) {
        return HIKE_ERR;
    }
    if (fprintf(fp, "PCOC-CHECKPOINT-V21\n") < 0) {
        return HIKE_ERR;
    }
    if (fprintf(fp, "slot_bytes %u\n", record->profile.slot_bytes) < 0 ||
        fprintf(fp, "terrain_bytes %u\n", record->profile.terrain_bytes) < 0 ||
        fprintf(fp, "sample_bytes %u\n", record->profile.sample_bytes) < 0 ||
        fprintf(fp, "steps_per_leg %llu\n", (unsigned long long)record->profile.steps_per_leg) <
            0) {
        return HIKE_ERR;
    }
    if (write_hex_line(fp, "public_key", record->public_key, 32) != 0 ||
        write_hex_line(fp, "seed_beacon", record->seed_beacon, 32) != 0 ||
        write_hex_line(fp, "seed", record->seed, 32) != 0) {
        return HIKE_ERR;
    }
    if (fprintf(fp, "stages %u\n", record->nstages) < 0) {
        return HIKE_ERR;
    }
    for (g = 0; g < record->nstages; g++) {
        const HikeStageRec *stage = &record->stages[g];
        if (fprintf(fp, "stage %u legs %u\n", g, stage->leg_count) < 0) {
            return HIKE_ERR;
        }
        if (write_hex_line(fp, "limb", stage->limb, 32) != 0) {
            return HIKE_ERR;
        }
        for (j = 0; j < stage->leg_count; j++) {
            char prev[65], result[17], ck[65];
            hike_hex_encode(stage->legs[j].previous, 32, prev);
            hike_hex_encode(stage->legs[j].result_le, 8, result);
            hike_hex_encode(stage->legs[j].checkpoint, 32, ck);
            if (fprintf(fp, "leg %u %s %s %s\n", j, prev, result, ck) < 0) {
                return HIKE_ERR;
            }
        }
        if (stage->has_interrupt) {
            if (write_hex_line(fp, "interrupt", stage->interrupt, 32) != 0) {
                return HIKE_ERR;
            }
        } else if (fprintf(fp, "interrupt -\n") < 0) {
            return HIKE_ERR;
        }
        if (stage->has_waypoint) {
            if (write_hex_line(fp, "waypoint", stage->waypoint, 32) != 0) {
                return HIKE_ERR;
            }
        } else if (fprintf(fp, "waypoint -\n") < 0) {
            return HIKE_ERR;
        }
    }
    return HIKE_OK;
}

static int expect_token(FILE *fp, const char *token)
{
    char got[64];
    if (fscanf(fp, "%63s", got) != 1) {
        return -1;
    }
    return strcmp(got, token) == 0 ? 0 : -1;
}

static int read_u32_line(FILE *fp, const char *key, uint32_t *out)
{
    unsigned value;
    if (expect_token(fp, key) != 0) {
        return -1;
    }
    if (fscanf(fp, "%u", &value) != 1) {
        return -1;
    }
    *out = value;
    return 0;
}

static int read_hex_line(FILE *fp, const char *key, uint8_t *out, size_t n)
{
    char text[160];
    if (n > 64 || expect_token(fp, key) != 0) {
        return -1;
    }
    if (fscanf(fp, "%159s", text) != 1) {
        return -1;
    }
    return hike_hex_decode(text, out, n);
}

int hike_record_read(FILE *fp, HikeRecord *record)
{
    uint32_t g;
    unsigned long long steps;
    char magic[64];

    if (fp == NULL || record == NULL) {
        return HIKE_ERR;
    }
    memset(record, 0, sizeof *record);
    if (fscanf(fp, "%63s", magic) != 1 || strcmp(magic, "PCOC-CHECKPOINT-V21") != 0) {
        return HIKE_ERR;
    }
    if (read_u32_line(fp, "slot_bytes", &record->profile.slot_bytes) != 0 ||
        read_u32_line(fp, "terrain_bytes", &record->profile.terrain_bytes) != 0 ||
        read_u32_line(fp, "sample_bytes", &record->profile.sample_bytes) != 0) {
        return HIKE_ERR;
    }
    if (expect_token(fp, "steps_per_leg") != 0 || fscanf(fp, "%llu", &steps) != 1) {
        return HIKE_ERR;
    }
    record->profile.steps_per_leg = (uint64_t)steps;
    if (!hike_profile_ok(&record->profile)) {
        return HIKE_ERR;
    }
    if (read_hex_line(fp, "public_key", record->public_key, 32) != 0 ||
        read_hex_line(fp, "seed_beacon", record->seed_beacon, 32) != 0 ||
        read_hex_line(fp, "seed", record->seed, 32) != 0 ||
        read_u32_line(fp, "stages", &record->nstages) != 0 || record->nstages == 0 ||
        record->nstages > 64) {
        return HIKE_ERR;
    }
    record->stages = (HikeStageRec *)calloc(record->nstages, sizeof(HikeStageRec));
    if (record->stages == NULL) {
        return HIKE_ERR;
    }
    for (g = 0; g < record->nstages; g++) {
        unsigned sg, nlegs, j;
        HikeStageRec *stage = &record->stages[g];
        char mark[80];
        if (expect_token(fp, "stage") != 0 || fscanf(fp, "%u", &sg) != 1 || sg != g ||
            expect_token(fp, "legs") != 0 || fscanf(fp, "%u", &nlegs) != 1 || nlegs < 1 ||
            nlegs > 1000000u) {
            hike_record_free(record);
            return HIKE_ERR;
        }
        stage->leg_count = nlegs;
        stage->legs = (HikeLegRec *)calloc(nlegs, sizeof(HikeLegRec));
        if (stage->legs == NULL || read_hex_line(fp, "limb", stage->limb, 32) != 0) {
            hike_record_free(record);
            return HIKE_ERR;
        }
        for (j = 0; j < nlegs; j++) {
            unsigned lj;
            char prev[80], result[40], ck[80];
            if (expect_token(fp, "leg") != 0 || fscanf(fp, "%u %79s %39s %79s", &lj, prev, result, ck) !=
                    4 ||
                lj != j || hike_hex_decode(prev, stage->legs[j].previous, 32) != 0 ||
                hike_hex_decode(result, stage->legs[j].result_le, 8) != 0 ||
                hike_hex_decode(ck, stage->legs[j].checkpoint, 32) != 0) {
                hike_record_free(record);
                return HIKE_ERR;
            }
        }
        if (fscanf(fp, "%79s", mark) != 1 || strcmp(mark, "interrupt") != 0 ||
            fscanf(fp, "%79s", mark) != 1) {
            hike_record_free(record);
            return HIKE_ERR;
        }
        if (strcmp(mark, "-") == 0) {
            stage->has_interrupt = 0;
        } else if (hike_hex_decode(mark, stage->interrupt, 32) != 0) {
            hike_record_free(record);
            return HIKE_ERR;
        } else {
            stage->has_interrupt = 1;
        }
        if (fscanf(fp, "%79s", mark) != 1 || strcmp(mark, "waypoint") != 0 ||
            fscanf(fp, "%79s", mark) != 1) {
            hike_record_free(record);
            return HIKE_ERR;
        }
        if (strcmp(mark, "-") == 0) {
            stage->has_waypoint = 0;
        } else if (hike_hex_decode(mark, stage->waypoint, 32) != 0) {
            hike_record_free(record);
            return HIKE_ERR;
        } else {
            stage->has_waypoint = 1;
        }
    }
    return HIKE_OK;
}
