/* Phase A checks. One binary, selected with argv: a1, a3, a5, a5x, a6, a7, a8, a9, a10, a12, a13, or all. */
#define _POSIX_C_SOURCE 200809L
#include "pcoc.h"
#include "negatives.h"

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

static char repo[512];
static char job[512];
static char here[512];

static void die(const char *m) {
  fprintf(stderr, "FAIL %s\n", m);
  exit(1);
}

static void need(int cond, const char *m) {
  if (!cond) die(m);
}

static int run_cmd(const char *cmd, char *out, size_t cap) {
  FILE *p = popen(cmd, "r");
  if (!p) return -1;
  size_t n = 0;
  if (out && cap) {
    while (n + 1 < cap) {
      size_t g = fread(out + n, 1, cap - 1 - n, p);
      if (g == 0) break;
      n += g;
    }
    out[n] = 0;
  } else {
    char buf[256];
    while (fread(buf, 1, sizeof buf, p) > 0) {
    }
  }
  int st = pclose(p);
  if (st < 0) return -1;
  if (WIFEXITED(st)) return WEXITSTATUS(st);
  return -1;
}

static int system_rc(const char *cmd) {
  int st = system(cmd);
  if (st < 0 || !WIFEXITED(st)) return -1;
  return WEXITSTATUS(st);
}

static void sha256_file(const char *path, char hex[65]) {
  FILE *f = fopen(path, "rb");
  if (!f) die(path);
  if (fseek(f, 0, SEEK_END) != 0) die("seek");
  long sz = ftell(f);
  rewind(f);
  uint8_t *b = malloc((size_t)sz + 1);
  if (!b || (sz > 0 && fread(b, 1, (size_t)sz, f) != (size_t)sz)) die("read");
  fclose(f);
  uint8_t d[32];
  pcoc_sha256(b, (size_t)sz, d);
  free(b);
  static const char *H = "0123456789abcdef";
  for (int i = 0; i < 32; i++) {
    hex[2 * i] = H[d[i] >> 4];
    hex[2 * i + 1] = H[d[i] & 0xf];
  }
  hex[64] = 0;
}

static void read_pin(char *tag, size_t tagcap, char *vec, size_t veccap) {
  char path[1024];
  snprintf(path, sizeof(path), "%s/sdk/SPEC_PIN", repo);
  FILE *f = fopen(path, "rb");
  if (!f) die("SPEC_PIN");
  char line[256];
  tag[0] = vec[0] = 0;
  while (fgets(line, sizeof(line), f)) {
    if (strncmp(line, "tag=", 4) == 0) {
      snprintf(tag, tagcap, "%s", line + 4);
      tag[strcspn(tag, "\r\n")] = 0;
    }
    if (strncmp(line, "vector-sha256=", 14) == 0) {
      snprintf(vec, veccap, "%s", line + 14);
      vec[strcspn(vec, "\r\n")] = 0;
    }
  }
  fclose(f);
}

static int ac_a1(void) {
  char cmd[1024], out[1 << 16];
  snprintf(cmd, sizeof(cmd), "git -C '%s' diff --name-only --diff-filter=A origin/main", repo);
  need(run_cmd(cmd, out, sizeof(out)) == 0, "git diff");
  int files = 0;
  char *p = out;
  while (*p) {
    char *nl = strchr(p, '\n');
    if (nl) *nl = 0;
    if (p[0]) {
      files++;
      if (strncmp(p, "sdk/c/", 6) != 0) die(p);
    }
    if (!nl) break;
    p = nl + 1;
  }
  need(files > 5, "no added sdk/c files");
  printf("a1 added files %d\n", files);
  printf("PASS a1\n");
  return 0;
}

static int ac_a3(void) {
  char tag[64], vec[80];
  read_pin(tag, sizeof(tag), vec, sizeof(vec));
  need(strcmp(tag, "spec-2.2.0") == 0, "tag");
  char path[1024], got[65];
  snprintf(path, sizeof(path), "%s/spec/conformance/pack/manifest.json", repo);
  sha256_file(path, got);
  need(strcmp(got, vec) == 0, "manifest pin");
  printf("a3 pin %s\n", got);
  printf("PASS a3\n");
  return 0;
}

static const char *CASE_FILES[] = {
    "seed", "nl4", "terrain", "fold", "kat-small", "test-3leg", "prod", "wes",
    "rate", "anchor", "draw", "unknown-kind", "hashcash", "negatives", NULL};

