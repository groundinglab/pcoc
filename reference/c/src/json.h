#ifndef HIKE_JSON_H
#define HIKE_JSON_H

/* Minimal JSON reader for the v2.1 conformance files. */

typedef struct Json {
    enum { JSON_NULL, JSON_BOOL, JSON_NUM, JSON_STR, JSON_ARR, JSON_OBJ } type;
    int boolean;
    long long number;
    char *string;
    struct Json **child;
    char **key;
    int count;
} Json;

Json *json_parse(const char *text, const char **error);
void json_free(Json *value);

const Json *json_object_get(const Json *object, const char *key);
const char *json_as_string(const Json *value);
int json_as_i64(const Json *value, long long *out);
int json_is_null(const Json *value);

#endif
