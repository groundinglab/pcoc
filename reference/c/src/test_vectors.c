#include "hike.h"
#include "json.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int g_fail = 0;
static int g_rows = 0;

static void row(const char *status, const char *name)
{
    printf("%-8s %s\n", status, name);
    g_rows++;
}

static void fail_msg(const char *name, const char *detail)
{
    printf("FAIL     %s %s\n", name, detail);
    g_fail++;
    g_rows++;
}

static char *slurp(const char *path)
{
    FILE *fp = fopen(path, "rb");
    long n;
    char *buf;
    if (fp == NULL) {
        return NULL;
    }
    if (fseek(fp, 0, SEEK_END) != 0) {
        fclose(fp);
        return NULL;
    }
    n = ftell(fp);
    if (n < 0) {
        fclose(fp);
        return NULL;
    }
    if (fseek(fp, 0, SEEK_SET) != 0) {
        fclose(fp);
        return NULL;
    }
    buf = (char *)malloc((size_t)n + 1);
    if (buf == NULL) {
        fclose(fp);
        return NULL;
    }
    if (fread(buf, 1, (size_t)n, fp) != (size_t)n) {
        free(buf);
        fclose(fp);
        return NULL;
    }
    fclose(fp);
    buf[n] = 0;
    return buf;
}

static Json *load_json(const char *dir, const char *name)
{
    char path[512];
    char *text;
    Json *v;
    const char *err = NULL;
    snprintf(path, sizeof path, "%s/%s", dir, name);
    text = slurp(path);
    if (text == NULL) {
        fprintf(stderr, "cannot read %s\n", path);
        return NULL;
    }
    v = json_parse(text, &err);
    free(text);
    if (v == NULL) {
        fprintf(stderr, "json %s: %s\n", path, err ? err : "?");
    }
    return v;
}

static int expect_hex(const char *name, const uint8_t *got, size_t n, const char *want)
{
    char enc[256];
    if (n * 2 + 1 > sizeof enc) {
        fail_msg(name, "hex too long");
        return -1;
    }
    hike_hex_encode(got, n, enc);
    if (strcmp(enc, want) != 0) {
        fprintf(stderr, "  %s got %s want %s\n", name, enc, want);
        fail_msg(name, "bytes");
        return -1;
    }
    return 0;
}

static int decode32(const char *hex, uint8_t out[32])
{
    return hike_hex_decode(hex, out, 32);
}

static int check_section_74(void)
{
    uint8_t dig[32];
    uint8_t pk[32], beacon[32], seed[32];
    uint64_t lane[4];
    uint64_t word;
    char hex[17];
    static const char *abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    static const char *seal =
        "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533";
    static const char *seed_hex =
        "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c";
    static const char *lanes[4] = {
        "545dfe297dc78951", "27858669c45e046a", "9cdac73ac1a5fe85", "7c81df6efbf55f11"
    };
    unsigned k;

    hike_sha256((const uint8_t *)"abc", 3, dig);
    if (expect_hex("sha256-abc", dig, 32, abc) != 0) {
        return -1;
    }
    hike_sha256_seal(0, dig);
    if (expect_hex("seal0", dig, 32, seal) != 0) {
        return -1;
    }
    if (hike_load_le64(dig) != 0x76042423afbd1648ULL) {
        fail_msg("seal0-s", "S'");
        return -1;
    }
    memset(pk, 0x11, 32);
    memset(beacon, 0x22, 32);
    hike_seed(pk, beacon, seed);
    if (expect_hex("seed-74", seed, 32, seed_hex) != 0) {
        return -1;
    }
    hike_lanes(seed, lane);
    for (k = 0; k < 4; k++) {
        hike_u64_hex(lane[k], hex);
        if (strcmp(hex, lanes[k]) != 0) {
            fprintf(stderr, "  lane %u got %s\n", k, hex);
            fail_msg("lanes-74", "lane");
            return -1;
        }
    }
    word = hike_base_word(lane, 0, 0);
    hike_u64_hex(word, hex);
    if (strcmp(hex, "c7582082a632c2c8") != 0) {
        fprintf(stderr, "  base word got %s\n", hex);
        fail_msg("nl4-74", "word");
        return -1;
    }
    row("MATCH", "section-7.4");
    return 0;
}