static int ac_a5(void) {
  char cmd[2048];
  snprintf(cmd, sizeof(cmd), "rm -rf /tmp/pcoc-c-cand && mkdir -p /tmp/pcoc-c-cand && nice -n 10 '%s/build/pcoc-gen' /tmp/pcoc-c-cand",
           here);
  need(system_rc(cmd) == 0, "gen");
  for (int i = 0; CASE_FILES[i]; i++) {
    snprintf(cmd, sizeof(cmd), "cmp -s /tmp/pcoc-c-cand/%s.txt '%s/ref-round2/C-grok/candidates/%s.txt'",
             CASE_FILES[i], job, CASE_FILES[i]);
    if (system_rc(cmd) != 0) {
      fprintf(stderr, "differs %s\n", CASE_FILES[i]);
      die("candidate bytes");
    }
    snprintf(cmd, sizeof(cmd), "cmp -s /tmp/pcoc-c-cand/%s.txt '%s/candidates/%s.txt'", CASE_FILES[i], here,
             CASE_FILES[i]);
    if (system_rc(cmd) != 0) die("committed candidates changed");
  }
  /* header_hash is SHA-256d; a single SHA-256 of the fee header differs. */
  snprintf(cmd, sizeof(cmd),
           "awk -F'\\t' '$1==\"hashcash.header_hash\"{print $2}' '%s/candidates/hashcash.txt'", here);
  char hh[80];
  need(run_cmd(cmd, hh, sizeof(hh)) == 0, "header_hash");
  hh[strcspn(hh, "\r\n")] = 0;
  need(strlen(hh) == 64, "header_hash len");
  snprintf(cmd, sizeof(cmd),
           "awk -F'\\t' '$1==\"hashcash.fee_header\"{print $2}' '%s/candidates/hashcash.txt'", here);
  char feehex[200];
  need(run_cmd(cmd, feehex, sizeof(feehex)) == 0, "fee");
  feehex[strcspn(feehex, "\r\n")] = 0;
  uint8_t fee[80], single[32], doubled[32];
  for (int i = 0; i < 80; i++) {
    unsigned v;
    sscanf(feehex + 2 * i, "%02x", &v);
    fee[i] = (uint8_t)v;
  }
  pcoc_sha256(fee, 80, single);
  pcoc_sha256d_80(fee, doubled);
  char sh[65], dh[65];
  static const char *H = "0123456789abcdef";
  for (int i = 0; i < 32; i++) {
    sh[2 * i] = H[single[i] >> 4];
    sh[2 * i + 1] = H[single[i] & 0xf];
    dh[2 * i] = H[doubled[i] >> 4];
    dh[2 * i + 1] = H[doubled[i] & 0xf];
  }
  sh[64] = dh[64] = 0;
  need(strcmp(sh, dh) != 0, "single SHA-256 equals SHA-256d");
  need(strcmp(dh, hh) == 0, "header_hash is not SHA-256d");
  snprintf(cmd, sizeof(cmd), "mkdir -p '%s/objects' && nice -n 10 '%s/build/pcoc' produce --rig s1 -o '%s/objects/O1-S1.cbor' && nice -n 10 '%s/build/pcoc' produce --rig s2 -o '%s/objects/O2-S2.cbor' && nice -n 10 '%s/build/pcoc' produce --rig prod -o '%s/objects/O3-PROD.cbor'",
           here, here, here, here, here, here, here);
  need(system_rc(cmd) == 0, "produce");
  char got[65];
  snprintf(cmd, sizeof(cmd), "%s/objects/O1-S1.cbor", here);
  sha256_file(cmd, got);
  need(strcmp(got, "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246") == 0, "O1");
  snprintf(cmd, sizeof(cmd), "%s/objects/O2-S2.cbor", here);
  sha256_file(cmd, got);
  need(strcmp(got, "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0") == 0, "O2");
  snprintf(cmd, sizeof(cmd), "%s/objects/O3-PROD.cbor", here);
  sha256_file(cmd, got);
  need(strcmp(got, "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7") == 0, "O3");
  snprintf(cmd, sizeof(cmd), "awk -F'\\t' '$1==\"prod.PROD.stage.0.leg.0.legs_le_hex\"{print $2}' '%s/candidates/prod.txt'",
           here);
  char leg[80];
  need(run_cmd(cmd, leg, sizeof(leg)) == 0, "prod leg");
  leg[strcspn(leg, "\r\n")] = 0;
  need(strcmp(leg, "88a04ac5beacb8a7") == 0, "PROD leg");
  printf("PASS a5\n");
  return 0;
}

