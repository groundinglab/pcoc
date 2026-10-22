/* Lean tests for the acceptance criteria. SPEC 2.2 C core. */
#define _POSIX_C_SOURCE 200809L
#include "pcoc.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int g_fail;

static void expect(int cond, const char *name) {
  if (!cond) {
    fprintf(stderr, "FAIL %s\n", name);
    g_fail++;
  } else {
    printf("PASS %s\n", name);
  }
}

static int hex_nibble(char c) {
  if (c >= '0' && c <= '9') {
    return c - '0';
  }
  if (c >= 'a' && c <= 'f') {
    return c - 'a' + 10;
  }
  return -1;
}

static void unhex(const char *s, uint8_t *o, size_t n) {
  for (size_t i = 0; i < n; i++) {
    o[i] = (uint8_t)((hex_nibble(s[2 * i]) << 4) | hex_nibble(s[2 * i + 1]));
  }
}

/* AC-C2. Read sdk/SPEC_PIN and hash the manifest. */
static void test_pin(void) {
  FILE *f = fopen("../SPEC_PIN", "rb");
  expect(f != NULL, "pin.file");
  if (!f) {
    return;
  }
  char line[256];
  char want[65] = {0};
  while (fgets(line, sizeof(line), f)) {
    if (strncmp(line, "vector-sha256=", 14) == 0) {
      memcpy(want, line + 14, 64);
      want[64] = 0;
    }
  }
  fclose(f);
  FILE *m = fopen("../../spec/conformance/pack/manifest.json", "rb");
  expect(m != NULL, "pin.manifest");
  if (!m) {
    return;
  }
  fseek(m, 0, SEEK_END);
  long sz = ftell(m);
  rewind(m);
  uint8_t *buf = malloc((size_t)sz);
  if (!buf || fread(buf, 1, (size_t)sz, m) != (size_t)sz) {
    free(buf);
    fclose(m);
    expect(0, "pin.read");
    return;
  }
  fclose(m);
  uint8_t dig[32];
  pcoc_sha256(buf, (size_t)sz, dig);
  char hx[65];
  static const char *H = "0123456789abcdef";
  for (int i = 0; i < 32; i++) {
    hx[2 * i] = H[dig[i] >> 4];
    hx[2 * i + 1] = H[dig[i] & 0xf];
  }
  hx[64] = 0;
  expect(strcmp(hx, want) == 0, "pin.vector-sha256");
  /* One flipped byte must not match. */
  buf[0] ^= 0x01;
  pcoc_sha256(buf, (size_t)sz, dig);
  for (int i = 0; i < 32; i++) {
    hx[2 * i] = H[dig[i] >> 4];
    hx[2 * i + 1] = H[dig[i] & 0xf];
  }
  expect(strcmp(hx, want) != 0, "pin.mismatch-on-one-byte");
  free(buf);
}

static void test_version(void) {
  expect(strcmp(pcoc_version(), "2.2.0") == 0, "version.2.2.0");
}

static void test_section74(void) {
  uint8_t out[32];
  pcoc_sha256((const uint8_t *)"abc", 3, out);
  uint8_t want[32];
  unhex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", want, 32);
  expect(memcmp(out, want, 32) == 0, "sha256.abc");
  uint8_t pre[20];
  memcpy(pre, "pcoc-walk-v1", 12);
  memset(pre + 12, 0, 8);
  pcoc_sha256(pre, 20, out);
  unhex("4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533", want, 32);
  expect(memcmp(out, want, 32) == 0, "sha256.seal0");
  expect(pcoc_u64_le(out) == 0x76042423afbd1648ull, "sha256.S-prime");
  uint8_t pre2[64];
  memset(pre2, 0x11, 32);
  memset(pre2 + 32, 0x22, 32);
  pcoc_sha256(pre2, 64, out);
  unhex("5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c", want, 32);
  expect(memcmp(out, want, 32) == 0, "seed.7.4");
  uint64_t P[4];
  for (int k = 0; k < 4; k++) {
    P[k] = pcoc_u64_le(out + 8 * k);
  }
  expect(P[0] == 0x545dfe297dc78951ull, "lane.P0");
  expect(pcoc_nl4_base(0, 0, P) == 0xc7582082a632c2c8ull, "nl4.i0j0");
}

