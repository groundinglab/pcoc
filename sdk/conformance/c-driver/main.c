/* Conformance driver for the C verifier.
 * The verify CLI leaves the index callback NULL (sdk/c/cli/main.c). This
 * program calls the same pcoc_verify, and fills pcoc_index_doc from
 * GET http://127.0.0.1/index/{height}. Loopback only.
 */
#define _POSIX_C_SOURCE 200809L
#include "pcoc.h"

#include <arpa/inet.h>
#include <ctype.h>
#include <netinet/in.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#define H_LO 989u
#define H_HI 1018u
#define H_N (H_HI - H_LO + 1)

typedef struct {
  int port;
  uint8_t *body[H_N];
  size_t len[H_N];
  int fetched[H_N];
} index_cache;

static int hex_nibble(int c) {
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  return -1;
}

static int write_all(int fd, const char *p, size_t n) {
  while (n) {
    ssize_t w = write(fd, p, n);
    if (w < 0) return -1;
    p += w;
    n -= (size_t)w;
  }
  return 0;
}

/* Returns 1 and a body on HTTP 200, 0 when the height is not a document, -1 on transport failure. */
static int http_get(int port, uint64_t height, uint8_t **out, size_t *out_len) {
  int fd = socket(AF_INET, SOCK_STREAM, 0);
  if (fd < 0) return -1;
  struct sockaddr_in addr;
  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_port = htons((uint16_t)port);
  if (inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr) != 1) {
    close(fd);
    return -1;
  }
  if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) != 0) {
    close(fd);
    return -1;
  }
  char req[192];
  int rn = snprintf(req, sizeof(req),
                    "GET /index/%llu HTTP/1.0\r\nHost: 127.0.0.1:%d\r\nConnection: close\r\n\r\n",
                    (unsigned long long)height, port);
  if (rn < 0 || write_all(fd, req, (size_t)rn) != 0) {
    close(fd);
    return -1;
  }
  size_t cap = 2048, len = 0;
  uint8_t *buf = malloc(cap);
  if (!buf) {
    close(fd);
    return -1;
  }
  for (;;) {
    if (len + 512 > cap) {
      if (cap > (1u << 20)) {
        free(buf);
        close(fd);
        return -1;
      }
      cap *= 2;
      uint8_t *nbuf = realloc(buf, cap);
      if (!nbuf) {
        free(buf);
        close(fd);
        return -1;
      }
      buf = nbuf;
    }
    ssize_t r = read(fd, buf + len, cap - len);
    if (r < 0) {
      free(buf);
      close(fd);
      return -1;
    }
    if (r == 0) break;
    len += (size_t)r;
  }
  close(fd);
  size_t sep = 0;
  int found = 0;
  for (size_t i = 0; i + 3 < len; i++) {
    if (buf[i] == '\r' && buf[i + 1] == '\n' && buf[i + 2] == '\r' && buf[i + 3] == '\n') {
      sep = i;
      found = 1;
      break;
    }
  }
  if (!found || len < 12 || memcmp(buf, "HTTP/1.", 7) != 0) {
    free(buf);
    return -1;
  }
  const uint8_t *sp = memchr(buf, ' ', sep < 16 ? sep : 16);
  if (!sp || (size_t)(sp - buf) + 4 > sep || memcmp(sp + 1, "200", 3) != 0) {
    free(buf);
    return 0;
  }
  size_t blen = len - (sep + 4);
  uint8_t *body = malloc(blen + 1);
  if (!body) {
    free(buf);
    return -1;
  }
  memcpy(body, buf + sep + 4, blen);
  body[blen] = 0;
  free(buf);
  *out = body;
  *out_len = blen;
  return 1;
}

/* The CASES document is one fixed spelling. Anything else is not a successful read. */
static int parse_doc(const uint8_t *b, size_t n, pcoc_index_doc *doc) {
  static const char prefix[] = "{\"network\":\"synthetic\",\"header\":{\"hash\":\"";
  const size_t plen = sizeof(prefix) - 1;
  memset(doc, 0, sizeof(*doc));
  if (n < plen + 64 + 16 || memcmp(b, prefix, plen) != 0) return 0;
  const uint8_t *p = b + plen;
  uint8_t display[32];
  for (int i = 0; i < 32; i++) {
    int hi = hex_nibble(p[0]);
    int lo = hex_nibble(p[1]);
    if (hi < 0 || lo < 0) return 0;
    display[i] = (uint8_t)((hi << 4) | lo);
    p += 2;
  }
  static const char mid[] = "\",\"height\":";
  if (n < (size_t)(p - b) + sizeof(mid)) return 0;
  if (memcmp(p, mid, sizeof(mid) - 1) != 0) return 0;
  p += sizeof(mid) - 1;
  if (!isdigit(*p) || (*p == '0' && isdigit(p[1]))) return 0;
  uint64_t height = 0;
  while (isdigit(*p)) {
    height = height * 10u + (uint64_t)(*p - '0');
    p++;
    if (p >= b + n) return 0;
  }
  static const char mid2[] = ",\"time\":";
  if (memcmp(p, mid2, sizeof(mid2) - 1) != 0) return 0;
  p += sizeof(mid2) - 1;
  if (p >= b + n || !isdigit(*p) || (*p == '0' && (size_t)(p + 1 - b) < n && isdigit(p[1]))) return 0;
  uint64_t tim = 0;
  while (p < b + n && isdigit(*p)) {
    tim = tim * 10u + (uint64_t)(*p - '0');
    p++;
  }
  if (p + 2 != b + n || p[0] != '}' || p[1] != '}') return 0;
  doc->present = 1;
  doc->has_header = 1;
  doc->header_hash_ok = 1;
  memcpy(doc->header_hash_display, display, 32);
  doc->header_height_ok = 1;
  doc->header_height = height;
  doc->header_time_ok = 1;
  doc->header_time = tim;
  doc->has_network = 1;
  memcpy(doc->network, "synthetic", 9);
  doc->network_len = 9;
  return 1;
}

