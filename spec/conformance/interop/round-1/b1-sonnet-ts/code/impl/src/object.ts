import { encode, type CborValue } from './cbor.ts';
import { KIND, FILL, nSlots, type Hike, type Profile } from './core.ts';

/** SPEC 7.3 / 8: encode a hike object. Stage 0 has no `path` key. */
export function encodeHike(h: Hike, p: Profile): Buffer {
  const stages = h.stages.map((s, g) => {
    const m = new Map<string, CborValue>();
    m.set('beacon', s.beacon);
    m.set('beacon_height', BigInt(s.beaconHeight));
    m.set('leg_count', BigInt(s.checkpoints.length));
    m.set('waypoint', s.waypoint);
    if (g >= 1) m.set('path', s.path ?? Buffer.alloc(0));
    m.set('checkpoints', s.checkpoints.map((c) => Uint8Array.from(c)));
    m.set('leg_results', s.legResults.map((c) => Uint8Array.from(c)));
    return m;
  });
  const top = new Map<string, CborValue>();
  top.set('kind', KIND);
  top.set('fill', FILL);
  top.set('slot_bytes', BigInt(p.slotBytes));
  top.set('n_slots', BigInt(nSlots(p)));
  top.set('sample_bytes', BigInt(p.sampleBytes));
  top.set('steps_per_leg', BigInt(p.stepsPerLeg));
  top.set('public_key', h.publicKey);
  top.set('seed_beacon', h.seedBeacon);
  top.set('stages', stages);
  top.set('last_interrupt', h.lastInterrupt);
  top.set('last_interrupt_height', BigInt(h.lastInterruptHeight));
  return encode(top);
}
