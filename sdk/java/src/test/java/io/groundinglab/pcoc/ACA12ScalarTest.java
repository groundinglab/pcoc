package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

import io.groundinglab.pcoc.produce.Cases;
import io.groundinglab.pcoc.produce.Hike;

/**
 * AC-A12. The only fold is the scalar §11.1 loop: every word, mix64(word XOR S), wrapping add.
 * The S1 step-0 accumulator is the agreed fold trace.
 */
public class ACA12ScalarTest {
    @Test
    public void s1StepZeroLoadsEveryWord() {
        Hike.Result hike = Cases.run(Cases.s1(), Cases.PK_A, Cases.legs(1));
        Profile profile = hike.profile;
        long s = Bytes.getLe64(hike.seed, 0);
        int slot = Step.slotIndex(s, profile.nSlots);
        long acc = Step.foldSlot(hike.terrain, profile.wordsPerSlot, slot, s);
        assertEquals(8, profile.wordsPerSlot);
        assertEquals("cc9e04ee133f53ac", Bytes.hexU64(acc));
        assertEquals(540, slot);
        long again = 0L;
        int base = slot * profile.wordsPerSlot;
        for (int w = 0; w < profile.wordsPerSlot; w++) {
            again += Mix.mix64(hike.terrain[base + w] ^ s);
        }
        assertEquals(acc, again);
    }
}
