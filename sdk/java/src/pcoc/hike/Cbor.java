package pcoc.hike;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Section 7.3 CBOR. Definite lengths, shortest unsigned integers on produce,
 * and the malformation names the locked pack records.
 */
public final class Cbor {
    private Cbor() {}

    /**
     * Implementation resource limit: the most arrays and maps one item may sit
     * inside, counting itself. A section 8 hike object nests at most 4 deep
     * (top-level map, stages array, stage map, checkpoints/leg_results array;
     * their elements are byte strings). 64 is 16 times that and is the same
     * value in the Rust, Go and Java SDKs. A deeper item is malformed CBOR.
     */
    public static final int MAX_CBOR_NESTING = 64;

    public static final class Val {
        public enum Kind { UINT, BYTES, TEXT, ARRAY, MAP }

        public final Kind kind;
        public final long uint;
        public final byte[] bytes;
        public final String text;
        public final List<Val> array;
        public final LinkedHashMap<String, Val> map;

        private Val(Kind kind, long uint, byte[] bytes, String text, List<Val> array, LinkedHashMap<String, Val> map) {
            this.kind = kind;
            this.uint = uint;
            this.bytes = bytes;
            this.text = text;
            this.array = array;
            this.map = map;
        }

        static Val uint(long value) {
            return new Val(Kind.UINT, value, null, null, null, null);
        }

        static Val bytes(byte[] value) {
            return new Val(Kind.BYTES, 0, value, null, null, null);
        }

        static Val text(String value) {
            return new Val(Kind.TEXT, 0, null, value, null, null);
        }

        static Val array(List<Val> value) {
            return new Val(Kind.ARRAY, 0, null, null, value, null);
        }

        static Val map(LinkedHashMap<String, Val> value) {
            return new Val(Kind.MAP, 0, null, null, null, value);
        }

        public boolean isUint() {
            return kind == Kind.UINT;
        }

        public byte[] bytesOrNull(int width) {
            if (kind != Kind.BYTES || bytes.length != width) {
                return null;
            }
            return bytes;
        }
    }

    public static final class Problem extends Exception {
        private static final long serialVersionUID = 1L;

        public final String issue;

        Problem(String issue) {
            super(issue);
            this.issue = issue;
        }
    }

    /** Empty when the buffer is one acceptable data item. */
    public static String issue(byte[] data) {
        try {
            Cursor cursor = new Cursor(data);
            cursor.item();
            if (cursor.i != data.length) {
                return "trailing";
            }
            return "";
        } catch (Problem problem) {
            return problem.issue;
        }
    }

    public static Val decode(byte[] data) throws Problem {
        Cursor cursor = new Cursor(data);
        Val value = cursor.item();
        if (cursor.i != data.length) {
            throw new Problem("trailing");
        }
        return value;
    }

    /** First item. {@code trailing} is true when bytes follow it. {@link #decode} still rejects those bytes. */
    public static final class Opened {
        public final Val value;
        public final boolean trailing;

        Opened(Val value, boolean trailing) {
            this.value = value;
            this.trailing = trailing;
        }
    }

    public static Opened decodeHead(byte[] data) throws Problem {
        Cursor cursor = new Cursor(data);
        Val value = cursor.item();
        return new Opened(value, cursor.i != data.length);
    }

    /** Non-shortest definite unsigned integers still yield their numeric value. */
    public static Long decodeUint(byte[] data) {
        try {
            Cursor cursor = new Cursor(data);
            Head head = cursor.head();
            if (head.major != 0 || head.argument == null || cursor.i != data.length) {
                return null;
            }
            return head.argument;
        } catch (Problem problem) {
            return null;
        }
    }

