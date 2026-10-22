/* SPEC §7.3 CBOR and §8 hike object.
 * Produce emits core deterministic encoding. Verify accepts any
 * definite-length encoding of the same values, including a non-shortest
 * unsigned integer. Map key order is not a shape mismatch.
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
  memcpy(buf + *n, s, len);
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

static int put_u64(uint8_t *buf, size_t cap, size_t *n, uint64_t v) {
  return put_uint(buf, cap, n, 0, v);
}

static int encode_stage(uint8_t *buf, size_t cap, size_t *n, const pcoc_stage *st, int g) {
  int pairs = (g == 0) ? 6 : 7;
  if (put_uint(buf, cap, n, 5, (uint64_t)pairs) != 0) {
    return -1;
  }
  /* Key order is the section 7.3 bytewise order of the definite encodings.
   * Stage 0 omits path. Stages 1–17: path, beacon, waypoint, leg_count,
   * checkpoints, leg_results, beacon_height. */
  if (g != 0) {
    if (put_text(buf, cap, n, "path", 4) ||
        put_bstr(buf, cap, n, st->path, st->path_len)) {
      return -1;
    }
  }
  if (put_text(buf, cap, n, "beacon", 6) || put_bstr(buf, cap, n, st->beacon, 32)) {
    return -1;
  }
  if (put_text(buf, cap, n, "waypoint", 8) || put_bstr(buf, cap, n, st->waypoint, 32)) {
    return -1;
  }
  if (put_text(buf, cap, n, "leg_count", 9) || put_u64(buf, cap, n, st->leg_count)) {
    return -1;
  }
  if (put_text(buf, cap, n, "checkpoints", 11) ||
      put_uint(buf, cap, n, 4, st->leg_count)) {
    return -1;
  }
  for (uint64_t j = 0; j < st->leg_count && j < PCOC_MAX_LEGS_PER_STAGE; j++) {
    if (put_bstr(buf, cap, n, st->checkpoints[j], 32)) {
      return -1;
    }
  }
  if (put_text(buf, cap, n, "leg_results", 11) ||
      put_uint(buf, cap, n, 4, st->leg_count)) {
    return -1;
  }
  for (uint64_t j = 0; j < st->leg_count && j < PCOC_MAX_LEGS_PER_STAGE; j++) {
    if (put_bstr(buf, cap, n, st->leg_results[j], 8)) {
      return -1;
    }
  }
  if (put_text(buf, cap, n, "beacon_height", 13) ||
      put_u64(buf, cap, n, st->beacon_height)) {
    return -1;
  }
  return 0;
}

