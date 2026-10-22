/* SPEC §7.3 CBOR and §8 hike object.
 * Produce emits the core deterministic encoding. Verify accepts a
 * non-shortest unsigned integer and any map key order. Indefinite
 * lengths, tags, floats, simple values, negative integers, a non-text
 * map key, a duplicate key, invalid UTF-8 in a text string, and any
 * byte after the one data item are a malformed CBOR value (erratum E14;
 * invalid UTF-8 is this SDK's reading of §7.3's UTF-8 text rule, recorded
 * in RESULTS.md). One well-formed data item that is not a map, or a map
 * with no text `kind`, is a decode failure.
 */
#include "pcoc.h"

#include <stdlib.h>
#include <string.h>

static int put_uint(uint8_t *buf, size_t cap, size_t *n, uint8_t major, uint64_t v) {
  uint8_t tmp[9];
  size_t k;
  if (v < 24) {
    tmp[0] = (uint8_t)((major << 5) | v);
    k = 1;
  } else if (v <= 0xffu) {
    tmp[0] = (uint8_t)((major << 5) | 24);
    tmp[1] = (uint8_t)v;
    k = 2;
  } else if (v <= 0xffffu) {
    tmp[0] = (uint8_t)((major << 5) | 25);
    tmp[1] = (uint8_t)(v >> 8);
    tmp[2] = (uint8_t)v;
    k = 3;
  } else if (v <= 0xffffffffu) {
    tmp[0] = (uint8_t)((major << 5) | 26);
    tmp[1] = (uint8_t)(v >> 24);
    tmp[2] = (uint8_t)(v >> 16);
    tmp[3] = (uint8_t)(v >> 8);
    tmp[4] = (uint8_t)v;
    k = 5;
  } else {
    tmp[0] = (uint8_t)((major << 5) | 27);
    for (int i = 0; i < 8; i++) {
      tmp[1 + i] = (uint8_t)(v >> (56 - 8 * i));
    }
    k = 9;
  }
  if (*n + k > cap) {
    return -1;
  }
  memcpy(buf + *n, tmp, k);
  *n += k;
  return 0;
}

static int put_text(uint8_t *buf, size_t cap, size_t *n, const char *s, size_t len) {
  if (put_uint(buf, cap, n, 3, len) != 0) {
    return -1;
  }
  if (*n + len > cap) {
    return -1;
  }
  if (len) {
    memcpy(buf + *n, s, len);
  }
  *n += len;
  return 0;
}

static int put_bstr(uint8_t *buf, size_t cap, size_t *n, const uint8_t *p, size_t len) {
  if (put_uint(buf, cap, n, 2, len) != 0) {
    return -1;
  }
  if (*n + len > cap) {
    return -1;
  }
  if (len) {
    memcpy(buf + *n, p, len);
  }
  *n += len;
  return 0;
}

static int encode_stage(uint8_t *buf, size_t cap, size_t *n, const pcoc_stage *st, int g) {
  int pairs = (g == 0) ? 6 : 7;
  if (put_uint(buf, cap, n, 5, (uint64_t)pairs) != 0) {
    return -1;
  }
  if (g != 0) {
    if (put_text(buf, cap, n, "path", 4) || put_bstr(buf, cap, n, st->path, st->path_len)) {
      return -1;
    }
  }
  if (put_text(buf, cap, n, "beacon", 6) || put_bstr(buf, cap, n, st->beacon, 32)) {
    return -1;
  }
  if (put_text(buf, cap, n, "waypoint", 8) || put_bstr(buf, cap, n, st->waypoint, 32)) {
    return -1;
  }
  if (put_text(buf, cap, n, "leg_count", 9) || put_uint(buf, cap, n, 0, st->leg_count)) {
    return -1;
  }
  if (put_text(buf, cap, n, "checkpoints", 11) || put_uint(buf, cap, n, 4, st->leg_count)) {
    return -1;
  }
  for (uint64_t j = 0; j < st->leg_count && j < PCOC_MAX_LEGS_PER_STAGE; j++) {
    if (put_bstr(buf, cap, n, st->checkpoints[j], 32)) {
      return -1;
    }
  }
  if (put_text(buf, cap, n, "leg_results", 11) || put_uint(buf, cap, n, 4, st->leg_count)) {
    return -1;
  }
  for (uint64_t j = 0; j < st->leg_count && j < PCOC_MAX_LEGS_PER_STAGE; j++) {
    if (put_bstr(buf, cap, n, st->leg_results[j], 8)) {
      return -1;
    }
  }
  if (put_text(buf, cap, n, "beacon_height", 13) || put_uint(buf, cap, n, 0, st->beacon_height)) {
    return -1;
  }
  return 0;
}

