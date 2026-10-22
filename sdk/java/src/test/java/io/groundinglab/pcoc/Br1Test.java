package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.util.Arrays;
import java.util.Locale;

import org.junit.jupiter.api.Test;

/** §13.6 Rule BR1. Retarget, window, and the paid-waypoint answer. */
public class Br1Test {
    @Test
    public void retargetClampsAndCompactRoundTrips() {
        Br1.Compact limit = Br1.setCompact(Br1.MAINNET_POW_LIMIT_BITS);
        assertFalse(limit.negative);
        assertFalse(limit.overflow);
        assertEquals(Br1.MAINNET_POW_LIMIT_BITS, Br1.getCompact(limit.target));
        Br1.Compact synth = Br1.setCompact(Br1.SYNTH_NBITS);
        assertEquals(Br1.SYNTH_NBITS, Br1.getCompact(synth.target));
        assertEquals(0x1d00ffff, Br1.nextBits(2015, 0x1d00ffff, 1233061996, 1231006505));
        assertEquals(0x1c3fffc0, Br1.nextBits(2015, 0x1d00ffff, 1231006505 + 302400, 1231006505));
        assertEquals(0x207fffff, Br1.nextBits(100, 0x207fffff, 1, 0));
    }

    @Test
    public void nonceAndHeaderSpellings() {
        assertEquals(0L, Br1.parseNonce("0").longValue());
        assertEquals(825113L, Br1.parseNonce("825113").longValue());
        assertEquals(Long.parseUnsignedLong("18446744073709551615"),
                Br1.parseNonce("18446744073709551615").longValue());
        assertNull(Br1.parseNonce("18446744073709551616"));
        assertNull(Br1.parseNonce("01"));
        assertNull(Br1.parseNonce("00"));
        assertNull(Br1.parseNonce("+1"));
        assertNull(Br1.parseNonce("-1"));
        assertNull(Br1.parseNonce(""));
        assertNull(Br1.parseNonce("1 "));
        assertNull(Br1.parseNonce(null));
        byte[] header = Br1.assembleHeader(fill(0x11), 1_700_000_000, Br1.SYNTH_NBITS, 4);
        assertEquals(80, header.length);
        assertArrayEquals(header, Br1.parseFeeHeader(Bytes.hex(header)));
        assertNull(Br1.parseFeeHeader(Bytes.hex(header).toUpperCase(Locale.ROOT)));
        assertNull(Br1.parseFeeHeader(Bytes.hex(header) + "00"));
        assertNull(Br1.parseFeeHeader(null));
    }

    @Test
    public void testPlaneAfterMeetsTheFixedTargetAndLiveCopyUsesTheTip() {
        Sha256 sha = new Sha256();
        byte[] tipHeader = Br1.assembleHeader(fill(0x11), 1_700_000_000, Br1.SYNTH_NBITS, 4);
        Br1.Tip tip = tipFrom(sha, tipHeader, 100, 0);
        assertEquals(Br1.Window.CURRENT, Br1.window(sha, tipHeader, tip, Br1.Plane.TEST));
        byte[] after = null;
        for (int nonce = 0; nonce < 64; nonce++) {
            byte[] header = Br1.assembleHeader(tip.hash, 1_700_000_010, Br1.SYNTH_NBITS, nonce);
            if (Br1.window(sha, header, tip, Br1.Plane.TEST) == Br1.Window.AFTER) {
                after = header;
                break;
            }
        }
        if (after == null) {
            throw new AssertionError("synth target");
        }
        assertEquals(Br1.Window.AFTER, Br1.window(sha, after, tip, Br1.Plane.LIVE));
        byte[] wrong = Br1.assembleHeader(tip.hash, 1_700_000_010, 0x1d00ffff, 1);
        assertEquals(Br1.Window.OUTSIDE, Br1.window(sha, wrong, tip, Br1.Plane.TEST));
        assertEquals(Br1.Window.OUTSIDE, Br1.window(sha, new byte[79], tip, Br1.Plane.TEST));
    }

    @Test
    public void negativeOrOverflowCompactIsNotAfter() {
        Sha256 sha = new Sha256();
        byte[] previous = fill(0x42);
        int negativeBits = 0x04800001;
        Br1.Tip negativeTip = new Br1.Tip(previous, new byte[32], 100, 1_700_000_000, negativeBits, 0);
        byte[] negativeHeader = Br1.assembleHeader(previous, 1_700_000_010, negativeBits, 0);
        assertEquals(Br1.Window.OUTSIDE, Br1.window(sha, negativeHeader, negativeTip, Br1.Plane.LIVE));
        int overflowBits = 0x2300ffff;
        Br1.Tip overflowTip = new Br1.Tip(previous, new byte[32], 100, 1_700_000_000, overflowBits, 0);
        byte[] overflowHeader = Br1.assembleHeader(previous, 1_700_000_010, overflowBits, 0);
        assertEquals(Br1.Window.OUTSIDE, Br1.window(sha, overflowHeader, overflowTip, Br1.Plane.LIVE));
    }