static void filler_natural(uint64_t h, const uint8_t seed_beacon[32], const uint8_t Ik[18][32],
                           uint8_t out[32]) {
  if (h == 1000) {
    memcpy(out, seed_beacon, 32);
    return;
  }
  if (h >= 1001 && h <= 1018) {
    memcpy(out, Ik[h - 1001], 32);
    return;
  }
  char name[64];
  snprintf(name, sizeof(name), "pcoc-pack-2.2/index/filler/%llu", (unsigned long long)h);
  pcoc_sha256((const uint8_t *)name, strlen(name), out);
}

typedef struct {
  uint8_t seed[32];
  uint8_t Ik[18][32];
} idxctx;

static int index_get(void *ctx, uint64_t h, pcoc_index_doc *doc) {
  idxctx *c = ctx;
  memset(doc, 0, sizeof(*doc));
  if (h < 989 || h > 1018) return 0;
  uint8_t natural[32];
  filler_natural(h, c->seed, c->Ik, natural);
  doc->present = 1;
  doc->has_header = 1;
  doc->header_hash_ok = 1;
  for (int i = 0; i < 32; i++) doc->header_hash_display[i] = natural[31 - i];
  doc->header_height_ok = 1;
  doc->header_height = h;
  doc->header_time_ok = 1;
  doc->header_time = 1700000000ull + 600ull * h;
  doc->has_network = 1;
  memcpy(doc->network, "synthetic", 9);
  doc->network_len = 9;
  return 1;
}

static void load_label_draw(const char *label, uint8_t **out, size_t *n) {
  size_t blocks = 512;
  uint8_t *chain = malloc(32 * blocks);
  pcoc_sha256((const uint8_t *)label, strlen(label), chain);
  for (size_t i = 0; i + 1 < blocks; i++) pcoc_sha256(chain + 32 * i, 32, chain + 32 * (i + 1));
  *out = chain;
  *n = 32 * blocks;
}

static int verify_file(const char *path, const pcoc_profile *prof, idxctx *ix, const uint8_t *draw,
                       size_t draw_len, char *rec, size_t cap) {
  FILE *f = fopen(path, "rb");
  if (!f) return -1;
  fseek(f, 0, SEEK_END);
  long sz = ftell(f);
  rewind(f);
  uint8_t *b = malloc((size_t)sz);
  if (!b || fread(b, 1, (size_t)sz, f) != (size_t)sz) {
    fclose(f);
    free(b);
    return -1;
  }
  fclose(f);
  pcoc_verify_input in;
  memset(&in, 0, sizeof(in));
  in.profile = prof;
  in.object_bytes = b;
  in.object_len = (size_t)sz;
  in.index_get = index_get;
  in.index_ctx = ix;
  in.draw_bytes = draw;
  in.draw_len = draw_len;
  int n = pcoc_verify(&in, rec, cap);
  free(b);
  return n;
}

