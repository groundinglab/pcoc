/* check_v21.c — independent check of spec/conformance/v2.1.
 *
 * Written from Satoshi Hike v2.1 (spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md):
 *   §10 seed and NL4, §10.1 mix64, §10.3 parents, §10.4 dag2 fill,
 *   §11.1 fold and seal, §11.2–§11.4 checkpoint, §12.2 waypoint,
 *   Rule B2 integer floor, including hike_span <= 0,
 *   the submission deadline (tip_height - last_beacon_height > K),
 *   §15.5 thorough rounds, Rule B4 anchored_total >= 6,
 *   Rule A4.10 median-time-past.
 * The §11.1 accumulator is wrapping addition. The withdrawn maps (one-lane,
 * linear four-lane, fill pin, the v2.0 fold, the XOR accumulator,
 * mix64 without MIX_ADD, a short slot read) are the §10.4 / §11.1
 * rejects. This file does not include the generator.
 *
 *   gcc -O2 -o check_v21 check_v21.c -lcrypto
 *   ./check_v21 spec/conformance/v2.1
 */
#include <openssl/sha.h>
#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MASK 0xffffffffffffffffULL
#define MIX_ADD  0x9E3779B97F4A7C15ULL
#define MIX_MUL1 0xBF58476D1CE4E5B9ULL
#define MIX_MUL2 0x94D049BB133111EBULL
#define WORD_XOR 0xC0FFEEULL
#define PARENT_MUL 0xD1B54A32D192ED03ULL
#define SLOT_MUL   0xA24BAED4963EE407ULL

static const uint8_t SEAL[12] = {'p','c','o','c','-','w','a','l','k','-','v','1'};
/* Withdrawn v2.0 fill-pin label. The fill-pin negative is the only use. */
static const uint8_t WITHDRAWN_V20_SEED_DOMAIN[8] = {'P','S','E','E','D','6','4',0};

static int g_fail;

static void die(const char *m) { fprintf(stderr, "check_v21: %s\n", m); exit(2); }

static uint64_t mix64(uint64_t x) {
    uint64_t y = x + MIX_ADD;
    y = (y ^ (y >> 30)) * MIX_MUL1;
    y = (y ^ (y >> 27)) * MIX_MUL2;
    return y ^ (y >> 31);
}

/* §11.1: a function that omits + MIX_ADD is not mix64. */
static uint64_t bare64(uint64_t x) {
    uint64_t y = x;
    y = (y ^ (y >> 30)) * MIX_MUL1;
    y = (y ^ (y >> 27)) * MIX_MUL2;
    return y ^ (y >> 31);
}

static uint64_t rotl(uint64_t w, unsigned r) {
    r &= 63;
    return r ? (w << r) | (w >> (64 - r)) : w;
}

static uint32_t reverse32(uint32_t v) {
    uint32_t r = 0;
    for (int k = 0; k < 32; k++) if (v & (1u << k)) r |= 1u << (31 - k);
    return r;
}

static int floor_log2(uint64_t n) {
    int b = 0;
    while ((1ULL << (b + 1)) <= n && b < 63) b++;
    return b;
}

static void parents_of(uint64_t i, uint64_t out[3]) {
    int bits = floor_log2(i);
    if (bits < 1) bits = 1;
    out[0] = i - 1;
    out[1] = ((uint64_t)(reverse32((uint32_t)i) >> (32 - bits))) % i;
    out[2] = (((i * MIX_ADD) >> 32) % i);
}

static uint64_t load_le(const uint8_t *b) {
    uint64_t v = 0;
    for (int i = 0; i < 8; i++) v |= (uint64_t)b[i] << (8 * i);
    return v;
}

static void store_le(uint8_t *b, uint64_t v) {
    for (int i = 0; i < 8; i++) b[i] = (uint8_t)(v >> (8 * i));
}

static uint64_t nl4_word(const uint64_t P[4], uint64_t i, uint64_t j) {
    uint64_t c = (i << 32) ^ j ^ WORD_XOR;
    uint64_t t = mix64(P[j % 4] ^ c);
    t = mix64(t ^ rotl(P[(j + 1) % 4], 17));
    t = mix64(t ^ rotl(P[(j + 2) % 4], 31));
    t = mix64(t ^ rotl(P[(j + 3) % 4], 47));
    return t;
}

/* One lane only: the first NL4 step, which §10.4 rejects. */
static uint64_t ref_word(const uint64_t P[4], uint64_t i, uint64_t j) {
    uint64_t c = (i << 32) ^ j ^ WORD_XOR;
    return mix64(P[j % 4] ^ c);
}

/* Linear mix of the four lanes in one mix64. §10.4 rejects it. */
static uint64_t all4_word(const uint64_t P[4], uint64_t i, uint64_t j) {
    uint64_t c = (i << 32) ^ j ^ WORD_XOR;
    uint64_t x = P[j % 4] ^ rotl(P[(j + 1) % 4], 17) ^ rotl(P[(j + 2) % 4], 31) ^ rotl(P[(j + 3) % 4], 47) ^ c;
    return mix64(x);
}

/* First two NL4 steps only. Not a terrain word. */
static uint64_t pair_word(const uint64_t P[4], uint64_t i, uint64_t j) {
    uint64_t c = (i << 32) ^ j ^ WORD_XOR;
    uint64_t t = mix64(P[j % 4] ^ c);
    return mix64(t ^ rotl(P[(j + 1) % 4], 17));
}

static uint64_t fill_pin(const uint8_t pre[64]) {
    uint64_t x = load_le(WITHDRAWN_V20_SEED_DOMAIN);
    for (uint64_t i = 0; i < 8; i++)
        x = mix64(x ^ load_le(pre + 8 * i) ^ (i * MIX_ADD));
    return x;
}

static uint64_t pin_word(uint64_t pin, uint64_t i, uint64_t j) {
    return mix64(pin ^ (i << 32) ^ j ^ WORD_XOR);
}

typedef uint64_t (*wordfn)(const void *ctx, uint64_t i, uint64_t j);

static uint64_t word_nl4(const void *ctx, uint64_t i, uint64_t j) {
    return nl4_word((const uint64_t *)ctx, i, j);
}
static uint64_t word_pin(const void *ctx, uint64_t i, uint64_t j) {
    return pin_word(*(const uint64_t *)ctx, i, j);
}
static uint64_t word_all4(const void *ctx, uint64_t i, uint64_t j) {
    return all4_word((const uint64_t *)ctx, i, j);
}

static uint8_t *fill_terrain(wordfn fn, const void *ctx, uint64_t slot_bytes, uint64_t terrain_bytes) {
    uint64_t words = slot_bytes / 8;
    uint64_t nslots = terrain_bytes / slot_bytes;
    uint64_t *slots = calloc(nslots * words, sizeof(uint64_t));
    uint8_t *out = calloc(1, terrain_bytes);
    if (!slots || !out) die("oom");
    uint64_t *w = calloc(words, sizeof(uint64_t));
    if (!w) die("oom");
    for (uint64_t i = 0; i < nslots; i++) {
        for (uint64_t j = 0; j < words; j++) w[j] = fn(ctx, i, j);
        if (i > 0) {
            uint64_t par[3];
            parents_of(i, par);
            for (int k = 1; k <= 3; k++) {
                uint64_t p = par[k - 1];
                for (uint64_t j = 0; j < words; j++) {
                    unsigned rot = (unsigned)((j + (uint64_t)k) % 64);
                    uint64_t pw = slots[p * words + j];
                    w[j] = mix64((w[j] ^ rotl(pw, rot)) ^ ((uint64_t)k * PARENT_MUL));
                }
            }
            uint64_t sm = i * SLOT_MUL;
            for (uint64_t j = 0; j < words; j++) w[j] = mix64(w[j] ^ sm);
        }
        for (uint64_t j = 0; j < words; j++) {
            slots[i * words + j] = w[j];
            store_le(out + i * slot_bytes + 8 * j, w[j]);
        }
    }
    free(w);
    free(slots);
    return out;
}

static void sha256(const uint8_t *p, size_t n, uint8_t out[32]) { SHA256(p, n, out); }

static uint64_t seal(uint64_t folded) {
    uint8_t buf[20], d[32];
    memcpy(buf, SEAL, 12);
    store_le(buf + 12, folded);
    sha256(buf, 20, d);
    return load_le(d);
}

