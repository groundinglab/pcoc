#ifndef PCOC_NEGATIVES_H
#define PCOC_NEGATIVES_H
#include "pcoc.h"
int pcoc_neg_leg_value(const char *name, const pcoc_sizes *sizes, const uint8_t *terrain_a,
                       const uint8_t *terrain_b, const uint8_t prev[32], uint64_t *out);
int pcoc_negatives_write(const pcoc_sizes *sizes, const uint64_t PA[4], const uint64_t PB[4],
                         const uint8_t *terrain_a, const uint8_t prev[32], char *out, size_t cap);
#endif