static int ac_a5x(void) {
  idxctx ix;
  pcoc_sha256((const uint8_t *)"pcoc-pack-2.2/beacon/seed", strlen("pcoc-pack-2.2/beacon/seed"), ix.seed);
  for (int k = 0; k < 18; k++) {
    char name[64];
    snprintf(name, sizeof(name), "pcoc-pack-2.2/beacon/%d", k);
    pcoc_sha256((const uint8_t *)name, strlen(name), ix.Ik[k]);
  }
  uint8_t *draw = NULL;
  size_t draw_len = 0;
  load_label_draw("pcoc-pack-2.2/draw", &draw, &draw_len);
  const char *names[] = {"O1-S1.cbor", "O2-S2.cbor", "O3-PROD.cbor"};
  pcoc_profile profs[3];
  pcoc_profile_small(&profs[0], 64, 32, 65536, 100);
  pcoc_profile_small(&profs[1], 256, 64, 1048576, 1000);
  pcoc_profile_production(&profs[2]);
  const char *builds[] = {"A-sonnet/impl", "B-sol", "C-grok", "D-gemini", "E-kimi", "F-glm/impl"};
  char *rec = malloc(4 << 20);
  char *first = malloc(4 << 20);
  for (int o = 0; o < 3; o++) {
    int have = 0;
    int first_n = 0;
    for (int b = 0; b < 6; b++) {
      char path[1024];
      snprintf(path, sizeof(path), "%s/ref-round2/%s/objects/%s", job, builds[b], names[o]);
      int n = verify_file(path, &profs[o], &ix, draw, draw_len, rec, 4 << 20);
      need(n > 0, path);
      rec[n] = 0;
      need(strstr(rec, "\"not read\"") == NULL, "not read");
      need(strstr(rec, "\"shape\":\"match\"") != NULL, "shape");
      need(strstr(rec, "\"mismatch\"") == NULL, "mismatch");
      if (!have) {
        memcpy(first, rec, (size_t)n + 1);
        first_n = n;
        have = 1;
      } else {
        need(n == first_n && memcmp(first, rec, (size_t)n) == 0, "record differs");
      }
    }
    char own[1024];
    snprintf(own, sizeof(own), "%s/objects/%s", here, names[o]);
    int n = verify_file(own, &profs[o], &ix, draw, draw_len, rec, 4 << 20);
    need(n > 0, own);
    rec[n] = 0;
    need(n == first_n && memcmp(first, rec, (size_t)n) == 0, "own record");
    printf("a5x %s record bytes %d\n", names[o], first_n);
  }
  free(draw);
  free(rec);
  free(first);
  printf("PASS a5x\n");
  return 0;
}

static const char *LEG_NEG[] = {
    "slot_summary_fold", "xor_accumulator_fold", "xor_w_fold",      "s_plus_w_fold",
    "prefold_hoist",     "final_stage_hoist",    "partial_slot_read", "partial_slot_read_2047",
    "f3_shared_terrain", "cached_leg",          NULL};

static int ac_a6(void) {
  /* Values are the committed negatives file, already compared in a5.
   * Here the six-leg check: honest S1 matches; each leg negative mismatches. */
  pcoc_profile pr;
  pcoc_profile_small(&pr, 64, 32, 65536, 100);
  pcoc_sizes sz;
  pcoc_sizes_from_profile(&pr, &sz);
  uint8_t pkA[32], pkB[32];
  const char *ah = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
  const char *bh = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";
  for (int i = 0; i < 32; i++) {
    unsigned v;
    sscanf(ah + 2 * i, "%02x", &v);
    pkA[i] = (uint8_t)v;
    sscanf(bh + 2 * i, "%02x", &v);
    pkB[i] = (uint8_t)v;
  }
  uint8_t seedb[32], beacons[19][32];
  uint64_t heights[19];
  pcoc_sha256((const uint8_t *)"pcoc-pack-2.2/beacon/seed", strlen("pcoc-pack-2.2/beacon/seed"), seedb);
  memcpy(beacons[0], seedb, 32);
  heights[0] = 1000;
  for (int k = 0; k < 18; k++) {
    char name[64];
    snprintf(name, sizeof(name), "pcoc-pack-2.2/beacon/%d", k);
    pcoc_sha256((const uint8_t *)name, strlen(name), beacons[k + 1]);
    heights[k + 1] = 1001 + (uint64_t)k;
  }
  uint8_t pre[64], D[32], DB[32];
  uint64_t PA[4], PB[4];
  pcoc_seed(pkA, seedb, pre, D, PA);
  pcoc_seed(pkB, seedb, pre, DB, PB);
  uint8_t *terA = malloc((size_t)sz.terrain_bytes);
  uint8_t *terB = malloc((size_t)sz.terrain_bytes);
  pcoc_terrain_fill(&sz, PA, terA);
  pcoc_terrain_fill(&sz, PB, terB);
  int legs[18];
  for (int g = 0; g < 18; g++) legs[g] = 1;
  pcoc_object honest;
  need(pcoc_hike_run(&sz, terA, pkA, seedb, beacons, heights, legs, &honest) == 0, "hike");
  idxctx ix;
  memcpy(ix.seed, seedb, 32);
  for (int k = 0; k < 18; k++) memcpy(ix.Ik[k], beacons[k + 1], 32);
  uint8_t draw[48];
  for (int i = 0; i < 6; i++) pcoc_le64((uint64_t)i, draw + 8 * i);
  uint8_t *buf = malloc(1 << 20);
  char *rec = malloc(1 << 20);
  int n = pcoc_object_encode(&honest, buf, 1 << 20);
  need(n > 0, "encode");
  pcoc_verify_input in;
  memset(&in, 0, sizeof(in));
  in.profile = &pr;
  in.object_bytes = buf;
  in.object_len = (size_t)n;
  in.index_get = index_get;
  in.index_ctx = &ix;
  in.draw_bytes = draw;
  in.draw_len = sizeof(draw);
  in.terrain = terA;
  int rn = pcoc_verify(&in, rec, 1 << 20);
  need(rn > 0, "verify honest");
  rec[rn] = 0;
  need(strstr(rec, "\"results\":[\"match\",\"match\",\"match\",\"match\",\"match\",\"match\"]") != NULL,
       "honest six_leg");
  for (int i = 0; LEG_NEG[i]; i++) {
    pcoc_object obj = honest;
    uint64_t neg = 0;
    need(pcoc_neg_leg_value(LEG_NEG[i], &sz, terA, terB, D, &neg) == 0, LEG_NEG[i]);
    pcoc_le64(neg, obj.stages[0].leg_results[0]);
    pcoc_checkpoint(&sz, terA, D, D, neg, obj.stages[0].checkpoints[0]);
    n = pcoc_object_encode(&obj, buf, 1 << 20);
    need(n > 0, "encode neg");
    in.object_len = (size_t)n;
    rn = pcoc_verify(&in, rec, 1 << 20);
    need(rn > 0, "verify neg");
    rec[rn] = 0;
    need(strstr(rec, "\"results\":[\"mismatch\"") != NULL, LEG_NEG[i]);
  }
  free(terA);
  free(terB);
  free(buf);
  free(rec);
  printf("PASS a6\n");
  return 0;
}

