package io.groundinglab.pcoc.negatives;

import io.groundinglab.pcoc.Bytes;
import io.groundinglab.pcoc.Checkpoint;
import io.groundinglab.pcoc.Const;
import io.groundinglab.pcoc.HikeObject;
import io.groundinglab.pcoc.Profile;
import io.groundinglab.pcoc.Seed;
import io.groundinglab.pcoc.Sha256;
import io.groundinglab.pcoc.Terrain;
import io.groundinglab.pcoc.Waypoint;
import io.groundinglab.pcoc.produce.Cases;
import io.groundinglab.pcoc.produce.Hike;

/**
 * A scripted-rig object whose legs use a negative leg function.
 * Checkpoints bind the negative leg result, so §14.3 still hashes that limb.
 * The §14.6 recomputation uses §11.1 and therefore disagrees.
 */
public final class NegativeHike {
    private NegativeHike() {}

    /** One leg from {@code S_0}. */
    public interface LegFn {
        long leg(long[] terrain, Profile profile, Sha256 sha, long s0);
    }

    public static LegFn ofFold(final Negatives.Fold fold) {
        return new LegFn() {
            public long leg(long[] terrain, Profile profile, Sha256 sha, long s0) {
                return Negatives.leg(terrain, profile, sha, s0, fold);
            }
        };
    }

    /**
     * S1 object for {@code pk_A}. {@code terrainOverride} null uses {@code pk_A}'s terrain.
     * The object still stores {@code pk_A} and the seed beacon.
     */
    public static byte[] s1(LegFn legs, long[] terrainOverride) {
        Sha256 sha = new Sha256();
        Profile profile = Cases.s1();
        byte[] beacon = Cases.seedBeacon(sha);
        byte[] seed = Seed.derive(sha, Cases.PK_A, beacon);
        long[] terrain = terrainOverride == null ? Terrain.fill(profile, Seed.lanes(seed)) : terrainOverride;
        byte[][] interrupts = new byte[Const.STAGES][];
        long[] heights = new long[Const.STAGES];
        for (int k = 0; k < Const.STAGES; k++) {
            interrupts[k] = Cases.interrupt(sha, k);
            heights[k] = Cases.interruptHeight(k);
        }
        HikeObject.Stage[] stages = new HikeObject.Stage[Const.STAGES];
        byte[] waypoint = seed;
        byte[] stageBeacon = beacon;
        long beaconHeight = Cases.SEED_HEIGHT;
        byte[] lastInterrupt = null;
        long lastHeight = 0L;
        int[] counts = Cases.legs(1);
        for (int g = 0; g < Const.STAGES; g++) {
            byte[] limb = waypoint;
            byte[] previous = waypoint;
            byte[][] cps = new byte[counts[g]][];
            long[] results = new long[counts[g]];
            for (int leg = 0; leg < counts[g]; leg++) {
                long s0 = Bytes.getLe64(previous, 0);
                long legResult = legs.leg(terrain, profile, sha, s0);
                int off = (int) Long.remainderUnsigned(s0, profile.offsetModulus);
                byte[] sample = Terrain.window(terrain, off, profile.sampleBytes);
                byte[] le = Bytes.le64(legResult);
                cps[leg] = Checkpoint.digest(sha, previous, limb, sample, le);
                results[leg] = legResult;
                previous = cps[leg];
            }
            HikeObject.Stage stage = new HikeObject.Stage();
            stage.beacon = stageBeacon;
            stage.beaconHeight = beaconHeight;
            stage.waypoint = waypoint;
            stage.checkpoints = cps;
            stage.legResults = results;
            stage.path = g == 0 ? null : new byte[0];
            stages[g] = stage;
            if (g < 17) {
                byte[] interrupt = interrupts[g];
                byte[] fold = Waypoint.fold(sha, cps);
                int slot = Waypoint.slotIndex(interrupt, profile.nSlots);
                byte[] slotSample = Terrain.slotBytes(terrain, slot, profile.wordsPerSlot);
                waypoint = Waypoint.compute(sha, interrupt, slotSample, stage.waypoint, fold);
                stageBeacon = interrupt;
                beaconHeight = heights[g];
            } else {
                lastInterrupt = interrupts[17];
                lastHeight = heights[17];
            }
        }
        Hike.Result hike = new Hike.Result(profile, Cases.PK_A, beacon, Cases.SEED_HEIGHT, seed, terrain,
                stages, lastInterrupt, lastHeight);
        return Cases.encode(hike);
    }
}
