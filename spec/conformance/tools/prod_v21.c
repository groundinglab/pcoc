/* prod_v21.c — production-size NL4 + §11.1 values for the generator.
 * The §11.1 accumulator is wrapping addition of mix64(word XOR S). No w term.
 * gen_v21.py shells out to this binary. It does not walk a production leg
 * in Python. check_v21.c recomputes the same pack file on its own.
 * gcc -O2 -o /tmp/prod_v21_helper prod_v21.c -lcrypto
 */
#include <openssl/sha.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MIX_ADD  0x9E3779B97F4A7C15ULL
#define MIX_MUL1 0xBF58476D1CE4E5B9ULL
#define MIX_MUL2 0x94D049BB133111EBULL
#define WORD_XOR 0xC0FFEEULL
#define PARENT_MUL 0xD1B54A32D192ED03ULL
#define SLOT_MUL   0xA24BAED4963EE407ULL
static const uint8_t SEAL[12] = {'p','c','o','c','-','w','a','l','k','-','v','1'};

static uint64_t mix64(uint64_t x) {
    uint64_t y = x + MIX_ADD;
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
static void sha256(const uint8_t *p, size_t n, uint8_t out[32]) { SHA256(p, n, out); }
static void hexof(const uint8_t *p, size_t n) {
    for (size_t i = 0; i < n; i++) printf("%02x", p[i]);
    printf("\n");
}

static uint8_t *fill(const uint64_t P[4], uint64_t slot_bytes, uint64_t nslots) {
    uint64_t words = slot_bytes / 8;
    uint64_t *slots = calloc(nslots * words, sizeof(uint64_t));
    uint8_t *out = malloc(nslots * slot_bytes);
    uint64_t *w = malloc(words * sizeof(uint64_t));
    if (!slots || !out || !w) { fprintf(stderr, "oom\n"); exit(2); }
    for (uint64_t i = 0; i < nslots; i++) {
        for (uint64_t j = 0; j < words; j++) w[j] = nl4_word(P, i, j);
        if (i > 0) {
            uint64_t par[3];
            parents_of(i, par);
            for (int k = 1; k <= 3; k++) {
                uint64_t p = par[k - 1];
                for (uint64_t j = 0; j < words; j++) {
                    unsigned rot = (unsigned)((j + (uint64_t)k) % 64);
                    w[j] = mix64((w[j] ^ rotl(slots[p * words + j], rot)) ^ ((uint64_t)k * PARENT_MUL));
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
    free(w); free(slots);
    return out;
}

static uint64_t seal(uint64_t folded) {
    uint8_t buf[20], d[32];
    memcpy(buf, SEAL, 12);
    store_le(buf + 12, folded);
    sha256(buf, 20, d);
    return load_le(d);
}
static uint64_t fold_c1(const uint8_t *slot, uint64_t words, uint64_t S) {
    uint64_t acc = 0;
    for (uint64_t w = 0; w < words; w++) {
        uint64_t word = load_le(slot + 8 * w);
        acc += mix64(word ^ S);
    }
    return acc;
}

int main(void) {
    uint8_t pk[32], be[32], d[32];
    memset(pk, 0x11, 32);
    memset(be, 0x22, 32);
    uint8_t pre[64];
    memcpy(pre, pk, 32); memcpy(pre + 32, be, 32);
    sha256(pre, 64, d);
    uint64_t P[4];
    for (int k = 0; k < 4; k++) P[k] = load_le(d + 8 * k);
    printf("seed "); hexof(d, 32);

    uint8_t *wrap = fill(P, 16384, 4);
    uint8_t dig[32];
    sha256(wrap, 16384ULL * 4, dig);
    printf("wrap16k "); hexof(dig, 32);
    free(wrap);

    const uint64_t slot_b = 16384, nslots = 16384, steps = 2000000, sample = 1024;
    uint8_t *terrain = fill(P, slot_b, nslots);
    sha256(terrain, slot_b * nslots, dig);
    printf("terrain "); hexof(dig, 32);

    uint64_t S = P[0];
    uint64_t words = slot_b / 8;
    for (uint64_t t = 0; t < steps; t++) {
        uint64_t i = S % nslots;
        uint64_t folded = fold_c1(terrain + i * slot_b, words, S);
        S = seal(folded);
    }
    printf("leg0 %016llx\n", (unsigned long long)S);
    uint64_t offmod = slot_b * nslots - sample + 1;
    uint64_t offset = P[0] % offmod;
    printf("offset %llu\n", (unsigned long long)offset);
    uint8_t *preimage = malloc(32 + 32 + sample + 8);
    memcpy(preimage, d, 32);
    memcpy(preimage + 32, d, 32);
    memcpy(preimage + 64, terrain + offset, sample);
    store_le(preimage + 64 + sample, S);
    sha256(preimage, 64 + sample + 8, dig);
    printf("cp0 "); hexof(dig, 32);
    free(preimage); free(terrain);
    return 0;
}
