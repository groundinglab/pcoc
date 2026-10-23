// SPEC §6 constants and the §16.1 conformance-small profile arithmetic.

export const SEAL_DOMAIN = Buffer.from('pcoc-walk-v1', 'ascii');
export const KIND = 'v2.2-pcoc-object';
export const FILL = 'dag2';
export const STAGES = 18;

export const MIX_ADD = 0x9e3779b97f4a7c15n;
export const MIX_MUL1 = 0xbf58476d1ce4e5b9n;
export const MIX_MUL2 = 0x94d049bb133111ebn;
export const WORD_XOR = 0xc0ffeen;
export const PARENT_MUL = 0xd1b54a32d192ed03n;
export const SLOT_MUL = 0xa24baed4963ee407n;

export const ANCHOR_WINDOW_OFFSETS = [2, 3, 4] as const;
export const HASHCASH_DIFFICULTY = 21;

export const MAX_SAFE = (1n << 53n) - 1n;
export const U64 = (1n << 64n) - 1n;

export interface Profile {
  readonly name: 'production' | 'conformance-small';
  readonly slotBytes: number;
  readonly sampleBytes: number;
  readonly terrainBytes: number;
  readonly stepsPerLeg: number;
  readonly wordsPerSlot: number;
  readonly nSlots: number;
  readonly offsetModulus: number;
}

const PROD = { slotBytes: 16384, sampleBytes: 1024, terrainBytes: 268435456, stepsPerLeg: 2000000 };

/** Builds a profile and enforces the §16.1 range constraints. */
export function makeProfile(
  name: Profile['name'],
  slotBytes: number,
  sampleBytes: number,
  terrainBytes: number,
  stepsPerLeg: number,
): Profile {
  const ints = { slotBytes, sampleBytes, terrainBytes, stepsPerLeg };
  for (const [k, v] of Object.entries(ints)) {
    if (!Number.isSafeInteger(v) || v < 1) throw new RangeError(`${k} must be a positive integer`);
  }
  if (slotBytes < 8 || slotBytes % 8 !== 0 || slotBytes > PROD.slotBytes) {
    throw new RangeError('SLOT_BYTES must be a multiple of 8 in 8..16384');
  }
  if (stepsPerLeg > PROD.stepsPerLeg) throw new RangeError('STEPS_PER_LEG must be at most 2000000');
  if (sampleBytes > PROD.sampleBytes) throw new RangeError('SAMPLE_BYTES must be at most 1024');
  if (terrainBytes > PROD.terrainBytes || terrainBytes % slotBytes !== 0 || terrainBytes < sampleBytes) {
    throw new RangeError('TERRAIN_BYTES must be a multiple of SLOT_BYTES, at least SAMPLE_BYTES, at most 268435456');
  }
  return {
    name,
    slotBytes,
    sampleBytes,
    terrainBytes,
    stepsPerLeg,
    wordsPerSlot: slotBytes / 8,
    nSlots: terrainBytes / slotBytes,
    offsetModulus: terrainBytes - sampleBytes + 1,
  };
}

export const PRODUCTION: Profile = makeProfile(
  'production',
  PROD.slotBytes,
  PROD.sampleBytes,
  PROD.terrainBytes,
  PROD.stepsPerLeg,
);
/** CASES.md fixture S1 (`kat-small`). */
export const S1: Profile = makeProfile('conformance-small', 64, 32, 65536, 100);
/** CASES.md fixture S2 (`test-3leg`). */
export const S2: Profile = makeProfile('conformance-small', 256, 64, 1048576, 1000);

export function profileEquals(a: Profile, b: Profile): boolean {
  return (
    a.slotBytes === b.slotBytes &&
    a.sampleBytes === b.sampleBytes &&
    a.terrainBytes === b.terrainBytes &&
    a.stepsPerLeg === b.stepsPerLeg
  );
}