int pcoc_object_encode(const pcoc_object *obj, uint8_t *buf, size_t cap) {
  size_t n = 0;
  if (put_uint(buf, cap, &n, 5, 11) != 0) {
    return -1;
  }
  if (put_text(buf, cap, &n, "fill", 4) || put_text(buf, cap, &n, obj->fill, obj->fill_len)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "kind", 4) || put_text(buf, cap, &n, obj->kind, obj->kind_len)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "stages", 6) || put_uint(buf, cap, &n, 4, PCOC_STAGES)) {
    return -1;
  }
  for (int g = 0; g < (int)PCOC_STAGES; g++) {
    if (encode_stage(buf, cap, &n, &obj->stages[g], g) != 0) {
      return -1;
    }
  }
  if (put_text(buf, cap, &n, "n_slots", 7) || put_uint(buf, cap, &n, 0, obj->n_slots)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "public_key", 10) || put_bstr(buf, cap, &n, obj->public_key, 32)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "slot_bytes", 10) || put_uint(buf, cap, &n, 0, obj->slot_bytes)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "seed_beacon", 11) || put_bstr(buf, cap, &n, obj->seed_beacon, 32)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "sample_bytes", 12) || put_uint(buf, cap, &n, 0, obj->sample_bytes)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "steps_per_leg", 13) ||
      put_uint(buf, cap, &n, 0, obj->steps_per_leg)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "last_interrupt", 14) ||
      put_bstr(buf, cap, &n, obj->last_interrupt, 32)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "last_interrupt_height", 21) ||
      put_uint(buf, cap, &n, 0, obj->last_interrupt_height)) {
    return -1;
  }
  return (int)n;
}

/* ---- decode ---- */

typedef struct {
  const uint8_t *p;
  size_t n;
  size_t i;
  int bad;
} cr;

static void bad(cr *c) { c->bad = 1; }

static int need(cr *c, size_t k) {
  if (c->bad || c->i + k > c->n) {
    bad(c);
    return -1;
  }
  return 0;
}

static int head(cr *c, uint8_t *major, uint64_t *val) {
  if (need(c, 1)) {
    return -1;
  }
  uint8_t b = c->p[c->i++];
  *major = b >> 5;
  uint8_t ai = b & 31;
  if (ai < 24) {
    *val = ai;
    return 0;
  }
  if (ai == 31) {
    bad(c);
    return -1;
  }
  size_t nbytes = 0;
  if (ai == 24) {
    nbytes = 1;
  } else if (ai == 25) {
    nbytes = 2;
  } else if (ai == 26) {
    nbytes = 4;
  } else if (ai == 27) {
    nbytes = 8;
  } else {
    bad(c);
    return -1;
  }
  if (need(c, nbytes)) {
    return -1;
  }
  uint64_t v = 0;
  for (size_t k = 0; k < nbytes; k++) {
    v = (v << 8) | c->p[c->i++];
  }
  *val = v;
  return 0;
}

static int utf8_ok(const uint8_t *s, size_t n) {
  size_t i = 0;
  while (i < n) {
    uint8_t c = s[i];
    if (c <= 0x7F) {
      i++;
      continue;
    }
    size_t needn = 0;
    uint32_t cp = 0;
    uint32_t mincp = 0;
    if ((c & 0xE0) == 0xC0) {
      needn = 2;
      cp = c & 0x1F;
      mincp = 0x80;
    } else if ((c & 0xF0) == 0xE0) {
      needn = 3;
      cp = c & 0x0F;
      mincp = 0x800;
    } else if ((c & 0xF8) == 0xF0 && c <= 0xF4) {
      needn = 4;
      cp = c & 0x07;
      mincp = 0x10000;
    } else {
      return 0;
    }
    if (i + needn > n) {
      return 0;
    }
    for (size_t k = 1; k < needn; k++) {
      if ((s[i + k] & 0xC0) != 0x80) {
        return 0;
      }
      cp = (cp << 6) | (s[i + k] & 0x3F);
    }
    if (cp < mincp || cp > 0x10FFFF || (cp >= 0xD800 && cp <= 0xDFFF)) {
      return 0;
    }
    i += needn;
  }
  return 1;
}