static int check_seed(const char *dir)
{
    Json *root = load_json(dir, "seed.json");
    uint8_t pk[32], beacon[32], seed[32], pre[64];
    uint64_t lane[4];
    const Json *lanes;
    const Json *eq;
    long long flag;
    unsigned k;
    int rc = -1;

    if (root == NULL) {
        fail_msg("seed", "load");
        return -1;
    }
    if (decode32(json_as_string(json_object_get(root, "public_key_hex")), pk) != 0 ||
        decode32(json_as_string(json_object_get(root, "seed_beacon_hex")), beacon) != 0) {
        fail_msg("seed", "hex");
        goto done;
    }
    hike_seed(pk, beacon, seed);
    if (expect_hex("seed", seed, 32, json_as_string(json_object_get(root, "seed_hex"))) != 0) {
        goto done;
    }
    memcpy(pre, pk, 32);
    memcpy(pre + 32, beacon, 32);
    if (expect_hex("seed-preimage", pre, 64,
                   json_as_string(json_object_get(root, "seed_preimage_hex"))) != 0) {
        goto done;
    }
    hike_lanes(seed, lane);
    lanes = json_object_get(root, "lanes_u64_hex");
    if (lanes == NULL || lanes->type != JSON_ARR || lanes->count != 4) {
        fail_msg("seed", "lanes");
        goto done;
    }
    for (k = 0; k < 4; k++) {
        char hex[17];
        hike_u64_hex(lane[k], hex);
        if (strcmp(hex, json_as_string(lanes->child[k])) != 0) {
            fail_msg("seed", "lane");
            goto done;
        }
    }
    eq = json_object_get(root, "waypoint_equals_seed");
    if (json_as_i64(eq, &flag) != 0 || flag != 1 || memcmp(seed, seed, 32) != 0) {
        fail_msg("seed", "waypoint_equals_seed");
        goto done;
    }
    row("MATCH", "seed");
    rc = 0;
done:
    json_free(root);
    return rc;
}

static int check_nl4(const char *dir)
{
    Json *root = load_json(dir, "nl4-words.json");
    uint8_t pk[32], beacon[32], seed[32];
    uint64_t lane[4];
    uint8_t *slot = NULL;
    uint8_t dig[32];
    const Json *samples;
    int i;
    int rc = -1;

    if (root == NULL) {
        fail_msg("nl4-words", "load");
        return -1;
    }
    if (decode32(json_as_string(json_object_get(root, "public_key_hex")), pk) != 0 ||
        decode32(json_as_string(json_object_get(root, "seed_beacon_hex")), beacon) != 0) {
        fail_msg("nl4-words", "hex");
        goto done;
    }
    hike_seed(pk, beacon, seed);
    hike_lanes(seed, lane);
    samples = json_object_get(root, "samples");
    if (samples == NULL || samples->type != JSON_ARR) {
        fail_msg("nl4-words", "samples");
        goto done;
    }
    for (i = 0; i < samples->count; i++) {
        const Json *s = samples->child[i];
        long long si, sj;
        uint64_t word;
        char hex[17];
        if (json_as_i64(json_object_get(s, "i"), &si) != 0 ||
            json_as_i64(json_object_get(s, "j"), &sj) != 0) {
            fail_msg("nl4-words", "index");
            goto done;
        }
        word = hike_base_word(lane, (uint32_t)si, (uint32_t)sj);
        hike_u64_hex(word, hex);
        if (strcmp(hex, json_as_string(json_object_get(s, "u64_hex"))) != 0) {
            fprintf(stderr, "  nl4 i=%lld j=%lld got %s want %s\n", si, sj, hex,
                    json_as_string(json_object_get(s, "u64_hex")));
            fail_msg("nl4-words", "base word");
            goto done;
        }
    }
    slot = (uint8_t *)malloc(16384);
    if (slot == NULL || hike_terrain_fill(seed, 16384, 16384, slot) != HIKE_OK) {
        fail_msg("nl4-words", "slot0");
        goto done;
    }
    hike_sha256(slot, 16384, dig);
    if (expect_hex("nl4-slot0", dig, 32, json_as_string(json_object_get(root, "slot0_sha256"))) !=
        0) {
        goto done;
    }
    row("MATCH", "nl4-words");
    rc = 0;
done:
    free(slot);
    json_free(root);
    return rc;
}

static int fill_hash(const uint8_t seed[32], uint32_t slot, uint32_t terrain, uint8_t dig[32])
{
    uint8_t *buf = (uint8_t *)malloc(terrain);
    if (buf == NULL) {
        return -1;
    }
    if (hike_terrain_fill(seed, slot, terrain, buf) != HIKE_OK) {
        free(buf);
        return -1;
    }
    hike_sha256(buf, terrain, dig);
    free(buf);
    return 0;
}

