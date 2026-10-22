/* Scripted hike run: terrain is the caller's, filled once per seed (§10.4).
 * Every leg is the §11.2 loop. No network, no pause.
 */
#include "pcoc.h"

#include <string.h>

int pcoc_hike_run(const pcoc_sizes *sizes, const uint8_t *terrain, const uint8_t public_key[32],
                  const uint8_t seed_beacon[32], const uint8_t beacons[19][32],
                  const uint64_t heights[19], const int legs[18], pcoc_object *obj) {
  if (!sizes || !terrain || !public_key || !seed_beacon || !beacons || !heights || !legs || !obj) {
    return -1;
  }
  memset(obj, 0, sizeof(*obj));
  uint8_t pre[64], D[32];
  uint64_t P[4];
  pcoc_seed(public_key, seed_beacon, pre, D, P);
  (void)P;

  memcpy(obj->kind, PCOC_KIND, strlen(PCOC_KIND));
  obj->kind_len = strlen(PCOC_KIND);
  obj->kind_is_text = 1;
  obj->has_kind = 1;
  memcpy(obj->fill, PCOC_FILL, 4);
  obj->fill_len = 4;
  obj->has_fill = 1;
  obj->slot_bytes = sizes->slot_bytes;
  obj->n_slots = sizes->n_slots;
  obj->sample_bytes = sizes->sample_bytes;
  obj->steps_per_leg = sizes->steps_per_leg;
  obj->has_slot_bytes = obj->has_n_slots = obj->has_sample_bytes = obj->has_steps_per_leg = 1;
  memcpy(obj->public_key, public_key, 32);
  obj->has_public_key = 1;
  memcpy(obj->seed_beacon, seed_beacon, 32);
  obj->has_seed_beacon = 1;
  obj->n_stages = PCOC_STAGES;
  obj->has_stages = 1;
  obj->cbor_class = PCOC_CBOR_OK;

  uint8_t cur[32];
  memcpy(cur, D, 32);
  for (int g = 0; g < (int)PCOC_STAGES; g++) {
    if (legs[g] < 1 || legs[g] > (int)PCOC_MAX_LEGS_PER_STAGE) {
      return -1;
    }
    pcoc_stage *st = &obj->stages[g];
    memcpy(st->beacon, beacons[g], 32);
    st->beacon_height = heights[g];
    st->leg_count = (uint64_t)legs[g];
    memcpy(st->waypoint, cur, 32);
    st->has_path = (g != 0);
    st->path_len = 0;
    st->n_checkpoints = st->leg_count;
    st->n_leg_results = st->leg_count;
    uint8_t previous[32];
    memcpy(previous, cur, 32);
    uint8_t cps[PCOC_MAX_LEGS_PER_STAGE * 32];
    for (int j = 0; j < legs[g]; j++) {
      uint64_t lr = pcoc_leg_result(sizes, terrain, previous);
      pcoc_le64(lr, st->leg_results[j]);
      pcoc_checkpoint(sizes, terrain, previous, cur, lr, st->checkpoints[j]);
      memcpy(cps + 32 * j, st->checkpoints[j], 32);
      memcpy(previous, st->checkpoints[j], 32);
    }
    if (g < 17) {
      uint8_t next[32];
      pcoc_waypoint(sizes, terrain, beacons[g + 1], cur, cps, (size_t)legs[g], next);
      memcpy(cur, next, 32);
    }
  }
  memcpy(obj->last_interrupt, beacons[18], 32);
  obj->last_interrupt_len = 32;
  obj->has_last_interrupt = 1;
  obj->last_interrupt_height = heights[18];
  obj->has_last_interrupt_height = 1;
  return 0;
}