/* Build one honest S1 stage-0 object: 18 stages, 1 leg each, empty paths. */
static int build_s1(pcoc_object *obj, uint8_t **terrain_out, pcoc_sizes *sizes) {
  memset(obj, 0, sizeof(*obj));
  memcpy(obj->kind, PCOC_KIND, strlen(PCOC_KIND));
  obj->kind_len = strlen(PCOC_KIND);
  memcpy(obj->fill, PCOC_FILL, 4);
  obj->fill_len = 4;
  pcoc_profile p;
  pcoc_profile_small(&p, 64, 32, 65536, 100);
  pcoc_sizes_from_profile(&p, sizes);
  obj->slot_bytes = sizes->slot_bytes;
  obj->n_slots = sizes->n_slots;
  obj->sample_bytes = sizes->sample_bytes;
  obj->steps_per_leg = sizes->steps_per_leg;
  unhex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9", obj->public_key,
        32);
  pcoc_sha256((const uint8_t *)"pcoc-pack-2.2/beacon/seed", 24, obj->seed_beacon);
  uint8_t pre[64], D[32];
  uint64_t P[4];
  pcoc_seed(obj->public_key, obj->seed_beacon, pre, D, P);
  uint8_t *terrain = malloc((size_t)sizes->terrain_bytes);
  pcoc_terrain_fill(sizes, P, terrain);
  uint8_t beacons[19][32];
  memcpy(beacons[0], obj->seed_beacon, 32);
  for (int k = 0; k < 18; k++) {
    char name[64];
    snprintf(name, sizeof(name), "pcoc-pack-2.2/beacon/%d", k);
    pcoc_sha256((const uint8_t *)name, strlen(name), beacons[k + 1]);
  }
  uint8_t waypoint[32];
  memcpy(waypoint, D, 32);
  uint8_t beacon[32];
  memcpy(beacon, beacons[0], 32);
  for (int g = 0; g < 18; g++) {
    pcoc_stage *st = &obj->stages[g];
    memcpy(st->beacon, beacon, 32);
    st->beacon_height = (uint64_t)g;
    memcpy(st->waypoint, waypoint, 32);
    st->leg_count = 1;
    st->n_checkpoints = 1;
    st->n_leg_results = 1;
    st->has_path = g > 0;
    st->path_len = 0;
    uint64_t lr = pcoc_leg_result(sizes, terrain, waypoint);
    pcoc_le64(lr, st->leg_results[0]);
    pcoc_checkpoint(sizes, terrain, waypoint, waypoint, lr, st->checkpoints[0]);
    if (g < 17) {
      uint8_t next[32];
      pcoc_waypoint(sizes, terrain, beacons[g + 1], waypoint, st->checkpoints[0], 1, next);
      memcpy(waypoint, next, 32);
      memcpy(beacon, beacons[g + 1], 32);
    }
  }
  memcpy(obj->last_interrupt, beacons[18], 32);
  obj->last_interrupt_height = 18;
  obj->n_stages = 18;
  *terrain_out = terrain;
  return 0;
}

static void test_roundtrip(void) {
  pcoc_object obj;
  uint8_t *terrain = NULL;
  pcoc_sizes sizes;
  build_s1(&obj, &terrain, &sizes);
  uint8_t *enc = malloc(1 << 20);
  int n = pcoc_object_encode(&obj, enc, 1 << 20);
  expect(n > 0, "encode.s1");
  pcoc_object back;
  size_t used = 0;
  int rc = pcoc_object_decode(enc, (size_t)n, &back, &used);
  expect(rc == 0, "decode.s1");
  uint8_t *enc2 = malloc(1 << 20);
  int n2 = pcoc_object_encode(&back, enc2, 1 << 20);
  expect(n2 == n && memcmp(enc, enc2, (size_t)n) == 0, "roundtrip.byte-identical");
  free(enc);
  free(enc2);
  free(terrain);
}