static int check_terrain(const char *dir)
{
    Json *root = load_json(dir, "terrain-small.json");
    uint8_t pk[32], beacon[32], seed[32], dig[32];
    const Json *cases;
    int i;
    int rc = -1;

    if (root == NULL) {
        fail_msg("terrain-small", "load");
        return -1;
    }
    if (decode32(json_as_string(json_object_get(root, "public_key_hex")), pk) != 0 ||
        decode32(json_as_string(json_object_get(root, "seed_beacon_hex")), beacon) != 0) {
        fail_msg("terrain-small", "hex");
        goto done;
    }
    hike_seed(pk, beacon, seed);
    cases = json_object_get(root, "cases");
    if (cases == NULL || cases->type != JSON_ARR) {
        fail_msg("terrain-small", "cases");
        goto done;
    }
    for (i = 0; i < cases->count; i++) {
        long long slot, bytes;
        const Json *c = cases->child[i];
        if (json_as_i64(json_object_get(c, "slot_bytes"), &slot) != 0 ||
            json_as_i64(json_object_get(c, "terrain_bytes"), &bytes) != 0 ||
            fill_hash(seed, (uint32_t)slot, (uint32_t)bytes, dig) != 0) {
            fail_msg("terrain-small", "fill");
            goto done;
        }
        if (expect_hex("terrain-small", dig, 32, json_as_string(json_object_get(c, "sha256"))) !=
            0) {
            goto done;
        }
    }
    row("MATCH", "terrain-small");
    rc = 0;
done:
    json_free(root);
    return rc;
}

static int check_step(const char *dir)
{
    Json *root = load_json(dir, "step-c1.json");
    uint8_t pk[32], beacon[32], seed[32];
    uint8_t *slot = NULL;
    uint8_t dig[32];
    const Json *steps;
    int i;
    int rc = -1;

    if (root == NULL) {
        fail_msg("step-c1", "load");
        return -1;
    }
    hike_sha256_seal(0, dig);
    if (expect_hex("step-seal0", dig, 32,
                   json_as_string(json_object_get(root, "seal0_digest_hex"))) != 0) {
        goto done;
    }
    {
        char hex[17];
        hike_u64_hex(hike_load_le64(dig), hex);
        if (strcmp(hex, json_as_string(json_object_get(root, "seal0_s_prime_u64_hex"))) != 0) {
            fail_msg("step-c1", "seal0 S'");
            goto done;
        }
    }
    if (decode32(json_as_string(json_object_get(root, "public_key_hex")), pk) != 0 ||
        decode32(json_as_string(json_object_get(root, "seed_beacon_hex")), beacon) != 0) {
        fail_msg("step-c1", "hex");
        goto done;
    }
    hike_seed(pk, beacon, seed);
    slot = (uint8_t *)malloc(16384);
    if (slot == NULL || hike_terrain_fill(seed, 16384, 16384, slot) != HIKE_OK) {
        fail_msg("step-c1", "slot");
        goto done;
    }
    steps = json_object_get(root, "steps");
    if (steps == NULL || steps->type != JSON_ARR) {
        fail_msg("step-c1", "steps");
        goto done;
    }
    for (i = 0; i < steps->count; i++) {
        const Json *s = steps->child[i];
        uint8_t sb[8];
        uint64_t state, folded, prime;
        char hex[17];
        HikeProfile profile;
        if (hike_hex_decode(json_as_string(json_object_get(s, "S_u64_hex")), sb, 8) != 0) {
            fail_msg("step-c1", "S");
            goto done;
        }
        /* Integer spelling, not little-endian bytes. */
        state = 0;
        {
            const char *t = json_as_string(json_object_get(s, "S_u64_hex"));
            size_t k;
            for (k = 0; t[k]; k++) {
                int d;
                char c = t[k];
                if (c >= '0' && c <= '9') {
                    d = c - '0';
                } else if (c >= 'a' && c <= 'f') {
                    d = c - 'a' + 10;
                } else {
                    fail_msg("step-c1", "S digit");
                    goto done;
                }
                state = (state << 4) | (uint64_t)d;
            }
        }
        (void)sb;
        folded = hike_fold_slot(slot, 2048, state);
        hike_u64_hex(folded, hex);
        if (strcmp(hex, json_as_string(json_object_get(s, "S_folded_u64_hex"))) != 0) {
            fprintf(stderr, "  folded got %s\n", hex);
            fail_msg("step-c1", "S_folded");
            goto done;
        }
        hike_sha256_seal(folded, dig);
        if (expect_hex("step-digest", dig, 32, json_as_string(json_object_get(s, "digest_hex"))) !=
            0) {
            goto done;
        }
        prime = hike_load_le64(dig);
        hike_u64_hex(prime, hex);
        if (strcmp(hex, json_as_string(json_object_get(s, "S_prime_u64_hex"))) != 0) {
            fail_msg("step-c1", "S'");
            goto done;
        }
        memset(&profile, 0, sizeof profile);
        profile.slot_bytes = 16384;
        profile.terrain_bytes = 16384;
        profile.sample_bytes = 1;
        profile.steps_per_leg = 1;
        if (hike_step(slot, &profile, state) != prime) {
            fail_msg("step-c1", "hike_step");
            goto done;
        }
    }
    row("MATCH", "step-c1");
    rc = 0;
done:
    free(slot);
    json_free(root);
    return rc;
}

