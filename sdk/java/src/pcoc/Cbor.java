package pcoc;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/** §7.3 CBOR. Definite lengths only. Produce uses the shortest unsigned form. */
public final class Cbor {
    private Cbor() {}

    public static final class Malformed extends Exception {
        public Malformed(String message) {
            super(message);
        }
    }

    public enum Kind { UINT, BYTES, TEXT, ARRAY, MAP }

    public static final class Item {
        public final Kind kind;
        public final BigInteger uint;
        public final byte[] bytes;
        public final String text;
        public final List<Item> array;
        public final List<Field> fields;

        private Item(Kind kind, BigInteger uint, byte[] bytes, String text, List<Item> array, List<Field> fields) {
            this.kind = kind;
            this.uint = uint;
            this.bytes = bytes;
            this.text = text;
            this.array = array;
            this.fields = fields;
        }

        static Item uint(BigInteger v) {
            return new Item(Kind.UINT, v, null, null, null, null);
        }

        static Item bytes(byte[] v) {
            return new Item(Kind.BYTES, null, v, null, null, null);
        }

        static Item text(String v) {
            return new Item(Kind.TEXT, null, null, v, null, null);
        }

        static Item array(List<Item> v) {
            return new Item(Kind.ARRAY, null, null, null, v, null);
        }

        static Item map(List<Field> v) {
            return new Item(Kind.MAP, null, null, null, null, v);
        }

        public Item field(String key) {
            if (fields == null) {
                return null;
            }
            Item found = null;
            for (Field f : fields) {
                if (f.key.equals(key)) {
                    found = f.value;
                }
            }
            return found;
        }
    }

    public static final class Field {
        public final String key;
        public final Item value;

        Field(String key, Item value) {
            this.key = key;
            this.value = value;
        }
    }

    public static final class Parsed {
        public final Item item;
        public final String kindText;
        /** Non-null when the bytes are not one well-formed definite item. */
        public final String error;

        Parsed(Item item, String kindText, String error) {
            this.item = item;
            this.kindText = kindText;
            this.error = error;
        }
    }

    public static Parsed parse(byte[] input) {
        Cursor c = new Cursor(input);
        try {
            Item item = c.readItem(true);
            String err = null;
            if (c.i != input.length) {
                err = "trailing";
            }
            String kind = c.kindText;
            if (kind == null && item.kind == Kind.MAP) {
                Item k = item.field("kind");
                if (k != null && k.kind == Kind.TEXT) {
                    kind = k.text;
                }
            }
            return new Parsed(item, kind, err);
        } catch (Malformed e) {
            return new Parsed(null, c.kindText, e.getMessage());
        }
    }

    public static void writeType(ByteArrayOutputStream o, int major, long value) {
        if (value < 0) {
            throw new IllegalArgumentException("length");
        }
        int mt = major << 5;
        if (value < 24) {
            o.write(mt | (int) value);
        } else if (value < 256) {
            o.write(mt | 24);
            o.write((int) value);
        } else if (value < 65536) {
            o.write(mt | 25);
            o.write((int) (value >>> 8));
            o.write((int) value);
        } else if (value < (1L << 32)) {
            o.write(mt | 26);
            o.write((int) (value >>> 24));
            o.write((int) (value >>> 16));
            o.write((int) (value >>> 8));
            o.write((int) value);
        } else {
            o.write(mt | 27);
            for (int s = 56; s >= 0; s -= 8) {
                o.write((int) (value >>> s));
            }
        }
    }

    public static void writeUint(ByteArrayOutputStream o, long value) {
        if (value < 0) {
            throw new IllegalArgumentException("uint");
        }
        writeType(o, 0, value);
    }

    public static void writeText(ByteArrayOutputStream o, String s) {
        byte[] b = s.getBytes(StandardCharsets.UTF_8);
        writeType(o, 3, b.length);
        o.write(b, 0, b.length);
    }