static void test_unknown(void) {
  const char *labels[] = {"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"};
  for (int i = 0; i < 4; i++) {
    /* A map whose only key is kind, definite CBOR. */
    const char *lab = labels[i];
    size_t ln = strlen(lab);
    uint8_t item[128];
    size_t n = 0;
    item[n++] = 0xa1; /* map of 1 */
    item[n++] = (uint8_t)(0x60 + 4);
    memcpy(item + n, "kind", 4);
    n += 4;
    if (ln < 24) {
      item[n++] = (uint8_t)(0x60 + ln);
    } else {
      item[n++] = 0x78;
      item[n++] = (uint8_t)ln;
    }
    memcpy(item + n, lab, ln);
    n += ln;
    pcoc_profile p;
    pcoc_profile_small(&p, 64, 32, 65536, 100);
    pcoc_verify_input in = {0};
    in.profile = &p;
    in.object_bytes = item;
    in.object_len = n;
    char rec[256];
    int rn = pcoc_verify(&in, rec, sizeof(rec));
    expect(rn > 0, "a5.length");
    char expect_rec[256];
    pcoc_unknown_kind_record(lab, ln, expect_rec, sizeof(expect_rec));
    expect(rn == (int)strlen(expect_rec) && memcmp(rec, expect_rec, (size_t)rn) == 0, "a5.record");
    /* Nothing else: no shape key. */
    expect(strstr(rec, "shape") == NULL, "a5.no-shape");
  }
}

/* Six-leg: honest match, then each leg-level negative mismatches.
 * These folds live in the test, the same constructions as src/negatives.c.
 * They are not called by the SDK. */
static uint64_t seal_of(uint64_t acc) {
  uint8_t pre[20];
  memcpy(pre, PCOC_SEAL_DOMAIN, 12);
  pcoc_le64(acc, pre + 12);
  uint8_t E[32];
  pcoc_sha256(pre, 20, E);
  return pcoc_u64_le(E);
}

static uint64_t fold_slot_summary(const pcoc_sizes *z, const uint8_t *t, uint64_t S) {
  uint64_t i = S % z->n_slots;
  const uint8_t *slot = t + (size_t)(i * z->slot_bytes);
  uint64_t x = 0;
  for (uint64_t w = 0; w < z->words_per_slot; w++) {
    x ^= pcoc_u64_le(slot + 8 * w) ^ w;
  }
  return seal_of(S ^ x);
}

static uint64_t fold_xor_acc(const pcoc_sizes *z, const uint8_t *t, uint64_t S) {
  uint64_t i = S % z->n_slots;
  const uint8_t *slot = t + (size_t)(i * z->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < z->words_per_slot; w++) {
    acc ^= pcoc_mix64(pcoc_u64_le(slot + 8 * w) ^ w ^ S);
  }
  return seal_of(acc);
}

static uint64_t fold_xor_w(const pcoc_sizes *z, const uint8_t *t, uint64_t S) {
  uint64_t i = S % z->n_slots;
  const uint8_t *slot = t + (size_t)(i * z->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < z->words_per_slot; w++) {
    acc += pcoc_mix64(pcoc_u64_le(slot + 8 * w) ^ w ^ S);
  }
  return seal_of(acc);
}

static uint64_t fold_s_plus_w(const pcoc_sizes *z, const uint8_t *t, uint64_t S) {
  uint64_t i = S % z->n_slots;
  const uint8_t *slot = t + (size_t)(i * z->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < z->words_per_slot; w++) {
    acc += pcoc_mix64(pcoc_u64_le(slot + 8 * w) ^ (S + w));
  }
  return seal_of(acc);
}

static uint64_t fold_prefold(const pcoc_sizes *z, const uint8_t *t, uint64_t S) {
  uint64_t i = S % z->n_slots;
  const uint8_t *slot = t + (size_t)(i * z->slot_bytes);
  uint64_t acc = 0;
  for (uint64_t w = 0; w < z->words_per_slot; w++) {
    acc += pcoc_mix64(pcoc_u64_le(slot + 8 * w) ^ w);
  }
  return seal_of(acc ^ S);
}