static int check_hike_file(const char *dir, const char *file, const char *id)
{
    Json *root = load_json(dir, file);
    const Json *prof;
    const Json *stages;
    HikeProfile profile;
    HikeStageIn *sched = NULL;
    HikeRecord rec;
    uint8_t pk[32], beacon[32], dig[32];
    uint8_t *terrain = NULL;
    long long v;
    int i;
    int rc = -1;
    FILE *fp = NULL;
    HikeRecord again;

    memset(&rec, 0, sizeof rec);
    memset(&again, 0, sizeof again);
    if (root == NULL) {
        fail_msg(id, "load");
        return -1;
    }
    prof = json_object_get(root, "profile");
    if (json_as_i64(json_object_get(prof, "slot_bytes"), &v) != 0) {
        fail_msg(id, "profile");
        goto done;
    }
    profile.slot_bytes = (uint32_t)v;
    if (json_as_i64(json_object_get(prof, "terrain_bytes"), &v) != 0) {
        fail_msg(id, "profile");
        goto done;
    }
    profile.terrain_bytes = (uint32_t)v;
    if (json_as_i64(json_object_get(prof, "sample_bytes"), &v) != 0) {
        fail_msg(id, "profile");
        goto done;
    }
    profile.sample_bytes = (uint32_t)v;
    if (json_as_i64(json_object_get(prof, "steps_per_leg"), &v) != 0) {
        fail_msg(id, "profile");
        goto done;
    }
    profile.steps_per_leg = (uint64_t)v;
    if (decode32(json_as_string(json_object_get(root, "public_key_hex")), pk) != 0 ||
        decode32(json_as_string(json_object_get(root, "seed_beacon_hex")), beacon) != 0) {
        fail_msg(id, "hex");
        goto done;
    }
    stages = json_object_get(root, "stages");
    if (stages == NULL || stages->type != JSON_ARR || stages->count < 1) {
        fail_msg(id, "stages");
        goto done;
    }
    sched = (HikeStageIn *)calloc((size_t)stages->count, sizeof(HikeStageIn));
    if (sched == NULL) {
        fail_msg(id, "memory");
        goto done;
    }
    for (i = 0; i < stages->count; i++) {
        const Json *st = stages->child[i];
        const Json *beacon_j = json_object_get(st, "next_beacon_hex");
        if (json_as_i64(json_object_get(st, "leg_count"), &v) != 0 || v < 1) {
            fail_msg(id, "leg_count");
            goto done;
        }
        sched[i].leg_count = (uint32_t)v;
        if (json_is_null(beacon_j)) {
            sched[i].has_interrupt = 0;
        } else if (decode32(json_as_string(beacon_j), sched[i].interrupt) != 0) {
            fail_msg(id, "beacon");
            goto done;
        } else {
            sched[i].has_interrupt = 1;
        }
    }
    if (hike_produce(pk, beacon, &profile, sched, (uint32_t)stages->count, &rec) != HIKE_OK) {
        fail_msg(id, "produce");
        goto done;
    }
    if (expect_hex(id, rec.seed, 32, json_as_string(json_object_get(root, "seed_hex"))) != 0) {
        goto done;
    }
    if (memcmp(rec.stages[0].limb, rec.seed, 32) != 0 ||
        expect_hex(id, rec.stages[0].limb, 32,
                   json_as_string(json_object_get(root, "opening_waypoint_hex"))) != 0) {
        fail_msg(id, "opening");
        goto done;
    }
    terrain = (uint8_t *)malloc(profile.terrain_bytes);
    if (terrain == NULL ||
        hike_terrain_fill(rec.seed, profile.slot_bytes, profile.terrain_bytes, terrain) != HIKE_OK) {
        fail_msg(id, "terrain");
        goto done;
    }
    hike_sha256(terrain, profile.terrain_bytes, dig);
    if (expect_hex(id, dig, 32, json_as_string(json_object_get(root, "terrain_sha256"))) != 0) {
        goto done;
    }
    for (i = 0; i < stages->count; i++) {
        const Json *st = stages->child[i];
        const Json *legs = json_object_get(st, "legs_le_hex");
        const Json *cks = json_object_get(st, "checkpoints_hex");
        const Json *wp = json_object_get(st, "next_waypoint_hex");
        int j;
        if (legs == NULL || cks == NULL || legs->count != (int)rec.stages[i].leg_count ||
            cks->count != legs->count) {
            fail_msg(id, "leg list");
            goto done;
        }
        for (j = 0; j < legs->count; j++) {
            char got[17];
            hike_hex_encode(rec.stages[i].legs[j].result_le, 8, got);
            if (strcmp(got, json_as_string(legs->child[j])) != 0) {
                fprintf(stderr, "  %s stage %d leg %d result %s want %s\n", id, i, j, got,
                        json_as_string(legs->child[j]));
                fail_msg(id, "leg");
                goto done;
            }
            if (expect_hex(id, rec.stages[i].legs[j].checkpoint, 32,
                           json_as_string(cks->child[j])) != 0) {
                goto done;
            }
        }
        if (json_is_null(wp)) {
            if (rec.stages[i].has_waypoint) {
                fail_msg(id, "unexpected waypoint");
                goto done;
            }
        } else if (!rec.stages[i].has_waypoint ||
                   expect_hex(id, rec.stages[i].waypoint, 32, json_as_string(wp)) != 0) {
            fail_msg(id, "waypoint");
            goto done;
        }
    }
    {
        const char *why = NULL;
        uint64_t at = 0;
        uint64_t only[2];
        if (hike_verify(&rec, NULL, 0, &at, &why) != HIKE_OK) {
            fprintf(stderr, "  verify %s at %llu why %s\n", id, (unsigned long long)at,
                    why ? why : "?");
            fail_msg(id, "verify");
            goto done;
        }
        only[0] = 0;
        only[1] = 0;
        {
            uint64_t total = 0;
            for (i = 0; i < stages->count; i++) {
                total += rec.stages[i].leg_count;
            }
            only[1] = total - 1;
        }
        if (hike_verify(&rec, only, 2, &at, &why) != HIKE_OK) {
            fail_msg(id, "chosen legs");
            goto done;
        }
    }
    fp = tmpfile();
    if (fp == NULL || hike_record_write(fp, &rec) != HIKE_OK) {
        fail_msg(id, "write");
        goto done;
    }
    rewind(fp);
    if (hike_record_read(fp, &again) != HIKE_OK) {
        fail_msg(id, "read");
        goto done;
    }
    if (again.nstages != rec.nstages || memcmp(again.seed, rec.seed, 32) != 0) {
        fail_msg(id, "roundtrip");
        goto done;
    }
    for (i = 0; i < (int)rec.nstages; i++) {
        uint32_t j;
        if (again.stages[i].leg_count != rec.stages[i].leg_count ||
            memcmp(again.stages[i].limb, rec.stages[i].limb, 32) != 0 ||
            again.stages[i].has_waypoint != rec.stages[i].has_waypoint) {
            fail_msg(id, "roundtrip stage");
            goto done;
        }
        for (j = 0; j < rec.stages[i].leg_count; j++) {
            if (memcmp(&again.stages[i].legs[j], &rec.stages[i].legs[j], sizeof(HikeLegRec)) != 0) {
                fail_msg(id, "roundtrip leg");
                goto done;
            }
        }
        if (rec.stages[i].has_waypoint &&
            memcmp(again.stages[i].waypoint, rec.stages[i].waypoint, 32) != 0) {
            fail_msg(id, "roundtrip waypoint");
            goto done;
        }
    }
    {
        const char *why = NULL;
        if (hike_verify(&again, NULL, 0, NULL, &why) != HIKE_OK) {
            fail_msg(id, "verify reread");
            goto done;
        }
    }
    row("MATCH", id);
    rc = 0;
done:
    if (fp) {
        fclose(fp);
    }
    hike_record_free(&rec);
    hike_record_free(&again);
    free(sched);
    free(terrain);
    json_free(root);
    return rc;
}