    public static void writeBytes(ByteArrayOutputStream o, byte[] b) {
        writeType(o, 2, b.length);
        o.write(b, 0, b.length);
    }

    /** A one-key map {@code {kind: text}}, used for Rule A5 inputs. */
    public static byte[] kindMap(String kindText) {
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        writeType(o, 5, 1);
        writeText(o, "kind");
        writeText(o, kindText);
        return o.toByteArray();
    }

    private static final class Cursor {
        final byte[] b;
        int i;
        /** Kind text of the top-level map, set as soon as that field is read. */
        String kindText;

        Cursor(byte[] b) {
            this.b = b;
        }

        Item readItem(boolean top) throws Malformed {
            if (i >= b.length) {
                throw new Malformed("truncated");
            }
            int head = b[i++] & 0xff;
            int major = head >>> 5;
            int ai = head & 0x1f;
            switch (major) {
                case 0:
                    return Item.uint(readUint(ai));
                case 1:
                    throw new Malformed("negative");
                case 2: {
                    int n = readCount(ai);
                    return Item.bytes(readRaw(n));
                }
                case 3: {
                    int n = readCount(ai);
                    byte[] raw = readRaw(n);
                    String text = utf8(raw);
                    if (text == null) {
                        throw new Malformed("utf8");
                    }
                    return Item.text(text);
                }
                case 4: {
                    int n = readCount(ai);
                    List<Item> items = new ArrayList<Item>(n);
                    for (int k = 0; k < n; k++) {
                        items.add(readItem(false));
                    }
                    return Item.array(items);
                }
                case 5: {
                    int n = readCount(ai);
                    List<Field> fields = new ArrayList<Field>(n);
                    for (int k = 0; k < n; k++) {
                        Item key = readItem(false);
                        if (key.kind != Kind.TEXT) {
                            throw new Malformed("map key");
                        }
                        for (Field f : fields) {
                            if (f.key.equals(key.text)) {
                                throw new Malformed("duplicate key");
                            }
                        }
                        Item val = readItem(false);
                        if (top && "kind".equals(key.text) && val.kind == Kind.TEXT) {
                            kindText = val.text;
                        }
                        fields.add(new Field(key.text, val));
                    }
                    return Item.map(fields);
                }
                case 6:
                    throw new Malformed("tag");
                default:
                    throw new Malformed("simple");
            }
        }

        BigInteger readUint(int ai) throws Malformed {
            if (ai < 24) {
                return BigInteger.valueOf(ai);
            }
            if (ai == 24) {
                return BigInteger.valueOf(u8());
            }
            if (ai == 25) {
                return BigInteger.valueOf(u16());
            }
            if (ai == 26) {
                return BigInteger.valueOf(u32());
            }
            if (ai == 27) {
                byte[] mag = readRaw(8);
                return new BigInteger(1, mag);
            }
            throw new Malformed("indefinite");
        }

        int readCount(int ai) throws Malformed {
            BigInteger v = readUint(ai);
            if (v.bitLength() > 31) {
                throw new Malformed("length");
            }
            int n = v.intValue();
            if (n < 0) {
                throw new Malformed("length");
            }
            return n;
        }

        byte[] readRaw(int n) throws Malformed {
            if (n < 0 || i > b.length - n) {
                throw new Malformed("truncated");
            }
            byte[] o = new byte[n];
            System.arraycopy(b, i, o, 0, n);
            i += n;
            return o;
        }

        int u8() throws Malformed {
            if (i >= b.length) {
                throw new Malformed("truncated");
            }
            return b[i++] & 0xff;
        }

        int u16() throws Malformed {
            return (u8() << 8) | u8();
        }

        long u32() throws Malformed {
            return ((long) u8() << 24) | ((long) u8() << 16) | ((long) u8() << 8) | u8();
        }
    }

    static String utf8(byte[] raw) {
        try {
            return StandardCharsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(raw))
                    .toString();
        } catch (CharacterCodingException e) {
            return null;
        }
    }
}
