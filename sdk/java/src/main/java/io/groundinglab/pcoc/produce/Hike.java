package io.groundinglab.pcoc.produce;

import io.groundinglab.pcoc.Bytes;
import io.groundinglab.pcoc.Checkpoint;
import io.groundinglab.pcoc.Const;
import io.groundinglab.pcoc.Leg;
import io.groundinglab.pcoc.Profile;
import io.groundinglab.pcoc.Seed;
import io.groundinglab.pcoc.Sha256;
import io.groundinglab.pcoc.Terrain;
import io.groundinglab.pcoc.HikeObject;
import io.groundinglab.pcoc.Waypoint;

/**
 * §13.2 drive with a rig-supplied tip schedule. No network, no post, no wait.
 * Path bytes on stages 1–17 are the empty path bytes of §13.3.
 */
public final class Hike {
    private Hike() {}

    public static final class Result {
        public final Profile profile;
        public final byte[] publicKey;
        public final byte[] seedBeacon;
        public final long seedHeight;
        public final byte[] seed;
        public final long[] terrain;
        public final HikeObject.Stage[] stages;
        public final byte[] lastInterrupt;
        public final long lastInterruptHeight;

        public Result(Profile profile, byte[] publicKey, byte[] seedBeacon, long seedHeight,
               byte[] seed, long[] terrain, HikeObject.Stage[] stages, byte[] lastInterrupt, long lastInterruptHeight) {
            this.profile = profile;
            this.publicKey = publicKey;
            this.seedBeacon = seedBeacon;
            this.seedHeight = seedHeight;
            this.seed = seed;
            this.terrain = terrain;
            this.stages = stages;
            this.lastInterrupt = lastInterrupt;
            this.lastInterruptHeight = lastInterruptHeight;
        }
    }

    /**
     * §13.2 with injected interrupts. {@code interrupts[g]} is {@code I_g} for g = 0 … 17.
     * {@code legsPerStage} has 18 counts, each at least 1. Heights are not hashed (§9).
     */
    public static Result run(Profile profile, byte[] publicKey, byte[] seedBeacon, long seedHeight,
                             byte[][] interrupts, long[] interruptHeights, int[] legsPerStage) {
        if (interrupts.length != Const.STAGES || interruptHeights.length != Const.STAGES
                || legsPerStage.length != Const.STAGES) {
            throw new IllegalArgumentException("stages");
        }
        Sha256 sha = new Sha256();
        byte[] seed = Seed.derive(sha, publicKey, seedBeacon);
        long[] terrain = Terrain.fill(profile, Seed.lanes(seed));
        HikeObject.Stage[] stages = new HikeObject.Stage[Const.STAGES];
        byte[] waypoint = seed;
        byte[] beacon = seedBeacon;
        long beaconHeight = seedHeight;
        byte[] lastInterrupt = null;
        long lastHeight = 0L;
        for (int g = 0; g < Const.STAGES; g++) {
            if (legsPerStage[g] < 1) {
                throw new IllegalArgumentException("leg_count");
            }
            byte[] limb = waypoint;
            byte[] previous = waypoint;
            byte[][] cps = new byte[legsPerStage[g]][];
            long[] results = new long[legsPerStage[g]];
            for (int leg = 0; leg < legsPerStage[g]; leg++) {
                long s0 = Bytes.getLe64(previous, 0);
                long legResult = Leg.legResult(terrain, profile, sha, s0);
                int off = Leg.sampleOffset(previous, profile.offsetModulus);
                byte[] sample = Terrain.window(terrain, off, profile.sampleBytes);
                byte[] le = Bytes.le64(legResult);
                byte[] cp = Checkpoint.digest(sha, previous, limb, sample, le);
                cps[leg] = cp;
                results[leg] = legResult;
                previous = cp;
            }
            HikeObject.Stage stage = new HikeObject.Stage();
            stage.beacon = beacon;
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
                beacon = interrupt;
                beaconHeight = interruptHeights[g];
            } else {
                lastInterrupt = interrupts[17];
                lastHeight = interruptHeights[17];
            }
        }
        return new Result(profile, publicKey, seedBeacon, seedHeight, seed, terrain, stages,
                lastInterrupt, lastHeight);
    }
}