static void capture(const char *cmd, int *rc, char *out, size_t oc, char *err, size_t ec) {
  char full[4096];
  snprintf(full, sizeof(full), "%s > /tmp/pcoc-c-out.txt 2> /tmp/pcoc-c-err.txt", cmd);
  *rc = system_rc(full);
  FILE *f = fopen("/tmp/pcoc-c-out.txt", "rb");
  size_t n = 0;
  if (f) {
    n = fread(out, 1, oc - 1, f);
    fclose(f);
  }
  out[n] = 0;
  f = fopen("/tmp/pcoc-c-err.txt", "rb");
  n = 0;
  if (f) {
    n = fread(err, 1, ec - 1, f);
    fclose(f);
  }
  err[n] = 0;
}

static int banned(const char *s) {
  const char *w[] = {"pass", "fail", "accept", "refuse", NULL};
  for (int i = 0; w[i]; i++) {
    if (strstr(s, w[i])) return 1;
  }
  return 0;
}

static int key_before(const char *rec, const char *a, const char *b) {
  const char *pa = strstr(rec, a);
  const char *pb = strstr(rec, b);
  return pa && pb && pa < pb;
}

static int ac_a7(void) {
  char bin[1024], cmd[2048], out[1 << 16], err[4096];
  snprintf(bin, sizeof(bin), "%s/build/pcoc", here);
  int rc;
  snprintf(cmd, sizeof(cmd), "'%s'", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 2, "usage exit");
  FILE *ef = fopen("/tmp/pcoc-c-empty.cbor", "wb");
  fclose(ef);
  snprintf(cmd, sizeof(cmd), "'%s' verify /tmp/pcoc-c-empty.cbor", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0, "empty exit");
  need(strstr(out, "\"malformed cbor\"") != NULL, "empty record");
  need(out[strlen(out) - 1] == '\n', "newline");
  need(key_before(out, "\"profile\":", "\"shape\":"), "order profile");
  need(key_before(out, "\"shape\":", "\"shape_item\":"), "order shape");
  need(key_before(out, "\"shape_item\":", "\"headers\":"), "order headers");
  need(key_before(out, "\"headers\":", "\"seed_value\":"), "order seed");
  need(key_before(out, "\"six_leg\":", "\"six_leg_runs\":"), "order six");
  need(key_before(out, "\"six_leg_runs\":", "\"analysis\":"), "order analysis");
  need(!banned(out) && !banned(err), "banned word");
  snprintf(cmd, sizeof(cmd), "'%s' verify /tmp/pcoc-c-no-such-file", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 1 && out[0] == 0, "missing file");
  FILE *uf = fopen("/tmp/pcoc-c-uint.cbor", "wb");
  fputc(0x00, uf);
  fclose(uf);
  snprintf(cmd, sizeof(cmd), "'%s' verify /tmp/pcoc-c-uint.cbor", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 1 && out[0] == 0, "decode failure uint");
  FILE *mf = fopen("/tmp/pcoc-c-map.cbor", "wb");
  fputc(0xa0, mf);
  fclose(mf);
  snprintf(cmd, sizeof(cmd), "'%s' verify /tmp/pcoc-c-map.cbor", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 1 && out[0] == 0, "map without kind");
  FILE *tf = fopen("/tmp/pcoc-c-trunc.cbor", "wb");
  fputc(0xa1, tf);
  fclose(tf);
  snprintf(cmd, sizeof(cmd), "'%s' verify /tmp/pcoc-c-trunc.cbor", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0 && strstr(out, "\"malformed cbor\"") != NULL, "truncated");
  FILE *tr = fopen("/tmp/pcoc-c-trail.cbor", "wb");
  fputc(0x00, tr);
  fputc(0x00, tr);
  fclose(tr);
  snprintf(cmd, sizeof(cmd), "'%s' verify /tmp/pcoc-c-trail.cbor", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0 && strstr(out, "\"malformed cbor\"") != NULL, "trailing");
  snprintf(cmd, sizeof(cmd), "'%s' verify --profile conformance-small /tmp/pcoc-c-empty.cbor", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 2, "small without integers");
  snprintf(cmd, sizeof(cmd),
           "'%s' verify --profile conformance-small --slot-bytes 64 --sample-bytes 32 "
           "--terrain-bytes 65536 --steps-per-leg 100 /tmp/pcoc-c-empty.cbor",
           bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0, "small exit");
  need(strstr(out, "\"profile\":\"conformance-small\"") != NULL, "profile");
  need(strstr(out, "\"sample_bytes\":32") != NULL, "sample");
  need(strstr(out, "\"slot_bytes\":64") != NULL, "slot");
  need(strstr(out, "\"terrain_bytes\":65536") != NULL, "terrain");
  need(strstr(out, "\"steps_per_leg\":100") != NULL, "steps");
  need(key_before(out, "\"profile\":", "\"sample_bytes\":"), "small order");
  need(key_before(out, "\"steps_per_leg\":", "\"shape\":"), "small shape");
  snprintf(cmd, sizeof(cmd), "'%s' verify --profile production --draw-label pcoc-pack-2.2/draw /tmp/pcoc-c-empty.cbor",
           bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0, "draw label exit");
  need(strcmp(err, "draw: injected label (rig input)\n") == 0, "draw stderr");
  need(!banned(out) && !banned(err), "banned");
  snprintf(cmd, sizeof(cmd), "'%s' verify --profile live --draw-label x /tmp/pcoc-c-empty.cbor", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0 && strcmp(err, "draw: injected label (rig input)\n") == 0, "live label");
  snprintf(cmd, sizeof(cmd), "'%s' verify --profile test /tmp/pcoc-c-empty.cbor", bin);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0 && strstr(out, "\"profile\":\"production\"") != NULL, "test profile");
  printf("PASS a7\n");
  return 0;
}

