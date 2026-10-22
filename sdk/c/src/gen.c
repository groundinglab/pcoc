/* Candidate generator. CASES.md fixtures, written from SPEC 2.2.
 * make candidates writes sdk/c/candidates.
 */
#define _POSIX_C_SOURCE 200809L
#include "pcoc.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

void pcoc_negatives_s1(const pcoc_sizes *sizes, const uint64_t P[4], const uint8_t *honest,
                       const uint8_t previous[32], uint64_t honest_leg, char *out, size_t cap);

static const char *PK_A = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
static const char *PK_B = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";

static int hex_nibble(char c) {
  if (c >= '0' && c <= '9') {
    return c - '0';
  }
  if (c >= 'a' && c <= 'f') {
    return c - 'a' + 10;
  }
  return -1;
}

static void unhex(const char *s, uint8_t *out, size_t n) {
  for (size_t i = 0; i < n; i++) {
    out[i] = (uint8_t)((hex_nibble(s[2 * i]) << 4) | hex_nibble(s[2 * i + 1]));
  }
}

static void hex_out(const uint8_t *p, size_t n, char *o) {
  static const char *H = "0123456789abcdef";
  for (size_t i = 0; i < n; i++) {
    o[2 * i] = H[p[i] >> 4];
    o[2 * i + 1] = H[p[i] & 0xf];
  }
  o[2 * n] = 0;
}

static void named_hash(const char *name, uint8_t out[32]) {
  pcoc_sha256((const uint8_t *)name, strlen(name), out);
}

typedef struct {
  char *s;
  size_t n;
  size_t cap;
} buf;

static void badd(buf *b, const char *s) {
  size_t k = strlen(s);
  if (b->n + k + 1 > b->cap) {
    b->cap = (b->n + k + 1) * 2 + 64;
    b->s = realloc(b->s, b->cap);
  }
  memcpy(b->s + b->n, s, k);
  b->n += k;
  b->s[b->n] = 0;
}

static void bfmt(buf *b, const char *fmt, const char *a, const char *c) {
  char line[512];
  snprintf(line, sizeof(line), fmt, a, c);
  badd(b, line);
}

static int cmp_line(const void *a, const void *b) {
  return strcmp(*(char *const *)a, *(char *const *)b);
}

static void write_sorted(const char *path, const char *text) {
  /* Split on LF, sort by key (the whole line, which starts with the key),
   * write with LF. No blank lines. */
  size_t nlines = 0;
  for (const char *p = text; *p; p++) {
    if (*p == '\n') {
      nlines++;
    }
  }
  char **lines = calloc(nlines, sizeof(char *));
  size_t k = 0;
  const char *start = text;
  for (const char *p = text; *p; p++) {
    if (*p == '\n') {
      size_t len = (size_t)(p - start);
      lines[k] = malloc(len + 1);
      memcpy(lines[k], start, len);
      lines[k][len] = 0;
      k++;
      start = p + 1;
    }
  }
  qsort(lines, k, sizeof(char *), cmp_line);
  FILE *f = fopen(path, "wb");
  if (!f) {
    perror(path);
    exit(1);
  }
  for (size_t i = 0; i < k; i++) {
    if (lines[i][0] == 0) {
      continue;
    }
    fputs(lines[i], f);
    fputc('\n', f);
    free(lines[i]);
  }
  free(lines);
  fclose(f);
}

static void u64_hex(uint64_t v, char out[17]) {
  snprintf(out, 17, "%016llx", (unsigned long long)v);
}

static void le_hex(uint64_t v, char out[17]) {
  uint8_t b[8];
  pcoc_le64(v, b);
  hex_out(b, 8, out);
}

typedef struct {
  uint8_t beacon[32];
  uint64_t height;
  uint8_t waypoint[32];
  uint64_t leg_count;
  uint8_t checkpoints[8][32];
  uint64_t leg_results[8];
} stage_run;