static uint64_t fold_partial(const pcoc_sizes *z, const uint8_t *t, uint64_t S) {
  uint64_t i = S % z->n_slots;
  const uint8_t *slot = t + (size_t)(i * z->slot_bytes);
  uint64_t acc = 0;
  uint64_t lim = z->words_per_slot / 2;
  for (uint64_t w = 0; w < lim; w++) {
    acc += pcoc_mix64(pcoc_u64_le(slot + 8 * w) ^ S);
  }
  return seal_of(acc);
}

static uint64_t fold_partial_2047(const pcoc_sizes *z, const uint8_t *t, uint64_t S) {
  uint64_t i = S % z->n_slots;
  const uint8_t *slot = t + (size_t)(i * z->slot_bytes);
  uint64_t acc = 0;
  uint64_t lim = 2047;
  if (lim >= z->words_per_slot) {
    lim = z->words_per_slot - 1;
  }
  for (uint64_t w = 0; w < lim; w++) {
    acc += pcoc_mix64(pcoc_u64_le(slot + 8 * w) ^ S);
  }
  return seal_of(acc);
}

static void test_six_leg(void) {
  pcoc_object obj;
  uint8_t *terrain = NULL;
  pcoc_sizes sizes;
  build_s1(&obj, &terrain, &sizes);
  uint8_t *enc = malloc(1 << 20);
  int n = pcoc_object_encode(&obj, enc, 1 << 20);
  pcoc_profile p;
  pcoc_profile_small(&p, 64, 32, 65536, 100);
  /* Deterministic draw bytes: zeros, which are below any limit for 18. */
  /* Six little-endian integers 0..5. Each is below the draw limit for 18 legs. */
  uint8_t draw[48];
  memset(draw, 0, sizeof(draw));
  for (int i = 0; i < 6; i++) {
    draw[8 * i] = (uint8_t)i;
  }
  pcoc_verify_input in = {0};
  in.profile = &p;
  in.object_bytes = enc;
  in.object_len = (size_t)n;
  in.draw_bytes = draw;
  in.draw_len = sizeof(draw);
  in.terrain = terrain;
  in.run_six_leg = 1;
  char *rec = malloc(1 << 20);
  int rn = pcoc_verify(&in, rec, 1 << 20);
  expect(rn > 0 && strstr(rec, "\"shape\":\"match\"") != NULL, "sixleg.shape-match");
  if (rn > 0 && strstr(rec, "\"shape\":\"match\"") == NULL) {
    const char *s = strstr(rec, "\"shape\"");
    if (s) {
      fwrite(s, 1, 180, stderr);
      fputc('\n', stderr);
    }
  }
  expect(rn > 0 && strstr(rec, "\"results\":[\"match\",\"match\",\"match\",\"match\",\"match\",\"match\"]") != NULL,
         "sixleg.honest-match");

  typedef uint64_t (*sfn)(const pcoc_sizes *, const uint8_t *, uint64_t);
  struct {
    const char *name;
    sfn fn;
  } negs[] = {
      {"slot_summary_fold", fold_slot_summary}, {"xor_accumulator_fold", fold_xor_acc},
      {"xor_w_fold", fold_xor_w},               {"s_plus_w_fold", fold_s_plus_w},
      {"prefold_hoist", fold_prefold},          {"partial_slot_read", fold_partial},
      {"partial_slot_read_2047", fold_partial_2047},
  };
  for (unsigned ni = 0; ni < sizeof(negs) / sizeof(negs[0]); ni++) {
    pcoc_object bad = obj;
    uint64_t S = pcoc_u64_le(bad.stages[0].waypoint);
    for (uint64_t t = 0; t < sizes.steps_per_leg; t++) {
      S = negs[ni].fn(&sizes, terrain, S);
    }
    pcoc_le64(S, bad.stages[0].leg_results[0]);
    int nb = pcoc_object_encode(&bad, enc, 1 << 20);
    in.object_bytes = enc;
    in.object_len = (size_t)nb;
    rn = pcoc_verify(&in, rec, 1 << 20);
    char msg[96];
    snprintf(msg, sizeof(msg), "sixleg.%s-mismatch", negs[ni].name);
    int saw = rn > 0 && strstr(rec, "\"mismatch\"") != NULL;
    expect(saw, msg);
  }
  free(rec);
  free(enc);
  free(terrain);
}

