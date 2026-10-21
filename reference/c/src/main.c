#include "hike.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static void usage(void)
{
    fprintf(stderr,
            "usage:\n"
            "  hike info\n"
            "  hike produce <schedule>\n"
            "  hike verify <record> [--leg N]...\n"
            "  hike rate --legs N --span N [--p 1 --q 5]\n"
            "  hike anchors <i,i,...>\n"
            "  hike bench\n"
            "\n"
            "schedule lines:\n"
            "  public_key <64 hex>\n"
            "  seed_beacon <64 hex>\n"
            "  slot_bytes <u32>\n"
            "  terrain_bytes <u32>\n"
            "  sample_bytes <u32>\n"
            "  steps_per_leg <u64>\n"
            "  stage <leg_count> <64 hex interrupt, or - on the last stage>\n");
}

static int cmd_info(void)
{
    printf("sha256_hw %d\n", hike_sha256_hw());
    printf("sha256_hw_fault %d\n", hike_sha256_hw_fault());
    printf("slot_bytes %d\n", HIKE_SLOT_BYTES);
    printf("terrain_bytes %d\n", HIKE_TERRAIN_BYTES);
    printf("sample_bytes %d\n", HIKE_SAMPLE_BYTES);
    printf("steps_per_leg %d\n", HIKE_STEPS_PER_LEG);
    printf("n_slots %d\n", HIKE_N_SLOTS);
    printf("words_per_slot %d\n", HIKE_WORDS_PER_SLOT);
    printf("stages %d\n", HIKE_STAGES);
    return 0;
}

static int cmd_produce(const char *path)
{
    FILE *fp;
    char key[100], val[200];
    uint8_t pk[32], beacon[32];
    HikeProfile profile;
    HikeStageIn *sched = NULL;
    uint32_t n = 0, cap = 0;
    HikeRecord rec;
    int saw_pk = 0, saw_beacon = 0, saw_profile = 0;
    int rc = 1;

    memset(&profile, 0, sizeof profile);
    memset(&rec, 0, sizeof rec);
    fp = fopen(path, "r");
    if (fp == NULL) {
        fprintf(stderr, "cannot read %s\n", path);
        return 1;
    }
    while (fscanf(fp, "%99s", key) == 1) {
        if (strcmp(key, "public_key") == 0) {
            if (fscanf(fp, "%199s", val) != 1 || hike_hex_decode(val, pk, 32) != 0) {
                fprintf(stderr, "public_key\n");
                goto done;
            }
            saw_pk = 1;
        } else if (strcmp(key, "seed_beacon") == 0) {
            if (fscanf(fp, "%199s", val) != 1 || hike_hex_decode(val, beacon, 32) != 0) {
                fprintf(stderr, "seed_beacon\n");
                goto done;
            }
            saw_beacon = 1;
        } else if (strcmp(key, "slot_bytes") == 0) {
            if (fscanf(fp, "%u", &profile.slot_bytes) != 1) {
                goto done;
            }
        } else if (strcmp(key, "terrain_bytes") == 0) {
            if (fscanf(fp, "%u", &profile.terrain_bytes) != 1) {
                goto done;
            }
        } else if (strcmp(key, "sample_bytes") == 0) {
            if (fscanf(fp, "%u", &profile.sample_bytes) != 1) {
                goto done;
            }
        } else if (strcmp(key, "steps_per_leg") == 0) {
            unsigned long long steps;
            if (fscanf(fp, "%llu", &steps) != 1) {
                goto done;
            }
            profile.steps_per_leg = (uint64_t)steps;
            saw_profile = 1;
        } else if (strcmp(key, "stage") == 0) {
            unsigned legs;
            HikeStageIn *grown;
            if (fscanf(fp, "%u %199s", &legs, val) != 2 || legs < 1) {
                fprintf(stderr, "stage\n");
                goto done;
            }
            if (n == cap) {
                cap = cap ? cap * 2 : 4;
                grown = (HikeStageIn *)realloc(sched, cap * sizeof(HikeStageIn));
                if (grown == NULL) {
                    goto done;
                }
                sched = grown;
            }
            memset(&sched[n], 0, sizeof sched[n]);
            sched[n].leg_count = legs;
            if (strcmp(val, "-") == 0) {
                sched[n].has_interrupt = 0;
            } else if (hike_hex_decode(val, sched[n].interrupt, 32) != 0) {
                fprintf(stderr, "interrupt\n");
                goto done;
            } else {
                sched[n].has_interrupt = 1;
            }
            n++;
        } else {
            fprintf(stderr, "unknown %s\n", key);
            goto done;
        }
    }
    if (!saw_pk || !saw_beacon || !saw_profile || n == 0 || !hike_profile_ok(&profile)) {
        fprintf(stderr, "schedule incomplete\n");
        goto done;
    }
    if (hike_produce(pk, beacon, &profile, sched, n, &rec) != HIKE_OK) {
        fprintf(stderr, "produce failed\n");
        goto done;
    }
    if (hike_record_write(stdout, &rec) != HIKE_OK) {
        fprintf(stderr, "write failed\n");
        hike_record_free(&rec);
        goto done;
    }
    hike_record_free(&rec);
    rc = 0;
done:
    free(sched);
    fclose(fp);
    return rc;
}