static void run_hike(const pcoc_sizes *sizes, const uint8_t *terrain, const uint8_t seed[32],
                     const uint8_t beacons[][32], const int *legs, stage_run stages[18],
                     uint8_t last_interrupt[32]) {
  uint8_t waypoint[32];
  memcpy(waypoint, seed, 32);
  uint8_t beacon[32];
  memcpy(beacon, beacons[0], 32);
  for (int g = 0; g < 18; g++) {
    memcpy(stages[g].waypoint, waypoint, 32);
    memcpy(stages[g].beacon, beacon, 32);
    stages[g].height = (uint64_t)g; /* synthetic heights, strictly increasing */
    uint8_t previous[32];
    memcpy(previous, waypoint, 32);
    int nlegs = legs[g];
    stages[g].leg_count = (uint64_t)nlegs;
    for (int j = 0; j < nlegs; j++) {
      uint64_t lr = pcoc_leg_result(sizes, terrain, previous);
      stages[g].leg_results[j] = lr;
      uint8_t cp[32];
      pcoc_checkpoint(sizes, terrain, previous, waypoint, lr, cp);
      memcpy(stages[g].checkpoints[j], cp, 32);
      memcpy(previous, cp, 32);
    }
    if (g < 17) {
      uint8_t fold_in[8 * 32];
      for (int j = 0; j < nlegs; j++) {
        memcpy(fold_in + 32 * j, stages[g].checkpoints[j], 32);
      }
      uint8_t next_w[32];
      pcoc_waypoint(sizes, terrain, beacons[g + 1], waypoint, fold_in, (size_t)nlegs, next_w);
      memcpy(waypoint, next_w, 32);
      memcpy(beacon, beacons[g + 1], 32);
    }
  }
  memcpy(last_interrupt, beacons[18], 32);
}

static void emit_hike(buf *b, const char *tag, stage_run stages[18]) {
  char num[32], hx[80];
  for (int g = 0; g < 18; g++) {
    snprintf(num, sizeof(num), "%d", g);
    for (uint64_t j = 0; j < stages[g].leg_count; j++) {
      char js[32];
      snprintf(js, sizeof(js), "%llu", (unsigned long long)j);
      le_hex(stages[g].leg_results[j], hx);
      char line[256];
      snprintf(line, sizeof(line), "%s.pk_A.stage.%s.leg.%s.legs_le_hex\t%s\n", tag, num, js, hx);
      badd(b, line);
      hex_out(stages[g].checkpoints[j], 32, hx);
      snprintf(line, sizeof(line), "%s.pk_A.stage.%s.leg.%s.checkpoint\t%s\n", tag, num, js, hx);
      badd(b, line);
    }
    hex_out(stages[g].waypoint, 32, hx);
    char line[256];
    snprintf(line, sizeof(line), "%s.pk_A.stage.%s.waypoint\t%s\n", tag, num, hx);
    badd(b, line);
  }
}