static int json_bool(const Json *v, int *out)
{
    if (v == NULL || v->type != JSON_BOOL) {
        return -1;
    }
    *out = v->boolean;
    return 0;
}

static int check_rate(const char *dir)
{
    Json *root = load_json(dir, "rate.json");
    const Json *floor;
    const Json *cases;
    long long p, q;
    int i;
    int rc = -1;

    if (root == NULL) {
        fail_msg("rate", "load");
        return -1;
    }
    floor = json_object_get(root, "floor");
    if (json_as_i64(json_object_get(floor, "p"), &p) != 0 ||
        json_as_i64(json_object_get(floor, "q"), &q) != 0) {
        fail_msg("rate", "floor");
        goto done;
    }
    cases = json_object_get(root, "cases");
    for (i = 0; cases && i < cases->count; i++) {
        const Json *c = cases->child[i];
        long long legs, span;
        int meets = 0, want = 0;
        const char *name = json_as_string(json_object_get(c, "name"));
        if (json_as_i64(json_object_get(c, "hike_legs"), &legs) != 0 ||
            json_as_i64(json_object_get(c, "hike_span"), &span) != 0 ||
            json_bool(json_object_get(c, "meets"), &want) != 0 ||
            hike_rate_meets((uint64_t)legs, span, (uint64_t)p, (uint64_t)q, &meets) != HIKE_OK) {
            fail_msg("rate", name ? name : "?");
            goto done;
        }
        if (meets != want) {
            fprintf(stderr, "  rate %s meets %d want %d\n", name, meets, want);
            fail_msg(name, "rate");
            goto done;
        }
        if (want) {
            row("MATCH", name);
        } else {
            row("REJECT", name);
        }
    }
    {
        int meets = 1;
        if (hike_rate_meets(10, 0, 1, 5, &meets) != HIKE_OK || meets != 0) {
            fail_msg("rate-span-zero", "meets");
            goto done;
        }
        if (hike_rate_meets(10, -1, 1, 5, &meets) != HIKE_OK || meets != 0) {
            fail_msg("rate-span-negative", "meets");
            goto done;
        }
        row("REJECT", "rate-span-zero");
    }
    rc = 0;
done:
    json_free(root);
    return rc;
}

