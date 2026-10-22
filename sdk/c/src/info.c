/* Rule I5 version, sdk/SPEC_PIN, selftest. */
#include "pcoc.h"

#include <stdio.h>
#include <string.h>

const char *pcoc_version(void) { return PCOC_SDK_VERSION; }

static void copy_pin_line(const char *line, const char *key, char *dst, size_t cap) {
  size_t k = strlen(key);
  if (strncmp(line, key, k) != 0) {
    return;
  }
  const char *v = line + k;
  size_t n = strlen(v);
  while (n && (v[n - 1] == '\n' || v[n - 1] == '\r')) {
    n--;
  }
  if (n >= cap) {
    n = cap - 1;
  }
  memcpy(dst, v, n);
  dst[n] = 0;
}

void pcoc_build_info_get(pcoc_build_info *info, const char *spec_pin_path) {
  memset(info, 0, sizeof(*info));
  snprintf(info->version, sizeof(info->version), "%s", PCOC_SDK_VERSION);
  snprintf(info->spec_tag, sizeof(info->spec_tag), "%s", PCOC_SPEC_TAG);
  snprintf(info->sha256_backend, sizeof(info->sha256_backend), "%s", pcoc_sha256_backend());
  snprintf(info->kernel, sizeof(info->kernel), "scalar");
  info->sha_ni = strcmp(info->sha256_backend, "sha-ni") == 0;
  if (!spec_pin_path) {
    return;
  }
  FILE *f = fopen(spec_pin_path, "rb");
  if (!f) {
    return;
  }
  char line[256];
  while (fgets(line, sizeof(line), f)) {
    if (line[0] == '#') {
      continue;
    }
    copy_pin_line(line, "tag=", info->spec_tag, sizeof(info->spec_tag));
    copy_pin_line(line, "vector-sha256=", info->pack_sha256, sizeof(info->pack_sha256));
  }
  fclose(f);
}

int pcoc_selftest(FILE *out, const char *spec_pin_path) {
  pcoc_build_info info;
  pcoc_build_info_get(&info, spec_pin_path);
  if (!out) {
    out = stdout;
  }
  fprintf(out, "sdk: %s\n", info.version);
  fprintf(out, "spec: %s\n", info.spec_tag);
  fprintf(out, "pack: %s\n", info.pack_sha256[0] ? info.pack_sha256 : "");
  fprintf(out, "kernel: %s\n", info.kernel);
  fprintf(out, "sha256: %s\n", info.sha256_backend);
  /* §7.4 abc, so a broken compress cannot report a backend quietly. */
  static const uint8_t abc[32] = {0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40,
                                  0xde, 0x5d, 0xae, 0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17,
                                  0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad};
  uint8_t got[32];
  pcoc_sha256((const uint8_t *)"abc", 3, got);
  fprintf(out, "kat_abc: %s\n", memcmp(got, abc, 32) == 0 ? "match" : "mismatch");
  return 0;
}