static int cmd_verify(int argc, char **argv)
{
    FILE *fp;
    HikeRecord rec;
    uint64_t only_buf[64];
    uint64_t *only = NULL;
    size_t n_only = 0;
    int i;
    const char *why = NULL;
    uint64_t at = 0;
    int vr;

    if (argc < 3) {
        usage();
        return 2;
    }
    memset(&rec, 0, sizeof rec);
    for (i = 3; i < argc; i++) {
        if (strcmp(argv[i], "--leg") == 0 && i + 1 < argc) {
            unsigned long long idx;
            if (n_only >= 64) {
                fprintf(stderr, "too many --leg\n");
                return 2;
            }
            idx = strtoull(argv[++i], NULL, 10);
            only_buf[n_only++] = (uint64_t)idx;
        } else {
            usage();
            return 2;
        }
    }
    fp = fopen(argv[2], "r");
    if (fp == NULL) {
        fprintf(stderr, "cannot read %s\n", argv[2]);
        return 1;
    }
    if (hike_record_read(fp, &rec) != HIKE_OK) {
        fprintf(stderr, "record\n");
        fclose(fp);
        return 1;
    }
    fclose(fp);
    if (n_only > 0) {
        only = only_buf;
    }
    vr = hike_verify(&rec, only, n_only, &at, &why);
    hike_record_free(&rec);
    if (vr == HIKE_OK) {
        printf("legs recomputed\n");
        return 0;
    }
    fprintf(stderr, "mismatch %s %llu\n", why ? why : "?", (unsigned long long)at);
    return 1;
}

static int cmd_rate(int argc, char **argv)
{
    uint64_t legs = 0, p = 1, q = 5;
    int64_t span = 0;
    int saw_legs = 0, saw_span = 0;
    int meets = 0;
    int i;

    for (i = 2; i < argc; i++) {
        if (strcmp(argv[i], "--legs") == 0 && i + 1 < argc) {
            legs = strtoull(argv[++i], NULL, 10);
            saw_legs = 1;
        } else if (strcmp(argv[i], "--span") == 0 && i + 1 < argc) {
            span = strtoll(argv[++i], NULL, 10);
            saw_span = 1;
        } else if (strcmp(argv[i], "--p") == 0 && i + 1 < argc) {
            p = strtoull(argv[++i], NULL, 10);
        } else if (strcmp(argv[i], "--q") == 0 && i + 1 < argc) {
            q = strtoull(argv[++i], NULL, 10);
        } else {
            usage();
            return 2;
        }
    }
    if (!saw_legs || !saw_span) {
        usage();
        return 2;
    }
    if (hike_rate_meets(legs, span, p, q, &meets) != HIKE_OK) {
        return 1;
    }
    printf("hike_legs %llu\n", (unsigned long long)legs);
    printf("hike_span %lld\n", (long long)span);
    printf("floor %llu/%llu\n", (unsigned long long)p, (unsigned long long)q);
    printf("meets %d\n", meets);
    return 0;
}