static int check_anchors(const char *dir)
{
    Json *root = load_json(dir, "anchors.json");
    const Json *cases;
    int i;
    int rc = -1;

    if (root == NULL) {
        fail_msg("anchors", "load");
        return -1;
    }
    cases = json_object_get(root, "cases");
    for (i = 0; cases && i < cases->count; i++) {
        const Json *c = cases->child[i];
        const Json *idx = json_object_get(c, "indices");
        const char *name = json_as_string(json_object_get(c, "name"));
        uint32_t *vals;
        int j, want = 0, got;
        if (idx == NULL || idx->type != JSON_ARR || json_bool(json_object_get(c, "meets"), &want) != 0) {
            fail_msg("anchors", "case");
            goto done;
        }
        vals = (uint32_t *)calloc((size_t)idx->count, sizeof(uint32_t));
        if (vals == NULL) {
            fail_msg("anchors", "memory");
            goto done;
        }
        for (j = 0; j < idx->count; j++) {
            long long g;
            if (json_as_i64(idx->child[j], &g) != 0) {
                free(vals);
                fail_msg("anchors", "index");
                goto done;
            }
            vals[j] = (uint32_t)g;
        }
        got = hike_anchors_meet(vals, (uint32_t)idx->count);
        free(vals);
        if (got != want) {
            fail_msg(name ? name : "anchors", "meets");
            goto done;
        }
        if (want) {
            row("MATCH", name);
        } else {
            row("REJECT", name);
        }
    }
    {
        uint32_t dup[8] = {1, 1, 1, 2, 3, 4, 5, 6};
        uint32_t out[6] = {17, 0, 1, 2, 3, 4};
        if (hike_anchor_count(dup, 8) != 6 || !hike_anchors_meet(dup, 8)) {
            fail_msg("anchors-distinct", "count");
            goto done;
        }
        if (hike_anchor_count(out, 6) != 4 || hike_anchors_meet(out, 6)) {
            fail_msg("anchors-range", "W_17");
            goto done;
        }
        row("MATCH", "anchors-distinct");
        row("REJECT", "anchors-out-of-range");
    }
    rc = 0;
done:
    json_free(root);
    return rc;
}