    public static byte[] encodeUint(long value) {
        if (Long.compareUnsigned(value, 24) < 0) {
            return new byte[] {(byte) value};
        }
        if (Long.compareUnsigned(value, 256) < 0) {
            return new byte[] {0x18, (byte) value};
        }
        if (Long.compareUnsigned(value, 65536) < 0) {
            return new byte[] {0x19, (byte) (value >>> 8), (byte) value};
        }
        if (Long.compareUnsigned(value, 1L << 32) < 0) {
            return new byte[] {
                0x1a,
                (byte) (value >>> 24),
                (byte) (value >>> 16),
                (byte) (value >>> 8),
                (byte) value
            };
        }
        return new byte[] {
            0x1b,
            (byte) (value >>> 56),
            (byte) (value >>> 48),
            (byte) (value >>> 40),
            (byte) (value >>> 32),
            (byte) (value >>> 24),
            (byte) (value >>> 16),
            (byte) (value >>> 8),
            (byte) value
        };
    }

    public static byte[] encodeBytes(byte[] data) {
        return join(head(2, data.length), data);
    }

    public static byte[] encodeText(String text) {
        byte[] raw = text.getBytes(java.nio.charset.StandardCharsets.UTF_8);
        return join(head(3, raw.length), raw);
    }

    public static byte[] encodeArray(List<byte[]> items) {
        int size = 0;
        byte[] prefix = head(4, items.size());
        for (byte[] item : items) {
            size += item.length;
        }
        byte[] out = new byte[prefix.length + size];
        System.arraycopy(prefix, 0, out, 0, prefix.length);
        int at = prefix.length;
        for (byte[] item : items) {
            System.arraycopy(item, 0, out, at, item.length);
            at += item.length;
        }
        return out;
    }

    public static byte[] encodeMap(List<Map.Entry<String, byte[]>> pairs) {
        List<Map.Entry<byte[], byte[]>> encoded = new ArrayList<>();
        for (Map.Entry<String, byte[]> pair : pairs) {
            encoded.add(Map.entry(encodeText(pair.getKey()), pair.getValue()));
        }
        encoded.sort((left, right) -> compare(left.getKey(), right.getKey()));
        int size = 0;
        for (Map.Entry<byte[], byte[]> pair : encoded) {
            size += pair.getKey().length + pair.getValue().length;
        }
        byte[] prefix = head(5, encoded.size());
        byte[] out = new byte[prefix.length + size];
        System.arraycopy(prefix, 0, out, 0, prefix.length);
        int at = prefix.length;
        for (Map.Entry<byte[], byte[]> pair : encoded) {
            System.arraycopy(pair.getKey(), 0, out, at, pair.getKey().length);
            at += pair.getKey().length;
            System.arraycopy(pair.getValue(), 0, out, at, pair.getValue().length);
            at += pair.getValue().length;
        }
        return out;
    }

    public static List<String> mapKeys(byte[] data) throws Problem {
        Val value = decode(data);
        if (value.kind != Val.Kind.MAP) {
            throw new Problem("not a map");
        }
        return new ArrayList<>(value.map.keySet());
    }

    public static List<String> stageKeys(byte[] data, int stage) throws Problem {
        Val value = decode(data);
        Val stages = value.map.get("stages");
        if (stages == null || stages.kind != Val.Kind.ARRAY || stage >= stages.array.size()) {
            throw new Problem("stage");
        }
        Val one = stages.array.get(stage);
        if (one.kind != Val.Kind.MAP) {
            throw new Problem("stage");
        }
        return new ArrayList<>(one.map.keySet());
    }

    private static byte[] head(int major, int length) {
        byte[] body = encodeUint(length);
        body[0] = (byte) (body[0] | (major << 5));
        return body;
    }

    private static byte[] join(byte[] left, byte[] right) {
        byte[] out = new byte[left.length + right.length];
        System.arraycopy(left, 0, out, 0, left.length);
        System.arraycopy(right, 0, out, left.length, right.length);
        return out;
    }

    private static int compare(byte[] left, byte[] right) {
        int n = Math.min(left.length, right.length);
        for (int i = 0; i < n; i++) {
            int diff = (left[i] & 255) - (right[i] & 255);
            if (diff != 0) {
                return diff;
            }
        }
        return left.length - right.length;
    }

    private static final class Head {
        final int major;
        final Long argument;

        Head(int major, Long argument) {
            this.major = major;
            this.argument = argument;
        }
    }

