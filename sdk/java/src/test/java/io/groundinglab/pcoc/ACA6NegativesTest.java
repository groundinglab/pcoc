package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

import java.util.LinkedHashMap;
import java.util.Map;

import org.junit.jupiter.api.Test;

import io.groundinglab.pcoc.negatives.NegativeHike;
import io.groundinglab.pcoc.negatives.Negatives;
import io.groundinglab.pcoc.produce.Cases;
import io.groundinglab.pcoc.produce.Hike;

/** AC-A6. The 13 permanent negatives are the erratum reference values and fail the six-leg check. */
public class ACA6NegativesTest {
    @Test
    public void valuesAndSixLeg() throws Exception {
        Repo.root();
        Sha256 sha = new Sha256();
        Profile profile = Cases.s1();
        Hike.Result hike = Cases.run(profile, Cases.PK_A, Cases.legs(1));
        long s0 = Bytes.getLe64(hike.seed, 0);
        long[] lanes = Seed.lanes(hike.seed);
        Map<String, String> got = new LinkedHashMap<String, String>();
        got.put("honest", Negatives.legsLeHex(hike.stages[0].legResults[0]));
        got.put("slot_summary_fold", Negatives.legsLeHex(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.SLOT_SUMMARY)));
        got.put("xor_accumulator_fold", Negatives.legsLeHex(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.XOR_ACCUMULATOR)));
        got.put("xor_w_fold", Negatives.legsLeHex(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.XOR_W)));
        got.put("s_plus_w_fold", Negatives.legsLeHex(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.S_PLUS_W)));
        got.put("prefold_hoist", Negatives.legsLeHex(Negatives.prefoldHoist(hike.terrain, profile, sha, s0)));
        long hoist = Negatives.leg(hike.terrain, profile, sha, s0, Negatives.FINAL_STAGE);
        long xor = Negatives.leg(hike.terrain, profile, sha, s0, Negatives.XOR_OF_MIX64);
        assertEquals(hoist, xor);
        got.put("final_stage_hoist", Negatives.legsLeHex(hoist));
        got.put("partial_slot_read", Negatives.legsLeHex(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.PARTIAL_SLOT)));
        got.put("partial_slot_read_2047", Negatives.legsLeHex(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.PARTIAL_2047)));
        long[] shared = Negatives.otherTerrain(profile, sha, Cases.PK_B, hike.seedBeacon);
        got.put("f3_shared_terrain", Negatives.legsLeHex(Leg.legResult(shared, profile, sha, s0)));
        got.put("cached_leg", Negatives.legsLeHex(Negatives.cachedLeg(hike.terrain, profile, sha, s0)));
        got.put("one_lane_flip", Bytes.hex(Terrain.digest(sha, Negatives.oneLane(profile, lanes))));
        got.put("all4_kernel", Bytes.hex(Terrain.digest(sha, Negatives.all4(profile, lanes))));
        got.put("squeezed_seed_terrain", Bytes.hex(Terrain.digest(sha, Negatives.squeezed(profile, lanes))));

        Map<String, String> expect = new LinkedHashMap<String, String>();
        expect.put("honest", "4c9a6a98574cbd31");
        expect.put("slot_summary_fold", "b87ce6fc485dd57c");
        expect.put("xor_accumulator_fold", "346188d2d8cf98e8");
        expect.put("xor_w_fold", "73a4c7fbd105aae1");
        expect.put("s_plus_w_fold", "07262df2d90e8ecb");
        expect.put("prefold_hoist", "a652562037b465fc");
        expect.put("final_stage_hoist", "b1808c8d3927a7d1");
        expect.put("partial_slot_read", "e1ae647ee5a5d7c5");
        expect.put("partial_slot_read_2047", "738755dabac824ab");
        expect.put("f3_shared_terrain", "a73019b8a2c43cd1");
        expect.put("cached_leg", "3b715e2a251a44d9");
        expect.put("one_lane_flip", "5c3eaeb82045b81036d39bc59d0390b738951b0ba7fbadda118388d0e3070608");
        expect.put("all4_kernel", "2d7410d07dc6b0e3f8659345a546f5570499a4c423d3d6f6f1df18f53a5cbb6e");
        expect.put("squeezed_seed_terrain", "ed0fc97468d0bc1675729776307ee35b9f9a03e2b5f6a671bd7afabf068e5c22");
        assertEquals(expect, got);
        String honest = got.get("honest");
        String[] values = got.values().toArray(new String[0]);
        for (int i = 1; i < values.length; i++) {
            assertNotEquals(honest, values[i]);
            for (int j = i + 1; j < values.length; j++) {
                assertNotEquals(values[i], values[j]);
            }
        }

        Index index = new SyntheticIndex();
        byte[] honestObj = Cases.encode(hike);
        assertEquals("match", six(honestObj, index));
        assertEquals("mismatch", six(NegativeHike.s1(NegativeHike.ofFold(Negatives.SLOT_SUMMARY), null), index));
        assertEquals("mismatch", six(NegativeHike.s1(NegativeHike.ofFold(Negatives.XOR_ACCUMULATOR), null), index));
        assertEquals("mismatch", six(NegativeHike.s1(NegativeHike.ofFold(Negatives.XOR_W), null), index));
        assertEquals("mismatch", six(NegativeHike.s1(NegativeHike.ofFold(Negatives.S_PLUS_W), null), index));
        assertEquals("mismatch", six(NegativeHike.s1(new NegativeHike.LegFn() {
            public long leg(long[] terrain, Profile p, Sha256 s, long state) {
                return Negatives.prefoldHoist(terrain, p, s, state);
            }
        }, null), index));
        assertEquals("mismatch", six(NegativeHike.s1(NegativeHike.ofFold(Negatives.FINAL_STAGE), null), index));
        assertEquals("mismatch", six(NegativeHike.s1(NegativeHike.ofFold(Negatives.PARTIAL_SLOT), null), index));
        assertEquals("mismatch", six(NegativeHike.s1(NegativeHike.ofFold(Negatives.PARTIAL_2047), null), index));
        assertEquals("mismatch", six(NegativeHike.s1(new NegativeHike.LegFn() {
            public long leg(long[] terrain, Profile p, Sha256 s, long state) {
                return Leg.legResult(terrain, p, s, state);
            }
        }, shared), index));
        assertEquals("mismatch", six(NegativeHike.s1(new NegativeHike.LegFn() {
            public long leg(long[] terrain, Profile p, Sha256 s, long state) {
                return Negatives.cachedLeg(terrain, p, s, state);
            }
        }, null), index));
    }

    private static String six(byte[] object, Index index) {
        Sha256 sha = new Sha256();
        byte[] b0 = Cases.asciiHash(sha, "pcoc-pack-2.2/draw");
        Verify.Report report = Verify.verifyRig(object, Cases.s1(), index, new Draw.HashChain(sha, b0));
        assertEquals(true, report.decoded);
        return report.sixLeg;
    }
}