int main(int argc, char **argv) {
  const char *dir = argc > 1 ? argv[1] : "candidates";
  uint8_t pkA[32], pkB[32];
  unhex(PK_A, pkA, 32);
  unhex(PK_B, pkB, 32);
  uint8_t seed_beacon[32];
  named_hash("pcoc-pack-2.2/beacon/seed", seed_beacon);
  uint8_t Ik[19][32];
  for (int k = 0; k < 19; k++) {
    char name[64];
    snprintf(name, sizeof(name), "pcoc-pack-2.2/beacon/%d", k);
    named_hash(name, Ik[k]);
  }

  /* ---- seed.txt ---- */
  {
    buf b = {0};
    const uint8_t *pks[2] = {pkA, pkB};
    const char *names[2] = {"pk_A", "pk_B"};
    for (int i = 0; i < 2; i++) {
      uint8_t pre[64], D[32];
      uint64_t P[4];
      pcoc_seed(pks[i], seed_beacon, pre, D, P);
      char hx[129], line[256], uh[17];
      hex_out(pre, 64, hx);
      snprintf(line, sizeof(line), "seed.%s.preimage\t%s\n", names[i], hx);
      badd(&b, line);
      hex_out(D, 32, hx);
      snprintf(line, sizeof(line), "seed.%s.D\t%s\n", names[i], hx);
      badd(&b, line);
      for (int k = 0; k < 4; k++) {
        u64_hex(P[k], uh);
        snprintf(line, sizeof(line), "seed.%s.P%d_u64\t%s\n", names[i], k, uh);
        badd(&b, line);
      }
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/seed.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  pcoc_profile s1p, s2p, prodp;
  pcoc_profile_small(&s1p, 64, 32, 65536, 100);
  pcoc_profile_small(&s2p, 256, 64, 1048576, 1000);
  pcoc_profile_production(&prodp);
  pcoc_sizes s1, s2, prod;
  pcoc_sizes_from_profile(&s1p, &s1);
  pcoc_sizes_from_profile(&s2p, &s2);
  pcoc_sizes_from_profile(&prodp, &prod);

  uint8_t pre[64], D[32];
  uint64_t PA[4];
  pcoc_seed(pkA, seed_beacon, pre, D, PA);

  uint8_t *ter1 = malloc((size_t)s1.terrain_bytes);
  fprintf(stderr, "gen: S1 terrain\n");
  pcoc_terrain_fill(&s1, PA, ter1);

  /* ---- nl4.txt ---- */
  {
    buf b = {0};
    uint64_t slots[3] = {0, 1, s1.n_slots - 1};
    const char *sn[3] = {"0", "1", "last"};
    for (int si = 0; si < 3; si++) {
      uint64_t i = slots[si];
      char label[32];
      if (si == 2) {
        snprintf(label, sizeof(label), "%llu", (unsigned long long)i);
      } else {
        snprintf(label, sizeof(label), "%s", sn[si]);
      }
      for (uint64_t j = 0; j < 16 && j < s1.words_per_slot; j++) {
        char uh[17], line[256];
        u64_hex(pcoc_nl4_base(i, j, PA), uh);
        snprintf(line, sizeof(line), "nl4.S1.pk_A.slot.%s.j.%llu.base_u64\t%s\n", label,
                 (unsigned long long)j, uh);
        badd(&b, line);
        const uint8_t *slot = ter1 + (size_t)(i * s1.slot_bytes);
        u64_hex(pcoc_u64_le(slot + 8 * j), uh);
        snprintf(line, sizeof(line), "nl4.S1.pk_A.slot.%s.j.%llu.stored_u64\t%s\n", label,
                 (unsigned long long)j, uh);
        badd(&b, line);
      }
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/nl4.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  /* ---- terrain.txt (S1 now, S2 and PROD filled in) ---- */
  uint8_t dig1[32], dig2[32], digp[32];
  pcoc_sha256(ter1, (size_t)s1.terrain_bytes, dig1);

  fprintf(stderr, "gen: S2 terrain\n");
  uint8_t *ter2 = malloc((size_t)s2.terrain_bytes);
  pcoc_terrain_fill(&s2, PA, ter2);
  pcoc_sha256(ter2, (size_t)s2.terrain_bytes, dig2);

  fprintf(stderr, "gen: PROD terrain\n");
  uint8_t *terp = malloc((size_t)prod.terrain_bytes);
  struct timespec ta, tb;
  clock_gettime(CLOCK_MONOTONIC, &ta);
  pcoc_terrain_fill(&prod, PA, terp);
  clock_gettime(CLOCK_MONOTONIC, &tb);
  double prod_terrain_s = (tb.tv_sec - ta.tv_sec) + (tb.tv_nsec - ta.tv_nsec) / 1e9;
  pcoc_sha256(terp, (size_t)prod.terrain_bytes, digp);
  fprintf(stderr, "gen: PROD terrain %.3f s\n", prod_terrain_s);

  {
    buf b = {0};
    char hx[65], line[256];
    hex_out(dig1, 32, hx);
    snprintf(line, sizeof(line), "terrain.S1.pk_A.sha256\t%s\n", hx);
    badd(&b, line);
    hex_out(dig2, 32, hx);
    snprintf(line, sizeof(line), "terrain.S2.pk_A.sha256\t%s\n", hx);
    badd(&b, line);
    hex_out(digp, 32, hx);
    snprintf(line, sizeof(line), "terrain.PROD.pk_A.sha256\t%s\n", hx);
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/terrain.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  /* ---- fold.txt : S1 stage 0 leg 0, steps 0..3 and the leg result ---- */
  {
    buf b = {0};
    uint64_t S = pcoc_u64_le(D);
    char uh[17], line[256];
    for (int step = 0; step < 4; step++) {
      uint64_t slot, acc;
      u64_hex(S, uh);
      snprintf(line, sizeof(line), "fold.S1.pk_A.stage.0.leg.0.step.%d.S_u64\t%s\n", step, uh);
      badd(&b, line);
      uint64_t nS = pcoc_step_parts(&s1, ter1, S, &slot, &acc);
      snprintf(line, sizeof(line), "fold.S1.pk_A.stage.0.leg.0.step.%d.slot\t%llu\n", step,
               (unsigned long long)slot);
      badd(&b, line);
      u64_hex(acc, uh);
      snprintf(line, sizeof(line), "fold.S1.pk_A.stage.0.leg.0.step.%d.acc_u64\t%s\n", step, uh);
      badd(&b, line);
      u64_hex(nS, uh);
      snprintf(line, sizeof(line), "fold.S1.pk_A.stage.0.leg.0.step.%d.S_out_u64\t%s\n", step, uh);
      badd(&b, line);
      S = nS;
    }
    uint64_t lr = pcoc_leg_result(&s1, ter1, D);
    le_hex(lr, uh);
    snprintf(line, sizeof(line), "fold.S1.pk_A.stage.0.leg.0.legs_le_hex\t%s\n", uh);
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/fold.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  int legs1[18], legs2[18];
  for (int g = 0; g < 18; g++) {
    legs1[g] = 1;
    legs2[g] = 3;
  }
  uint8_t beacons[19][32];
  memcpy(beacons[0], seed_beacon, 32);
  for (int k = 0; k < 18; k++) {
    memcpy(beacons[k + 1], Ik[k], 32);
  }

  fprintf(stderr, "gen: S1 hike\n");
  stage_run h1[18];
  uint8_t last1[32];
  run_hike(&s1, ter1, D, beacons, legs1, h1, last1);
  {
    buf b = {0};
    emit_hike(&b, "kat-small.S1", h1);
    char path[512];
    snprintf(path, sizeof(path), "%s/kat-small.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  fprintf(stderr, "gen: S2 hike\n");
  stage_run h2[18];
  uint8_t last2[32];
  run_hike(&s2, ter2, D, beacons, legs2, h2, last2);
  {
    buf b = {0};
    emit_hike(&b, "test-3leg.S2", h2);
    char path[512];
    snprintf(path, sizeof(path), "%s/test-3leg.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  fprintf(stderr, "gen: PROD leg\n");
  clock_gettime(CLOCK_MONOTONIC, &ta);
  uint64_t prod_leg = pcoc_leg_result(&prod, terp, D);
  clock_gettime(CLOCK_MONOTONIC, &tb);
  double prod_leg_s = (tb.tv_sec - ta.tv_sec) + (tb.tv_nsec - ta.tv_nsec) / 1e9;
  fprintf(stderr, "gen: PROD leg %.3f s\n", prod_leg_s);
  uint8_t prod_cp[32];
  pcoc_checkpoint(&prod, terp, D, D, prod_leg, prod_cp);
  {
    buf b = {0};
    char hx[65], line[256];
    hex_out(digp, 32, hx);
    snprintf(line, sizeof(line), "prod.PROD.pk_A.terrain_sha256\t%s\n", hx);
    badd(&b, line);
    le_hex(prod_leg, hx);
    snprintf(line, sizeof(line), "prod.PROD.pk_A.stage.0.leg.0.legs_le_hex\t%s\n", hx);
    badd(&b, line);
    hex_out(prod_cp, 32, hx);
    snprintf(line, sizeof(line), "prod.PROD.pk_A.stage.0.leg.0.checkpoint\t%s\n", hx);
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/prod.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
    FILE *tf = fopen("/tmp/pcoc-c-prod-time.txt", "w");
    if (tf) {
      fprintf(tf, "prod_terrain_s %.6f\nprod_leg_s %.6f\n", prod_terrain_s, prod_leg_s);
      fclose(tf);
    }
  }

  /* ---- wes.txt ---- */
  {
    buf b = {0};
    char hx[65], line[256];
    hex_out(h1[0].waypoint, 32, hx);
    snprintf(line, sizeof(line), "wes.S1.pk_A.stage.0.waypoint\t%s\n", hx);
    badd(&b, line);
    hex_out(D, 32, hx);
    snprintf(line, sizeof(line), "wes.S1.pk_A.D\t%s\n", hx);
    badd(&b, line);
    int eq = memcmp(h1[0].waypoint, D, 32) == 0;
    snprintf(line, sizeof(line), "wes.S1.pk_A.equal\t%s\n", eq ? "true" : "false");
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/wes.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  /* ---- rate.txt : Rule B2 outcome words ---- */
  {
    buf b = {0};
    uint64_t legs_g[] = {0, 1, 5, 6, 1000};
    int64_t spans[] = {1, 300, 3600};
    uint64_t ps[] = {1, 1, 7};
    uint64_t qs[] = {5, 4, 20};
    for (unsigned i = 0; i < 5; i++) {
      for (unsigned j = 0; j < 3; j++) {
        for (unsigned k = 0; k < 3; k++) {
          const char *outc = NULL;
          pcoc_leg_rate_floor(legs_g[i], 0, spans[j], ps[k], qs[k], &outc);
          char line[256];
          snprintf(line, sizeof(line),
                   "rate.legs.%llu.span.%lld.floor.%llu_%llu\t%s\n",
                   (unsigned long long)legs_g[i], (long long)spans[j],
                   (unsigned long long)ps[k], (unsigned long long)qs[k], outc);
          badd(&b, line);
        }
      }
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/rate.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  /* ---- anchor.txt ---- */
  {
    buf b = {0};
    for (uint64_t h = 100; h <= 106; h++) {
      const char *outc = NULL;
      pcoc_anchor_window(100, h, &outc);
      char line[128];
      snprintf(line, sizeof(line), "anchor.H.100.header.%llu\t%s\n", (unsigned long long)h, outc);
      badd(&b, line);
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/anchor.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  /* ---- draw.txt ---- */
  {
    buf b = {0};
    uint8_t b0[32];
    named_hash("pcoc-pack-2.2/draw", b0);
    /* Chain of SHA-256, concatenated, enough for the draws. */
    uint8_t stream[32 * 64];
    memcpy(stream, b0, 32);
    for (int i = 0; i < 63; i++) {
      pcoc_sha256(stream + 32 * i, 32, stream + 32 * (i + 1));
    }
    uint64_t totals[] = {6, 18, 54};
    size_t off = 0;
    for (int t = 0; t < 3; t++) {
      struct {
        const uint8_t *src;
        size_t len, i;
      } ctx = {stream + off, sizeof(stream) - off, 0};
      /* Use pcoc_six_leg_draw with a file-scope-compatible function.
       * Redefined here as a static through a wrapper. */
      uint64_t idx[6];
      int status = 0;
      /* inline the draw by calling the library with a read that uses a
       * global. Set the global. */
      extern void pcoc_gen_set_draw(const uint8_t *s, size_t n);
      extern int pcoc_gen_draw_read(void *c, uint8_t out[8]);
      extern size_t pcoc_gen_draw_used(void);
      pcoc_gen_set_draw(stream + off, sizeof(stream) - off);
      pcoc_six_leg_draw(totals[t], pcoc_gen_draw_read, &ctx, idx, &status);
      off += pcoc_gen_draw_used();
      /* The library reader ignores ctx and uses the global, and advances
       * a global offset. Recover how many bytes were consumed from that. */
      char line[256];
      snprintf(line, sizeof(line), "draw.legs.%llu.indices\t", (unsigned long long)totals[t]);
      badd(&b, line);
      for (int i = 0; i < 6; i++) {
        char piece[32];
        snprintf(piece, sizeof(piece), "%s%llu", i ? "," : "", (unsigned long long)idx[i]);
        badd(&b, piece);
      }
      badd(&b, "\n");
      (void)off;
      (void)ctx;
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/draw.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  /* ---- unknown-kind.txt ---- */
  {
    buf b = {0};
    const char *labels[] = {"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"};
    const char *keys[] = {"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "empty",
                          "v2.2-pcoc-objecT"};
    for (int i = 0; i < 4; i++) {
      char rec[256];
      pcoc_unknown_kind_record(labels[i], strlen(labels[i]), rec, sizeof(rec));
      char hx[512];
      hex_out((const uint8_t *)rec, strlen(rec), hx);
      char line[700];
      snprintf(line, sizeof(line), "unknown-kind.%s.record_hex\t%s\n", keys[i], hx);
      badd(&b, line);
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/unknown-kind.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  /* ---- hashcash.txt ---- */
  {
    buf b = {0};
    uint8_t origin_hash_unused[32];
    const char *origin = "https://bari.example";
    /* origin label recorded so other cores can see the chosen name. */
    uint8_t fee[32];
    named_hash("pcoc-pack-2.2/hashcash.fee-header", fee);
    uint8_t challenge[32], digest0[32];
    pcoc_hashcash_challenge(origin, strlen(origin), fee, challenge);
    uint8_t waypoint[32];
    named_hash("pcoc-pack-2.2/hashcash.waypoint", waypoint);
    pcoc_hashcash_digest(challenge, waypoint, 0, digest0);
    uint64_t nonce = 0;
    int rc = pcoc_hashcash_solve(challenge, waypoint, 21, 2000000ull, &nonce);
    uint8_t digestn[32];
    if (rc == 0) {
      pcoc_hashcash_digest(challenge, waypoint, nonce, digestn);
    }
    char hx[65], line[400];
    hex_out(challenge, 32, hx);
    snprintf(line, sizeof(line), "hashcash.origin_label\t%s\n", origin);
    badd(&b, line);
    snprintf(line, sizeof(line), "hashcash.fee_header_label\tpcoc-pack-2.2/hashcash.fee-header\n");
    badd(&b, line);
    snprintf(line, sizeof(line), "hashcash.waypoint_label\tpcoc-pack-2.2/hashcash.waypoint\n");
    badd(&b, line);
    snprintf(line, sizeof(line), "hashcash.challenge\t%s\n", hx);
    badd(&b, line);
    hex_out(digest0, 32, hx);
    snprintf(line, sizeof(line), "hashcash.digest_nonce0\t%s\n", hx);
    badd(&b, line);
    snprintf(line, sizeof(line), "hashcash.nonce\t%llu\n", (unsigned long long)nonce);
    badd(&b, line);
    if (rc == 0) {
      hex_out(digestn, 32, hx);
      snprintf(line, sizeof(line), "hashcash.digest\t%s\n", hx);
      badd(&b, line);
    }
    snprintf(line, sizeof(line), "hashcash.bits\t21\n");
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/hashcash.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
    (void)origin_hash_unused;
  }

  /* ---- negatives.txt ---- */
  {
    char *big = malloc(1 << 20);
    uint64_t honest = h1[0].leg_results[0];
    pcoc_negatives_s1(&s1, PA, ter1, D, honest, big, 1 << 20);
    char path[512];
    snprintf(path, sizeof(path), "%s/negatives.txt", dir);
    write_sorted(path, big);
    free(big);
  }

  free(ter1);
  free(ter2);
  free(terp);
  fprintf(stderr, "gen: done\n");
  return 0;
}

/* Draw-byte cursor for the generator. The library's read callback has no
 * other channel, so the cursor lives here. */
static const uint8_t *g_draw;
static size_t g_draw_n;
static size_t g_draw_i;

void pcoc_gen_set_draw(const uint8_t *s, size_t n) {
  g_draw = s;
  g_draw_n = n;
  g_draw_i = 0;
}

size_t pcoc_gen_draw_used(void) { return g_draw_i; }

int pcoc_gen_draw_read(void *c, uint8_t out[8]) {
  (void)c;
  if (g_draw_i + 8 > g_draw_n) {
    return -1;
  }
  memcpy(out, g_draw + g_draw_i, 8);
  g_draw_i += 8;
  return 0;
}