    private static final class Cursor {
        final byte[] data;
        int i;
        /** Arrays and maps open around the item being read. */
        int depth;

        Cursor(byte[] data) {
            this.data = data;
        }

        Val item() throws Problem {
            Head head = head();
            if (head.argument == null) {
                throw new Problem("indefinite");
            }
            long argument = head.argument;
            switch (head.major) {
                case 0:
                    return Val.uint(argument);
                case 1:
                    throw new Problem("negative");
                case 2:
                    return Val.bytes(take(argument));
                case 3:
                    byte[] raw = take(argument);
                    String text;
                    try {
                        text = new String(raw, java.nio.charset.StandardCharsets.UTF_8);
                    } catch (RuntimeException ex) {
                        throw new Problem("truncated");
                    }
                    if (!java.util.Arrays.equals(raw, text.getBytes(java.nio.charset.StandardCharsets.UTF_8))) {
                        throw new Problem("truncated");
                    }
                    return Val.text(text);
                case 4: {
                    enter();
                    // One item is at least one byte. A count past the bytes
                    // left, or past what an int can hold, is truncated. The
                    // list is not sized from that count.
                    if (countExceeds(argument, 1)) {
                        throw new Problem("truncated");
                    }
                    List<Val> items = new ArrayList<>((int) argument);
                    for (long n = 0; n < argument; n++) {
                        items.add(item());
                    }
                    depth--;
                    return Val.array(items);
                }
                case 5: {
                    enter();
                    // A map entry is a key and a value, at least two bytes.
                    if (countExceeds(argument, 2)) {
                        throw new Problem("truncated");
                    }
                    LinkedHashMap<String, Val> map = new LinkedHashMap<>();
                    for (long n = 0; n < argument; n++) {
                        Val key = item();
                        if (key.kind != Val.Kind.TEXT) {
                            throw new Problem("non-text");
                        }
                        if (map.containsKey(key.text)) {
                            throw new Problem("duplicate");
                        }
                        map.put(key.text, item());
                    }
                    depth--;
                    return Val.map(map);
                }
                case 6:
                    throw new Problem("tag");
                default:
                    throw new Problem("float-or-simple");
            }
        }

        void enter() throws Problem {
            if (depth >= MAX_CBOR_NESTING) {
                throw new Problem("nesting depth");
            }
            depth++;
        }

        Head head() throws Problem {
            if (i >= data.length) {
                throw new Problem("truncated");
            }
            int initial = data[i++] & 255;
            int major = initial >> 5;
            int additional = initial & 31;
            if (additional < 24) {
                return new Head(major, (long) additional);
            }
            int width = switch (additional) {
                case 24 -> 1;
                case 25 -> 2;
                case 26 -> 4;
                case 27 -> 8;
                case 31 -> -1;
                default -> 0;
            };
            if (width == -1) {
                return new Head(major, null);
            }
            if (width == 0) {
                throw new Problem("truncated");
            }
            if (i + width > data.length) {
                throw new Problem("truncated");
            }
            long value = 0;
            for (int n = 0; n < width; n++) {
                value = (value << 8) | (data[i++] & 255L);
            }
            return new Head(major, value);
        }

        byte[] take(long length) throws Problem {
            // i + length overflows a signed int when length is near 2^63.
            // Compare the declared length as an unsigned long against the
            // bytes still in the buffer, and refuse a length an array cannot hold.
            if (Long.compareUnsigned(length, Integer.MAX_VALUE) > 0
                    || Long.compareUnsigned(length, data.length - i) > 0) {
                throw new Problem("truncated");
            }
            int n = (int) length;
            byte[] out = new byte[n];
            System.arraycopy(data, i, out, 0, n);
            i += n;
            return out;
        }

        /** A count of n items, each at least per bytes, past the bytes left. */
        boolean countExceeds(long count, int per) {
            long left = data.length - i;
            if (Long.compareUnsigned(count, Integer.MAX_VALUE) > 0) {
                return true;
            }
            return Long.compareUnsigned(count, left / per) > 0;
        }
    }
}