static void test_terrain_negative(void) {
  /* A squeezed seed (all four lanes one pin) does not hash to the honest
   * S1 terrain. The negative is computed here, not by the SDK fill. */
  pcoc_profile p;
  pcoc_profile_small(&p, 64, 32, 65536, 100);
  pcoc_sizes s;
  pcoc_sizes_from_profile(&p, &s);
  uint8_t pk[32], beacon[32], pre[64], D[32];
  unhex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9", pk, 32);
  pcoc_sha256((const uint8_t *)"pcoc-pack-2.2/beacon/seed", 24, beacon);
  uint64_t P[4];
  pcoc_seed(pk, beacon, pre, D, P);
  uint8_t *honest = malloc((size_t)s.terrain_bytes);
  uint8_t *sq = malloc((size_t)s.terrain_bytes);
  pcoc_terrain_fill(&s, P, honest);
  uint64_t pin = P[0] ^ P[1] ^ P[2] ^ P[3];
  uint64_t Q[4] = {pin, pin, pin, pin};
  pcoc_terrain_fill(&s, Q, sq);
  uint8_t dh[32], ds[32];
  pcoc_sha256(honest, (size_t)s.terrain_bytes, dh);
  pcoc_sha256(sq, (size_t)s.terrain_bytes, ds);
  expect(memcmp(dh, ds, 32) != 0, "terrain.squeezed-not-honest");
  free(honest);
  free(sq);
}

static int files_equal(const char *a, const char *b) {
  FILE *fa = fopen(a, "rb");
  FILE *fb = fopen(b, "rb");
  if (!fa || !fb) {
    if (fa) {
      fclose(fa);
    }
    if (fb) {
      fclose(fb);
    }
    return 0;
  }
  int ok = 1;
  while (ok) {
    int ca = fgetc(fa);
    int cb = fgetc(fb);
    if (ca != cb) {
      ok = 0;
    }
    if (ca == EOF) {
      break;
    }
  }
  fclose(fa);
  fclose(fb);
  return ok;
}

static void test_candidates_stable(void) {
  /* Re-run the generator into a temp dir and compare. PROD dominates the
   * time, so this test is the AC-C4 check and is expected to be slow. */
  int rc = system("rm -rf /tmp/pcoc-c-cand && mkdir -p /tmp/pcoc-c-cand && "
                  "nice -n 10 ./build/gen-candidates /tmp/pcoc-c-cand");
  expect(rc == 0, "candidates.rerun");
  const char *names[] = {"seed.txt",        "nl4.txt",   "terrain.txt", "fold.txt",
                         "kat-small.txt",   "test-3leg.txt", "prod.txt", "wes.txt",
                         "rate.txt",        "anchor.txt", "draw.txt",    "unknown-kind.txt",
                         "hashcash.txt",    "negatives.txt"};
  for (unsigned i = 0; i < sizeof(names) / sizeof(names[0]); i++) {
    char a[256], b[256], msg[128];
    snprintf(a, sizeof(a), "candidates/%s", names[i]);
    snprintf(b, sizeof(b), "/tmp/pcoc-c-cand/%s", names[i]);
    snprintf(msg, sizeof(msg), "candidates.stable.%s", names[i]);
    expect(files_equal(a, b), msg);
  }
}

int main(void) {
  test_version();
  test_section74();
  test_pin();
  test_roundtrip();
  test_unknown();
  test_six_leg();
  test_terrain_negative();
  if (getenv("PCOC_SKIP_CANDIDATES") == NULL) {
    test_candidates_stable();
  }
  if (g_fail) {
    fprintf(stderr, "%d failed\n", g_fail);
    return 1;
  }
  printf("all passed\n");
  return 0;
}
