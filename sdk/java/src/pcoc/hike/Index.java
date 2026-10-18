package pcoc.hike;

import pcoc.json.Json;

/** Section 9 portable-index body, and the section 14.2 header measurement. */
public final class Index {
    public static final long JSON_INT_MAX = (1L << 53) - 1;

    public final byte[] natural;
    public final long height;
    public final Long time;

    private Index(byte[] natural, long height, Long time) {
        this.natural = natural;
        this.height = height;
        this.time = time;
    }

    public static Index readDocument(String body) {
        if (body == null || body.isEmpty() || body.charAt(0) == '\uFEFF') {
            return null;
        }
        Json parsed;
        try {
            parsed = Json.parseStrict(body);
        } catch (RuntimeException ex) {
            return null;
        }
        if (!(parsed instanceof Json.Obj top)) {
            return null;
        }
        Json headerValue = top.fields().get("header");
        if (!(headerValue instanceof Json.Obj header)) {
            return null;
        }
        Json hashValue = header.fields().get("hash");
        Json heightValue = header.fields().get("height");
        if (!(hashValue instanceof Json.Str hashText) || !(heightValue instanceof Json.Num heightNum)) {
            return null;
        }
        if (!integerToken(heightNum.digits)) {
            return null;
        }
        long height;
        try {
            height = Long.parseUnsignedLong(heightNum.digits);
        } catch (NumberFormatException ex) {
            return null;
        }
        if (Long.compareUnsigned(height, JSON_INT_MAX) > 0) {
            return null;
        }
        if (hashText.value.length() != 64) {
            return null;
        }
        byte[] display = Bytes.unhex(hashText.value);
        if (display == null || display.length != 32) {
            return null;
        }
        byte[] natural = new byte[32];
        for (int i = 0; i < 32; i++) {
            natural[i] = display[31 - i];
        }
        Long time = null;
        Json timeValue = header.fields().get("time");
        if (timeValue instanceof Json.Num timeNum && integerToken(timeNum.digits)) {
            try {
                long candidate = Long.parseUnsignedLong(timeNum.digits);
                if (Long.compareUnsigned(candidate, 0xFFFFFFFFL) <= 0) {
                    time = candidate;
                }
            } catch (NumberFormatException ex) {
                time = null;
            }
        }
        return new Index(natural, height, time);
    }

    /** comparison token, then the T token (an integer spelling or not read). */
    public static String[] headerMeasurement(byte[] storedHash, long height, String document, boolean present) {
        if (!present || document == null) {
            return new String[] {"not read", "not read"};
        }
        Index read = readDocument(document);
        if (read == null) {
            return new String[] {"not read", "not read"};
        }
        boolean hashAgrees = Bytes.eq(read.natural, storedHash);
        boolean heightAgrees = read.height == height;
        if (!hashAgrees || !heightAgrees) {
            return new String[] {"mismatch", "not read"};
        }
        if (read.time == null) {
            return new String[] {"match", "not read"};
        }
        return new String[] {"match", Long.toString(read.time)};
    }

    static boolean integerToken(String digits) {
        int start = 0;
        if (digits.startsWith("-")) {
            return false;
        }
        if (digits.isEmpty()) {
            return false;
        }
        for (int i = start; i < digits.length(); i++) {
            char c = digits.charAt(i);
            if (c < '0' || c > '9') {
                return false;
            }
        }
        return true;
    }
}
