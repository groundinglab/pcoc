/* External timing harness for GLM's Swift build (R2-F). Links the C kernel objects that
   `swift build -c release` produced (unchanged) and calls the same dispatching entry points
   the SDK uses. Needed because SHikeCLI bench calls shk_terrain_fill_avx512 unconditionally
   (SIGILL on CPUs without AVX-512). usage: glm_bench <D hex 64> */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>
#include "shk.h"
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
int main(int c,char**v){
  uint8_t D[32]; for(int i=0;i<32;i++) sscanf(v[1]+2*i,"%2hhx",&D[i]);
  uint64_t P[4]; memcpy(P,D,32); /* little-endian lanes */
  size_t size=268435456; uint8_t *a=aligned_alloc(64,size),*b=aligned_alloc(64,size);
  memset(a,0,size); memset(b,0,size);
  double t0=now(); shk_terrain_fill_scalar(a,16384,2048,P); double ts=now()-t0;
  t0=now(); shk_terrain_fill(b,16384,2048,P); double td=now()-t0;
  printf("fill_scalar_s=%.3f fill_dispatch_s=%.3f identical=%d avx512dq=%d avx2=%d\n",ts,td,memcmp(a,b,size)==0,shk_cpu_has_avx512dq(),shk_cpu_has_avx2());
  uint64_t S0; memcpy(&S0,D,8);
  t0=now(); uint64_t r0=shk_leg_run(b,16384,2048,2000000,S0,0); double l0=now()-t0;
  t0=now(); uint64_t r1=shk_leg_run(b,16384,2048,2000000,S0,1); double l1=now()-t0;
  uint8_t le[8]; memcpy(le,&r1,8);
  printf("leg_scalar_s=%.3f leg_avx2_s=%.3f equal=%d leg_le_hex=",l0,l1,r0==r1); for(int i=0;i<8;i++)printf("%02x",le[i]); printf("\n");
  return 0; }