static int reject_u64(const char *name, uint64_t got, const char *conforming, const char *forbidden)
{
    char hex[17];
    hike_u64_hex(got, hex);
    if (strcmp(hex, conforming) != 0) {
        fprintf(stderr, "  %s got %s conforming %s\n", name, hex, conforming);
        fail_msg(name, "conforming");
        return -1;
    }
    if (strcmp(hex, forbidden) == 0) {
        fail_msg(name, "emitted forbidden");
        return -1;
    }
    row("REJECT", name);
    return 0;
}

static int reject_hash(const char *name, const uint8_t dig[32], const char *conforming,
                       const char *forbidden)
{
    char hex[65];
    hike_hex_encode(dig, 32, hex);
    if (strcmp(hex, conforming) != 0) {
        fprintf(stderr, "  %s got %s\n", name, hex);
        fail_msg(name, "conforming");
        return -1;
    }
    if (strcmp(hex, forbidden) == 0) {
        fail_msg(name, "emitted forbidden");
        return -1;
    }
    row("REJECT", name);
    return 0;
}

static int check_negatives(const char *dir)
{
    Json *root = load_json(dir, "negatives.json");
    uint8_t pk[32], beacon[32], seed[32], dig[32];
    const Json *cases;
    int i;
    int rc = -1;

    if (root == NULL) {
        fail_msg("negatives", "load");
        return -1;
    }
    if (decode32(json_as_string(json_object_get(root, "public_key_hex")), pk) != 0 ||
        decode32(json_as_string(json_object_get(root, "seed_beacon_hex")), beacon) != 0) {
        fail_msg("negatives", "hex");
        goto done;
    }
    hike_seed(pk, beacon, seed);
    cases = json_object_get(root, "cases");
    if (cases == NULL) {
        fail_msg("negatives", "cases");
        goto done;
    }
    for (i = 0; i < cases->count; i++) {
        const Json *c = cases->child[i];
        const char *name = json_as_string(json_object_get(c, "name"));
        if (strcmp(name, "fill_pin_terrain") == 0) {
            long long slot, bytes;
            if (json_as_i64(json_object_get(c, "slot_bytes"), &slot) != 0 ||
                json_as_i64(json_object_get(c, "terrain_bytes"), &bytes) != 0 ||
                fill_hash(seed, (uint32_t)slot, (uint32_t)bytes, dig) != 0 ||
                reject_hash(name, dig, json_as_string(json_object_get(c, "conforming_sha256")),
                            json_as_string(json_object_get(c, "forbidden_sha256"))) != 0) {
                if (g_fail == 0) {
                    fail_msg(name, "fill");
                }
                goto done;
            }
        } else if (strcmp(name, "ref_word") == 0 || strcmp(name, "all4_word") == 0) {
            uint64_t lane[4];
            long long si, sj;
            hike_lanes(seed, lane);
            if (json_as_i64(json_object_get(c, "i"), &si) != 0 ||
                json_as_i64(json_object_get(c, "j"), &sj) != 0 ||
                reject_u64(name, hike_base_word(lane, (uint32_t)si, (uint32_t)sj),
                           json_as_string(json_object_get(c, "conforming_u64_hex")),
                           json_as_string(json_object_get(c, "forbidden_u64_hex"))) != 0) {
                goto done;
            }
        } else if (strcmp(name, "mix64_without_mix_add") == 0 || strcmp(name, "fatslot_fold") == 0 ||
                   strcmp(name, "partial_slot_read") == 0) {
            uint8_t *slot = (uint8_t *)malloc(16384);
            uint64_t folded;
            if (slot == NULL || hike_terrain_fill(seed, 16384, 16384, slot) != HIKE_OK) {
                free(slot);
                fail_msg(name, "slot");
                goto done;
            }
            folded = hike_fold_slot(slot, 2048, 0);
            free(slot);
            if (reject_u64(name, folded, json_as_string(json_object_get(c, "conforming_S_folded_u64_hex")),
                           json_as_string(json_object_get(c, "forbidden_S_folded_u64_hex"))) != 0) {
                goto done;
            }
        } else if (strcmp(name, "f3_shared_terrain") == 0) {
            uint8_t pk2[32], seed2[32], dig2[32];
            long long slot, bytes;
            if (decode32(json_as_string(json_object_get(c, "pk2_hex")), pk2) != 0 ||
                json_as_i64(json_object_get(c, "slot_bytes"), &slot) != 0 ||
                json_as_i64(json_object_get(c, "terrain_bytes"), &bytes) != 0) {
                fail_msg(name, "fields");
                goto done;
            }
            hike_seed(pk2, beacon, seed2);
            if (fill_hash(seed, (uint32_t)slot, (uint32_t)bytes, dig) != 0 ||
                fill_hash(seed2, (uint32_t)slot, (uint32_t)bytes, dig2) != 0) {
                fail_msg(name, "fill");
                goto done;
            }
            if (memcmp(dig, dig2, 32) == 0) {
                fail_msg(name, "terrains collided");
                goto done;
            }
            if (reject_hash("f3-pk1", dig, json_as_string(json_object_get(c, "conforming_sha256_pk1")),
                            json_as_string(json_object_get(c, "forbidden_sha256_pk1"))) != 0) {
                goto done;
            }
            if (reject_hash("f3-pk2", dig2,
                            json_as_string(json_object_get(c, "conforming_sha256_pk2")),
                            json_as_string(json_object_get(c, "forbidden_sha256_pk2"))) != 0) {
                goto done;
            }
            row("REJECT", name);
        } else {
            fprintf(stderr, "unknown negative %s\n", name);
            fail_msg(name, "unknown");
            goto done;
        }
    }
    rc = 0;
done:
    json_free(root);
    return rc;
}