/* §11.1: acc += mix64(word XOR S). No w term. bare=1 omits MIX_ADD. */
static uint64_t fold_mix(const uint8_t *slot, uint64_t words, uint64_t S, int bare, uint64_t nread) {
    uint64_t acc = 0;
    uint64_t lim = nread < words ? nread : words;
    for (uint64_t w = 0; w < lim; w++) {
        uint64_t word = load_le(slot + 8 * w);
        uint64_t x = word ^ S;
        acc += bare ? bare64(x) : mix64(x);
    }
    return acc;
}

/* Withdrawn fold: acc += mix64(word XOR w XOR S). Negative xor_w_fold. */
static uint64_t fold_xor_w(const uint8_t *slot, uint64_t words, uint64_t S) {
    uint64_t acc = 0;
    for (uint64_t w = 0; w < words; w++) {
        uint64_t word = load_le(slot + 8 * w);
        acc += mix64(word ^ w ^ S);
    }
    return acc;
}

/* Withdrawn c886ef2 fold: acc += mix64(word XOR (S + w)). Negative s_plus_w_fold. */
static uint64_t fold_s_plus_w(const uint8_t *slot, uint64_t words, uint64_t S) {
    uint64_t acc = 0;
    for (uint64_t w = 0; w < words; w++) {
        uint64_t word = load_le(slot + 8 * w);
        acc += mix64(word ^ (S + w));
    }
    return acc;
}

/* Withdrawn XOR accumulator of mix64 outputs. Not §11.1 and not the v2.0 fold. */
static uint64_t fold_xor_acc(const uint8_t *slot, uint64_t words, uint64_t S) {
    uint64_t acc = 0;
    for (uint64_t w = 0; w < words; w++) {
        uint64_t word = load_le(slot + 8 * w);
        acc ^= mix64(word ^ w ^ S);
    }
    return acc;
}

/* Withdrawn v2.0 fold: S XOR (XOR of (word XOR w)). Not the §11.1 fold. */
static uint64_t fold_v2_0(const uint8_t *slot, uint64_t words, uint64_t S) {
    uint64_t acc = 0;
    for (uint64_t w = 0; w < words; w++) acc ^= load_le(slot + 8 * w) ^ w;
    return S ^ acc;
}

/* --- minimal JSON --- */
typedef enum { JNULL, JBOOL, JNUM, JSTR, JARR, JOBJ } JKind;
typedef struct JVal JVal;
typedef struct { char *k; JVal *v; } JPair;
struct JVal {
    JKind k;
    int b;
    long long n;
    char *s;
    JVal **a; int an, ac;
    JPair *o; int on, oc;
};

static void skip(const char **p) { while (**p && isspace((unsigned char)**p)) (*p)++; }

static JVal *jv(JKind k) { JVal *v = calloc(1, sizeof *v); if (!v) die("oom"); v->k = k; return v; }

static int hexval(char c);
static char *parse_str(const char **p) {
    if (**p != '"') die("string");
    (*p)++;
    size_t n = 0, c = 64;
    char *s = malloc(c);
    if (!s) die("oom");
    while (**p && **p != '"') {
        char ch = *(*p)++;
        if (ch == '\\') {
            char e = *(*p)++;
            if (e == 'u') {
                int cp = 0;
                for (int k = 0; k < 4; k++) {
                    int h = hexval(*(*p)++);
                    if (h < 0) die("hex escape");
                    cp = (cp << 4) | h;
                }
                if (cp < 0x80) ch = (char)cp;
                else if (cp < 0x800) {
                    if (n + 2 >= c) { c *= 2; s = realloc(s, c); if (!s) die("oom"); }
                    s[n++] = (char)(0xC0 | (cp >> 6));
                    ch = (char)(0x80 | (cp & 0x3F));
                } else {
                    if (n + 3 >= c) { c *= 2; s = realloc(s, c); if (!s) die("oom"); }
                    s[n++] = (char)(0xE0 | (cp >> 12));
                    s[n++] = (char)(0x80 | ((cp >> 6) & 0x3F));
                    ch = (char)(0x80 | (cp & 0x3F));
                }
            } else if (e == 'n') ch = '\n';
            else if (e == 't') ch = '\t';
            else if (e == '"' || e == '\\' || e == '/') ch = e;
            else die("escape");
        }
        if (n + 1 >= c) { c *= 2; s = realloc(s, c); if (!s) die("oom"); }
        s[n++] = ch;
    }
    if (**p != '"') die("unterminated");
    (*p)++;
    s[n] = 0;
    return s;
}

static JVal *parse_val(const char **p);

static JVal *parse_val(const char **p) {
    skip(p);
    if (**p == '"') { JVal *v = jv(JSTR); v->s = parse_str(p); return v; }
    if (**p == 'n') { *p += 4; return jv(JNULL); }
    if (**p == 't') { *p += 4; JVal *v = jv(JBOOL); v->b = 1; return v; }
    if (**p == 'f') { *p += 5; return jv(JBOOL); }
    if (**p == '-' || isdigit((unsigned char)**p)) {
        JVal *v = jv(JNUM);
        v->n = strtoll(*p, (char **)p, 10);
        return v;
    }
    if (**p == '[') {
        (*p)++;
        JVal *v = jv(JARR);
        skip(p);
        if (**p == ']') { (*p)++; return v; }
        for (;;) {
            if (v->an == v->ac) { v->ac = v->ac ? v->ac * 2 : 4; v->a = realloc(v->a, v->ac * sizeof *v->a); }
            v->a[v->an++] = parse_val(p);
            skip(p);
            if (**p == ',') { (*p)++; continue; }
            if (**p == ']') { (*p)++; break; }
            die("array");
        }
        return v;
    }
    if (**p == '{') {
        (*p)++;
        JVal *v = jv(JOBJ);
        skip(p);
        if (**p == '}') { (*p)++; return v; }
        for (;;) {
            skip(p);
            char *k = parse_str(p);
            skip(p);
            if (**p != ':') die("colon");
            (*p)++;
            if (v->on == v->oc) { v->oc = v->oc ? v->oc * 2 : 4; v->o = realloc(v->o, v->oc * sizeof *v->o); }
            v->o[v->on].k = k;
            v->o[v->on].v = parse_val(p);
            v->on++;
            skip(p);
            if (**p == ',') { (*p)++; continue; }
            if (**p == '}') { (*p)++; break; }
            die("object");
        }
        return v;
    }
    die("value");
    return NULL;
}

static JVal *jget(const JVal *o, const char *k) {
    if (!o || o->k != JOBJ) return NULL;
    for (int i = 0; i < o->on; i++) if (strcmp(o->o[i].k, k) == 0) return o->o[i].v;
    return NULL;
}

static const char *jstr(const JVal *o, const char *k) {
    JVal *v = jget(o, k);
    if (!v || v->k != JSTR) die(k);
    return v->s;
}

static const char *jstr_or_null(const JVal *o, const char *k, int *is_null) {
    JVal *v = jget(o, k);
    if (!v) die(k);
    if (v->k == JNULL) { *is_null = 1; return NULL; }
    if (v->k != JSTR) die(k);
    *is_null = 0;
    return v->s;
}

static long long jnum(const JVal *o, const char *k) {
    JVal *v = jget(o, k);
    if (!v || v->k != JNUM) die(k);
    return v->n;
}

static int jbool(const JVal *o, const char *k) {
    JVal *v = jget(o, k);
    if (!v || v->k != JBOOL) die(k);
    return v->b;
}