static int skip_item(cr *c);

static int skip_item(cr *c) {
  uint8_t maj;
  uint64_t val;
  if (head(c, &maj, &val)) {
    return -1;
  }
  /* §7.3: negative, tag, float, and simple are a malformed value. */
  if (maj == 1 || maj == 6 || maj == 7) {
    bad(c);
    return -1;
  }
  if (maj == 0) {
    return 0;
  }
  if (maj == 2 || maj == 3) {
    if (val > c->n - c->i) {
      bad(c);
      return -1;
    }
    if (maj == 3 && !utf8_ok(c->p + c->i, (size_t)val)) {
      bad(c);
      return -1;
    }
    c->i += (size_t)val;
    return 0;
  }
  if (maj == 4) {
    for (uint64_t i = 0; i < val; i++) {
      if (skip_item(c)) {
        return -1;
      }
    }
    return 0;
  }
  if (maj == 5) {
    for (uint64_t i = 0; i < val; i++) {
      if (skip_item(c) || skip_item(c)) {
        return -1;
      }
    }
    return 0;
  }
  bad(c);
  return -1;
}

static int key_eq(const char *k, size_t n, const char *lit) {
  size_t m = strlen(lit);
  return n == m && memcmp(k, lit, n) == 0;
}

#define SEEN_MAX 24
typedef struct {
  char k[40];
  size_t n;
  int used;
} seen_key;

static int seen_add(seen_key *s, int *ns, const char *k, size_t n) {
  for (int i = 0; i < *ns; i++) {
    if (s[i].n == n && memcmp(s[i].k, k, n) == 0) {
      return -1; /* duplicate */
    }
  }
  if (*ns >= SEEN_MAX || n >= sizeof(s[0].k)) {
    return -2;
  }
  memcpy(s[*ns].k, k, n);
  s[*ns].n = n;
  s[*ns].used = 1;
  (*ns)++;
  return 0;
}

/* Read a text key. Returns 0, or -1 malformed. */
static int take_key(cr *c, char *dst, size_t cap, size_t *out_len) {
  uint8_t maj;
  uint64_t val;
  if (head(c, &maj, &val)) {
    return -1;
  }
  if (maj != 3) {
    bad(c); /* map key that is not a text string */
    return -1;
  }
  if (val > c->n - c->i) {
    bad(c);
    return -1;
  }
  if (!utf8_ok(c->p + c->i, (size_t)val)) {
    bad(c);
    return -1;
  }
  size_t copy = (size_t)val;
  if (copy >= cap) {
    copy = cap - 1;
  }
  if (dst && copy) {
    memcpy(dst, c->p + c->i, copy);
  }
  if (dst) {
    dst[copy] = 0;
  }
  if (out_len) {
    *out_len = (size_t)val;
  }
  c->i += (size_t)val;
  return 0;
}

static int take_bstr_into(cr *c, uint8_t *dst, size_t cap, size_t *out_len) {
  uint8_t maj;
  uint64_t val;
  if (head(c, &maj, &val) || maj != 2) {
    bad(c);
    return -1;
  }
  if (val > c->n - c->i) {
    bad(c);
    return -1;
  }
  if (out_len) {
    *out_len = (size_t)val;
  }
  size_t copy = (size_t)val < cap ? (size_t)val : cap;
  if (dst && copy) {
    memcpy(dst, c->p + c->i, copy);
  }
  c->i += (size_t)val;
  return 0;
}

static int take_u64_into(cr *c, uint64_t *v) {
  uint8_t maj;
  uint64_t val;
  if (head(c, &maj, &val) || maj != 0) {
    bad(c);
    return -1;
  }
  *v = val;
  return 0;
}

static int peek_maj(cr *c, uint8_t *maj) {
  if (c->bad || c->i >= c->n) {
    bad(c);
    return -1;
  }
  *maj = c->p[c->i] >> 5;
  return 0;
}

