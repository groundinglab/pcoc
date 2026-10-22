/* Candidate generator. CASES.md fixtures. Negatives live only in this target. */
#include "pcoc.h"
#include "negatives.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *PK_A = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
static const char *PK_B = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";

static int hex_nibble(char c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
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
  size_t n, cap;
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

static int cmp_line(const void *a, const void *b) {
  return strcmp(*(char *const *)a, *(char *const *)b);
}

static void write_sorted(const char *path, const char *text) {
  size_t nlines = 0;
  for (const char *p = text; *p; p++) {
    if (*p == '\n') nlines++;
  }
  char **lines = calloc(nlines ? nlines : 1, sizeof(char *));
  size_t k = 0;
  const char *start = text;
  for (const char *p = text; p && *p; p++) {
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
      free(lines[i]);
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

static void emit_hike_lines(buf *b, const char *tag, const pcoc_object *obj) {
  char hx[80], line[320];
  for (int g = 0; g < 18; g++) {
    for (uint64_t j = 0; j < obj->stages[g].leg_count; j++) {
      hex_out(obj->stages[g].leg_results[j], 8, hx);
      snprintf(line, sizeof(line), "%s.stage.%d.leg.%llu.legs_le_hex\t%s\n", tag, g,
               (unsigned long long)j, hx);
      badd(b, line);
      hex_out(obj->stages[g].checkpoints[j], 32, hx);
      snprintf(line, sizeof(line), "%s.stage.%d.leg.%llu.checkpoint\t%s\n", tag, g,
               (unsigned long long)j, hx);
      badd(b, line);
    }
    hex_out(obj->stages[g].waypoint, 32, hx);
    snprintf(line, sizeof(line), "%s.stage.%d.waypoint\t%s\n", tag, g, hx);
    badd(b, line);
  }
}

typedef struct {
  const uint8_t *s;
  size_t n, i;
} drawsrc;

static int draw_read(void *c, uint8_t out[8]) {
  drawsrc *d = c;
  if (d->i + 8 > d->n) return -1;
  memcpy(out, d->s + d->i, 8);
  d->i += 8;
  return 0;
}

static void load_beacons(uint8_t seed_beacon[32], uint8_t beacons[19][32], uint64_t heights[19]) {
  named_hash("pcoc-pack-2.2/beacon/seed", seed_beacon);
  memcpy(beacons[0], seed_beacon, 32);
  heights[0] = 1000;
  for (int k = 0; k < 18; k++) {
    char name[64];
    snprintf(name, sizeof(name), "pcoc-pack-2.2/beacon/%d", k);
    named_hash(name, beacons[k + 1]);
    heights[k + 1] = 1000 + (uint64_t)k + 1;
  }
}

int main(int argc, char **argv) {
  const char *dir = argc > 1 ? argv[1] : "candidates";
  uint8_t pkA[32], pkB[32];
  unhex(PK_A, pkA, 32);
  unhex(PK_B, pkB, 32);
  uint8_t seed_beacon[32], beacons[19][32];
  uint64_t heights[19];
  load_beacons(seed_beacon, beacons, heights);

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

  uint8_t pre[64], D[32], DB[32];
  uint64_t PA[4], PB[4];
  pcoc_seed(pkA, seed_beacon, pre, D, PA);
  pcoc_seed(pkB, seed_beacon, pre, DB, PB);

  uint8_t *ter1 = malloc((size_t)s1.terrain_bytes);
  pcoc_terrain_fill(&s1, PA, ter1);

  {
    buf b = {0};
    uint64_t slots[3] = {0, 1, 1023};
    for (int si = 0; si < 3; si++) {
      uint64_t i = slots[si];
      for (uint64_t j = 0; j < 8; j++) {
        char uh[17], line[256];
        u64_hex(pcoc_nl4_base(i, j, PA), uh);
        snprintf(line, sizeof(line), "nl4.S1.slot.%llu.word.%llu.base_u64\t%s\n",
                 (unsigned long long)i, (unsigned long long)j, uh);
        badd(&b, line);
        const uint8_t *slot = ter1 + (size_t)(i * s1.slot_bytes);
        u64_hex(pcoc_u64_le(slot + 8 * j), uh);
        snprintf(line, sizeof(line), "nl4.S1.slot.%llu.word.%llu.stored_u64\t%s\n",
                 (unsigned long long)i, (unsigned long long)j, uh);
        badd(&b, line);
      }
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/nl4.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  uint8_t dig1[32];
  pcoc_sha256(ter1, (size_t)s1.terrain_bytes, dig1);

  uint8_t *ter2 = malloc((size_t)s2.terrain_bytes);
  pcoc_terrain_fill(&s2, PA, ter2);
  uint8_t dig2[32];
  pcoc_sha256(ter2, (size_t)s2.terrain_bytes, dig2);

  fprintf(stderr, "gen: PROD terrain\n");
  uint8_t *terp = malloc((size_t)prod.terrain_bytes);
  if (!ter1 || !ter2 || !terp) {
    fprintf(stderr, "gen: out of memory\n");
    return 1;
  }
  pcoc_terrain_fill(&prod, PA, terp);
  uint8_t digp[32];
  pcoc_sha256(terp, (size_t)prod.terrain_bytes, digp);
  {
    buf b = {0};
    char hx[65], line[256];
    hex_out(digp, 32, hx);
    snprintf(line, sizeof(line), "terrain.PROD.terrain_sha256\t%s\n", hx);
    badd(&b, line);
    hex_out(dig1, 32, hx);
    snprintf(line, sizeof(line), "terrain.S1.terrain_sha256\t%s\n", hx);
    badd(&b, line);
    hex_out(dig2, 32, hx);
    snprintf(line, sizeof(line), "terrain.S2.terrain_sha256\t%s\n", hx);
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/terrain.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  {
    buf b = {0};
    uint64_t S = pcoc_u64_le(D);
    char uh[17], line[256];
    for (int step = 0; step < 4; step++) {
      uint64_t slot, acc;
      u64_hex(S, uh);
      snprintf(line, sizeof(line), "fold.S1.step.%d.S_u64\t%s\n", step, uh);
      badd(&b, line);
      uint64_t nS = pcoc_step_parts(&s1, ter1, S, &slot, &acc);
      snprintf(line, sizeof(line), "fold.S1.step.%d.slot_index\t%llu\n", step,
               (unsigned long long)slot);
      badd(&b, line);
      u64_hex(acc, uh);
      snprintf(line, sizeof(line), "fold.S1.step.%d.acc_u64\t%s\n", step, uh);
      badd(&b, line);
      u64_hex(nS, uh);
      snprintf(line, sizeof(line), "fold.S1.step.%d.S_next_u64\t%s\n", step, uh);
      badd(&b, line);
      S = nS;
    }
    uint64_t lr = pcoc_leg_result(&s1, ter1, D);
    u64_hex(lr, uh);
    snprintf(line, sizeof(line), "fold.S1.leg_result_u64\t%s\n", uh);
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
  pcoc_object h1, h2;
  fprintf(stderr, "gen: S1 hike\n");
  if (pcoc_hike_run(&s1, ter1, pkA, seed_beacon, beacons, heights, legs1, &h1) != 0) return 1;
  {
    buf b = {0};
    emit_hike_lines(&b, "kat-small.S1", &h1);
    char hx[80], line[160];
    hex_out(dig1, 32, hx);
    snprintf(line, sizeof(line), "kat-small.S1.terrain_sha256\t%s\n", hx);
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/kat-small.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }
  fprintf(stderr, "gen: S2 hike\n");
  if (pcoc_hike_run(&s2, ter2, pkA, seed_beacon, beacons, heights, legs2, &h2) != 0) return 1;
  {
    buf b = {0};
    emit_hike_lines(&b, "test-3leg.S2", &h2);
    char hx[80], line[160];
    hex_out(dig2, 32, hx);
    snprintf(line, sizeof(line), "test-3leg.S2.terrain_sha256\t%s\n", hx);
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/test-3leg.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  fprintf(stderr, "gen: PROD leg\n");
  uint64_t prod_leg = pcoc_leg_result(&prod, terp, D);
  uint8_t prod_cp[32];
  pcoc_checkpoint(&prod, terp, D, D, prod_leg, prod_cp);
  {
    buf b = {0};
    char hx[80], line[256];
    hex_out(digp, 32, hx);
    snprintf(line, sizeof(line), "prod.PROD.terrain_sha256\t%s\n", hx);
    badd(&b, line);
    le_hex(prod_leg, hx);
    snprintf(line, sizeof(line), "prod.PROD.stage.0.leg.0.legs_le_hex\t%s\n", hx);
    badd(&b, line);
    hex_out(prod_cp, 32, hx);
    snprintf(line, sizeof(line), "prod.PROD.stage.0.leg.0.checkpoint\t%s\n", hx);
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/prod.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  {
    buf b = {0};
    char hx[80], line[256];
    hex_out(D, 32, hx);
    snprintf(line, sizeof(line), "wes.S1.D\t%s\n", hx);
    badd(&b, line);
    hex_out(h1.stages[0].waypoint, 32, hx);
    snprintf(line, sizeof(line), "wes.S1.stage.0.waypoint\t%s\n", hx);
    badd(&b, line);
    int eq = memcmp(h1.stages[0].waypoint, D, 32) == 0;
    snprintf(line, sizeof(line), "wes.S1.equal\t%s\n", eq ? "true" : "false");
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/wes.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

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
          snprintf(line, sizeof(line), "rate.legs.%llu.span.%lld.floor.%llu_%llu\t%s\n",
                   (unsigned long long)legs_g[i], (long long)spans[j], (unsigned long long)ps[k],
                   (unsigned long long)qs[k], outc);
          badd(&b, line);
        }
      }
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/rate.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  {
    buf b = {0};
    for (uint64_t h = 100; h <= 106; h++) {
      const char *outc = NULL;
      pcoc_anchor_window(100, h, &outc);
      char line[128];
      snprintf(line, sizeof(line), "anchor.H.100.height.%llu\t%s\n", (unsigned long long)h, outc);
      badd(&b, line);
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/anchor.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  {
    buf b = {0};
    uint8_t stream[32 * 80];
    named_hash("pcoc-pack-2.2/draw", stream);
    for (int i = 0; i < 79; i++) {
      pcoc_sha256(stream + 32 * i, 32, stream + 32 * (i + 1));
    }
    badd(&b, "draw.b0_label\tpcoc-pack-2.2/draw\n");
    badd(&b, "draw.thorough_rounds\t3\n");
    uint64_t totals[] = {6, 18, 54};
    for (int t = 0; t < 3; t++) {
      drawsrc ds = {stream, sizeof(stream), 0};
      uint64_t idx[6];
      int status = 0;
      pcoc_six_leg_draw_rig(totals[t], draw_read, &ds, idx, &status);
      char line[256];
      snprintf(line, sizeof(line), "draw.legs.%llu.six_leg.indices\t", (unsigned long long)totals[t]);
      badd(&b, line);
      for (int i = 0; i < 6; i++) {
        char piece[32];
        snprintf(piece, sizeof(piece), "%s%llu", i ? "," : "", (unsigned long long)idx[i]);
        badd(&b, piece);
      }
      badd(&b, "\n");
      ds.i = 0;
      uint64_t th[24];
      size_t n = 0;
      pcoc_thorough_draw_rig(totals[t], 3, draw_read, &ds, th, 24, &n, &status);
      snprintf(line, sizeof(line), "draw.legs.%llu.thorough.indices\t", (unsigned long long)totals[t]);
      badd(&b, line);
      for (size_t i = 0; i < n; i++) {
        char piece[32];
        snprintf(piece, sizeof(piece), "%s%llu", i ? "," : "", (unsigned long long)th[i]);
        badd(&b, piece);
      }
      badd(&b, "\n");
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/draw.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  {
    buf b = {0};
    const char *labels[] = {"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"};
    for (int i = 0; i < 4; i++) {
      char rec[512];
      size_t klen = strlen(labels[i]);
      if (pcoc_unknown_kind_record((const uint8_t *)labels[i], klen, rec, sizeof(rec)) < 0) return 1;
      char hx[1200];
      hex_out((const uint8_t *)labels[i], klen, hx);
      char line[1600];
      snprintf(line, sizeof(line), "unknown-kind.%d.kind_text_hex\t%s\n", i, hx);
      badd(&b, line);
      hex_out((const uint8_t *)rec, strlen(rec), hx);
      snprintf(line, sizeof(line), "unknown-kind.%d.record_hex\t%s\n", i, hx);
      badd(&b, line);
    }
    char path[512];
    snprintf(path, sizeof(path), "%s/unknown-kind.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  {
    buf b = {0};
    uint8_t a[32], c[32], e[32];
    named_hash("pcoc-pack-2.2/hashcash/fee_header/0", a);
    named_hash("pcoc-pack-2.2/hashcash/fee_header/1", c);
    named_hash("pcoc-pack-2.2/hashcash/fee_header/2", e);
    uint8_t fee[80];
    memcpy(fee, a, 32);
    memcpy(fee + 32, c, 32);
    memcpy(fee + 64, e, 16);
    uint8_t header_hash[32], single[32], waypoint[32], challenge[32], digest[32];
    pcoc_sha256(fee, 80, single);
    pcoc_sha256d_80(fee, header_hash);
    if (memcmp(single, header_hash, 32) == 0) {
      fprintf(stderr, "gen: header_hash collapsed to a single SHA-256\n");
      return 1;
    }
    named_hash("pcoc-pack-2.2/hashcash/waypoint", waypoint);
    const char *origin = "https://example.com";
    pcoc_hashcash_challenge(origin, strlen(origin), header_hash, challenge);
    uint64_t nonce = 0;
    if (pcoc_hashcash_solve(challenge, waypoint, 21, 5000000ull, &nonce) != 0) return 1;
    pcoc_hashcash_digest(challenge, waypoint, nonce, digest);
    char hx[200], line[400];
    snprintf(line, sizeof(line), "hashcash.bari_origin\t%s\n", origin);
    badd(&b, line);
    hex_out(challenge, 32, hx);
    snprintf(line, sizeof(line), "hashcash.challenge\t%s\n", hx);
    badd(&b, line);
    snprintf(line, sizeof(line), "hashcash.difficulty_bits\t21\n");
    badd(&b, line);
    hex_out(digest, 32, hx);
    snprintf(line, sizeof(line), "hashcash.digest\t%s\n", hx);
    badd(&b, line);
    hex_out(fee, 80, hx);
    snprintf(line, sizeof(line), "hashcash.fee_header\t%s\n", hx);
    badd(&b, line);
    hex_out(header_hash, 32, hx);
    snprintf(line, sizeof(line), "hashcash.header_hash\t%s\n", hx);
    badd(&b, line);
    snprintf(line, sizeof(line), "hashcash.leading_zero_bits\t%d\n", pcoc_leading_zero_bits(digest));
    badd(&b, line);
    snprintf(line, sizeof(line), "hashcash.nonce\t%llu\n", (unsigned long long)nonce);
    badd(&b, line);
    hex_out(waypoint, 32, hx);
    snprintf(line, sizeof(line), "hashcash.waypoint\t%s\n", hx);
    badd(&b, line);
    char path[512];
    snprintf(path, sizeof(path), "%s/hashcash.txt", dir);
    write_sorted(path, b.s);
    free(b.s);
  }

  {
    char *big = malloc(1 << 16);
    if (pcoc_negatives_write(&s1, PA, PB, ter1, D, big, 1 << 16) != 0) {
      fprintf(stderr, "gen: negatives rejected\n");
      return 1;
    }
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