static int cmd_anchors(const char *text)
{
    uint32_t vals[64];
    uint32_t n = 0;
    const char *p = text;
    if (text == NULL || text[0] == 0) {
        usage();
        return 2;
    }
    while (*p) {
        char *end = NULL;
        unsigned long v = strtoul(p, &end, 10);
        if (end == p || n >= 64) {
            usage();
            return 2;
        }
        vals[n++] = (uint32_t)v;
        if (*end == ',') {
            p = end + 1;
        } else if (*end == 0) {
            break;
        } else {
            usage();
            return 2;
        }
    }
    printf("anchored_total %u\n", hike_anchor_count(vals, n));
    printf("meets %d\n", hike_anchors_meet(vals, n));
    return 0;
}

static int cmd_bench(void)
{
    HikeProfile profile;
    uint8_t pk[32], beacon[32], seed[32], previous[32];
    uint8_t *terrain;
    struct timespec a, b;
    double fill_s, leg_s;
    uint64_t result;
    char hex[17];
    FILE *cpu;

    memset(pk, 0x11, 32);
    memset(beacon, 0x22, 32);
    hike_seed(pk, beacon, seed);
    memcpy(previous, seed, 32);
    profile.slot_bytes = HIKE_SLOT_BYTES;
    profile.terrain_bytes = HIKE_TERRAIN_BYTES;
    profile.sample_bytes = HIKE_SAMPLE_BYTES;
    profile.steps_per_leg = HIKE_STEPS_PER_LEG;
    terrain = (uint8_t *)malloc(profile.terrain_bytes);
    if (terrain == NULL) {
        fprintf(stderr, "terrain alloc\n");
        return 1;
    }
    fprintf(stderr, "bench fill %u bytes\n", profile.terrain_bytes);
    clock_gettime(CLOCK_MONOTONIC, &a);
    if (hike_terrain_fill(seed, profile.slot_bytes, profile.terrain_bytes, terrain) != HIKE_OK) {
        free(terrain);
        return 1;
    }
    clock_gettime(CLOCK_MONOTONIC, &b);
    fill_s = (double)(b.tv_sec - a.tv_sec) + (double)(b.tv_nsec - a.tv_nsec) / 1e9;
    fprintf(stderr, "bench leg %llu steps\n", (unsigned long long)profile.steps_per_leg);
    clock_gettime(CLOCK_MONOTONIC, &a);
    result = hike_leg(terrain, &profile, previous);
    clock_gettime(CLOCK_MONOTONIC, &b);
    leg_s = (double)(b.tv_sec - a.tv_sec) + (double)(b.tv_nsec - a.tv_nsec) / 1e9;
    free(terrain);
    hike_u64_hex(result, hex);
    cpu = fopen("/proc/cpuinfo", "r");
    if (cpu) {
        char line[256];
        while (fgets(line, sizeof line, cpu)) {
            if (strncmp(line, "model name", 10) == 0) {
                fputs(line, stdout);
                break;
            }
        }
        fclose(cpu);
    }
    printf("sha256_hw %d\n", hike_sha256_hw());
    printf("sha256_hw_fault %d\n", hike_sha256_hw_fault());
    printf("fill_s %.6f\n", fill_s);
    printf("leg_s %.6f\n", leg_s);
    printf("leg_result %s\n", hex);
    printf("steps_per_leg %llu\n", (unsigned long long)profile.steps_per_leg);
    printf("slot_bytes %u\n", profile.slot_bytes);
    printf("terrain_bytes %u\n", profile.terrain_bytes);
    return 0;
}

int main(int argc, char **argv)
{
    if (argc < 2) {
        usage();
        return 2;
    }
    if (strcmp(argv[1], "info") == 0) {
        return cmd_info();
    }
    if (strcmp(argv[1], "produce") == 0 && argc == 3) {
        return cmd_produce(argv[2]);
    }
    if (strcmp(argv[1], "verify") == 0) {
        return cmd_verify(argc, argv);
    }
    if (strcmp(argv[1], "rate") == 0) {
        return cmd_rate(argc, argv);
    }
    if (strcmp(argv[1], "anchors") == 0 && argc == 3) {
        return cmd_anchors(argv[2]);
    }
    if (strcmp(argv[1], "bench") == 0) {
        return cmd_bench();
    }
    usage();
    return 2;
}