static int decode_stage(cr *c, pcoc_stage *st) {
  memset(st, 0, sizeof(*st));
  uint8_t maj;
  uint64_t npair;
  if (head(c, &maj, &npair) || maj != 5) {
    bad(c);
    return -1;
  }
  seen_key seen[SEEN_MAX];
  int nseen = 0;
  memset(seen, 0, sizeof(seen));
  for (uint64_t pi = 0; pi < npair; pi++) {
    char key[64];
    size_t klen = 0;
    if (take_key(c, key, sizeof(key), &klen)) {
      return -1;
    }
    if (klen >= sizeof(key)) {
      bad(c);
      return -1;
    }
    int dup = seen_add(seen, &nseen, key, klen);
    if (dup == -1) {
      bad(c);
      return -1;
    }
    uint8_t vm;
    if (peek_maj(c, &vm)) {
      return -1;
    }
    if (key_eq(key, klen, "beacon") || key_eq(key, klen, "waypoint") ||
        key_eq(key, klen, "path")) {
      if (vm != 2) {
        st->field_type_bad = 1;
        if (skip_item(c)) {
          return -1;
        }
        continue;
      }
      uint8_t tmp[256];
      size_t bl = 0;
      size_t cap = key_eq(key, klen, "path") ? sizeof(st->path) : 32;
      if (take_bstr_into(c, tmp, sizeof(tmp), &bl)) {
        return -1;
      }
      if (key_eq(key, klen, "path")) {
        st->has_path = 1;
        st->path_len = bl;
        size_t copy = bl < sizeof(st->path) ? bl : sizeof(st->path);
        memcpy(st->path, tmp, copy);
      } else if (key_eq(key, klen, "beacon")) {
        if (bl == 32) {
          memcpy(st->beacon, tmp, 32);
        } else {
          st->beacon_len_bad = 1;
        }
      } else {
        if (bl == 32) {
          memcpy(st->waypoint, tmp, 32);
        } else {
          st->waypoint_len_bad = 1;
        }
      }
      (void)cap;
    } else if (key_eq(key, klen, "beacon_height") || key_eq(key, klen, "leg_count")) {
      if (vm != 0) {
        st->field_type_bad = 1;
        if (skip_item(c)) {
          return -1;
        }
        continue;
      }
      uint64_t v = 0;
      if (take_u64_into(c, &v)) {
        return -1;
      }
      if (key_eq(key, klen, "beacon_height")) {
        st->beacon_height = v;
      } else {
        st->leg_count = v;
      }
    } else if (key_eq(key, klen, "checkpoints") || key_eq(key, klen, "leg_results")) {
      if (vm != 4) {
        st->field_type_bad = 1;
        if (skip_item(c)) {
          return -1;
        }
        continue;
      }
      uint64_t na;
      if (head(c, &maj, &na) || maj != 4) {
        bad(c);
        return -1;
      }
      int is_cp = key_eq(key, klen, "checkpoints");
      if (is_cp) {
        st->n_checkpoints = na;
      } else {
        st->n_leg_results = na;
      }
      size_t expect = is_cp ? 32 : 8;
      for (uint64_t j = 0; j < na; j++) {
        uint8_t pm;
        if (peek_maj(c, &pm)) {
          return -1;
        }
        if (pm != 2) {
          st->field_type_bad = 1;
          if (skip_item(c)) {
            return -1;
          }
          if (is_cp) {
            st->checkpoint_width_bad = 1;
          } else {
            st->leg_result_width_bad = 1;
          }
          continue;
        }
        uint8_t tmp[32];
        size_t bl = 0;
        if (take_bstr_into(c, tmp, sizeof(tmp), &bl)) {
          return -1;
        }
        if (bl != expect) {
          if (is_cp) {
            st->checkpoint_width_bad = 1;
          } else {
            st->leg_result_width_bad = 1;
          }
        } else if (j < PCOC_MAX_LEGS_PER_STAGE) {
          if (is_cp) {
            memcpy(st->checkpoints[j], tmp, 32);
          } else {
            memcpy(st->leg_results[j], tmp, 8);
          }
        }
      }
    } else {
      st->extra_key = 1;
      if (skip_item(c)) {
        return -1;
      }
    }
  }
  return 0;
}

