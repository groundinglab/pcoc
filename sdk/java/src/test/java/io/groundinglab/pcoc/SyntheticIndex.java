package io.groundinglab.pcoc;

import java.nio.charset.StandardCharsets;

import io.groundinglab.pcoc.produce.Cases;

/** CASES synthetic index for heights 989 … 1018. */
public final class SyntheticIndex implements Index {
    private final Sha256 sha = new Sha256();

    public byte[] get(long height) {
        if (height < 989L || height > 1018L) {
            return null;
        }
        byte[] natural;
        if (height == Cases.SEED_HEIGHT) {
            natural = Cases.seedBeacon(sha);
        } else if (height >= 1001L && height <= 1018L) {
            natural = Cases.interrupt(sha, (int) (height - 1001L));
        } else {
            natural = Cases.asciiHash(sha, "pcoc-pack-2.2/index/filler/" + height);
        }
        String hash = Bytes.hex(Bytes.reverse(natural));
        long time = 1700000000L + 600L * height;
        String json = "{\"network\":\"synthetic\",\"header\":{\"hash\":\"" + hash
                + "\",\"height\":" + height + ",\"time\":" + time + "}}";
        return json.getBytes(StandardCharsets.US_ASCII);
    }
}