static int check_cached_leg(void)
{
    HikeProfile profile;
    HikeStageIn sched[2];
    HikeRecord rec;
    uint8_t pk[32], beacon[32];
    uint8_t terrain[256];
    uint8_t sample[32];
    uint8_t digest[32];
    uint64_t stolen, real;
    const char *why = "none";
    uint64_t at = 99;
    int vr;

    memset(&rec, 0, sizeof rec);
    memset(pk, 0x42, 32);
    memset(beacon, 0, 32);
    beacon[0] = 1;
    profile.slot_bytes = 64;
    profile.terrain_bytes = 256;
    profile.sample_bytes = 32;
    profile.steps_per_leg = 32;
    memset(sched, 0, sizeof sched);
    sched[0].leg_count = 1;
    sched[0].has_interrupt = 1;
    sched[0].interrupt[0] = 2;
    sched[1].leg_count = 1;
    if (hike_produce(pk, beacon, &profile, sched, 2, &rec) != HIKE_OK) {
        fail_msg("cached_leg", "produce");
        return -1;
    }
    stolen = hike_load_le64(rec.stages[0].legs[0].result_le);
    real = hike_load_le64(rec.stages[1].legs[0].result_le);
    if (stolen == real) {
        hike_record_free(&rec);
        fail_msg("cached_leg", "fixture collision");
        return -1;
    }
    if (hike_terrain_fill(rec.seed, 64, 256, terrain) != HIKE_OK) {
        hike_record_free(&rec);
        fail_msg("cached_leg", "terrain");
        return -1;
    }
    hike_sample(terrain, &profile, rec.stages[1].legs[0].previous, sample);
    hike_checkpoint(rec.stages[1].legs[0].previous, rec.stages[1].limb, sample, 32, stolen, digest);
    hike_store_le64(rec.stages[1].legs[0].result_le, stolen);
    memcpy(rec.stages[1].legs[0].checkpoint, digest, 32);
    vr = hike_verify(&rec, NULL, 0, &at, &why);
    hike_record_free(&rec);
    if (vr != HIKE_MISMATCH || why == NULL || strcmp(why, "leg") != 0 || at != 1) {
        fprintf(stderr, "  cached_leg rc %d at %llu why %s\n", vr, (unsigned long long)at,
                why ? why : "?");
        fail_msg("cached_leg", "expected leg mismatch");
        return -1;
    }
    row("REJECT", "cached_leg");
    return 0;
}

int main(int argc, char **argv)
{
    const char *dir;

    if (argc != 2) {
        fprintf(stderr, "usage: hike-test <v2.1-dir>\n");
        return 2;
    }
    dir = argv[1];
    printf("sha256_hw %d\n", hike_sha256_hw());
    printf("sha256_hw_fault %d\n", hike_sha256_hw_fault());
    check_section_74();
    check_seed(dir);
    check_nl4(dir);
    check_terrain(dir);
    check_step(dir);
    check_rate(dir);
    check_anchors(dir);
    check_negatives(dir);
    check_hike_file(dir, "test-3leg.json", "test-3leg");
    check_hike_file(dir, "kat-small.json", "kat-small");
    check_cached_leg();
    if (g_fail) {
        printf("FAILED %d\n", g_fail);
        return 1;
    }
    printf("ALL MATCH rows %d\n", g_rows);
    return 0;
}