static int finish_class(pcoc_object *obj, cr *c, size_t *consumed) {
  if (consumed) {
    *consumed = c->i;
  }
  if (c->bad || c->i != c->n) {
    obj->malformed = 1;
    obj->cbor_class = PCOC_CBOR_MALFORMED;
    return -1;
  }
  if (!obj->has_kind || !obj->kind_is_text) {
    obj->cbor_class = PCOC_CBOR_DECODE_FAILURE;
    return -2;
  }
  obj->cbor_class = PCOC_CBOR_OK;
  return 0;
}

int pcoc_object_decode(const uint8_t *buf, size_t len, pcoc_object *obj, size_t *consumed) {
  memset(obj, 0, sizeof(*obj));
  if (!buf || len == 0) {
    obj->malformed = 1;
    obj->cbor_class = PCOC_CBOR_MALFORMED;
    if (consumed) {
      *consumed = 0;
    }
    return -1;
  }
  uint8_t top = buf[0] >> 5;
  cr c = {buf, len, 0, 0};
  /* A well-formed negative, tag, float, or simple is malformed (§7.3). */
  if (top == 1 || top == 6 || top == 7) {
    obj->malformed = 1;
    obj->cbor_class = PCOC_CBOR_MALFORMED;
    return -1;
  }
  if (top != 5) {
    if (skip_item(&c) != 0 || c.bad || c.i != len) {
      obj->malformed = 1;
      obj->cbor_class = PCOC_CBOR_MALFORMED;
      if (consumed) {
        *consumed = c.i;
      }
      return -1;
    }
    obj->cbor_class = PCOC_CBOR_DECODE_FAILURE;
    if (consumed) {
      *consumed = c.i;
    }
    return -2;
  }
  uint8_t maj;
  uint64_t npair;
  if (head(&c, &maj, &npair) || maj != 5) {
    obj->malformed = 1;
    obj->cbor_class = PCOC_CBOR_MALFORMED;
    if (consumed) {
      *consumed = c.i;
    }
    return -1;
  }
  seen_key seen[SEEN_MAX];
  int nseen = 0;
  memset(seen, 0, sizeof(seen));
  for (uint64_t pi = 0; pi < npair; pi++) {
    char key[64];
    size_t klen = 0;
    if (take_key(&c, key, sizeof(key), &klen)) {
      obj->malformed = 1;
      obj->cbor_class = PCOC_CBOR_MALFORMED;
      if (consumed) {
        *consumed = c.i;
      }
      return -1;
    }
    if (klen >= sizeof(key)) {
      obj->malformed = 1;
      obj->cbor_class = PCOC_CBOR_MALFORMED;
      return -1;
    }
    if (seen_add(seen, &nseen, key, klen) == -1) {
      obj->duplicate_key = 1;
      obj->malformed = 1;
      obj->cbor_class = PCOC_CBOR_MALFORMED;
      return -1;
    }
    uint8_t vm;
    if (peek_maj(&c, &vm)) {
      obj->malformed = 1;
      obj->cbor_class = PCOC_CBOR_MALFORMED;
      return -1;
    }
    if (key_eq(key, klen, "kind")) {
      obj->has_kind = 1;
      if (vm != 3) {
        obj->kind_is_text = 0;
        if (skip_item(&c)) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
      } else {
        uint64_t val;
        if (head(&c, &maj, &val) || maj != 3) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
        if (val > c.n - c.i || !utf8_ok(c.p + c.i, (size_t)val)) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
        size_t copy = (size_t)val < sizeof(obj->kind) - 1 ? (size_t)val : sizeof(obj->kind) - 1;
        memcpy(obj->kind, c.p + c.i, copy);
        obj->kind[copy] = 0;
        obj->kind_len = (size_t)val;
        obj->kind_is_text = 1;
        c.i += (size_t)val;
      }
    } else if (key_eq(key, klen, "fill")) {
      obj->has_fill = 1;
      if (vm != 3) {
        obj->field_type_bad = 1;
        if (skip_item(&c)) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
      } else {
        uint64_t val;
        if (head(&c, &maj, &val) || maj != 3 || val > c.n - c.i ||
            !utf8_ok(c.p + c.i, (size_t)val)) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
        size_t copy = (size_t)val < sizeof(obj->fill) - 1 ? (size_t)val : sizeof(obj->fill) - 1;
        memcpy(obj->fill, c.p + c.i, copy);
        obj->fill[copy] = 0;
        obj->fill_len = (size_t)val;
        c.i += (size_t)val;
      }
    } else if (key_eq(key, klen, "public_key") || key_eq(key, klen, "seed_beacon") ||
               key_eq(key, klen, "last_interrupt")) {
      if (vm != 2) {
        obj->field_type_bad = 1;
        if (skip_item(&c)) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
      } else {
        uint8_t tmp[32];
        size_t bl = 0;
        if (take_bstr_into(&c, tmp, 32, &bl)) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
        if (key_eq(key, klen, "public_key")) {
          obj->has_public_key = 1;
          if (bl == 32) {
            memcpy(obj->public_key, tmp, 32);
          } else {
            obj->public_key_len_bad = 1;
          }
        } else if (key_eq(key, klen, "seed_beacon")) {
          obj->has_seed_beacon = 1;
          if (bl == 32) {
            memcpy(obj->seed_beacon, tmp, 32);
          } else {
            obj->seed_beacon_len_bad = 1;
          }
        } else {
          obj->has_last_interrupt = 1;
          obj->last_interrupt_len = bl;
          if (bl == 32) {
            memcpy(obj->last_interrupt, tmp, 32);
          } else {
            obj->last_interrupt_len_bad = 1;
          }
        }
      }
    } else if (key_eq(key, klen, "slot_bytes") || key_eq(key, klen, "n_slots") ||
               key_eq(key, klen, "sample_bytes") || key_eq(key, klen, "steps_per_leg") ||
               key_eq(key, klen, "last_interrupt_height")) {
      if (vm != 0) {
        obj->field_type_bad = 1;
        if (skip_item(&c)) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
      } else {
        uint64_t v = 0;
        if (take_u64_into(&c, &v)) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
        if (key_eq(key, klen, "slot_bytes")) {
          obj->has_slot_bytes = 1;
          obj->slot_bytes = v;
        } else if (key_eq(key, klen, "n_slots")) {
          obj->has_n_slots = 1;
          obj->n_slots = v;
        } else if (key_eq(key, klen, "sample_bytes")) {
          obj->has_sample_bytes = 1;
          obj->sample_bytes = v;
        } else if (key_eq(key, klen, "steps_per_leg")) {
          obj->has_steps_per_leg = 1;
          obj->steps_per_leg = v;
        } else {
          obj->has_last_interrupt_height = 1;
          obj->last_interrupt_height = v;
        }
      }
    } else if (key_eq(key, klen, "stages")) {
      obj->has_stages = 1;
      if (vm != 4) {
        obj->field_type_bad = 1;
        if (skip_item(&c)) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
      } else {
        uint64_t na;
        if (head(&c, &maj, &na) || maj != 4) {
          obj->malformed = 1;
          obj->cbor_class = PCOC_CBOR_MALFORMED;
          return -1;
        }
        obj->n_stages = (size_t)na;
        for (uint64_t g = 0; g < na; g++) {
          uint8_t sm;
          if (peek_maj(&c, &sm)) {
            obj->malformed = 1;
            obj->cbor_class = PCOC_CBOR_MALFORMED;
            return -1;
          }
          if (sm != 5) {
            obj->field_type_bad = 1;
            if (skip_item(&c)) {
              obj->malformed = 1;
              obj->cbor_class = PCOC_CBOR_MALFORMED;
              return -1;
            }
            continue;
          }
          pcoc_stage tmp;
          pcoc_stage *dst = (g < PCOC_STAGES) ? &obj->stages[g] : &tmp;
          if (decode_stage(&c, dst)) {
            obj->malformed = 1;
            obj->cbor_class = PCOC_CBOR_MALFORMED;
            if (consumed) {
              *consumed = c.i;
            }
            return -1;
          }
        }
      }
    } else {
      obj->extra_key = 1;
      if (skip_item(&c)) {
        obj->malformed = 1;
        obj->cbor_class = PCOC_CBOR_MALFORMED;
        return -1;
      }
    }
  }
  return finish_class(obj, &c, consumed);
}
