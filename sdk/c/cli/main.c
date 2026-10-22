/* pcoc — verify, selftest, candidates, produce --rig.
 * Stdout of verify is the section 14 record (§18). No admission wording.
 */
#define _POSIX_C_SOURCE 200809L
#include "pcoc.h"

#include <errno.h>
#include <libgen.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void usage(void) {
  fprintf(stderr,
          "usage: pcoc verify [--profile production|conformance-small|test|live]\n"
          "                   [--slot-bytes N] [--sample-bytes N] [--terrain-bytes N]\n"
          "                   [--steps-per-leg N] [--draw-label LABEL] OBJECT\n"
          "       pcoc selftest\n"
          "       pcoc candidates [DIR]\n"
          "       pcoc produce --rig s1|s2|prod -o FILE\n"
          "       pcoc --version\n");
}

static int find_spec_pin(char *out, size_t cap) {
  char cwd[4096];
  if (!getcwd(cwd, sizeof(cwd))) {
    return -1;
  }
  for (;;) {
    snprintf(out, cap, "%s/sdk/SPEC_PIN", cwd);
    if (access(out, R_OK) == 0) {
      return 0;
    }
    snprintf(out, cap, "%s/SPEC_PIN", cwd);
    if (access(out, R_OK) == 0) {
      return 0;
    }
    char *slash = strrchr(cwd, '/');
    if (!slash || slash == cwd) {
      break;
    }
    *slash = 0;
  }
  return -1;
}

static int exec_gen(int argc, char **argv) {
  char self[4096];
  ssize_t n = readlink("/proc/self/exe", self, sizeof(self) - 1);
  if (n < 0) {
    return 1;
  }
  self[n] = 0;
  char *dir = dirname(self);
  char gen[4096];
  snprintf(gen, sizeof(gen), "%s/pcoc-gen", dir);
  char *args[8];
  int k = 0;
  args[k++] = gen;
  for (int i = 0; i < argc; i++) {
    args[k++] = argv[i];
  }
  args[k] = NULL;
  execv(gen, args);
  perror(gen);
  return 1;
}

static void load_beacons(uint8_t seed_beacon[32], uint8_t beacons[19][32], uint64_t heights[19]) {
  pcoc_sha256((const uint8_t *)"pcoc-pack-2.2/beacon/seed", strlen("pcoc-pack-2.2/beacon/seed"),
              seed_beacon);
  memcpy(beacons[0], seed_beacon, 32);
  heights[0] = 1000;
  for (int k = 0; k < 18; k++) {
    char name[64];
    snprintf(name, sizeof(name), "pcoc-pack-2.2/beacon/%d", k);
    pcoc_sha256((const uint8_t *)name, strlen(name), beacons[k + 1]);
    heights[k + 1] = 1000ull + (uint64_t)k + 1;
  }
}

static int unhex32(const char *s, uint8_t out[32]) {
  for (int i = 0; i < 32; i++) {
    unsigned v;
    if (sscanf(s + 2 * i, "%02x", &v) != 1) {
      return -1;
    }
    out[i] = (uint8_t)v;
  }
  return 0;
}

static int cmd_produce(int argc, char **argv) {
  const char *which = NULL;
  const char *out_path = NULL;
  for (int i = 0; i < argc; i++) {
    if (strcmp(argv[i], "--rig") == 0 && i + 1 < argc) {
      which = argv[++i];
    } else if (strcmp(argv[i], "-o") == 0 && i + 1 < argc) {
      out_path = argv[++i];
    } else {
      usage();
      return 2;
    }
  }
  if (!which || !out_path) {
    usage();
    return 2;
  }
  pcoc_profile prof;
  int legs_n = 1;
  if (strcmp(which, "s1") == 0) {
    if (pcoc_profile_small(&prof, 64, 32, 65536, 100) != 0) return 1;
  } else if (strcmp(which, "s2") == 0) {
    if (pcoc_profile_small(&prof, 256, 64, 1048576, 1000) != 0) return 1;
    legs_n = 3;
  } else if (strcmp(which, "prod") == 0) {
    pcoc_profile_production(&prof);
  } else {
    usage();
    return 2;
  }
  pcoc_sizes sizes;
  if (pcoc_sizes_from_profile(&prof, &sizes) != 0) return 1;
  uint8_t pk[32];
  if (unhex32("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9", pk) != 0) {
    return 1;
  }
  uint8_t seed_beacon[32], beacons[19][32];
  uint64_t heights[19];
  load_beacons(seed_beacon, beacons, heights);
  uint8_t pre[64], D[32];
  uint64_t P[4];
  pcoc_seed(pk, seed_beacon, pre, D, P);
  uint8_t *terrain = malloc((size_t)sizes.terrain_bytes);
  if (!terrain) return 1;
  pcoc_terrain_fill(&sizes, P, terrain);
  int legs[18];
  for (int g = 0; g < 18; g++) legs[g] = legs_n;
  pcoc_object obj;
  int rc = pcoc_hike_run(&sizes, terrain, pk, seed_beacon, beacons, heights, legs, &obj);
  free(terrain);
  if (rc != 0) return 1;
  uint8_t *buf = malloc(8 << 20);
  if (!buf) return 1;
  int n = pcoc_object_encode(&obj, buf, 8 << 20);
  if (n < 0) {
    free(buf);
    return 1;
  }
  FILE *f = fopen(out_path, "wb");
  if (!f) {
    free(buf);
    return 1;
  }
  size_t wr = fwrite(buf, 1, (size_t)n, f);
  fclose(f);
  free(buf);
  return wr == (size_t)n ? 0 : 1;
}