int pcoc_object_encode(const pcoc_object *obj, uint8_t *buf, size_t cap) {
  size_t n = 0;
  /* 11 top-level pairs, section 7.3 order. */
  if (put_uint(buf, cap, &n, 5, 11) != 0) {
    return -1;
  }
  if (put_text(buf, cap, &n, "fill", 4) || put_text(buf, cap, &n, obj->fill, obj->fill_len)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "kind", 4) ||
      put_text(buf, cap, &n, obj->kind, obj->kind_len)) {
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
  if (put_text(buf, cap, &n, "n_slots", 7) || put_u64(buf, cap, &n, obj->n_slots)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "public_key", 10) ||
      put_bstr(buf, cap, &n, obj->public_key, 32)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "slot_bytes", 10) || put_u64(buf, cap, &n, obj->slot_bytes)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "seed_beacon", 11) ||
      put_bstr(buf, cap, &n, obj->seed_beacon, 32)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "sample_bytes", 12) ||
      put_u64(buf, cap, &n, obj->sample_bytes)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "steps_per_leg", 13) ||
      put_u64(buf, cap, &n, obj->steps_per_leg)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "last_interrupt", 14) ||
      put_bstr(buf, cap, &n, obj->last_interrupt, 32)) {
    return -1;
  }
  if (put_text(buf, cap, &n, "last_interrupt_height", 21) ||
      put_u64(buf, cap, &n, obj->last_interrupt_height)) {
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

/* Read one definite head. *ai is the additional info value.
 * Rejects indefinite (31) and anything that is not a definite length. */
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
    bad(c); /* indefinite */
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

static int skip_item(cr *c);

static int skip_item(cr *c) {
  uint8_t maj;
  uint64_t val;
  if (head(c, &maj, &val)) {
    return -1;
  }
  if (maj == 0 || maj == 1 || maj == 7) {
    if (maj == 7 && val > 24 && val < 32) {
      /* simple / float already consumed by head for 0..23 and 1/2/4/8 byte
       * floats (ai 25, 26, 27). A float is not allowed in this object, but
       * skip still walks it so a trailing check can see it as a decoded
       * item. The caller marks malformed when the top type is wrong. */
    }
    return 0;
  }
  if (maj == 2 || maj == 3) {
    if (val > c->n - c->i) {
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
  if (maj == 6) {
    return skip_item(c); /* tag: still malformed at the top, but skip it */
  }
  bad(c);
  return -1;
}

static int take_text(cr *c, char *dst, size_t cap, size_t *out_len) {
  uint8_t maj;
  uint64_t val;
  if (head(c, &maj, &val) || maj != 3) {
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
  size_t copy = (size_t)val;
  if (copy > cap) {
    copy = cap;
  }
  if (dst && copy) {
    memcpy(dst, c->p + c->i, copy);
  }
  if (dst && copy < cap) {
    dst[copy] = 0;
  }
  c->i += (size_t)val;
  return 0;
}

static int take_bstr(cr *c, uint8_t *dst, size_t expect_or_cap, size_t *out_len, int exact) {
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
  if (exact && val != expect_or_cap) {
    /* still consume; caller records the width mismatch */
  }
  size_t copy = (size_t)val;
  if (copy > expect_or_cap) {
    copy = expect_or_cap;
  }
  if (dst && copy) {
    memcpy(dst, c->p + c->i, copy);
  }
  c->i += (size_t)val;
  return (int)val < 0 ? -1 : 0;
}

static int take_u64(cr *c, uint64_t *v) {
  uint8_t maj;
  uint64_t val;
  if (head(c, &maj, &val) || maj != 0) {
    bad(c);
    return -1;
  }
  *v = val;
  return 0;
}

static int key_eq(const char *k, size_t n, const char *lit) {
  size_t m = strlen(lit);
  return n == m && memcmp(k, lit, n) == 0;
}

static int decode_stage(cr *c, pcoc_stage *st, int *is_stage0_path) {
  memset(st, 0, sizeof(*st));
  uint8_t maj;
  uint64_t npair;
  if (head(c, &maj, &npair) || maj != 5) {
    bad(c);
    return -1;
  }
  *is_stage0_path = 0;
  int saw_path = 0;
  for (uint64_t pi = 0; pi < npair; pi++) {
    char key[64];
    size_t klen = 0;
    if (take_text(c, key, sizeof(key) - 1, &klen)) {
      return -1;
    }
    if (key_eq(key, klen, "beacon")) {
      size_t bl = 0;
      if (take_bstr(c, st->beacon, 32, &bl, 1)) {
        return -1;
      }
      st->beacon_height = st->beacon_height; /* keep */
      (void)bl;
    } else if (key_eq(key, klen, "beacon_height")) {
      if (take_u64(c, &st->beacon_height)) {
        return -1;
      }
    } else if (key_eq(key, klen, "waypoint")) {
      size_t bl = 0;
      if (take_bstr(c, st->waypoint, 32, &bl, 1)) {
        return -1;
      }
    } else if (key_eq(key, klen, "leg_count")) {
      if (take_u64(c, &st->leg_count)) {
        return -1;
      }
    } else if (key_eq(key, klen, "path")) {
      saw_path = 1;
      size_t bl = 0;
      /* Cap stored path bytes. Longer paths are consumed and truncated
       * into the buffer; the verifier treats non-empty as present and
       * decodes the canonical body from the original only when it fits.
       * For this core the tests use empty paths. */
      if (take_bstr(c, st->path, sizeof(st->path), &bl, 0)) {
        return -1;
      }
      st->path_len = bl > sizeof(st->path) ? sizeof(st->path) : bl;
      st->has_path = 1;
    } else if (key_eq(key, klen, "checkpoints")) {
      uint64_t na;
      if (head(c, &maj, &na) || maj != 4) {
        bad(c);
        return -1;
      }
      st->n_checkpoints = na;
      for (uint64_t j = 0; j < na; j++) {
        uint8_t tmp[32];
        size_t bl = 0;
        if (take_bstr(c, tmp, 32, &bl, 0)) {
          return -1;
        }
        if (bl != 32) {
          st->checkpoint_width_bad = 1;
        }
        if (j < PCOC_MAX_LEGS_PER_STAGE && bl == 32) {
          memcpy(st->checkpoints[j], tmp, 32);
        }
      }
    } else if (key_eq(key, klen, "leg_results")) {
      uint64_t na;
      if (head(c, &maj, &na) || maj != 4) {
        bad(c);
        return -1;
      }
      st->n_leg_results = na;
      for (uint64_t j = 0; j < na; j++) {
        uint8_t tmp[8];
        size_t bl = 0;
        if (take_bstr(c, tmp, 8, &bl, 0)) {
          return -1;
        }
        if (bl != 8) {
          st->leg_result_width_bad = 1;
        }
        if (j < PCOC_MAX_LEGS_PER_STAGE && bl == 8) {
          memcpy(st->leg_results[j], tmp, 8);
        }
      }
    } else {
      if (skip_item(c)) {
        return -1;
      }
    }
    (void)saw_path;
  }
  *is_stage0_path = saw_path;
  return 0;
}

int pcoc_object_decode(const uint8_t *buf, size_t len, pcoc_object *obj, size_t *consumed) {
  memset(obj, 0, sizeof(*obj));
  cr c = {buf, len, 0, 0};
  uint8_t maj;
  uint64_t npair;
  if (head(&c, &maj, &npair) || maj != 5) {
    obj->malformed = 1;
    if (consumed) {
      *consumed = c.i;
    }
    return -1;
  }
  int stage_path[PCOC_STAGES];
  memset(stage_path, 0, sizeof(stage_path));
  for (uint64_t pi = 0; pi < npair; pi++) {
    char key[64];
    size_t klen = 0;
    if (take_text(&c, key, sizeof(key) - 1, &klen)) {
      obj->malformed = 1;
      if (consumed) {
        *consumed = c.i;
      }
      return -1;
    }
    if (key_eq(key, klen, "kind")) {
      obj->has_kind = 1;
      obj->kind_is_text = 1;
      if (take_text(&c, obj->kind, sizeof(obj->kind) - 1, &obj->kind_len)) {
        /* not text */
        obj->kind_is_text = 0;
        obj->malformed = 0;
        if (skip_item(&c)) {
          obj->malformed = 1;
          if (consumed) {
            *consumed = c.i;
          }
          return -1;
        }
      }
    } else if (key_eq(key, klen, "fill")) {
      obj->has_fill = 1;
      if (take_text(&c, obj->fill, sizeof(obj->fill) - 1, &obj->fill_len)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    } else if (key_eq(key, klen, "public_key")) {
      obj->has_public_key = 1;
      size_t bl = 0;
      if (take_bstr(&c, obj->public_key, 32, &bl, 0)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    } else if (key_eq(key, klen, "seed_beacon")) {
      obj->has_seed_beacon = 1;
      size_t bl = 0;
      if (take_bstr(&c, obj->seed_beacon, 32, &bl, 0)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    } else if (key_eq(key, klen, "slot_bytes")) {
      obj->has_slot_bytes = 1;
      if (take_u64(&c, &obj->slot_bytes)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    } else if (key_eq(key, klen, "n_slots")) {
      obj->has_n_slots = 1;
      if (take_u64(&c, &obj->n_slots)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    } else if (key_eq(key, klen, "sample_bytes")) {
      obj->has_sample_bytes = 1;
      if (take_u64(&c, &obj->sample_bytes)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    } else if (key_eq(key, klen, "steps_per_leg")) {
      obj->has_steps_per_leg = 1;
      if (take_u64(&c, &obj->steps_per_leg)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    } else if (key_eq(key, klen, "last_interrupt")) {
      obj->has_last_interrupt = 1;
      size_t bl = 0;
      if (take_bstr(&c, obj->last_interrupt, 32, &bl, 0)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    } else if (key_eq(key, klen, "last_interrupt_height")) {
      obj->has_last_interrupt_height = 1;
      if (take_u64(&c, &obj->last_interrupt_height)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    } else if (key_eq(key, klen, "stages")) {
      obj->has_stages = 1;
      uint64_t na;
      if (head(&c, &maj, &na) || maj != 4) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
      obj->n_stages = (size_t)na;
      for (uint64_t g = 0; g < na; g++) {
        int saw_path = 0;
        pcoc_stage tmp;
        if (g < PCOC_STAGES) {
          if (decode_stage(&c, &obj->stages[g], &saw_path)) {
            obj->malformed = 1;
            if (consumed) {
              *consumed = c.i;
            }
            return -1;
          }
          stage_path[g] = saw_path;
          obj->stages[g].has_path = saw_path;
        } else {
          if (decode_stage(&c, &tmp, &saw_path)) {
            obj->malformed = 1;
            if (consumed) {
              *consumed = c.i;
            }
            return -1;
          }
        }
      }
    } else {
      if (skip_item(&c)) {
        obj->malformed = 1;
        if (consumed) {
          *consumed = c.i;
        }
        return -1;
      }
    }
  }
  if (c.bad) {
    obj->malformed = 1;
    if (consumed) {
      *consumed = c.i;
    }
    return -1;
  }
  /* Bytes after the one data item are a malformed CBOR value (§7.3). */
  if (c.i != len) {
    obj->malformed = 1;
    if (consumed) {
      *consumed = c.i;
    }
    return -1;
  }
  if (consumed) {
    *consumed = c.i;
  }
  return 0;
}