static int index_get(void *ctx, uint64_t h, pcoc_index_doc *doc) {
  index_cache *c = ctx;
  memset(doc, 0, sizeof(*doc));
  if (h < H_LO || h > H_HI) return 0;
  unsigned slot = (unsigned)(h - H_LO);
  if (!c->fetched[slot]) {
    c->fetched[slot] = 1;
    uint8_t *body = NULL;
    size_t n = 0;
    int rc = http_get(c->port, h, &body, &n);
    if (rc < 0) {
      fprintf(stderr, "index read failed for height %llu\n", (unsigned long long)h);
      return 0;
    }
    if (rc == 1) {
      c->body[slot] = body;
      c->len[slot] = n;
    }
  }
  if (!c->body[slot]) return 0;
  if (!parse_doc(c->body[slot], c->len[slot], doc)) return 0;
  return 1;
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

static void usage(void) {
  fprintf(stderr,
          "usage: xverify-c --index-url http://127.0.0.1:PORT\n"
          "         [--profile production|conformance-small]\n"
          "         [--slot-bytes N --sample-bytes N --terrain-bytes N --steps-per-leg N]\n"
          "         [--draw-label LABEL] OBJECT\n");
}

int main(int argc, char **argv) {
  const char *index_url = NULL;
  const char *profile_name = "production";
  const char *draw_label = NULL;
  const char *path = NULL;
  uint64_t slot = 0, sample = 0, terrain_b = 0, steps = 0;
  int nsmall = 0;
  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--index-url") == 0 && i + 1 < argc) {
      index_url = argv[++i];
    } else if (strcmp(argv[i], "--profile") == 0 && i + 1 < argc) {
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
  if (!path || !index_url) {
    usage();
    return 2;
  }
  unsigned port = 0;
  if (sscanf(index_url, "http://127.0.0.1:%u", &port) != 1 || port == 0 || port > 65535) {
    fprintf(stderr, "index url must be http://127.0.0.1:PORT\n");
    return 2;
  }
  pcoc_profile prof;
  if (strcmp(profile_name, "production") == 0 || strcmp(profile_name, "test") == 0 ||
      strcmp(profile_name, "live") == 0) {
    if (nsmall) {
      usage();
      return 2;
    }
    pcoc_profile_production(&prof);
  } else if (strcmp(profile_name, "conformance-small") == 0) {
    if (nsmall != 4 || pcoc_profile_small(&prof, slot, sample, terrain_b, steps) != 0) {
      fprintf(stderr, "conformance-small requires the four section 16.1 integers\n");
      return 2;
    }
  } else {
    usage();
    return 2;
  }
  FILE *f = fopen(path, "rb");
  if (!f) return 1;
  if (fseek(f, 0, SEEK_END) != 0) {
    fclose(f);
    return 1;
  }
  long sz = ftell(f);
  if (sz < 0 || fseek(f, 0, SEEK_SET) != 0) {
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
  if (draw_label) {
    fprintf(stderr, "draw: injected label (rig input)\n");
    if (expand_label(draw_label, &draw, &draw_len) != 0) {
      free(bytes);
      return 1;
    }
  }
  index_cache cache;
  memset(&cache, 0, sizeof(cache));
  cache.port = (int)port;
  pcoc_verify_input in;
  memset(&in, 0, sizeof(in));
  in.profile = &prof;
  in.object_bytes = bytes;
  in.object_len = (size_t)sz;
  in.index_get = index_get;
  in.index_ctx = &cache;
  in.draw_bytes = draw;
  in.draw_len = draw_len;
  char *rec = malloc(4 << 20);
  if (!rec) return 1;
  int n = pcoc_verify(&in, rec, 4 << 20);
  free(bytes);
  free(draw);
  for (unsigned i = 0; i < H_N; i++) free(cache.body[i]);
  if (n == -2) {
    fprintf(stderr, "section 7.3: no record\n");
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