static int ac_a8(void) {
  char cpu[1 << 16];
  FILE *f = fopen("/proc/cpuinfo", "rb");
  need(f != NULL, "cpuinfo");
  size_t n = fread(cpu, 1, sizeof(cpu) - 1, f);
  fclose(f);
  cpu[n] = 0;
  int has = strstr(cpu, "sha_ni") != NULL;
  char out[4096], err[1024];
  int rc;
  char cmd[1024];
  snprintf(cmd, sizeof(cmd), "'%s/build/pcoc' selftest", here);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0, "selftest");
  printf("%s", out);
  if (has) need(strstr(out, "sha256: sha-ni\n") != NULL, "software on sha-ni host");
  need(strcmp(pcoc_sha256_backend(), has ? "sha-ni" : "software") == 0, "backend");
  printf("PASS a8\n");
  return 0;
}

static int ac_a9(void) {
  char path[1024], ver[64];
  snprintf(path, sizeof(path), "%s/sdk/VERSION", repo);
  FILE *f = fopen(path, "rb");
  need(f && fgets(ver, sizeof(ver), f), "VERSION");
  if (f) fclose(f);
  ver[strcspn(ver, "\r\n")] = 0;
  need(strcmp(ver, "2.2.0") == 0, "VERSION file");
  need(strcmp(pcoc_version(), ver) == 0, "library constant");
  char tag[64], vec[80], out[1024], err[256];
  read_pin(tag, sizeof(tag), vec, sizeof(vec));
  int rc;
  char cmd[1024];
  snprintf(cmd, sizeof(cmd), "cd '%s' && '%s/build/pcoc' --version", repo, here);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0, "version exit");
  need(strstr(out, "2.2.0\n") == out, "version line");
  need(strstr(out, tag) != NULL, "spec tag");
  need(strstr(out, vec) != NULL, "pack pin");
  printf("PASS a9\n");
  return 0;
}

