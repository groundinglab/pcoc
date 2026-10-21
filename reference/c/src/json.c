#include "json.h"

#include <ctype.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    const char *p;
    const char *end;
    const char *error;
} Parse;

static void skip(Parse *ps)
{
    while (ps->p < ps->end) {
        char c = *ps->p;
        if (c != ' ' && c != '\t' && c != '\n' && c != '\r') {
            break;
        }
        ps->p++;
    }
}

static Json *make(int type)
{
    Json *v = (Json *)calloc(1, sizeof(Json));
    if (v != NULL) {
        v->type = type;
    }
    return v;
}

static int push(Json *parent, const char *key, Json *child)
{
    int n = parent->count;
    Json **nc;
    char **nk = parent->key;
    nc = (Json **)realloc(parent->child, (size_t)(n + 1) * sizeof(Json *));
    if (nc == NULL) {
        return -1;
    }
    parent->child = nc;
    if (key != NULL) {
        nk = (char **)realloc(parent->key, (size_t)(n + 1) * sizeof(char *));
        if (nk == NULL) {
            return -1;
        }
        parent->key = nk;
        parent->key[n] = (char *)key;
    }
    parent->child[n] = child;
    parent->count = n + 1;
    return 0;
}

static Json *parse_value(Parse *ps);

static int hex_nibble(char c)
{
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

static Json *parse_string(Parse *ps)
{
    size_t cap = 32;
    size_t n = 0;
    char *buf;
    Json *v;
    if (ps->p >= ps->end || *ps->p != '"') {
        ps->error = "string";
        return NULL;
    }
    ps->p++;
    buf = (char *)malloc(cap);
    if (buf == NULL) {
        ps->error = "memory";
        return NULL;
    }
    while (ps->p < ps->end) {
        unsigned char c = (unsigned char)*ps->p++;
        if (c == '"') {
            buf[n] = 0;
            v = make(JSON_STR);
            if (v == NULL) {
                free(buf);
                ps->error = "memory";
                return NULL;
            }
            v->string = buf;
            return v;
        }
        if (c == '\\') {
            char e;
            if (ps->p >= ps->end) {
                break;
            }
            e = *ps->p++;
            if (e == 'u') {
                int h0, h1, h2, h3, cp;
                if (ps->p + 4 > ps->end) {
                    break;
                }
                h0 = hex_nibble(ps->p[0]);
                h1 = hex_nibble(ps->p[1]);
                h2 = hex_nibble(ps->p[2]);
                h3 = hex_nibble(ps->p[3]);
                ps->p += 4;
                if (h0 < 0 || h1 < 0 || h2 < 0 || h3 < 0) {
                    break;
                }
                cp = (h0 << 12) | (h1 << 8) | (h2 << 4) | h3;
                if (cp < 0x80) {
                    c = (unsigned char)cp;
                } else if (cp < 0x800) {
                    /* stored as UTF-8, two bytes, handled below by expanding */
                    if (n + 2 >= cap) {
                        cap *= 2;
                        buf = (char *)realloc(buf, cap);
                        if (buf == NULL) {
                            ps->error = "memory";
                            return NULL;
                        }
                    }
                    buf[n++] = (char)(0xc0 | (cp >> 6));
                    buf[n++] = (char)(0x80 | (cp & 0x3f));
                    continue;
                } else {
                    if (n + 3 >= cap) {
                        cap *= 2;
                        buf = (char *)realloc(buf, cap);
                        if (buf == NULL) {
                            ps->error = "memory";
                            return NULL;
                        }
                    }
                    buf[n++] = (char)(0xe0 | (cp >> 12));
                    buf[n++] = (char)(0x80 | ((cp >> 6) & 0x3f));
                    buf[n++] = (char)(0x80 | (cp & 0x3f));
                    continue;
                }
            } else if (e == '"' || e == '\\' || e == '/') {
                c = (unsigned char)e;
            } else if (e == 'b') {
                c = '\b';
            } else if (e == 'f') {
                c = '\f';
            } else if (e == 'n') {
                c = '\n';
            } else if (e == 'r') {
                c = '\r';
            } else if (e == 't') {
                c = '\t';
            } else {
                break;
            }
        }
        if (n + 1 >= cap) {
            cap *= 2;
            buf = (char *)realloc(buf, cap);
            if (buf == NULL) {
                ps->error = "memory";
                return NULL;
            }
        }
        buf[n++] = (char)c;
    }
    free(buf);
    ps->error = "string";
    return NULL;
}

static Json *parse_number(Parse *ps)
{
    int neg = 0;
    long long v = 0;
    Json *out;
    if (ps->p < ps->end && *ps->p == '-') {
        neg = 1;
        ps->p++;
    }
    if (ps->p >= ps->end || !isdigit((unsigned char)*ps->p)) {
        ps->error = "number";
        return NULL;
    }
    if (*ps->p == '0') {
        ps->p++;
    } else {
        while (ps->p < ps->end && isdigit((unsigned char)*ps->p)) {
            v = v * 10 + (*ps->p - '0');
            ps->p++;
        }
    }
    if (ps->p < ps->end && (*ps->p == '.' || *ps->p == 'e' || *ps->p == 'E')) {
        ps->error = "number";
        return NULL;
    }
    out = make(JSON_NUM);
    if (out == NULL) {
        ps->error = "memory";
        return NULL;
    }
    out->number = neg ? -v : v;
    return out;
}

static Json *parse_array(Parse *ps)
{
    Json *arr = make(JSON_ARR);
    if (arr == NULL) {
        ps->error = "memory";
        return NULL;
    }
    ps->p++;
    skip(ps);
    if (ps->p < ps->end && *ps->p == ']') {
        ps->p++;
        return arr;
    }
    for (;;) {
        Json *child = parse_value(ps);
        if (child == NULL || push(arr, NULL, child) != 0) {
            json_free(child);
            json_free(arr);
            if (ps->error == NULL) {
                ps->error = "array";
            }
            return NULL;
        }
        skip(ps);
        if (ps->p < ps->end && *ps->p == ',') {
            ps->p++;
            continue;
        }
        if (ps->p < ps->end && *ps->p == ']') {
            ps->p++;
            return arr;
        }
        json_free(arr);
        ps->error = "array";
        return NULL;
    }
}

static Json *parse_object(Parse *ps)
{
    Json *obj = make(JSON_OBJ);
    if (obj == NULL) {
        ps->error = "memory";
        return NULL;
    }
    ps->p++;
    skip(ps);
    if (ps->p < ps->end && *ps->p == '}') {
        ps->p++;
        return obj;
    }
    for (;;) {
        Json *key;
        Json *child;
        skip(ps);
        key = parse_string(ps);
        if (key == NULL) {
            json_free(obj);
            return NULL;
        }
        skip(ps);
        if (ps->p >= ps->end || *ps->p != ':') {
            json_free(key);
            json_free(obj);
            ps->error = "object";
            return NULL;
        }
        ps->p++;
        child = parse_value(ps);
        if (child == NULL || push(obj, key->string, child) != 0) {
            json_free(child);
            key->string = NULL;
            json_free(key);
            json_free(obj);
            if (ps->error == NULL) {
                ps->error = "object";
            }
            return NULL;
        }
        key->string = NULL;
        json_free(key);
        skip(ps);
        if (ps->p < ps->end && *ps->p == ',') {
            ps->p++;
            continue;
        }
        if (ps->p < ps->end && *ps->p == '}') {
            ps->p++;
            return obj;
        }
        json_free(obj);
        ps->error = "object";
        return NULL;
    }
}

static int keyword(Parse *ps, const char *word)
{
    size_t n = strlen(word);
    if ((size_t)(ps->end - ps->p) < n || memcmp(ps->p, word, n) != 0) {
        return 0;
    }
    ps->p += n;
    return 1;
}

static Json *parse_value(Parse *ps)
{
    skip(ps);
    if (ps->p >= ps->end) {
        ps->error = "value";
        return NULL;
    }
    if (*ps->p == '"') {
        return parse_string(ps);
    }
    if (*ps->p == '{') {
        return parse_object(ps);
    }
    if (*ps->p == '[') {
        return parse_array(ps);
    }
    if (*ps->p == 't' && keyword(ps, "true")) {
        Json *v = make(JSON_BOOL);
        if (v) {
            v->boolean = 1;
        }
        return v;
    }
    if (*ps->p == 'f' && keyword(ps, "false")) {
        Json *v = make(JSON_BOOL);
        if (v) {
            v->boolean = 0;
        }
        return v;
    }
    if (*ps->p == 'n' && keyword(ps, "null")) {
        return make(JSON_NULL);
    }
    if (*ps->p == '-' || isdigit((unsigned char)*ps->p)) {
        return parse_number(ps);
    }
    ps->error = "value";
    return NULL;
}

Json *json_parse(const char *text, const char **error)
{
    Parse ps;
    Json *v;
    ps.p = text;
    ps.end = text + strlen(text);
    ps.error = NULL;
    v = parse_value(&ps);
    if (v == NULL) {
        if (error) {
            *error = ps.error ? ps.error : "parse";
        }
        return NULL;
    }
    skip(&ps);
    if (ps.p != ps.end) {
        json_free(v);
        if (error) {
            *error = "trailing";
        }
        return NULL;
    }
    if (error) {
        *error = NULL;
    }
    return v;
}

void json_free(Json *value)
{
    int i;
    if (value == NULL) {
        return;
    }
    free(value->string);
    for (i = 0; i < value->count; i++) {
        json_free(value->child[i]);
        if (value->key) {
            free(value->key[i]);
        }
    }
    free(value->child);
    free(value->key);
    free(value);
}

const Json *json_object_get(const Json *object, const char *key)
{
    int i;
    if (object == NULL || object->type != JSON_OBJ) {
        return NULL;
    }
    for (i = 0; i < object->count; i++) {
        if (strcmp(object->key[i], key) == 0) {
            return object->child[i];
        }
    }
    return NULL;
}

const char *json_as_string(const Json *value)
{
    if (value == NULL || value->type != JSON_STR) {
        return NULL;
    }
    return value->string;
}

int json_as_i64(const Json *value, long long *out)
{
    if (value == NULL || value->type != JSON_NUM || out == NULL) {
        return -1;
    }
    *out = value->number;
    return 0;
}

int json_is_null(const Json *value)
{
    return value != NULL && value->type == JSON_NULL;
}
