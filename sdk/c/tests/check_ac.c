/* Phase A checks. One binary, selected with argv: a1, a3, a5, a5x, a6, a7, a8, a9, a10, a12, a13, or all. */
#define _POSIX_C_SOURCE 200809L
#include "pcoc.h"
#include "negatives.h"

#include <ctype.h>
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <netinet/in.h>
#include <unistd.h>

static char repo[512];
static char here[512]; /* sdk/c, under the repository root */

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

/* Walk parents from the current directory. The root is the first directory
 * that holds both a regular file sdk/VERSION and a directory spec/. */
static int is_repo_root(const char *dir) {
  char ver[600], spec[600];
  struct stat sv, ss;
  snprintf(ver, sizeof(ver), "%s/sdk/VERSION", dir);
  snprintf(spec, sizeof(spec), "%s/spec", dir);
  if (stat(ver, &sv) != 0 || !S_ISREG(sv.st_mode)) return 0;
  if (stat(spec, &ss) != 0 || !S_ISDIR(ss.st_mode)) return 0;
  return 1;
}

static void locate_repo(void) {
  char cur[512];
  if (!getcwd(cur, sizeof(cur))) {
    die("cannot locate repository root (getcwd failed)");
  }
  for (;;) {
    if (is_repo_root(cur)) {
      struct stat sc;
      size_t n = strlen(cur);
      if (n + 1 > sizeof(repo) || n + strlen("/sdk/c") + 1 > sizeof(here)) {
        die("cannot locate repository root (path too long)");
      }
      memcpy(repo, cur, n + 1);
      memcpy(here, cur, n);
      memcpy(here + n, "/sdk/c", strlen("/sdk/c") + 1);
      if (stat(here, &sc) != 0 || !S_ISDIR(sc.st_mode)) {
        die("cannot locate repository root (sdk/c missing under sdk/VERSION and spec/)");
      }
      return;
    }
    if (strcmp(cur, "/") == 0) break;
    char *slash = strrchr(cur, '/');
    if (!slash) break;
    if (slash == cur) {
      cur[1] = 0;
    } else {
      *slash = 0;
    }
  }
  die("cannot locate repository root (no directory holding sdk/VERSION and spec/)");
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

/* Blank rev-list output means HEAD is origin/main, or the branch is already merged. */
static int range_empty(const char *text) {
  for (const char *p = text; *p; p++) {
    if (*p != ' ' && *p != '\t' && *p != '\n' && *p != '\r') return 0;
  }
  return 1;
}

static int ac_a1(void) {
  char cmd[1200], out[1 << 16];
  /* The per-file check proved sdk/c was added on the 2.2 branch. That was
     met when the tree reached main. On main, origin/main..HEAD is empty.
     On a later branch those files exist at the branch cut. Return before
     the walk when origin/main already contains the 2.2 C SDK, or when the
     range is empty.
     sdk/c/include/pcoc.h is the public header the pre-2.2 tree did not have.
     sdk/c/src/br1.c is a later addition and does not mark that tree. */
  const char *marker = "sdk/c/include/pcoc.h";
  snprintf(cmd, sizeof(cmd), "git -C '%s' cat-file -e 'origin/main:%s' 2>/dev/null", repo, marker);
  if (run_cmd(cmd, NULL, 0) == 0) {
    printf("AC-A1: origin/main already contains the 2.2 C SDK (%s); skipping the per-file check\n",
           marker);
    printf("PASS a1\n");
    return 0;
  }
  snprintf(cmd, sizeof(cmd), "git -C '%s' rev-list --reverse origin/main..HEAD", repo);
  need(run_cmd(cmd, out, sizeof(out)) == 0, "git rev-list");
  if (range_empty(out)) {
    printf("AC-A1: origin/main..HEAD is empty; skipping the per-file check\n");
    printf("PASS a1\n");
    return 0;
  }
  /* Added paths versus origin/main. sdk/c has to be among them. The
     integration branch also adds the other SDKs, the consumers, and CI.
     Those paths are in scope. This check only requires the C tree. */
  snprintf(cmd, sizeof(cmd), "git -C '%s' diff --name-only --diff-filter=A origin/main", repo);
  need(run_cmd(cmd, out, sizeof(out)) == 0, "git diff");
  int cfiles = 0;
  char *p = out;
  while (*p) {
    char *nl = strchr(p, '\n');
    if (nl) *nl = 0;
    if (strncmp(p, "sdk/c/", 6) == 0) cfiles++;
    if (!nl) break;
    p = nl + 1;
  }
  need(cfiles > 5, "no added sdk/c files");
  printf("a1 added sdk/c files %d\n", cfiles);
  printf("PASS a1\n");
  return 0;
}

static int ac_a3(void) {
  char tag[64], vec[80];
  read_pin(tag, sizeof(tag), vec, sizeof(vec));
  need(strcmp(tag, "spec-2.3.0") == 0, "tag");
  char path[1024], got[65];
  snprintf(path, sizeof(path), "%s/spec/conformance/pack/manifest.json", repo);
  sha256_file(path, got);
  need(strcmp(got, vec) == 0, "manifest pin");
  printf("a3 pin %s\n", got);
  printf("PASS a3\n");
  return 0;
}

/* Agreed round-2 bytes. SHA-256 of each committed candidate file, and of each
 * committed object. The cross-build comparison lives in the separate gate. */
static const char *CAND_PINS[][2] = {
    {"anchor.txt", "fe58c0b64263d11daa7dcbae7c6daa3e0b0daecb1084b44d47f6f2570da3811a"},
    {"br1.txt", "76b36c948ae0f37ad9dffd0355db3e507f23fddedc203dde4d58d131b212fa99"},
    {"draw.txt", "45821e7ed5d9bbb6b03c686dfc467c623203027a6adf5160414c4e4074a85e0c"},
    {"fold.txt", "2f14b5ef6953e00673c4c7f124f0057cb5498f17d354057639d944e441ffd271"},
    {"hashcash.txt", "92f010b46c15483877343f635164c485500e55527721305bedd0a34def7ca9d5"},
    {"kat-small.txt", "f999097217af06b00a8b879dfc9422263f63cc80b1462d41ff64d09ea866b192"},
    {"negatives.txt", "a236c2966ae46b3e2a9bd83f4414fb4cf8f166106ce0553443aa4bc2e91462d4"},
    {"nl4.txt", "f3fc7424ecd76dbe8ca7976d2fc780bb2a448206e6f116defa4b7cb67fb67a0b"},
    {"prod.txt", "0eb0a567aa73350b80822c86ea3b3ec71ce8f3078bb59012019662cb5850c995"},
    {"rate.txt", "153fbec4acbd3678953f4420d4e5af2f405e866ea16361cc50ab8142cfeef34f"},
    {"seed.txt", "10179736fe1394a3275ad010ad87d5db03214ab3993d94d7cd27ec5c43802002"},
    {"terrain.txt", "f5664d41b9bdc811adc2d7b321f518d33b13808746cf7016837948269019e348"},
    {"test-3leg.txt", "621277600af9a31e40f244ac6010dbe059e602b3b48e9b92a8ad0c0b6d8f7eea"},
    {"unknown-kind.txt", "351fe1fa59d6acd69039ca08704720dba7505155f0d5b26a7bb927321fbcccc2"},
    {"wes.txt", "91c24b4c0e64b5e75bbe5422341cf01e09b25a12833196ac9ed8b898b5f6d768"},
    {NULL, NULL}};

static const char *OBJ_PINS[][2] = {
    {"O1-S1.cbor", "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246"},
    {"O2-S2.cbor", "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0"},
    {"O3-PROD.cbor", "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7"},
    {NULL, NULL}};

static const char *PROD_LEG_PIN = "88a04ac5beacb8a7";

static void pin_fail(const char *kind, const char *name) {
  fprintf(stderr, "FAIL %s pin %s\n", kind, name);
  exit(1);
}

static int files_equal(const char *a, const char *b) {
  FILE *fa = fopen(a, "rb");
  FILE *fb = fopen(b, "rb");
  if (!fa || !fb) {
    if (fa) fclose(fa);
    if (fb) fclose(fb);
    return 0;
  }
  int eq = 1;
  for (;;) {
    unsigned char ba[4096], bb[4096];
    size_t na = fread(ba, 1, sizeof ba, fa);
    size_t nb = fread(bb, 1, sizeof bb, fb);
    if (na != nb || memcmp(ba, bb, na) != 0) {
      eq = 0;
      break;
    }
    if (na == 0) break;
  }
  if (ferror(fa) || ferror(fb)) eq = 0;
  fclose(fa);
  fclose(fb);
  return eq;
}

static void check_candidate_pins(void) {
  for (int i = 0; CAND_PINS[i][0]; i++) {
    char path[1024], got[65];
    snprintf(path, sizeof(path), "%s/candidates/%s", here, CAND_PINS[i][0]);
    if (access(path, R_OK) != 0) pin_fail("candidate", CAND_PINS[i][0]);
    sha256_file(path, got);
    if (strcmp(got, CAND_PINS[i][1]) != 0) pin_fail("candidate", CAND_PINS[i][0]);
  }
}

static void check_object_pins(void) {
  for (int i = 0; OBJ_PINS[i][0]; i++) {
    char path[1024], got[65];
    snprintf(path, sizeof(path), "%s/objects/%s", here, OBJ_PINS[i][0]);
    if (access(path, R_OK) != 0) pin_fail("object", OBJ_PINS[i][0]);
    sha256_file(path, got);
    if (strcmp(got, OBJ_PINS[i][1]) != 0) pin_fail("object", OBJ_PINS[i][0]);
  }
}

static void check_prod_leg(void) {
  char cmd[2048], leg[80];
  snprintf(cmd, sizeof(cmd),
           "awk -F'\\t' '$1==\"prod.PROD.stage.0.leg.0.legs_le_hex\"{print $2}' '%s/candidates/prod.txt'",
           here);
  need(run_cmd(cmd, leg, sizeof(leg)) == 0, "prod leg");
  leg[strcspn(leg, "\r\n")] = 0;
  if (strcmp(leg, PROD_LEG_PIN) != 0) pin_fail("PROD leg", PROD_LEG_PIN);
}

static void regen_candidates(void) {
  char dir[] = "/tmp/pcoc-c-cand-XXXXXX";
  if (!mkdtemp(dir)) die("candidate dir");
  char cmd[2048];
  snprintf(cmd, sizeof(cmd), "nice -n 10 '%s/build/pcoc-gen' '%s'", here, dir);
  if (system_rc(cmd) != 0) die("gen");
  for (int i = 0; CAND_PINS[i][0]; i++) {
    char got[1024], committed[1024];
    snprintf(got, sizeof(got), "%s/%s", dir, CAND_PINS[i][0]);
    snprintf(committed, sizeof(committed), "%s/candidates/%s", here, CAND_PINS[i][0]);
    if (!files_equal(got, committed)) {
      fprintf(stderr, "FAIL candidate bytes %s\n", CAND_PINS[i][0]);
      exit(1);
    }
  }
  snprintf(cmd, sizeof(cmd), "rm -rf '%s'", dir);
  system_rc(cmd);
}

static void produce_objects(void) {
  char dir[] = "/tmp/pcoc-c-obj-XXXXXX";
  if (!mkdtemp(dir)) die("object dir");
  const char *rigs[] = {"s1", "s2", "prod"};
  for (int i = 0; OBJ_PINS[i][0]; i++) {
    char out[1024], committed[1024], cmd[2048];
    snprintf(out, sizeof(out), "%s/%s", dir, OBJ_PINS[i][0]);
    snprintf(cmd, sizeof(cmd), "nice -n 10 '%s/build/pcoc' produce --rig %s -o '%s'", here, rigs[i], out);
    if (system_rc(cmd) != 0) {
      fprintf(stderr, "FAIL produce %s\n", OBJ_PINS[i][0]);
      exit(1);
    }
    snprintf(committed, sizeof(committed), "%s/objects/%s", here, OBJ_PINS[i][0]);
    if (!files_equal(out, committed)) {
      fprintf(stderr, "FAIL object bytes %s\n", OBJ_PINS[i][0]);
      exit(1);
    }
  }
  char cmd[1024];
  snprintf(cmd, sizeof(cmd), "rm -rf '%s'", dir);
  system_rc(cmd);
}

static int br1_nibble(int c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  return -1;
}

static int br1_unhex(const char *s, uint8_t *out, size_t n) {
  if (strlen(s) != n * 2) return -1;
  for (size_t i = 0; i < n; i++) {
    int hi = br1_nibble((unsigned char)s[2 * i]);
    int lo = br1_nibble((unsigned char)s[2 * i + 1]);
    if (hi < 0 || lo < 0) return -1;
    out[i] = (uint8_t)((hi << 4) | lo);
  }
  return 0;
}

static int br1_u32(const char *s, int base, uint32_t *out) {
  char *end = NULL;
  errno = 0;
  unsigned long v = strtoul(s, &end, base);
  if (!s[0] || !end || *end != '\0' || errno == ERANGE || v > 0xfffffffful) return -1;
  *out = (uint32_t)v;
  return 0;
}

static int br1_field(const char *id, const char *field, char *out, size_t cap) {
  char path[1024], key[96];
  int n = snprintf(key, sizeof(key), "br1.%s.%s", id, field);
  if (n < 0 || (size_t)n >= sizeof(key)) return -1;
  snprintf(path, sizeof(path), "%s/candidates/br1.txt", here);
  FILE *f = fopen(path, "rb");
  if (!f) return -1;
  char line[512];
  int found = 0;
  size_t klen = strlen(key);
  while (fgets(line, sizeof(line), f)) {
    if (strncmp(line, key, klen) == 0 && line[klen] == '\t') {
      char *v = line + klen + 1;
      v[strcspn(v, "\r\n")] = 0;
      if (strlen(v) + 1 > cap) break;
      memcpy(out, v, strlen(v) + 1);
      found = 1;
      break;
    }
  }
  fclose(f);
  return found ? 0 : -1;
}

static void check_br1_one(const char *id, unsigned expect) {
  char origin[160], nonce[40], fee[176], plane_s[16];
  char hash_h[80], prev_h[80], wp_h[80];
  char height_s[32], time_s[32], bits_s[16], first_s[32], label[64];
  snprintf(label, sizeof(label), "br1 %s field", id);
  need(br1_field(id, "origin", origin, sizeof(origin)) == 0, label);
  need(br1_field(id, "nonce", nonce, sizeof(nonce)) == 0, label);
  need(br1_field(id, "fee_header", fee, sizeof(fee)) == 0, label);
  need(br1_field(id, "plane", plane_s, sizeof(plane_s)) == 0, label);
  need(br1_field(id, "waypoint", wp_h, sizeof(wp_h)) == 0, label);
  need(br1_field(id, "tip_hash", hash_h, sizeof(hash_h)) == 0, label);
  need(br1_field(id, "tip_previous", prev_h, sizeof(prev_h)) == 0, label);
  need(br1_field(id, "tip_height", height_s, sizeof(height_s)) == 0, label);
  need(br1_field(id, "tip_time", time_s, sizeof(time_s)) == 0, label);
  need(br1_field(id, "tip_bits", bits_s, sizeof(bits_s)) == 0, label);
  need(br1_field(id, "period_first_time", first_s, sizeof(first_s)) == 0, label);
  uint8_t waypoint[32];
  pcoc_br1_tip tip;
  memset(&tip, 0, sizeof(tip));
  need(br1_unhex(wp_h, waypoint, 32) == 0, "br1 waypoint");
  need(br1_unhex(hash_h, tip.hash, 32) == 0, "br1 tip hash");
  need(br1_unhex(prev_h, tip.previous, 32) == 0, "br1 tip previous");
  need(br1_u32(height_s, 10, &tip.height) == 0, "br1 height");
  need(br1_u32(time_s, 10, &tip.time) == 0, "br1 time");
  need(br1_u32(bits_s, 16, &tip.bits) == 0, "br1 bits");
  need(br1_u32(first_s, 10, &tip.period_first_time) == 0, "br1 period");
  int plane = -1;
  if (strcmp(plane_s, "test") == 0) plane = PCOC_BR1_PLANE_TEST;
  if (strcmp(plane_s, "live") == 0) plane = PCOC_BR1_PLANE_LIVE;
  need(plane >= 0, "br1 plane");
  unsigned st = pcoc_br1_status(waypoint, 32, nonce, fee, origin, &tip, plane);
  if (st != expect) {
    fprintf(stderr, "FAIL br1 %s status %u\n", id, st);
    exit(1);
  }
}

static void check_br1_rows(void) {
  need(pcoc_br1_mainnet_next_bits(2015u, 0x1d00ffffu, 1233061996u, 1231006505u) == 0x1d00ffffu,
       "br1 historical nBits");
  need(pcoc_br1_mainnet_next_bits(2015u, 0x1d00ffffu, 1231006505u + 302400u, 1231006505u) ==
           0x1c3fffc0u,
       "br1 harder nBits");
  check_br1_one("current", 204);
  check_br1_one("after_live", 204);
  check_br1_one("display_current", 402);
}

static int ac_a5(void) {
  char cmd[2048];
  check_candidate_pins();
  check_br1_rows();
  check_object_pins();
  check_prod_leg();
  regen_candidates();
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
  produce_objects();
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

/* CASES synthetic index: GET /index/<h> for h = 989 … 1018, on loopback. */
static const uint64_t INDEX_LO = 989;
static const uint64_t INDEX_HI = 1018;

static int hex_nibble(int c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  return -1;
}

static void natural_at(uint64_t h, uint8_t out[32]) {
  char name[64];
  if (h == 1000) {
    snprintf(name, sizeof(name), "pcoc-pack-2.2/beacon/seed");
  } else if (h >= 1001 && h <= 1018) {
    snprintf(name, sizeof(name), "pcoc-pack-2.2/beacon/%llu", (unsigned long long)(h - 1001));
  } else {
    snprintf(name, sizeof(name), "pcoc-pack-2.2/index/filler/%llu", (unsigned long long)h);
  }
  pcoc_sha256((const uint8_t *)name, strlen(name), out);
}

static void index_json(uint64_t h, char *out, size_t cap) {
  uint8_t natural[32];
  natural_at(h, natural);
  char hex[65];
  static const char *H = "0123456789abcdef";
  for (int i = 0; i < 32; i++) {
    uint8_t b = natural[31 - i];
    hex[2 * i] = H[b >> 4];
    hex[2 * i + 1] = H[b & 0xf];
  }
  hex[64] = 0;
  snprintf(out, cap,
           "{\"network\":\"synthetic\",\"header\":{\"hash\":\"%s\",\"height\":%llu,\"time\":%llu}}",
           hex, (unsigned long long)h, (unsigned long long)(1700000000ull + 600ull * h));
}

static void write_full_fd(int fd, const void *p, size_t n) {
  const char *c = p;
  size_t off = 0;
  while (off < n) {
    ssize_t w = write(fd, c + off, n - off);
    if (w < 0) {
      if (errno == EINTR) continue;
      return;
    }
    if (w == 0) return;
    off += (size_t)w;
  }
}

static pid_t index_pid = -1;

static void stop_index(void) {
  if (index_pid > 0) {
    kill(index_pid, SIGTERM);
    waitpid(index_pid, NULL, 0);
    index_pid = -1;
  }
}

static void serve_index(int port_fd) {
  int s = socket(AF_INET, SOCK_STREAM, 0);
  if (s < 0) _exit(1);
  int one = 1;
  setsockopt(s, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
  struct sockaddr_in addr;
  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
  addr.sin_port = htons(0);
  if (bind(s, (struct sockaddr *)&addr, sizeof(addr)) != 0) _exit(1);
  socklen_t alen = sizeof(addr);
  if (getsockname(s, (struct sockaddr *)&addr, &alen) != 0) _exit(1);
  if (listen(s, 16) != 0) _exit(1);
  uint16_t port = ntohs(addr.sin_port);
  write_full_fd(port_fd, &port, sizeof(port));
  close(port_fd);
  for (;;) {
    int c = accept(s, NULL, NULL);
    if (c < 0) {
      if (errno == EINTR) continue;
      _exit(0);
    }
    char req[1024];
    size_t n = 0;
    int complete = 0;
    while (n + 1 < sizeof(req)) {
      ssize_t g = read(c, req + n, sizeof(req) - 1 - n);
      if (g < 0) {
        if (errno == EINTR) continue;
        break;
      }
      if (g == 0) break;
      n += (size_t)g;
      req[n] = 0;
      if (strstr(req, "\r\n\r\n")) {
        complete = 1;
        break;
      }
    }
    req[n] = 0;
    int status = 404;
    char body[256];
    body[0] = 0;
    if (complete) {
      const char *p = strstr(req, "GET /index/");
      if (p) {
        p += 11;
        char *end = NULL;
        unsigned long long v = strtoull(p, &end, 10);
        if (end && end != p && *end == ' ' && v >= INDEX_LO && v <= INDEX_HI) {
          index_json((uint64_t)v, body, sizeof(body));
          status = 200;
        }
      }
    }
    char hdr[192];
    if (status == 200) {
      snprintf(hdr, sizeof(hdr),
               "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: %zu\r\n"
               "Connection: close\r\n\r\n",
               strlen(body));
      write_full_fd(c, hdr, strlen(hdr));
      write_full_fd(c, body, strlen(body));
    } else {
      const char *nf = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
      write_full_fd(c, nf, strlen(nf));
    }
    close(c);
  }
}

static uint16_t start_index(void) {
  static int hooked = 0;
  if (!hooked) {
    atexit(stop_index);
    hooked = 1;
  }
  stop_index();
  int sp[2];
  if (pipe(sp) != 0) die("index pipe");
  pid_t pid = fork();
  if (pid < 0) die("index fork");
  if (pid == 0) {
    close(sp[0]);
    signal(SIGPIPE, SIG_IGN);
    serve_index(sp[1]);
    _exit(0);
  }
  index_pid = pid;
  close(sp[1]);
  uint16_t port = 0;
  size_t got = 0;
  while (got < sizeof(port)) {
    ssize_t g = read(sp[0], (char *)&port + got, sizeof(port) - got);
    if (g < 0) {
      if (errno == EINTR) continue;
      break;
    }
    if (g == 0) break;
    got += (size_t)g;
  }
  close(sp[0]);
  if (got != sizeof(port) || port == 0) die("index listen");
  return port;
}

static int http_get(uint16_t port, const char *path, char *body, size_t cap, int *status) {
  int fd = socket(AF_INET, SOCK_STREAM, 0);
  if (fd < 0) return -1;
  struct sockaddr_in addr;
  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
  addr.sin_port = htons(port);
  if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) != 0) {
    close(fd);
    return -1;
  }
  char req[256];
  int rn = snprintf(req, sizeof(req), "GET %s HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                    path);
  if (rn < 0 || (size_t)rn >= sizeof(req)) {
    close(fd);
    return -1;
  }
  write_full_fd(fd, req, (size_t)rn);
  char buf[2048];
  size_t n = 0;
  while (n + 1 < sizeof(buf)) {
    ssize_t g = read(fd, buf + n, sizeof(buf) - 1 - n);
    if (g < 0) {
      if (errno == EINTR) continue;
      close(fd);
      return -1;
    }
    if (g == 0) break;
    n += (size_t)g;
  }
  close(fd);
  buf[n] = 0;
  if (strncmp(buf, "HTTP/1.", 7) != 0) return -1;
  *status = atoi(buf + 9);
  char *b = strstr(buf, "\r\n\r\n");
  if (!b) return -1;
  b += 4;
  if (strlen(b) + 1 > cap) return -1;
  memcpy(body, b, strlen(b) + 1);
  return 0;
}

static int parse_index_json(const char *body, uint64_t expect_h, pcoc_index_doc *doc) {
  memset(doc, 0, sizeof(*doc));
  if (!strstr(body, "\"network\":\"synthetic\"")) return -1;
  const char *hp = strstr(body, "\"hash\":\"");
  if (!hp) return -1;
  hp += 8;
  if (strlen(hp) < 65 || hp[64] != '"') return -1;
  for (int i = 0; i < 32; i++) {
    int hi = hex_nibble(hp[2 * i]);
    int lo = hex_nibble(hp[2 * i + 1]);
    if (hi < 0 || lo < 0) return -1;
    doc->header_hash_display[i] = (uint8_t)((hi << 4) | lo);
  }
  const char *ht = strstr(body, "\"height\":");
  if (!ht) return -1;
  char *end = NULL;
  unsigned long long height = strtoull(ht + 9, &end, 10);
  if (!end || end == ht + 9 || height != expect_h) return -1;
  const char *tm = strstr(body, "\"time\":");
  if (!tm) return -1;
  unsigned long long t = strtoull(tm + 7, &end, 10);
  if (!end || end == tm + 7) return -1;
  doc->present = 1;
  doc->has_header = 1;
  doc->header_hash_ok = 1;
  doc->header_height_ok = 1;
  doc->header_height = (uint64_t)height;
  doc->header_time_ok = 1;
  doc->header_time = (uint64_t)t;
  doc->has_network = 1;
  memcpy(doc->network, "synthetic", 9);
  doc->network_len = 9;
  return 0;
}

typedef struct {
  uint16_t port;
} index_client;

static int index_get_loopback(void *ctx, uint64_t h, pcoc_index_doc *doc) {
  index_client *c = ctx;
  char path[64], body[512];
  snprintf(path, sizeof(path), "/index/%llu", (unsigned long long)h);
  int status = 0;
  if (http_get(c->port, path, body, sizeof(body), &status) != 0) die("index loopback");
  if (status == 404) {
    memset(doc, 0, sizeof(*doc));
    return 0;
  }
  if (status != 200) die("index loopback status");
  if (parse_index_json(body, h, doc) != 0) die("index document");
  return 1;
}

static void preflight_index(uint16_t port) {
  uint64_t hs[4] = {989, 1000, 1001, 1018};
  for (int i = 0; i < 4; i++) {
    char path[64], body[512];
    snprintf(path, sizeof(path), "/index/%llu", (unsigned long long)hs[i]);
    int status = 0;
    need(http_get(port, path, body, sizeof(body), &status) == 0, "index preflight");
    need(status == 200, "index preflight status");
    pcoc_index_doc doc;
    need(parse_index_json(body, hs[i], &doc) == 0, "index preflight document");
    uint8_t natural[32];
    natural_at(hs[i], natural);
    for (int b = 0; b < 32; b++) {
      need(doc.header_hash_display[b] == natural[31 - b], "index display hash");
    }
    need(doc.header_time == 1700000000ull + 600ull * hs[i], "index time");
    need(strcmp(doc.network, "synthetic") == 0, "index network");
  }
  char body[64];
  int status = 0;
  need(http_get(port, "/index/1", body, sizeof(body), &status) == 0, "index outside");
  need(status == 404, "index outside status");
}

static int verify_file(const char *path, const pcoc_profile *prof, index_client *ix, const uint8_t *draw,
                       size_t draw_len, char *rec, size_t cap) {
  FILE *f = fopen(path, "rb");
  if (!f) return -1;
  if (fseek(f, 0, SEEK_END) != 0) {
    fclose(f);
    return -1;
  }
  long sz = ftell(f);
  if (sz < 0) {
    fclose(f);
    return -1;
  }
  rewind(f);
  uint8_t *b = malloc((size_t)sz + 1);
  if (!b || (sz > 0 && fread(b, 1, (size_t)sz, f) != (size_t)sz)) {
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
  in.index_get = index_get_loopback;
  in.index_ctx = ix;
  in.draw_bytes = draw;
  in.draw_len = draw_len;
  int n = pcoc_verify(&in, rec, cap);
  free(b);
  return n;
}

static int ac_a5x(void) {
  check_candidate_pins();
  check_object_pins();
  check_prod_leg();
  regen_candidates();
  signal(SIGPIPE, SIG_IGN);
  uint16_t port = start_index();
  preflight_index(port);
  index_client ix;
  ix.port = port;
  uint8_t *draw = NULL;
  size_t draw_len = 0;
  load_label_draw("pcoc-pack-2.2/draw", &draw, &draw_len);
  pcoc_profile profs[3];
  pcoc_profile_small(&profs[0], 64, 32, 65536, 100);
  pcoc_profile_small(&profs[1], 256, 64, 1048576, 1000);
  pcoc_profile_production(&profs[2]);
  char *rec = malloc(4 << 20);
  need(rec != NULL, "record buffer");
  for (int o = 0; OBJ_PINS[o][0]; o++) {
    char path[1024];
    snprintf(path, sizeof(path), "%s/objects/%s", here, OBJ_PINS[o][0]);
    printf("a5x %s\n", OBJ_PINS[o][0]);
    fflush(stdout);
    int n = verify_file(path, &profs[o], &ix, draw, draw_len, rec, 4 << 20);
    need(n > 0, OBJ_PINS[o][0]);
    rec[n] = 0;
    need(strstr(rec, "\"not read\"") == NULL, "not read");
    need(strstr(rec, "\"shape\":\"match\"") != NULL, "shape");
    need(strstr(rec, "\"mismatch\"") == NULL, "mismatch");
    printf("a5x %s record bytes %d\n", OBJ_PINS[o][0], n);
    fflush(stdout);
  }
  free(draw);
  free(rec);
  stop_index();
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
  need(strcmp(ver, "2.3.0") == 0, "VERSION file");
  need(strcmp(pcoc_version(), ver) == 0, "library constant");
  char tag[64], vec[80], out[1024], err[256];
  read_pin(tag, sizeof(tag), vec, sizeof(vec));
  int rc;
  char cmd[1024];
  snprintf(cmd, sizeof(cmd), "cd '%s' && '%s/build/pcoc' --version", repo, here);
  capture(cmd, &rc, out, sizeof(out), err, sizeof(err));
  need(rc == 0, "version exit");
  need(strstr(out, "2.3.0\n") == out, "version line");
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
  /* The public header is the Rule I2 surface. RESULTS.md, committed with the
   * candidates, names the phases that are still pending. */
  char path[1024];
  snprintf(path, sizeof(path), "%s/include/pcoc.h", here);
  FILE *f = fopen(path, "rb");
  need(f != NULL, "pcoc.h");
  char *s = malloc(1 << 16);
  size_t n = fread(s, 1, (1 << 16) - 1, f);
  fclose(f);
  s[n] = 0;
  const char *fns[] = {"pcoc_seed(",
                       "pcoc_terrain_fill(",
                       "pcoc_step(",
                       "pcoc_leg_result(",
                       "pcoc_checkpoint(",
                       "pcoc_waypoint(",
                       "pcoc_hashcash_challenge(",
                       "pcoc_verify(",
                       "pcoc_six_leg_draw_rig(",
                       "pcoc_thorough_draw_rig(",
                       "pcoc_leg_rate_floor(",
                       "pcoc_anchor_window(",
                       NULL};
  for (int i = 0; fns[i]; i++) need(strstr(s, fns[i]) != NULL, fns[i]);
  free(s);
  snprintf(path, sizeof(path), "%s/RESULTS.md", here);
  f = fopen(path, "rb");
  need(f != NULL, "RESULTS.md");
  s = malloc(1 << 16);
  n = fread(s, 1, (1 << 16) - 1, f);
  fclose(f);
  s[n] = 0;
  need(strstr(s, "Phase B") && strstr(s, "Phase C"), "pending phases");
  free(s);
  printf("PASS a13\n");
  return 0;
}

int main(int argc, char **argv) {
  const char *which = argc > 1 && argv[1][0] ? argv[1] : "all";
  locate_repo();
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
      fflush(stdout);
      ran++;
    }
  }
  if (!ran) die("unknown AC");
  return 0;
}
