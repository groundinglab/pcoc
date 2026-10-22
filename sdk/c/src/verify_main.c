/* Verify CLI, SPEC §18.
 * Exit 0: a record was produced. Exit 1: the object file cannot be read
 * or decoded. Exit 2: usage. Stdout is the section 14 record, or the
 * Rule A5 record, with one trailing newline. No admission wording.
 */
#include "pcoc.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void usage(void) {
  fprintf(stderr,
          "usage: pcoc-verify [--profile production|conformance-small] "
          "[--slot-bytes N --sample-bytes N --terrain-bytes N --steps-per-leg N] "
          "[--draw-bytes HEX] OBJECT\n");
}

static int hex_nibble(char c) {
  if (c >= '0' && c <= '9') {
    return c - '0';
  }
  if (c >= 'a' && c <= 'f') {
    return c - 'a' + 10;
  }
  if (c >= 'A' && c <= 'F') {
    return c - 'A' + 10;
  }
  return -1;
}

int main(int argc, char **argv) {
  const char *profile_name = "production";
  uint64_t slot = 0, sample = 0, terrain = 0, steps = 0;
  int nsmall = 0;
  const char *draw_hex = NULL;
  const char *path = NULL;
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--profile") == 0 && i + 1 < argc) {
      profile_name = argv[++i];
    } else if (strcmp(argv[i], "--slot-bytes") == 0 && i + 1 < argc) {
      slot = strtoull(argv[++i], NULL, 10);
      nsmall++;
    } else if (strcmp(argv[i], "--sample-bytes") == 0 && i + 1 < argc) {
      sample = strtoull(argv[++i], NULL, 10);
      nsmall++;
    } else if (strcmp(argv[i], "--terrain-bytes") == 0 && i + 1 < argc) {
      terrain = strtoull(argv[++i], NULL, 10);
      nsmall++;
    } else if (strcmp(argv[i], "--steps-per-leg") == 0 && i + 1 < argc) {
      steps = strtoull(argv[++i], NULL, 10);
      nsmall++;
    } else if (strcmp(argv[i], "--draw-bytes") == 0 && i + 1 < argc) {
      draw_hex = argv[++i];
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
      fprintf(stderr, "conformance-small requires all four integers\n");
      return 2;
    }
    if (pcoc_profile_small(&prof, slot, sample, terrain, steps) != 0) {
      fprintf(stderr, "conformance-small integers are outside section 16.1\n");
      return 2;
    }
  } else {
    usage();
    return 2;
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
  rewind(f);
  uint8_t *bytes = malloc((size_t)sz + 1);
  if (!bytes || fread(bytes, 1, (size_t)sz, f) != (size_t)sz) {
    free(bytes);
    fclose(f);
    return 1;
  }
  fclose(f);

  uint8_t *draw = NULL;
  size_t draw_len = 0;
  if (draw_hex) {
    size_t n = strlen(draw_hex);
    if (n % 2) {
      free(bytes);
      return 2;
    }
    draw_len = n / 2;
    draw = malloc(draw_len);
    for (size_t i = 0; i < draw_len; i++) {
      int hi = hex_nibble(draw_hex[2 * i]);
      int lo = hex_nibble(draw_hex[2 * i + 1]);
      if (hi < 0 || lo < 0) {
        free(bytes);
        free(draw);
        return 2;
      }
      draw[i] = (uint8_t)((hi << 4) | lo);
    }
  }

  pcoc_verify_input in = {0};
  in.profile = &prof;
  in.object_bytes = bytes;
  in.object_len = (size_t)sz;
  in.draw_bytes = draw;
  in.draw_len = draw_len;
  in.run_six_leg = 1;
  char *rec = malloc(1 << 20);
  int n = pcoc_verify(&in, rec, 1 << 20);
  if (n < 0) {
    free(bytes);
    free(draw);
    free(rec);
    return 1;
  }
  fwrite(rec, 1, (size_t)n, stdout);
  fputc('\n', stdout);
  free(bytes);
  free(draw);
  free(rec);
  return 0;
}