static int hexval(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

static uint8_t *unhex(const char *s, size_t *n) {
    size_t L = strlen(s);
    if (L % 2) die("odd hex");
    *n = L / 2;
    uint8_t *b = malloc(*n ? *n : 1);
    if (!b) die("oom");
    for (size_t i = 0; i < *n; i++) {
        int hi = hexval(s[2 * i]), lo = hexval(s[2 * i + 1]);
        if (hi < 0 || lo < 0) die("hex");
        b[i] = (uint8_t)((hi << 4) | lo);
    }
    return b;
}

static void hex_of(const uint8_t *b, size_t n, char *o) {
    static const char *H = "0123456789abcdef";
    for (size_t i = 0; i < n; i++) { o[2 * i] = H[b[i] >> 4]; o[2 * i + 1] = H[b[i] & 15]; }
    o[2 * n] = 0;
}

static void lanes_of(const uint8_t d[32], uint64_t P[4]) {
    for (int k = 0; k < 4; k++) P[k] = load_le(d + 8 * k);
}

static void seed_of(const uint8_t *pk, const uint8_t *be, uint8_t d[32], uint64_t P[4]) {
    uint8_t pre[64];
    memcpy(pre, pk, 32);
    memcpy(pre + 32, be, 32);
    sha256(pre, 64, d);
    lanes_of(d, P);
}

static void expect_str(const char *what, const char *got, const char *want) {
    if (strcmp(got, want) != 0) {
        fprintf(stderr, "FAIL %s\n  got  %s\n  want %s\n", what, got, want);
        g_fail++;
    }
}

static void pass(const char *id) { printf("PASS %s\n", id); }
static void reject(const char *id) { printf("REJECT %s\n", id); }

static JVal *load_json(const char *path) {
    FILE *f = fopen(path, "rb");
    if (!f) die(path);
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *buf = malloc((size_t)n + 1);
    if (!buf || fread(buf, 1, (size_t)n, f) != (size_t)n) die("read");
    buf[n] = 0;
    fclose(f);
    const char *p = buf;
    JVal *v = parse_val(&p);
    free(buf);
    return v;
}

static char *path_join(const char *dir, const char *name) {
    size_t n = strlen(dir) + 1 + strlen(name) + 1;
    char *s = malloc(n);
    snprintf(s, n, "%s/%s", dir, name);
    return s;
}

static void slot0_base(const uint64_t P[4], uint64_t nwords, uint8_t *out) {
    for (uint64_t j = 0; j < nwords; j++) store_le(out + 8 * j, nl4_word(P, 0, j));
}

static uint8_t *checkpoint(const uint8_t *terrain, uint64_t terrain_bytes, uint64_t sample_bytes,
                           const uint8_t prev[32], const uint8_t limb[32], uint64_t leg) {
    uint64_t offmod = terrain_bytes - sample_bytes + 1;
    uint64_t offset = load_le(prev) % offmod;
    size_t n = 64 + sample_bytes + 8;
    uint8_t *pre = malloc(n);
    memcpy(pre, prev, 32);
    memcpy(pre + 32, limb, 32);
    memcpy(pre + 64, terrain + offset, sample_bytes);
    store_le(pre + 64 + sample_bytes, leg);
    uint8_t *out = malloc(32);
    sha256(pre, n, out);
    free(pre);
    return out;
}

static uint8_t *wayp(const uint8_t *terrain, uint64_t slot_bytes, uint64_t nslots,
                     const uint8_t *beacon, const uint8_t *stage_w, uint8_t cps[][32], int nc) {
    uint8_t fold[32];
    memset(fold, 0, 32);
    for (int i = 0; i < nc; i++) {
        uint8_t buf[64];
        memcpy(buf, fold, 32);
        memcpy(buf + 32, cps[i], 32);
        sha256(buf, 64, fold);
    }
    uint64_t si = load_le(beacon) % nslots;
    size_t L = 64 + slot_bytes + 32;
    uint8_t *pre = malloc(L);
    memcpy(pre, beacon, 32);
    memcpy(pre + 32, terrain + si * slot_bytes, slot_bytes);
    memcpy(pre + 32 + slot_bytes, stage_w, 32);
    memcpy(pre + 64 + slot_bytes, fold, 32);
    uint8_t *out = malloc(32);
    sha256(pre, L, out);
    free(pre);
    return out;
}

static uint64_t do_leg(const uint8_t *terrain, uint64_t terrain_bytes, uint64_t slot_bytes, uint64_t steps, uint64_t S) {
    uint64_t nslots = terrain_bytes / slot_bytes;
    uint64_t words = slot_bytes / 8;
    for (uint64_t t = 0; t < steps; t++) {
        uint64_t i = S % nslots;
        S = seal(fold_mix(terrain + i * slot_bytes, words, S, 0, words));
    }
    return S;
}

static void check_hike(const char *dir, const char *file) {
    int before_hike = g_fail;
    char *path = path_join(dir, file);
    JVal *root = load_json(path);
    size_t np, nb;
    uint8_t *pk = unhex(jstr(root, "public_key_hex"), &np);
    uint8_t *be = unhex(jstr(root, "seed_beacon_hex"), &nb);
    if (np != 32 || nb != 32) die("key len");
    uint8_t d[32];
    uint64_t P[4];
    seed_of(pk, be, d, P);
    char hx[80];
    hex_of(d, 32, hx);
    expect_str(file, hx, jstr(root, "seed_hex"));
    expect_str("opening", jstr(root, "opening_waypoint_hex"), hx);
    if (jnum(root, "waypoint_equals_seed") != 1) { fprintf(stderr, "FAIL %s waypoint_equals_seed\n", file); g_fail++; }
    JVal *prof = jget(root, "profile");
    uint64_t slot_b = (uint64_t)jnum(prof, "slot_bytes");
    uint64_t terr_b = (uint64_t)jnum(prof, "terrain_bytes");
    uint64_t sample_b = (uint64_t)jnum(prof, "sample_bytes");
    uint64_t steps = (uint64_t)jnum(prof, "steps_per_leg");
    uint8_t *terrain = fill_terrain(word_nl4, P, slot_b, terr_b);
    uint8_t th[32];
    sha256(terrain, terr_b, th);
    hex_of(th, 32, hx);
    expect_str("terrain", hx, jstr(root, "terrain_sha256"));
    uint8_t W[32];
    memcpy(W, d, 32);
    JVal *stages = jget(root, "stages");
    for (int g = 0; g < stages->an; g++) {
        JVal *st = stages->a[g];
        int nlegs = (int)jnum(st, "leg_count");
        uint8_t prev[32], limb[32];
        memcpy(limb, W, 32);
        memcpy(prev, W, 32);
        uint8_t cps[64][32];
        JVal *legs = jget(st, "legs_le_hex");
        JVal *cks = jget(st, "checkpoints_hex");
        if (legs->an != nlegs || cks->an != nlegs) die("leg count");
        for (int j = 0; j < nlegs; j++) {
            uint64_t r = do_leg(terrain, terr_b, slot_b, steps, load_le(prev));
            uint8_t le[8];
            store_le(le, r);
            hex_of(le, 8, hx);
            expect_str("leg", hx, legs->a[j]->s);
            uint8_t *c = checkpoint(terrain, terr_b, sample_b, prev, limb, r);
            hex_of(c, 32, hx);
            expect_str("checkpoint", hx, cks->a[j]->s);
            memcpy(cps[j], c, 32);
            memcpy(prev, c, 32);
            free(c);
        }
        int is_null = 0;
        const char *nbx = jstr_or_null(st, "next_beacon_hex", &is_null);
        const char *nw = jstr_or_null(st, "next_waypoint_hex", &is_null);
        if (!nbx) {
            if (nw) { fprintf(stderr, "FAIL %s stage %d waypoint present\n", file, g); g_fail++; }
        } else {
            size_t nbl;
            uint8_t *B = unhex(nbx, &nbl);
            uint8_t *o = wayp(terrain, slot_b, terr_b / slot_b, B, limb, cps, nlegs);
            hex_of(o, 32, hx);
            expect_str("waypoint", hx, nw);
            memcpy(W, o, 32);
            free(B);
            free(o);
        }
    }
    if (g_fail == before_hike) pass(jstr(root, "id"));
    free(terrain); free(pk); free(be); free(path);
}

/* §15.5. Returns 0, or -1 when the byte source ends early.
 * out holds up to want rounds of 6. *nrounds is how many rounds filled. */
static int draw_rounds(long long leg_total, const uint64_t *draws, int ndraws,
                       int want, int out[][6], int *nrounds) {
    *nrounds = 0;
    if (leg_total < 6 || want <= 0) return 0;
    unsigned __int128 rem = ((unsigned __int128)1 << 64) % (unsigned __int128)leg_total;
    uint64_t limit_lo = rem == 0 ? 0 : (uint64_t)(0 - (uint64_t)rem);
    int phase[64];
    int np = 0;
    int ui = 0;
    for (int r = 0; r < want; r++) {
        int nr = 0;
        int guard = 0;
        while (nr < 6) {
            if (ui >= ndraws || guard++ > 10000) return -1;
            uint64_t u = draws[ui++];
            if (rem != 0 && u >= limit_lo) continue;
            int q = (int)(u % (uint64_t)leg_total);
            int in_round = 0;
            for (int k = 0; k < nr; k++) if (out[r][k] == q) in_round = 1;
            if (in_round) continue;
            int in_phase = 0;
            for (int k = 0; k < np; k++) if (phase[k] == q) in_phase = 1;
            int unused = 0;
            if (in_phase) {
                for (long long x = 0; x < leg_total; x++) {
                    int seen = 0;
                    for (int k = 0; k < np; k++) if (phase[k] == (int)x) seen = 1;
                    if (!seen) { unused = 1; break; }
                }
            }
            if (in_phase && unused) continue;
            if (np >= 64) return -1;
            out[r][nr++] = q;
            phase[np++] = q;
        }
        *nrounds = r + 1;
    }
    return 0;
}

/* BIP113: sort n times, take index n/2. Returns 0, or -1 when a time is missing. */
static int mtp_value(const JVal *times, long long *out) {
    int n = times->an;
    if (n <= 0 || n > 11) return -1;
    long long s[11];
    for (int i = 0; i < n; i++) {
        if (times->a[i]->k != JNUM) return -1;
        s[i] = times->a[i]->n;
    }
    for (int i = 1; i < n; i++) {
        long long v = s[i];
        int j = i;
        while (j > 0 && s[j - 1] > v) { s[j] = s[j - 1]; j--; }
        s[j] = v;
    }
    *out = s[n / 2];
    return 0;
}

/* 0 = read, 1 = a time is missing, -1 = the window does not match the rule. */
static int check_window(const JVal *w, long long *mtp, long long *raw) {
    long long height = jnum(w, "height");
    long long n = jnum(w, "n");
    long long expect_n = height + 1 < 11 ? height + 1 : 11;
    if (n != expect_n) return -1;
    JVal *reads = jget(w, "reads");
    JVal *times = jget(w, "times");
    JVal *stated = jget(w, "mtp");
    if (!reads || !times || !stated || reads->an != n || times->an != n) return -1;
    for (int i = 0; i < (int)n; i++) {
        if (reads->a[i]->k != JNUM || reads->a[i]->n != height - n + 1 + i) return -1;
    }
    for (int i = 0; i < (int)n; i++) if (times->a[i]->k != JNUM) {
        if (stated->k != JNULL) return -1;
        return 1;
    }
    if (mtp_value(times, mtp) != 0 || stated->k != JNUM || stated->n != *mtp) return -1;
    *raw = times->a[n - 1]->n;
    if (jget(w, "raw_ntime")->k != JNUM || jget(w, "raw_ntime")->n != *raw) return -1;
    return 0;
}

static int before;

static void section(const char *id) {
    if (g_fail == before) pass(id);
    before = g_fail;
}

int main(int argc, char **argv) {
    const char *dir = argc > 1 ? argv[1] : "spec/conformance/v2.1";
    char hx[80];

    /* seed */
    {
        char *path = path_join(dir, "seed.json");
        JVal *o = load_json(path);
        size_t np, nb;
        uint8_t *pk = unhex(jstr(o, "public_key_hex"), &np);
        uint8_t *be = unhex(jstr(o, "seed_beacon_hex"), &nb);
        uint8_t d[32]; uint64_t P[4];
        seed_of(pk, be, d, P);
        hex_of(d, 32, hx);
        before = g_fail;
        expect_str("seed", hx, jstr(o, "seed_hex"));
        JVal *lanes = jget(o, "lanes_u64_hex");
        for (int k = 0; k < 4; k++) {
            char got[17];
            snprintf(got, sizeof got, "%016llx", (unsigned long long)P[k]);
            expect_str("lane", got, lanes->a[k]->s);
        }
        if (jnum(o, "waypoint_equals_seed") != 1) { fprintf(stderr, "FAIL waypoint_equals_seed\n"); g_fail++; }
        size_t npre;
        uint8_t *pre = unhex(jstr(o, "seed_preimage_hex"), &npre);
        if (npre != 64 || memcmp(pre, pk, 32) || memcmp(pre + 32, be, 32)) {
            fprintf(stderr, "FAIL seed preimage\n"); g_fail++;
        }
        free(pre); free(pk); free(be); free(path);
        section("seed");
    }

    /* NL4 words and slot-0 hash */
    {
        char *path = path_join(dir, "nl4-words.json");
        JVal *o = load_json(path);
        size_t np, nb;
        uint8_t *pk = unhex(jstr(o, "public_key_hex"), &np);
        uint8_t *be = unhex(jstr(o, "seed_beacon_hex"), &nb);
        uint8_t d[32]; uint64_t P[4];
        seed_of(pk, be, d, P);
        before = g_fail;
        JVal *samples = jget(o, "samples");
        for (int i = 0; i < samples->an; i++) {
            JVal *s = samples->a[i];
            uint64_t w = nl4_word(P, (uint64_t)jnum(s, "i"), (uint64_t)jnum(s, "j"));
            char got[17];
            snprintf(got, sizeof got, "%016llx", (unsigned long long)w);
            expect_str("nl4", got, jstr(s, "u64_hex"));
        }
        uint64_t nwords = (uint64_t)jnum(o, "slot0_words");
        uint8_t *slot = malloc(nwords * 8);
        slot0_base(P, nwords, slot);
        uint8_t dig[32];
        sha256(slot, nwords * 8, dig);
        hex_of(dig, 32, hx);
        expect_str("slot0", hx, jstr(o, "slot0_sha256"));
        free(slot); free(pk); free(be); free(path);
        section("nl4-words");
    }

    /* small terrains */
    {
        char *path = path_join(dir, "terrain-small.json");
        JVal *o = load_json(path);
        size_t np, nb;
        uint8_t *pk = unhex(jstr(o, "public_key_hex"), &np);
        uint8_t *be = unhex(jstr(o, "seed_beacon_hex"), &nb);
        uint8_t d[32]; uint64_t P[4];
        seed_of(pk, be, d, P);
        before = g_fail;
        JVal *cases = jget(o, "cases");
        for (int i = 0; i < cases->an; i++) {
            JVal *c = cases->a[i];
            uint64_t sb = (uint64_t)jnum(c, "slot_bytes");
            uint64_t tb = (uint64_t)jnum(c, "terrain_bytes");
            uint8_t *t = fill_terrain(word_nl4, P, sb, tb);
            uint8_t dig[32];
            sha256(t, tb, dig);
            hex_of(dig, 32, hx);
            expect_str("terrain-small", hx, jstr(c, "sha256"));
            free(t);
        }
        free(pk); free(be); free(path);
        section("terrain-small");
    }

    /* one step and seal(0) */
    {
        char *path = path_join(dir, "step-c1.json");
        JVal *o = load_json(path);
        before = g_fail;
        uint8_t buf[20], dig[32];
        memcpy(buf, SEAL, 12);
        store_le(buf + 12, 0);
        sha256(buf, 20, dig);
        hex_of(dig, 32, hx);
        expect_str("seal0", hx, jstr(o, "seal0_digest_hex"));
        char sp[17];
        snprintf(sp, sizeof sp, "%016llx", (unsigned long long)load_le(dig));
        expect_str("seal0-s", sp, jstr(o, "seal0_s_prime_u64_hex"));
        size_t np, nb;
        uint8_t *pk = unhex(jstr(o, "public_key_hex"), &np);
        uint8_t *be = unhex(jstr(o, "seed_beacon_hex"), &nb);
        uint8_t sd[32]; uint64_t P[4];
        seed_of(pk, be, sd, P);
        uint64_t nwords = (uint64_t)jnum(o, "words");
        uint8_t *slot = malloc(nwords * 8);
        slot0_base(P, nwords, slot);
        JVal *steps = jget(o, "steps");
        for (int i = 0; i < steps->an; i++) {
            JVal *s = steps->a[i];
            uint64_t S = strtoull(jstr(s, "S_u64_hex"), NULL, 16);
            uint64_t folded = fold_mix(slot, nwords, S, 0, nwords);
            snprintf(sp, sizeof sp, "%016llx", (unsigned long long)folded);
            expect_str("S_folded", sp, jstr(s, "S_folded_u64_hex"));
            uint64_t spr = seal(folded);
            snprintf(sp, sizeof sp, "%016llx", (unsigned long long)spr);
            expect_str("S_prime", sp, jstr(s, "S_prime_u64_hex"));
        }
        free(slot); free(pk); free(be); free(path);
        section("step-c1");
    }

    check_hike(dir, "kat-small.json");
    check_hike(dir, "test-3leg.json");

    /* rate */
    {
        char *path = path_join(dir, "rate.json");
        JVal *o = load_json(path);
        long long p = jnum(jget(o, "floor"), "p");
        long long q = jnum(jget(o, "floor"), "q");
        const char *mark = jstr(jget(o, "floor"), "mark");
        before = g_fail;
        if (strcmp(mark, "placeholder, not ratified") != 0) {
            fprintf(stderr, "FAIL floor mark\n"); g_fail++;
        }
        JVal *cases = jget(o, "cases");
        for (int i = 0; i < cases->an; i++) {
            JVal *c = cases->a[i];
            long long legs = jnum(c, "hike_legs");
            long long span = jnum(c, "hike_span");
            int meets = span > 0 && q * legs >= p * span;
            if (span <= 0) {
                if (meets || strcmp(jstr(c, "outcome"), "refuse") != 0) {
                    fprintf(stderr, "FAIL rate %s span<=0\n", jstr(c, "name")); g_fail++;
                }
                reject(jstr(c, "name"));
                continue;
            }
            if (meets != jbool(c, "meets")) { fprintf(stderr, "FAIL rate %s\n", jstr(c, "name")); g_fail++; }
            if (jget(c, "must_reject") && jbool(c, "must_reject")) {
                if (meets) { fprintf(stderr, "FAIL rate negative met the floor\n"); g_fail++; }
                else reject(jstr(c, "name"));
            } else if (meets) {
                pass(jstr(c, "name"));
            } else {
                fprintf(stderr, "FAIL rate positive missed\n"); g_fail++;
            }
        }
        free(path);
        section("rate");
    }

    /* thorough rounds, §15.5 */
    {
        char *path = path_join(dir, "rounds.json");
        JVal *o = load_json(path);
        before = g_fail;
        if (jnum(o, "thorough_rounds") != 3 || jnum(o, "per_round") != 6) {
            fprintf(stderr, "FAIL rounds shape\n"); g_fail++;
        }
        JVal *cases = jget(o, "cases");
        for (int i = 0; i < cases->an; i++) {
            JVal *c = cases->a[i];
            const char *name = jstr(c, "name");
            long long leg_total = jnum(c, "leg_total");
            int started = (int)jnum(c, "rounds_started");
            JVal *published = jget(c, "rounds");
            int exhausted = jget(c, "byte_source") && strcmp(jstr(c, "byte_source"), "exhausted") == 0;
            const char *expect_out = "recorded";
            if (leg_total < 6 || jget(c, "mismatch_on_round")) expect_out = "refuse";
            if (exhausted) expect_out = "not read";
            if (strcmp(jstr(c, "outcome"), expect_out) != 0) {
                fprintf(stderr, "FAIL %s outcome\n", name); g_fail++;
            }
            if (exhausted) {
                if (jget(c, "must_reject")) { fprintf(stderr, "FAIL %s must_reject\n", name); g_fail++; continue; }
                JVal *hexes = jget(c, "draws_u64_hex");
                uint64_t draws[64];
                if (!hexes || hexes->an > 64) { fprintf(stderr, "FAIL %s draws\n", name); g_fail++; continue; }
                for (int k = 0; k < hexes->an; k++) draws[k] = strtoull(hexes->a[k]->s, NULL, 16);
                int got[4][6];
                int nrounds = 0;
                int extra = 0;
                if (draw_rounds(leg_total, draws, hexes->an, started, got, &nrounds) != 0
                    || nrounds != started
                    || draw_rounds(leg_total, draws, hexes->an, started + 1, got, &extra) == 0
                    || published->an != started) {
                    fprintf(stderr, "FAIL %s exhausted\n", name); g_fail++; continue;
                }
                int ok_rows = 1;
                for (int r = 0; r < started; r++) {
                    JVal *row = published->a[r];
                    if (row->an != 6) ok_rows = 0;
                    for (int k = 0; ok_rows && k < 6; k++)
                        if (row->a[k]->k != JNUM || row->a[k]->n != got[r][k]) ok_rows = 0;
                }
                if (!ok_rows) { fprintf(stderr, "FAIL %s rows\n", name); g_fail++; continue; }
                pass(name);
                continue;
            }
            if (leg_total < 6) {
                if (strcmp(jstr(c, "draw"), "not defined") != 0 || started != 0 || published->an != 0) {
                    fprintf(stderr, "FAIL %s below six\n", name); g_fail++;
                } else reject(name);
                continue;
            }
            JVal *hexes = jget(c, "draws_u64_hex");
            uint64_t draws[64];
            if (!hexes || hexes->an > 64) { fprintf(stderr, "FAIL %s draws\n", name); g_fail++; continue; }
            for (int k = 0; k < hexes->an; k++) draws[k] = strtoull(hexes->a[k]->s, NULL, 16);
            int want = jget(c, "mismatch_on_round") ? (int)jnum(c, "mismatch_on_round") : 3;
            int got[3][6];
            int nrounds = 0;
            if (draw_rounds(leg_total, draws, hexes->an, want, got, &nrounds) != 0 || nrounds != want || started != want) {
                fprintf(stderr, "FAIL %s draw\n", name); g_fail++; continue;
            }
            if (published->an != want) { fprintf(stderr, "FAIL %s round count\n", name); g_fail++; continue; }
            int distinct_ok = 1;
            int seen[64];
            int nseen = 0;
            for (int r = 0; r < want; r++) {
                JVal *row = published->a[r];
                if (row->an != 6) { distinct_ok = 0; break; }
                for (int k = 0; k < 6; k++) {
                    if (row->a[k]->k != JNUM || row->a[k]->n != got[r][k]) distinct_ok = 0;
                    for (int t = 0; t < k; t++) if (got[r][t] == got[r][k]) distinct_ok = 0;
                    if (leg_total >= 18 && !jget(c, "mismatch_on_round")) {
                        for (int t = 0; t < nseen; t++) if (seen[t] == got[r][k]) distinct_ok = 0;
                        if (nseen < 64) seen[nseen++] = got[r][k];
                    }
                }
            }
            if (!distinct_ok) { fprintf(stderr, "FAIL %s indices\n", name); g_fail++; continue; }
            if (jget(c, "must_reject") && jbool(c, "must_reject")) reject(name);
            else pass(name);
        }
        free(path);
        section("rounds");
    }

    /* anchors */
    {
        char *path = path_join(dir, "anchors.json");
        JVal *o = load_json(path);
        long long m = jnum(o, "minimum");
        before = g_fail;
        if (m != 6) { fprintf(stderr, "FAIL anchor minimum\n"); g_fail++; }
        JVal *cases = jget(o, "cases");
        for (int i = 0; i < cases->an; i++) {
            JVal *c = cases->a[i];
            JVal *ix = jget(c, "indices");
            int meets = ix->an >= m;
            if (meets != jbool(c, "meets")) { fprintf(stderr, "FAIL anchors %s\n", jstr(c, "name")); g_fail++; }
            if (jget(c, "must_reject") && jbool(c, "must_reject")) {
                if (meets) { fprintf(stderr, "FAIL five anchors met\n"); g_fail++; }
                else reject(jstr(c, "name"));
            } else pass(jstr(c, "name"));
        }
        free(path);
        section("anchors");
    }

    /* negatives */
    {
        char *path = path_join(dir, "negatives.json");
        JVal *o = load_json(path);
        size_t np, nb;
        uint8_t *pk = unhex(jstr(o, "public_key_hex"), &np);
        uint8_t *be = unhex(jstr(o, "seed_beacon_hex"), &nb);
        uint8_t d[32]; uint64_t P[4];
        seed_of(pk, be, d, P);
        uint8_t *slot = malloc(2048 * 8);
        slot0_base(P, 2048, slot);
        before = g_fail;
        JVal *cases = jget(o, "cases");
        for (int i = 0; i < cases->an; i++) {
            JVal *c = cases->a[i];
            const char *name = jstr(c, "name");
            if (!jbool(c, "must_reject")) { fprintf(stderr, "FAIL %s not must_reject\n", name); g_fail++; continue; }
            if (strcmp(name, "fill_pin_terrain") == 0) {
                uint8_t pre[64];
                memcpy(pre, pk, 32); memcpy(pre + 32, be, 32);
                uint64_t pin = fill_pin(pre);
                uint64_t sb = (uint64_t)jnum(c, "slot_bytes"), tb = (uint64_t)jnum(c, "terrain_bytes");
                uint8_t *bad = fill_terrain(word_pin, &pin, sb, tb);
                uint8_t *good = fill_terrain(word_nl4, P, sb, tb);
                uint8_t db[32], dg[32];
                sha256(bad, tb, db); sha256(good, tb, dg);
                hex_of(db, 32, hx); expect_str("fill_pin", hx, jstr(c, "forbidden_sha256"));
                hex_of(dg, 32, hx); expect_str("nl4-terrain", hx, jstr(c, "conforming_sha256"));
                if (memcmp(db, dg, 32) == 0) { fprintf(stderr, "FAIL fill_pin did not differ\n"); g_fail++; }
                else reject(name);
                free(bad); free(good);
            } else if (strcmp(name, "ref_word") == 0) {
                char got[17];
                snprintf(got, sizeof got, "%016llx", (unsigned long long)ref_word(P, (uint64_t)jnum(c, "i"), (uint64_t)jnum(c, "j")));
                expect_str("ref", got, jstr(c, "forbidden_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)nl4_word(P, 0, 0));
                expect_str("nl4-vs-ref", got, jstr(c, "conforming_u64_hex"));
                if (strcmp(jstr(c, "forbidden_u64_hex"), jstr(c, "conforming_u64_hex")) == 0) g_fail++;
                else reject(name);
            } else if (strcmp(name, "all4_word") == 0) {
                char got[17];
                snprintf(got, sizeof got, "%016llx", (unsigned long long)all4_word(P, (uint64_t)jnum(c, "i"), (uint64_t)jnum(c, "j")));
                expect_str("all4", got, jstr(c, "forbidden_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)nl4_word(P, 0, 0));
                expect_str("nl4-vs-all4", got, jstr(c, "conforming_u64_hex"));
                if (strcmp(jstr(c, "forbidden_u64_hex"), jstr(c, "conforming_u64_hex")) == 0) g_fail++;
                else reject(name);
            } else if (strcmp(name, "mix64_without_mix_add") == 0) {
                uint64_t S = strtoull(jstr(c, "S_u64_hex"), NULL, 16);
                uint64_t bad = fold_mix(slot, 2048, S, 1, 2048);
                uint64_t good = fold_mix(slot, 2048, S, 0, 2048);
                char got[17];
                snprintf(got, sizeof got, "%016llx", (unsigned long long)bad);
                expect_str("bare", got, jstr(c, "forbidden_S_folded_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)good);
                expect_str("c1", got, jstr(c, "conforming_S_folded_u64_hex"));
                if (bad == good) g_fail++; else reject(name);
            } else if (strcmp(name, "v2_0_fold") == 0) {
                uint64_t S = strtoull(jstr(c, "S_u64_hex"), NULL, 16);
                uint64_t bad = fold_v2_0(slot, 2048, S);
                uint64_t good = fold_mix(slot, 2048, S, 0, 2048);
                char got[17];
                snprintf(got, sizeof got, "%016llx", (unsigned long long)bad);
                expect_str("v2_0", got, jstr(c, "forbidden_S_folded_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)good);
                expect_str("c1-v2_0", got, jstr(c, "conforming_S_folded_u64_hex"));
                if (bad == good) g_fail++; else reject(name);
            } else if (strcmp(name, "xor_accumulator_fold") == 0) {
                uint64_t S = strtoull(jstr(c, "S_u64_hex"), NULL, 16);
                uint64_t bad = fold_xor_acc(slot, 2048, S);
                uint64_t good = fold_mix(slot, 2048, S, 0, 2048);
                char got[17];
                snprintf(got, sizeof got, "%016llx", (unsigned long long)bad);
                expect_str("xor-acc", got, jstr(c, "forbidden_S_folded_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)good);
                expect_str("add-acc", got, jstr(c, "conforming_S_folded_u64_hex"));
                if (bad == good) g_fail++; else reject(name);
            } else if (strcmp(name, "xor_w_fold") == 0) {
                uint64_t S = strtoull(jstr(c, "S_u64_hex"), NULL, 16);
                uint64_t bad = fold_xor_w(slot, 2048, S);
                uint64_t good = fold_mix(slot, 2048, S, 0, 2048);
                char got[17];
                snprintf(got, sizeof got, "%016llx", (unsigned long long)bad);
                expect_str("xor-w", got, jstr(c, "forbidden_S_folded_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)good);
                expect_str("word-xor-s", got, jstr(c, "conforming_S_folded_u64_hex"));
                if (bad == good) g_fail++; else reject(name);
            } else if (strcmp(name, "s_plus_w_fold") == 0) {
                uint64_t S = strtoull(jstr(c, "S_u64_hex"), NULL, 16);
                uint64_t bad = fold_s_plus_w(slot, 2048, S);
                uint64_t good = fold_mix(slot, 2048, S, 0, 2048);
                char got[17];
                snprintf(got, sizeof got, "%016llx", (unsigned long long)bad);
                expect_str("s-plus-w", got, jstr(c, "forbidden_S_folded_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)good);
                expect_str("word-xor-s", got, jstr(c, "conforming_S_folded_u64_hex"));
                if (bad == good) g_fail++; else reject(name);
            } else if (strcmp(name, "prefold_hoist") == 0) {
                uint64_t S = strtoull(jstr(c, "S_u64_hex"), NULL, 16);
                uint64_t words = (uint64_t)jnum(c, "words");
                uint64_t steps = (uint64_t)jnum(c, "steps");
                if (words != 2048 || steps == 0 || strcmp(jstr(c, "transform"), "word XOR w") != 0) {
                    fprintf(stderr, "FAIL %s shape\n", name); g_fail++; continue;
                }
                uint8_t *hoist = malloc(words * 8);
                if (!hoist) { fprintf(stderr, "FAIL %s alloc\n", name); g_fail++; continue; }
                memcpy(hoist, slot, words * 8);
                for (uint64_t w = 0; w < words; w++) store_le(hoist + 8 * w, load_le(hoist + 8 * w) ^ w);
                uint64_t bad_f = fold_mix(hoist, words, S, 0, words);
                uint64_t good_f = fold_mix(slot, words, S, 0, words);
                uint64_t bad_p = seal(bad_f), good_p = seal(good_f);
                uint64_t bad_leg = S, good_leg = S;
                for (uint64_t t = 0; t < steps; t++) {
                    bad_leg = seal(fold_mix(hoist, words, bad_leg, 0, words));
                    good_leg = seal(fold_mix(slot, words, good_leg, 0, words));
                }
                char got[17];
                snprintf(got, sizeof got, "%016llx", (unsigned long long)bad_f);
                expect_str("hoist-fold", got, jstr(c, "forbidden_S_folded_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)good_f);
                expect_str("real-fold", got, jstr(c, "conforming_S_folded_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)bad_p);
                expect_str("hoist-step", got, jstr(c, "forbidden_S_prime_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)good_p);
                expect_str("real-step", got, jstr(c, "conforming_S_prime_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)bad_leg);
                expect_str("hoist-leg", got, jstr(c, "forbidden_leg_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)good_leg);
                expect_str("real-leg", got, jstr(c, "conforming_leg_u64_hex"));
                if (bad_f == good_f || bad_p == good_p || bad_leg == good_leg) g_fail++;
                else reject(name);
                free(hoist);
            } else if (strcmp(name, "partial_slot_read") == 0 || strcmp(name, "partial_slot_read_2047") == 0) {
                uint64_t S = strtoull(jstr(c, "S_u64_hex"), NULL, 16);
                uint64_t nwords = (uint64_t)jnum(c, "words");
                uint64_t nread = (uint64_t)jnum(c, "words_read");
                uint64_t bad = fold_mix(slot, nwords, S, 0, nread);
                uint64_t good = fold_mix(slot, nwords, S, 0, nwords);
                char got[17];
                snprintf(got, sizeof got, "%016llx", (unsigned long long)bad);
                expect_str("partial", got, jstr(c, "forbidden_S_folded_u64_hex"));
                snprintf(got, sizeof got, "%016llx", (unsigned long long)good);
                expect_str("full", got, jstr(c, "conforming_S_folded_u64_hex"));
                if (bad == good || nread >= nwords) g_fail++; else reject(name);
            } else if (strcmp(name, "one_lane_flip") == 0) {
                uint64_t Q[4];
                memcpy(Q, P, sizeof Q);
                Q[jnum(c, "lane")] ^= strtoull(jstr(c, "flip_u64_hex"), NULL, 16);
                uint64_t sb = (uint64_t)jnum(c, "slot_bytes"), tb = (uint64_t)jnum(c, "terrain_bytes");
                uint64_t words = sb / 8, nslots = tb / sb;
                long long nl4c = 0, refc = 0, pairc = 0, total = 0;
                for (uint64_t si = 0; si < nslots; si++) {
                    for (uint64_t j = 0; j < words; j++) {
                        total++;
                        if (nl4_word(P, si, j) != nl4_word(Q, si, j)) nl4c++;
                        if (ref_word(P, si, j) != ref_word(Q, si, j)) refc++;
                        if (pair_word(P, si, j) != pair_word(Q, si, j)) pairc++;
                    }
                }
                if (total != jnum(c, "words") || nl4c != jnum(c, "nl4_changed")
                    || refc != jnum(c, "ref_changed") || pairc != jnum(c, "pair_changed")) {
                    fprintf(stderr, "FAIL %s counts %lld %lld %lld\n", name, nl4c, refc, pairc);
                    g_fail++;
                }
                uint8_t *a = fill_terrain(word_nl4, P, sb, tb);
                uint8_t *b = fill_terrain(word_nl4, Q, sb, tb);
                uint8_t da[32], db[32];
                sha256(a, tb, da); sha256(b, tb, db);
                hex_of(da, 32, hx); expect_str("flip-base", hx, jstr(c, "conforming_sha256"));
                hex_of(db, 32, hx); expect_str("flip-nl4", hx, jstr(c, "forbidden_sha256"));
                if (memcmp(da, db, 32) == 0 || nl4c != total) g_fail++;
                else reject(name);
                free(a); free(b);
            } else if (strcmp(name, "all4_kernel") == 0) {
                uint64_t Q[4];
                memcpy(Q, P, sizeof Q);
                uint64_t flip = strtoull(jstr(c, "xor_u64_hex"), NULL, 16);
                JVal *lanes = jget(c, "lanes");
                for (int k = 0; k < lanes->an; k++) Q[lanes->a[k]->n] ^= flip;
                uint64_t sb = (uint64_t)jnum(c, "slot_bytes"), tb = (uint64_t)jnum(c, "terrain_bytes");
                uint64_t words = sb / 8, nslots = tb / sb;
                long long nl4c = 0, total = 0;
                for (uint64_t si = 0; si < nslots; si++)
                    for (uint64_t j = 0; j < words; j++) {
                        total++;
                        if (nl4_word(P, si, j) != nl4_word(Q, si, j)) nl4c++;
                    }
                if (nl4c != jnum(c, "nl4_changed") || total != jnum(c, "words")) {
                    fprintf(stderr, "FAIL %s nl4 count\n", name); g_fail++;
                }
                uint8_t *aa = fill_terrain(word_all4, P, sb, tb);
                uint8_t *ab = fill_terrain(word_all4, Q, sb, tb);
                uint8_t *nb = fill_terrain(word_nl4, Q, sb, tb);
                uint8_t *na = fill_terrain(word_nl4, P, sb, tb);
                uint8_t da[32], db[32], dn[32], dbase[32];
                sha256(aa, tb, da); sha256(ab, tb, db); sha256(nb, tb, dn); sha256(na, tb, dbase);
                hex_of(da, 32, hx); expect_str("all4", hx, jstr(c, "all4_sha256"));
                hex_of(db, 32, hx); expect_str("all4-flip", hx, jstr(c, "all4_sha256_flipped"));
                hex_of(dbase, 32, hx); expect_str("all4-base", hx, jstr(c, "conforming_sha256"));
                hex_of(dn, 32, hx); expect_str("all4-nl4", hx, jstr(c, "forbidden_sha256"));
                if (memcmp(da, db, 32) != 0 || memcmp(dbase, dn, 32) == 0) g_fail++;
                else reject(name);
                free(aa); free(ab); free(na); free(nb);
            } else if (strcmp(name, "forged_window") == 0) {
                if (jbool(c, "fake_leg_ignore")) { fprintf(stderr, "FAIL forged window is a ban\n"); g_fail++; }
                if (strcmp(jstr(c, "reason"), "A4.3 mismatch") != 0) { fprintf(stderr, "FAIL forged reason\n"); g_fail++; }
                if (strcmp(jstr(c, "forbidden_window"), "shifted-one") != 0) die("forged window rule");
                uint64_t sb = (uint64_t)jnum(c, "slot_bytes"), tb = (uint64_t)jnum(c, "terrain_bytes");
                uint64_t sample = (uint64_t)jnum(c, "sample_bytes");
                uint64_t leg = strtoull(jstr(c, "leg_u64_hex"), NULL, 16);
                uint8_t *terrain = fill_terrain(word_nl4, P, sb, tb);
                uint8_t *good = checkpoint(terrain, tb, sample, d, d, leg);
                uint64_t offset = load_le(d) % (tb - sample + 1);
                if ((uint64_t)jnum(c, "sample_offset") != offset) {
                    fprintf(stderr, "FAIL forged offset\n"); g_fail++;
                }
                if (offset + 1 + sample > tb) die("forged shift");
                size_t npre = 64 + sample + 8;
                uint8_t *pre = malloc(npre);
                memcpy(pre, d, 32);
                memcpy(pre + 32, d, 32);
                memcpy(pre + 64, terrain + offset + 1, sample);
                store_le(pre + 64 + sample, leg);
                uint8_t bad[32];
                sha256(pre, npre, bad);
                hex_of(good, 32, hx); expect_str("forged-good", hx, jstr(c, "conforming_hex"));
                hex_of(bad, 32, hx); expect_str("forged-bad", hx, jstr(c, "forbidden_hex"));
                if (memcmp(good, bad, 32) == 0) g_fail++; else reject(name);
                free(pre); free(good); free(terrain);
            } else if (strcmp(name, "f3_shared_terrain") == 0) {
                size_t n2;
                uint8_t *pk2 = unhex(jstr(c, "pk2_hex"), &n2);
                uint8_t pre1[64], pre2[64];
                memcpy(pre1, pk, 32); memcpy(pre1 + 32, be, 32);
                memcpy(pre2, pk2, 32); memcpy(pre2 + 32, be, 32);
                uint64_t pin1 = fill_pin(pre1), pin2 = fill_pin(pre2);
                uint64_t sb = (uint64_t)jnum(c, "slot_bytes"), tb = (uint64_t)jnum(c, "terrain_bytes");
                uint8_t *a = fill_terrain(word_pin, &pin1, sb, tb);
                uint8_t *b = fill_terrain(word_pin, &pin2, sb, tb);
                uint8_t da[32], db[32];
                sha256(a, tb, da); sha256(b, tb, db);
                hex_of(da, 32, hx); expect_str("f3-pin1", hx, jstr(c, "forbidden_sha256_pk1"));
                hex_of(db, 32, hx); expect_str("f3-pin2", hx, jstr(c, "forbidden_sha256_pk2"));
                if (memcmp(da, db, 32) != 0) { fprintf(stderr, "FAIL fill_pin pair did not collide\n"); g_fail++; }
                uint8_t d2[32]; uint64_t P2[4];
                seed_of(pk2, be, d2, P2);
                uint8_t *nA = fill_terrain(word_nl4, P, sb, tb);
                uint8_t *nB = fill_terrain(word_nl4, P2, sb, tb);
                uint8_t na[32], nbh[32];
                sha256(nA, tb, na); sha256(nB, tb, nbh);
                hex_of(na, 32, hx); expect_str("f3-nl4-1", hx, jstr(c, "conforming_sha256_pk1"));
                hex_of(nbh, 32, hx); expect_str("f3-nl4-2", hx, jstr(c, "conforming_sha256_pk2"));
                if (memcmp(na, nbh, 32) == 0) { fprintf(stderr, "FAIL NL4 pair shared terrain\n"); g_fail++; }
                else reject(name);
                free(a); free(b); free(nA); free(nB); free(pk2);
            } else {
                fprintf(stderr, "FAIL unknown negative %s\n", name); g_fail++;
            }
        }
        free(slot); free(pk); free(be); free(path);
        section("negatives");
    }

    /* kind text */
    {
        char *path = path_join(dir, "kind.json");
        JVal *o = load_json(path);
        before = g_fail;
        const char *k = jstr(o, "kind");
        if (strcmp(k, "satoshi-hike-v2.1-object") != 0 || strlen(k) != (size_t)jnum(o, "kind_bytes") || strlen(k) != 24) {
            fprintf(stderr, "FAIL kind text\n"); g_fail++;
        }
        JVal *earlier = jget(o, "earlier");
        if (!earlier || earlier->an != 2
            || strcmp(earlier->a[0]->s, "satoshi-hike-v2.0-object") != 0
            || strcmp(earlier->a[1]->s, "satoshi-hike-v1.35-object") != 0
            || strcmp(jstr(o, "earlier_reason"), "earlier_version") != 0) {
            fprintf(stderr, "FAIL earlier kind\n"); g_fail++;
        }
        free(path);
        section("kind");
    }

    /* median-time-past, Rule A4.10 */
    {
        char *path = path_join(dir, "mtp.json");
        JVal *o = load_json(path);
        before = g_fail;
        JVal *cases = jget(o, "cases");
        for (int i = 0; i < cases->an; i++) {
            JVal *c = cases->a[i];
            const char *name = jstr(c, "name");
            const char *outcome = jstr(c, "outcome");
            long long m0 = 0, r0 = 0, m1 = 0, r1 = 0;
            int a = check_window(jget(c, "first"), &m0, &r0);
            if (a < 0) { fprintf(stderr, "FAIL %s first window\n", name); g_fail++; continue; }
            JVal *last = jget(c, "last");
            if (!last) {
                if (a != 0 || strcmp(outcome, "read") != 0) {
                    fprintf(stderr, "FAIL %s short window\n", name); g_fail++;
                } else pass(name);
                continue;
            }
            int b = check_window(last, &m1, &r1);
            if (b < 0) { fprintf(stderr, "FAIL %s last window\n", name); g_fail++; continue; }
            if (a == 1 || b == 1) {
                int hs_null = 0, raw_null = 0;
                const char *hs = jstr_or_null(c, "hike_span", &hs_null);
                const char *raws = jstr_or_null(c, "raw_span", &raw_null);
                if (hs_null || raw_null || !hs || !raws
                    || strcmp(hs, "not read") != 0 || strcmp(raws, "not read") != 0
                    || strcmp(outcome, "held") != 0) {
                    fprintf(stderr, "FAIL %s held\n", name); g_fail++;
                } else pass(name);
                continue;
            }
            long long span = m1 - m0;
            long long raw = r1 - r0;
            if (jnum(c, "hike_span") != span || jnum(c, "raw_span") != raw) {
                fprintf(stderr, "FAIL %s span\n", name); g_fail++; continue;
            }
            if (strcmp(outcome, "refuse") == 0) {
                if (span > 0 || !jbool(c, "must_reject")) {
                    fprintf(stderr, "FAIL %s refuse\n", name); g_fail++;
                } else reject(name);
            } else if (strcmp(outcome, "read") == 0) {
                if (span <= 0 || raw == span) {
                    fprintf(stderr, "FAIL %s read\n", name); g_fail++;
                } else pass(name);
            } else {
                fprintf(stderr, "FAIL %s outcome\n", name); g_fail++;
            }
        }
        free(path);
        section("mtp");
    }

    /* submission deadline: tip_height - last_beacon_height > K refuses */
    {
        char *path = path_join(dir, "deadline.json");
        JVal *o = load_json(path);
        before = g_fail;
        if (jnum(o, "K") != 3 || strcmp(jstr(o, "K_mark"), "placeholder, not ratified") != 0
            || strcmp(jstr(o, "reason"), "submission_deadline") != 0) {
            fprintf(stderr, "FAIL deadline policy\n"); g_fail++;
        }
        JVal *cases = jget(o, "cases");
        for (int i = 0; i < cases->an; i++) {
            JVal *c = cases->a[i];
            const char *name = jstr(c, "name");
            long long K = jnum(c, "K");
            long long last = jnum(c, "last_beacon_height");
            const char *outcome = jstr(c, "outcome");
            if (jbool(c, "fake_leg_ignore")) { fprintf(stderr, "FAIL %s ban\n", name); g_fail++; }
            JVal *tipv = jget(c, "tip_height");
            if (!tipv) { fprintf(stderr, "FAIL %s tip\n", name); g_fail++; continue; }
            if (tipv->k == JNULL) {
                JVal *gapv = jget(c, "gap");
                if (!gapv || gapv->k != JSTR || strcmp(gapv->s, "not read") != 0
                    || strcmp(outcome, "held") != 0) {
                    fprintf(stderr, "FAIL %s not read\n", name); g_fail++;
                } else pass(name);
                continue;
            }
            if (tipv->k != JNUM) { fprintf(stderr, "FAIL %s tip type\n", name); g_fail++; continue; }
            long long tip = tipv->n;
            long long gap = tip - last;
            if (jnum(c, "gap") != gap) { fprintf(stderr, "FAIL %s gap\n", name); g_fail++; continue; }
            const char *expect = "not refused by this rule";
            if (tip < last) expect = "held";
            else if (gap > K) expect = "refuse";
            if (strcmp(outcome, expect) != 0) {
                fprintf(stderr, "FAIL %s outcome\n", name); g_fail++; continue;
            }
            if (strcmp(expect, "refuse") == 0) {
                if (!jbool(c, "must_reject") || strcmp(jstr(c, "reason"), "submission_deadline") != 0) {
                    fprintf(stderr, "FAIL %s refuse\n", name); g_fail++;
                } else reject(name);
            } else pass(name);
        }
        free(path);
        section("deadline");
    }

    /* production leg: 256 MiB terrain, one 2,000,000-step leg, one checkpoint */
    {
        char *path = path_join(dir, "prod-leg0.json");
        JVal *o = load_json(path);
        before = g_fail;
        size_t np, nb;
        uint8_t *pk = unhex(jstr(o, "public_key_hex"), &np);
        uint8_t *be = unhex(jstr(o, "seed_beacon_hex"), &nb);
        uint8_t d[32]; uint64_t P[4];
        seed_of(pk, be, d, P);
        uint64_t slot = (uint64_t)jnum(o, "slot_bytes");
        uint64_t tb = (uint64_t)jnum(o, "terrain_bytes");
        uint64_t sample = (uint64_t)jnum(o, "sample_bytes");
        uint64_t steps = (uint64_t)jnum(o, "steps");
        if (slot != 16384 || tb != 256ull * 1024ull * 1024ull || sample != 1024 || steps != 2000000ull) {
            fprintf(stderr, "FAIL prod profile\n"); g_fail++;
        }
        uint8_t *terrain = fill_terrain(word_nl4, P, slot, tb);
        uint8_t dig[32];
        sha256(terrain, tb, dig);
        hex_of(dig, 32, hx);
        expect_str("prod-terrain", hx, jstr(o, "terrain_sha256"));
        uint64_t S = do_leg(terrain, tb, slot, steps, P[0]);
        char got[17];
        snprintf(got, sizeof got, "%016llx", (unsigned long long)S);
        expect_str("prod-leg0", got, jstr(o, "leg0_u64_hex"));
        uint64_t offset = load_le(d) % (tb - sample + 1);
        if ((uint64_t)jnum(o, "sample_offset") != offset || (uint64_t)jnum(o, "offset_modulus") != tb - sample + 1) {
            fprintf(stderr, "FAIL prod offset\n"); g_fail++;
        }
        uint8_t *cp = checkpoint(terrain, tb, sample, d, d, S);
        hex_of(cp, 32, hx);
        expect_str("prod-cp0", hx, jstr(o, "checkpoint0_hex"));
        free(cp); free(terrain); free(pk); free(be); free(path);
        section("prod-leg0");
    }

    if (g_fail) {
        fprintf(stderr, "%d check(s) failed\n", g_fail);
        return 1;
    }
    printf("all checks ok\n");
    return 0;
}