    @Test
    public void displayOrderReversalIsNotCurrentBeforeOrAfter() {
        Sha256 sha = new Sha256();
        byte[] tipHeader = Br1.assembleHeader(fill(0x10), 1_700_000_000, Br1.SYNTH_NBITS, 4);
        Br1.Tip tip = tipFrom(sha, tipHeader, 50, 0);
        byte[] reversedHash = Bytes.reverse(tip.hash);
        Br1.Tip reversed = new Br1.Tip(reversedHash, tip.previous, tip.height, tip.time, tip.bits, tip.periodFirstTime);
        assertEquals(Br1.Window.OUTSIDE, Br1.window(sha, tipHeader, reversed, Br1.Plane.TEST));
        byte[] before = Br1.assembleHeader(fill(0x12), 1_700_000_000, Br1.SYNTH_NBITS, 5);
        Br1.Tip tipBefore = new Br1.Tip(
                tip.hash, Br1.headerHash(sha, before), tip.height, tip.time, tip.bits, tip.periodFirstTime);
        assertEquals(Br1.Window.BEFORE, Br1.window(sha, before, tipBefore, Br1.Plane.TEST));
        Br1.Tip flippedPrevious = new Br1.Tip(
                tip.hash, Bytes.reverse(tipBefore.previous), tip.height, tip.time, tip.bits, tip.periodFirstTime);
        assertEquals(Br1.Window.OUTSIDE, Br1.window(sha, before, flippedPrevious, Br1.Plane.TEST));
        byte[] after = null;
        for (int nonce = 0; nonce < 64; nonce++) {
            byte[] header = Br1.assembleHeader(tip.hash, 1_700_000_010, Br1.SYNTH_NBITS, nonce);
            if (Br1.window(sha, header, tip, Br1.Plane.TEST) == Br1.Window.AFTER) {
                after = header;
                break;
            }
        }
        byte[] flipped = Arrays.copyOf(after, after.length);
        for (int i = 0; i < 16; i++) {
            byte t = flipped[4 + i];
            flipped[4 + i] = flipped[35 - i];
            flipped[35 - i] = t;
        }
        assertEquals(Br1.Window.OUTSIDE, Br1.window(sha, flipped, tip, Br1.Plane.TEST));
    }

    @Test
    public void paidPostIs204AndABadSpellingOrOriginIs402() {
        Sha256 sha = new Sha256();
        byte[] tipHeader = Br1.assembleHeader(fill(0x11), 1_700_000_000, Br1.SYNTH_NBITS, 4);
        Br1.Tip tip = tipFrom(sha, tipHeader, 100, 0);
        String origin = "https://example.com";
        byte[] waypoint = fill(0x22);
        byte[] challenge = Hashcash.challenge(sha, origin, Br1.headerHash(sha, tipHeader));
        long nonce = Hashcash.solve(sha, challenge, waypoint);
        String hexHeader = Bytes.hex(tipHeader);
        String nonceText = Long.toUnsignedString(nonce);
        assertEquals(204, Br1.status(sha, waypoint, nonceText, hexHeader, origin, tip, Br1.Plane.TEST));
        assertEquals(402, Br1.status(sha, waypoint, nonceText, hexHeader, "https://other.example", tip, Br1.Plane.TEST));
        assertEquals(402, Br1.status(sha, waypoint, "01", hexHeader, origin, tip, Br1.Plane.TEST));
        assertEquals(402, Br1.status(sha, waypoint, nonceText, hexHeader.toUpperCase(Locale.ROOT), origin, tip, Br1.Plane.TEST));
        assertEquals(402, Br1.status(sha, new byte[31], nonceText, hexHeader, origin, tip, Br1.Plane.TEST));
    }

    private static Br1.Tip tipFrom(Sha256 sha, byte[] header, int height, int periodFirstTime) {
        byte[] previous = Arrays.copyOfRange(header, 4, 36);
        int time = (header[68] & 0xff)
                | ((header[69] & 0xff) << 8)
                | ((header[70] & 0xff) << 16)
                | ((header[71] & 0xff) << 24);
        int bits = (header[72] & 0xff)
                | ((header[73] & 0xff) << 8)
                | ((header[74] & 0xff) << 16)
                | ((header[75] & 0xff) << 24);
        return new Br1.Tip(Br1.headerHash(sha, header), previous, height, time, bits, periodFirstTime);
    }

    private static byte[] fill(int b) {
        byte[] a = new byte[32];
        Arrays.fill(a, (byte) b);
        return a;
    }
}