static int expand_label(const char *label, uint8_t **out, size_t *out_len) {
  size_t blocks = 4096;
  uint8_t *chain = malloc(32 * blocks);
  if (!chain) return -1;
  pcoc_sha256((const uint8_t *)label, strlen(label), chain);
  for (size_t i = 0; i + 1 < blocks; i++) {
    pcoc_sha256(chain + 32 * i, 32, chain + 32 * (i + 1));
  }
  *out = chain;
  *out_len = 32 * blocks;
  return 0;
}

static int cmd_verify(int argc, char **argv) {
  const char *profile_name = "production";
  uint64_t slot = 0, sample = 0, terrain_b = 0, steps = 0;
  int nsmall = 0;
  const char *draw_label = NULL;
  const char *path = NULL;
  for (int i = 0; i < argc; i++) {
    if (strcmp(argv[i], "--profile") == 0 && i + 1 < argc) {
      profile_name = argv[++i];
    } else if (strcmp(argv[i], "--slot-bytes") == 0 && i + 1 < argc) {
      slot = strtoull(argv[++i], NULL, 10);
      nsmall++;
    } else if (strcmp(argv[i], "--sample-bytes") == 0 && i + 1 < argc) {
      sample = strtoull(argv[++i], NULL, 10);
      nsmall++;
    } else if (strcmp(argv[i], "--terrain-bytes") == 0 && i + 1 < argc) {
      terrain_b = strtoull(argv[++i], NULL, 10);
      nsmall++;
    } else if (strcmp(argv[i], "--steps-per-leg") == 0 && i + 1 < argc) {
      steps = strtoull(argv[++i], NULL, 10);
      nsmall++;
    } else if (strcmp(argv[i], "--draw-label") == 0 && i + 1 < argc) {
      draw_label = argv[++i];
    } else if (argv[i][0] == '-') {
      usage();
      return 2;
    } else if (!path) {
      path = argv[i];
    } else {
      usage();
      return 2;
    }
  }
  if (!path) {
    usage();
    return 2;
  }
  pcoc_profile prof;
  if (strcmp(profile_name, "production") == 0 || strcmp(profile_name, "test") == 0 ||
      strcmp(profile_name, "live") == 0) {
    pcoc_profile_production(&prof);
  } else if (strcmp(profile_name, "conformance-small") == 0) {
    if (nsmall != 4) {
      fprintf(stderr, "conformance-small requires slot-bytes, sample-bytes, terrain-bytes, steps-per-leg\n");
      return 2;
    }
    if (pcoc_profile_small(&prof, slot, sample, terrain_b, steps) != 0) {
      fprintf(stderr, "conformance-small integers are outside section 16.1\n");
      return 2;
    }
  } else {
    usage();
    return 2;
  }
  if (draw_label) {
    fprintf(stderr, "draw: injected label (rig input)\n");
  }
  FILE *f = fopen(path, "rb");
  if (!f) {
    return 1;
  }
  if (fseek(f, 0, SEEK_END) != 0) {
    fclose(f);
    return 1;
  }
  long sz = ftell(f);
  if (sz < 0) {
    fclose(f);
    return 1;
  }
  if (fseek(f, 0, SEEK_SET) != 0) {
    fclose(f);
    return 1;
  }
  uint8_t *bytes = malloc((size_t)sz + 1);
  if (!bytes) {
    fclose(f);
    return 1;
  }
  if (sz > 0 && fread(bytes, 1, (size_t)sz, f) != (size_t)sz) {
    free(bytes);
    fclose(f);
    return 1;
  }
  fclose(f);
  uint8_t *draw = NULL;
  size_t draw_len = 0;
  if (draw_label && expand_label(draw_label, &draw, &draw_len) != 0) {
    free(bytes);
    return 1;
  }
  pcoc_verify_input in;
  memset(&in, 0, sizeof(in));
  in.profile = &prof;
  in.object_bytes = bytes;
  in.object_len = (size_t)sz;
  in.draw_bytes = draw;
  in.draw_len = draw_len;
  char *rec = malloc(4 << 20);
  if (!rec) {
    free(bytes);
    free(draw);
    return 1;
  }
  int n = pcoc_verify(&in, rec, 4 << 20);
  free(bytes);
  free(draw);
  if (n == -2) {
    free(rec);
    return 1;
  }
  if (n < 0) {
    free(rec);
    return 1;
  }
  fwrite(rec, 1, (size_t)n, stdout);
  fputc('\n', stdout);
  free(rec);
  return 0;
}

static int cmd_version(void) {
  char pin[4096];
  const char *path = NULL;
  if (find_spec_pin(pin, sizeof(pin)) == 0) {
    path = pin;
  }
  pcoc_build_info info;
  pcoc_build_info_get(&info, path);
  printf("%s\n%s\n%s\n", info.version, info.spec_tag, info.pack_sha256);
  return 0;
}

int main(int argc, char **argv) {
  if (argc < 2) {
    usage();
    return 2;
  }
  if (strcmp(argv[1], "--version") == 0 || strcmp(argv[1], "version") == 0) {
    return cmd_version();
  }
  if (strcmp(argv[1], "selftest") == 0) {
    char pin[4096];
    const char *path = NULL;
    if (find_spec_pin(pin, sizeof(pin)) == 0) path = pin;
    return pcoc_selftest(stdout, path);
  }
  if (strcmp(argv[1], "candidates") == 0) {
    return exec_gen(argc - 2, argv + 2);
  }
  if (strcmp(argv[1], "produce") == 0) {
    return cmd_produce(argc - 2, argv + 2);
  }
  if (strcmp(argv[1], "verify") == 0) {
    return cmd_verify(argc - 2, argv + 2);
  }
  usage();
  return 2;
}
