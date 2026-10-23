// Hike object (SPEC §8): the model, the deterministic encoding (§7.3), and the §7.3 decode order.

import { CborError, decodeCbor, encodeCbor, type CborMap, type CborValue } from './cbor.js';
import { FILL, KIND, type Profile } from './profile.js';

export interface StageRecord {
  beacon: Uint8Array;
  beaconHeight: number;
  waypoint: Uint8Array;
  /** absent on stage 0 */
  path?: Uint8Array;
  checkpoints: Uint8Array[];
  legResults: Uint8Array[];
}

export interface HikeObject {
  kind: string;
  fill: string;
  slotBytes: number;
  nSlots: number;
  sampleBytes: number;
  stepsPerLeg: number;
  publicKey: Uint8Array;
  seedBeacon: Uint8Array;
  stages: StageRecord[];
  lastInterrupt: Uint8Array;
  lastInterruptHeight: number;
}

export function objectToCbor(o: HikeObject): Map<string, unknown> {
  const stages = o.stages.map((s, g) => {
    const m = new Map<string, unknown>([
      ['beacon', s.beacon],
      ['waypoint', s.waypoint],
      ['leg_count', s.checkpoints.length],
      ['checkpoints', s.checkpoints],
      ['leg_results', s.legResults],
      ['beacon_height', s.beaconHeight],
    ]);
    if (g > 0) m.set('path', s.path ?? new Uint8Array(0));
    return m;
  });
  return new Map<string, unknown>([
    ['kind', o.kind],
    ['fill', o.fill],
    ['slot_bytes', o.slotBytes],
    ['n_slots', o.nSlots],
    ['sample_bytes', o.sampleBytes],
    ['steps_per_leg', o.stepsPerLeg],
    ['public_key', o.publicKey],
    ['seed_beacon', o.seedBeacon],
    ['stages', stages],
    ['last_interrupt', o.lastInterrupt],
    ['last_interrupt_height', o.lastInterruptHeight],
  ]);
}

/** §7.3: the core deterministic encoding of the object. Map key order falls out of the bytewise key sort. */
export function encodeObject(o: HikeObject): Uint8Array {
  return encodeCbor(objectToCbor(o) as never);
}

export function constantsFor(profile: Profile): Pick<HikeObject, 'kind' | 'fill' | 'slotBytes' | 'nSlots' | 'sampleBytes' | 'stepsPerLeg'> {
  return {
    kind: KIND,
    fill: FILL,
    slotBytes: profile.slotBytes,
    nSlots: profile.nSlots,
    sampleBytes: profile.sampleBytes,
    stepsPerLeg: profile.stepsPerLeg,
  };
}

// ---- §7.3 decode order ------------------------------------------------------------------------

export type Classified =
  | { class: 'malformed'; reason: string }
  | { class: 'decode-failure' }
  | { class: 'unknown-kind'; kindText: string }
  | { class: 'object'; map: CborMap };

/**
 * First: not one definite-length CBOR data item under §7.3 -> malformed CBOR value (Rule A5 is not applied).
 * Second: a well-formed item that is not a map, or a map whose `kind` is absent or not text -> decode failure.
 * Third: `kind` text other than KIND -> Rule A5. Everything else is measured by §14.
 */
export function classify(bytes: Uint8Array): Classified {
  let v: CborValue;
  try {
    v = decodeCbor(bytes);
  } catch (e) {
    if (e instanceof CborError) return { class: 'malformed', reason: e.message };
    if (e instanceof RangeError) return { class: 'malformed', reason: 'resource limit' };
    throw e;
  }
  if (!(v instanceof Map)) return { class: 'decode-failure' };
  const kind = v.get('kind');
  if (typeof kind !== 'string') return { class: 'decode-failure' };
  if (kind !== KIND) return { class: 'unknown-kind', kindText: kind };
  return { class: 'object', map: v };
}

/** Rule A5 record bytes: `{"kind":"unknown kind","kind_text":<RFC 8785 string>}`. */
export function unknownKindRecord(kindText: string): string {
  return `{"kind":"unknown kind","kind_text":${jcsString(kindText)}}`;
}

/** RFC 8785 §3.2.2.2 string serialisation, which is the ECMAScript JSON.stringify form for a string. */
export function jcsString(s: string): string {
  return JSON.stringify(s);
}