static int ac_a10(void) {
  char cmd[1024];
  snprintf(cmd, sizeof(cmd), "sh '%s/check/negatives.sh'", here);
  need(system_rc(cmd) == 0, "negatives.sh");
  printf("PASS a10\n");
  return 0;
}

static int ac_a12(void) {
  char path[1024];
  snprintf(path, sizeof(path), "%s/src/terrain.c", here);
  FILE *f = fopen(path, "rb");
  need(f != NULL, "terrain.c");
  char *src = malloc(1 << 16);
  size_t n = fread(src, 1, (1 << 16) - 1, f);
  fclose(f);
  src[n] = 0;
  need(strstr(src, "for (uint64_t w = 0; w < wps; w++)") != NULL, "word loop");
  need(strstr(src, "acc = acc + pcoc_mix64(word ^ S)") != NULL, "mix S");
  snprintf(path, sizeof(path), "%s/Makefile", here);
  f = fopen(path, "rb");
  n = fread(src, 1, (1 << 16) - 1, f);
  fclose(f);
  src[n] = 0;
  need(strstr(src, "-march=x86-64") != NULL, "baseline march");
  need(strstr(src, "march=native") == NULL, "native");
  need(strstr(src, "avx2") == NULL && strstr(src, "avx512") == NULL, "simd flag");
  free(src);
  printf("PASS a12\n");
  return 0;
}

static int ac_a13(void) {
  char path[1024];
  snprintf(path, sizeof(path), "%s/DONE.md", job);
  FILE *f = fopen(path, "rb");
  need(f != NULL, "DONE.md");
  char *s = malloc(1 << 16);
  size_t n = fread(s, 1, (1 << 16) - 1, f);
  fclose(f);
  s[n] = 0;
  need(strstr(s, "pcoc_seed") && strstr(s, "pcoc_terrain_fill") && strstr(s, "pcoc_step") &&
           strstr(s, "pcoc_leg_result") && strstr(s, "pcoc_checkpoint") && strstr(s, "pcoc_waypoint") &&
           strstr(s, "pcoc_hashcash_challenge") && strstr(s, "pcoc_verify") &&
           strstr(s, "pcoc_six_leg_draw_rig") && strstr(s, "pcoc_thorough_draw_rig") &&
           strstr(s, "pcoc_leg_rate_floor") && strstr(s, "pcoc_anchor_window"),
       "I2 table");
  need(strstr(s, "Phase C") && strstr(s, "Phase B"), "pending phases");
  free(s);
  printf("PASS a13\n");
  return 0;
}

int main(int argc, char **argv) {
  const char *which = argc > 1 && argv[1][0] ? argv[1] : "all";
  if (!getcwd(here, sizeof(here))) die("cwd");
  const char *er = getenv("PCOC_REPO");
  const char *ej = getenv("PCOC_JOB");
  if (er) snprintf(repo, sizeof(repo), "%s", er);
  else snprintf(repo, sizeof(repo), "%s/../..", here);
  if (ej) snprintf(job, sizeof(job), "%s", ej);
  else snprintf(job, sizeof(job), "%s/../../..", here);
  struct {
    const char *id;
    int (*fn)(void);
  } tab[] = {{"a1", ac_a1}, {"a3", ac_a3}, {"a5", ac_a5}, {"a5x", ac_a5x}, {"a6", ac_a6},
             {"a7", ac_a7}, {"a8", ac_a8}, {"a9", ac_a9}, {"a10", ac_a10}, {"a12", ac_a12},
             {"a13", ac_a13}, {NULL, NULL}};
  int ran = 0;
  for (int i = 0; tab[i].id; i++) {
    if (strcmp(which, "all") == 0 || strcmp(which, tab[i].id) == 0) {
      tab[i].fn();
      ran++;
    }
  }
  if (!ran) die("unknown AC");
  return 0;
}
