package pcoc;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Compact JSON for the section 14 record, and a section 9 reader for index documents. */
public final class Json {
    private Json() {}

    /** RFC 8259 string, including the quotes. */
    public static String quote(String s) {
        StringBuilder b = new StringBuilder(s.length() + 2);
        b.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"':
                    b.append("\\\"");
                    break;
                case '\\':
                    b.append("\\\\");
                    break;
                case '\b':
                    b.append("\\b");
                    break;
                case '\f':
                    b.append("\\f");
                    break;
                case '\n':
                    b.append("\\n");
                    break;
                case '\r':
                    b.append("\\r");
                    break;
                case '\t':
                    b.append("\\t");
                    break;
                default:
                    if (c < 0x20) {
                        b.append(String.format("\\u%04x", (int) c));
                    } else {
                        b.append(c);
                    }
            }
        }
        b.append('"');
        return b.toString();
    }

    public static final class Obj {
        private final List<String> parts = new ArrayList<String>();

        public Obj add(String key, String raw) {
            parts.add(quote(key) + ":" + raw);
            return this;
        }

        public String build() {
            StringBuilder b = new StringBuilder();
            b.append('{');
            for (int i = 0; i < parts.size(); i++) {
                if (i > 0) {
                    b.append(',');
                }
                b.append(parts.get(i));
            }
            b.append('}');
            return b.toString();
        }
    }

    public static final class Arr {
        private final List<String> parts = new ArrayList<String>();

        public Arr add(String raw) {
            parts.add(raw);
            return this;
        }

        public String build() {
            StringBuilder b = new StringBuilder();
            b.append('[');
            for (int i = 0; i < parts.size(); i++) {
                if (i > 0) {
                    b.append(',');
                }
                b.append(parts.get(i));
            }
            b.append(']');
            return b.toString();
        }
    }

    /** A parsed JSON value. {@code null} from {@link #parse} means the body is not a successful read. */
    public abstract static class Val {
        public ObjMap asObj() {
            return this instanceof ObjMap ? (ObjMap) this : null;
        }

        public ArrList asArr() {
            return this instanceof ArrList ? (ArrList) this : null;
        }

        public Str asStr() {
            return this instanceof Str ? (Str) this : null;
        }

        public Num asNum() {
            return this instanceof Num ? (Num) this : null;
        }
    }

    public static final class ObjMap extends Val {
        public final Map<String, Val> map;

        ObjMap(Map<String, Val> map) {
            this.map = map;
        }

        public Val get(String key) {
            return map.get(key);
        }
    }

    public static final class ArrList extends Val {
        public final List<Val> list;

        ArrList(List<Val> list) {
            this.list = list;
        }
    }

    public static final class Str extends Val {
        public final String value;

        Str(String value) {
            this.value = value;
        }
    }

    public static final class Num extends Val {
        public final String raw;
        public final boolean integral;

        Num(String raw, boolean integral) {
            this.raw = raw;
            this.integral = integral;
        }

        /** A non-negative integer in {@code 0 … 2^53−1}, or null. */
        public Long jsonUint() {
            if (!integral || raw.charAt(0) == '-') {
                return null;
            }
            if (raw.length() > 16) {
                return null;
            }
            long v;
            try {
                v = Long.parseLong(raw);
            } catch (NumberFormatException e) {
                return null;
            }
            if (v < 0 || v > 9007199254740991L) {
                return null;
            }
            return v;
        }
    }

    public static final class Bool extends Val {
        public final boolean value;

        Bool(boolean value) {
            this.value = value;
        }
    }

    public static final class Null extends Val {
    }

    /**
     * Section 9. One JSON value, no second value, no duplicate member name.
     * Returns null when the body is not a successful read.
     */
    public static Val parse(byte[] utf8) {
        if (utf8.length >= 3 && (utf8[0] & 0xff) == 0xef && (utf8[1] & 0xff) == 0xbb && (utf8[2] & 0xff) == 0xbf) {
            return null;
        }
        String s;
        try {
            s = new String(utf8, java.nio.charset.StandardCharsets.UTF_8);
            byte[] back = s.getBytes(java.nio.charset.StandardCharsets.UTF_8);
            if (!java.util.Arrays.equals(utf8, back)) {
                return null;
            }
        } catch (RuntimeException e) {
            return null;
        }
        Parser p = new Parser(s);
        try {
            Val v = p.value();
            p.skip();
            if (p.i != s.length() || p.bad) {
                return null;
            }
            return v;
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static final class Parser {
        final String s;
        int i;
        boolean bad;

        Parser(String s) {
            this.s = s;
        }

        void skip() {
            while (i < s.length()) {
                char c = s.charAt(i);
                if (c == ' ' || c == '\n' || c == '\r' || c == '\t') {
                    i++;
                } else {
                    break;
                }
            }
        }

        Val value() {
            skip();
            if (i >= s.length()) {
                bad = true;
                return null;
            }
            char c = s.charAt(i);
            if (c == '{') {
                return object();
            }
            if (c == '[') {
                return array();
            }
            if (c == '"') {
                return new Str(string());
            }
            if (c == 't') {
                return literal("true", new Bool(true));
            }
            if (c == 'f') {
                return literal("false", new Bool(false));
            }
            if (c == 'n') {
                return literal("null", new Null());
            }
            if (c == '-' || (c >= '0' && c <= '9')) {
                return number();
            }
            bad = true;
            return null;
        }

        Val literal(String word, Val v) {
            if (s.startsWith(word, i)) {
                i += word.length();
                return v;
            }
            bad = true;
            return null;
        }

        Val object() {
            i++;
            Map<String, Val> map = new LinkedHashMap<String, Val>();
            skip();
            if (i < s.length() && s.charAt(i) == '}') {
                i++;
                return new ObjMap(map);
            }
            while (i < s.length()) {
                skip();
                if (i >= s.length() || s.charAt(i) != '"') {
                    bad = true;
                    return null;
                }
                String key = string();
                skip();
                if (i >= s.length() || s.charAt(i) != ':') {
                    bad = true;
                    return null;
                }
                i++;
                Val val = value();
                if (bad || val == null) {
                    bad = true;
                    return null;
                }
                if (map.containsKey(key)) {
                    bad = true;
                    return null;
                }
                map.put(key, val);
                skip();
                if (i >= s.length()) {
                    bad = true;
                    return null;
                }
                char c = s.charAt(i);
                if (c == ',') {
                    i++;
                    continue;
                }
                if (c == '}') {
                    i++;
                    return new ObjMap(map);
                }
                bad = true;
                return null;
            }
            bad = true;
            return null;
        }

        Val array() {
            i++;
            List<Val> list = new ArrayList<Val>();
            skip();
            if (i < s.length() && s.charAt(i) == ']') {
                i++;
                return new ArrList(list);
            }
            while (i < s.length()) {
                Val val = value();
                if (bad || val == null) {
                    bad = true;
                    return null;
                }
                list.add(val);
                skip();
                if (i >= s.length()) {
                    bad = true;
                    return null;
                }
                char c = s.charAt(i);
                if (c == ',') {
                    i++;
                    continue;
                }
                if (c == ']') {
                    i++;
                    return new ArrList(list);
                }
                bad = true;
                return null;
            }
            bad = true;
            return null;
        }

        String string() {
            i++;
            StringBuilder b = new StringBuilder();
            while (i < s.length()) {
                char c = s.charAt(i++);
                if (c == '"') {
                    return b.toString();
                }
                if (c == '\\') {
                    if (i >= s.length()) {
                        bad = true;
                        return "";
                    }
                    char e = s.charAt(i++);
                    switch (e) {
                        case '"':
                        case '\\':
                        case '/':
                            b.append(e);
                            break;
                        case 'b':
                            b.append('\b');
                            break;
                        case 'f':
                            b.append('\f');
                            break;
                        case 'n':
                            b.append('\n');
                            break;
                        case 'r':
                            b.append('\r');
                            break;
                        case 't':
                            b.append('\t');
                            break;
                        case 'u':
                            if (i + 4 > s.length()) {
                                bad = true;
                                return "";
                            }
                            int cp = 0;
                            for (int k = 0; k < 4; k++) {
                                int d = Character.digit(s.charAt(i++), 16);
                                if (d < 0) {
                                    bad = true;
                                    return "";
                                }
                                cp = (cp << 4) | d;
                            }
                            b.append((char) cp);
                            break;
                        default:
                            bad = true;
                            return "";
                    }
                } else if (c < 0x20) {
                    bad = true;
                    return "";
                } else {
                    b.append(c);
                }
            }
            bad = true;
            return "";
        }

        Val number() {
            int start = i;
            if (s.charAt(i) == '-') {
                i++;
            }
            if (i >= s.length() || s.charAt(i) < '0' || s.charAt(i) > '9') {
                bad = true;
                return null;
            }
            if (s.charAt(i) == '0') {
                i++;
            } else {
                while (i < s.length() && s.charAt(i) >= '0' && s.charAt(i) <= '9') {
                    i++;
                }
            }
            boolean integral = true;
            if (i < s.length() && s.charAt(i) == '.') {
                integral = false;
                i++;
                if (i >= s.length() || s.charAt(i) < '0' || s.charAt(i) > '9') {
                    bad = true;
                    return null;
                }
                while (i < s.length() && s.charAt(i) >= '0' && s.charAt(i) <= '9') {
                    i++;
                }
            }
            if (i < s.length() && (s.charAt(i) == 'e' || s.charAt(i) == 'E')) {
                integral = false;
                i++;
                if (i < s.length() && (s.charAt(i) == '+' || s.charAt(i) == '-')) {
                    i++;
                }
                if (i >= s.length() || s.charAt(i) < '0' || s.charAt(i) > '9') {
                    bad = true;
                    return null;
                }
                while (i < s.length() && s.charAt(i) >= '0' && s.charAt(i) <= '9') {
                    i++;
                }
            }
            return new Num(s.substring(start, i), integral);
        }
    }
}
