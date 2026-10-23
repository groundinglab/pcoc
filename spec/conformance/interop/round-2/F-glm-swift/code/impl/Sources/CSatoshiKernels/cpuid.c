#include "shk.h"
#include <string.h>

#if defined(__x86_64__) || defined(__i386__)
#include <cpuid.h>
#endif

int shk_cpu_has_sha_ni(void) {
#if defined(__x86_64__)
    return __builtin_cpu_supports("sha") && __builtin_cpu_supports("sse4.1") &&
           __builtin_cpu_supports("ssse3");
#else
    return 0;
#endif
}

int shk_cpu_has_avx2(void) {
#if defined(__x86_64__)
    return __builtin_cpu_supports("avx2");
#else
    return 0;
#endif
}

int shk_cpu_has_avx512dq(void) {
#if defined(__x86_64__)
    return __builtin_cpu_supports("avx512f") && __builtin_cpu_supports("avx512dq") &&
           __builtin_cpu_supports("avx512vl");
#else
    return 0;
#endif
}

void shk_cpu_brand(char *buf, size_t buflen) {
    buf[0] = '\0';
#if defined(__x86_64__) || defined(__i386__)
    {
        uint32_t a, b, c, d;
        char brand[49];
        uint32_t *p = (uint32_t *)brand;
        __cpuid(0x80000002u, a, b, c, d);
        p[0] = a; p[1] = b; p[2] = c; p[3] = d;
        __cpuid(0x80000003u, a, b, c, d);
        p[4] = a; p[5] = b; p[6] = c; p[7] = d;
        __cpuid(0x80000004u, a, b, c, d);
        p[8] = a; p[9] = b; p[10] = c; p[11] = d;
        brand[48] = '\0';
        strncpy(buf, brand, buflen - 1);
        buf[buflen - 1] = '\0';
    }
#endif
}

/* OS CSPRNG read for the section 14.6 draw (getrandom on Linux). */
#if defined(__linux__)
#include <sys/random.h>
#include <errno.h>
#endif
int shk_os_getrandom(uint8_t *buf, size_t n) {
#if defined(__linux__)
    size_t done = 0;
    while (done < n) {
        ssize_t r = getrandom(buf + done, n - done, 0);
        if (r <= 0) {
            if (errno == EINTR) continue;
            return -1;
        }
        done += (size_t)r;
    }
    return 0;
#else
    return -1;
#endif
}
