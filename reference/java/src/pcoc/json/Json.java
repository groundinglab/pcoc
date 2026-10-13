package pcoc.json;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Compact JSON values. Numbers keep the digits the text spelled. */
public abstract class Json {
    public abstract String compact();

    public static Json str(String s) {
        return new Str(s);
    }

    public static Json num(long n) {
        return new Num(Long.toString(n));
    }

    public static Json bool(boolean v) {
        return v ? Bool.TRUE : Bool.FALSE;
    }

    public static Obj obj() {
        return new Obj();
    }

    public static Arr arr() {
        return new Arr();
    }

    public static final class Obj extends Json {
        private final LinkedHashMap<String, Json> fields = new LinkedHashMap<>();

        public Obj put(String key, Json value) {
            fields.put(key, value);
            return this;
        }

        public Obj put(String key, String value) {
            return put(key, str(value));
        }

        public Obj put(String key, long value) {
            return put(key, num(value));
        }

        public Map<String, Json> fields() {
            return fields;
        }

        @Override
        public String compact() {
            StringBuilder out = new StringBuilder();
            out.append('{');
            boolean first = true;
            for (Map.Entry<String, Json> entry : fields.entrySet()) {
                if (!first) {
                    out.append(',');
                }
                first = false;
                out.append('"');
                escape(out, entry.getKey());
                out.append('"').append(':');
                out.append(entry.getValue().compact());
            }
            out.append('}');
            return out.toString();
        }
    }

    public static final class Arr extends Json {
        private final List<Json> items = new ArrayList<>();

        public Arr add(Json value) {
            items.add(value);
            return this;
        }

        public Arr add(String value) {
            return add(str(value));
        }

        public Arr add(long value) {
            return add(num(value));
        }

        public List<Json> items() {
            return items;
        }

        @Override
        public String compact() {
            StringBuilder out = new StringBuilder();
            out.append('[');
            for (int i = 0; i < items.size(); i++) {
                if (i > 0) {
                    out.append(',');
                }
                out.append(items.get(i).compact());
            }
            out.append(']');
            return out.toString();
        }
    }

    public static final class Str extends Json {
        public final String value;

        Str(String value) {
            this.value = value;
        }

        @Override
        public String compact() {
            StringBuilder out = new StringBuilder();
            out.append('"');
            escape(out, value);
            out.append('"');
            return out.toString();
        }
    }

    public static final class Num extends Json {
        public final String digits;

        Num(String digits) {
            this.digits = digits;
        }

        @Override
        public String compact() {
            return digits;
        }
    }

    public static final class Bool extends Json {
        static final Bool TRUE = new Bool(true);
        static final Bool FALSE = new Bool(false);
        public final boolean value;

        Bool(boolean value) {
            this.value = value;
        }

        @Override
        public String compact() {
            return value ? "true" : "false";
        }
    }

    public static final class Null extends Json {
        public static final Null INSTANCE = new Null();

        @Override
        public String compact() {
            return "null";
        }
    }

    public static void escape(StringBuilder out, String value) {
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            switch (c) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\b' -> out.append("\\b");
                case '\f' -> out.append("\\f");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                default -> {
                    if (c < 0x20) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
                }
            }
        }
    }

    public static Json parse(String text) {
        Parser parser = new Parser(text, false);
        Json value = parser.parseValue();
        parser.skip();
        if (parser.i != parser.n) {
            throw new IllegalArgumentException("trailing json");
        }
        return value;
    }

    /** Section 9 index body. Duplicate keys and a second value are rejected. */
    public static Json parseStrict(String text) {
        Parser parser = new Parser(text, true);
        Json value = parser.parseValue();
        parser.skip();
        if (parser.i != parser.n) {
            throw new IllegalArgumentException("trailing json");
        }
        return value;
    }

    public static boolean deepEqual(Json left, Json right) {
        if (left instanceof Obj lo && right instanceof Obj ro) {
            if (lo.fields.size() != ro.fields.size()) {
                return false;
            }
            for (Map.Entry<String, Json> entry : lo.fields.entrySet()) {
                Json other = ro.fields.get(entry.getKey());
                if (other == null || !deepEqual(entry.getValue(), other)) {
                    return false;
                }
            }
            return true;
        }
        if (left instanceof Arr la && right instanceof Arr ra) {
            if (la.items.size() != ra.items.size()) {
                return false;
            }
            for (int i = 0; i < la.items.size(); i++) {
                if (!deepEqual(la.items.get(i), ra.items.get(i))) {
                    return false;
                }
            }
            return true;
        }
        if (left instanceof Num ln && right instanceof Num rn) {
            return new BigDecimal(ln.digits).compareTo(new BigDecimal(rn.digits)) == 0;
        }
        if (left instanceof Str ls && right instanceof Str rs) {
            return ls.value.equals(rs.value);
        }
        if (left instanceof Bool lb && right instanceof Bool rb) {
            return lb.value == rb.value;
        }
        return left instanceof Null && right instanceof Null;
    }

    public static Obj asObj(Json value) {
        if (value instanceof Obj obj) {
            return obj;
        }
        throw new IllegalArgumentException("object");
    }

    public static Arr asArr(Json value) {
        if (value instanceof Arr arr) {
            return arr;
        }
        throw new IllegalArgumentException("array");
    }

    public static String asString(Json value) {
        if (value instanceof Str str) {
            return str.value;
        }
        throw new IllegalArgumentException("string");
    }

    public static String asNumber(Json value) {
        if (value instanceof Num num) {
            return num.digits;
        }
        throw new IllegalArgumentException("number");
    }

    private static final class Parser {
        final String s;
        final int n;
        final boolean strictKeys;
        int i;

        Parser(String s, boolean strictKeys) {
            this.s = s;
            this.n = s.length();
            this.strictKeys = strictKeys;
        }

        Json parseValue() {
            skip();
            if (i >= n) {
                throw new IllegalArgumentException("truncated json");
            }
            char c = s.charAt(i);
            return switch (c) {
                case '{' -> object();
                case '[' -> array();
                case '"' -> new Str(string());
                case 't' -> literal("true", Bool.TRUE);
                case 'f' -> literal("false", Bool.FALSE);
                case 'n' -> literal("null", Null.INSTANCE);
                default -> number();
            };
        }

        Obj object() {
            expect('{');
            Obj obj = new Obj();
            skip();
            if (peek('}')) {
                i++;
                return obj;
            }
            while (true) {
                skip();
                String key = string();
                skip();
                expect(':');
                Json value = parseValue();
                if (strictKeys && obj.fields.containsKey(key)) {
                    throw new IllegalArgumentException("duplicate json key");
                }
                obj.fields.put(key, value);
                skip();
                if (peek('}')) {
                    i++;
                    return obj;
                }
                expect(',');
            }
        }

        Arr array() {
            expect('[');
            Arr arr = new Arr();
            skip();
            if (peek(']')) {
                i++;
                return arr;
            }
            while (true) {
                arr.items.add(parseValue());
                skip();
                if (peek(']')) {
                    i++;
                    return arr;
                }
                expect(',');
            }
        }

        String string() {
            expect('"');
            StringBuilder out = new StringBuilder();
            while (i < n) {
                char c = s.charAt(i++);
                if (c == '"') {
                    return out.toString();
                }
                if (c == '\\') {
                    if (i >= n) {
                        throw new IllegalArgumentException("truncated json");
                    }
                    char e = s.charAt(i++);
                    switch (e) {
                        case '"', '\\', '/' -> out.append(e);
                        case 'b' -> out.append('\b');
                        case 'f' -> out.append('\f');
                        case 'n' -> out.append('\n');
                        case 'r' -> out.append('\r');
                        case 't' -> out.append('\t');
                        case 'u' -> {
                            if (i + 4 > n) {
                                throw new IllegalArgumentException("truncated json");
                            }
                            int cp = Integer.parseInt(s.substring(i, i + 4), 16);
                            out.append((char) cp);
                            i += 4;
                        }
                        default -> throw new IllegalArgumentException("escape");
                    }
                } else if (c < 0x20) {
                    throw new IllegalArgumentException("control");
                } else {
                    out.append(c);
                }
            }
            throw new IllegalArgumentException("truncated json");
        }

        Json number() {
            int start = i;
            if (peek('-')) {
                i++;
            }
            if (i >= n || !digit(s.charAt(i))) {
                throw new IllegalArgumentException("number");
            }
            if (s.charAt(i) == '0') {
                i++;
            } else {
                while (i < n && digit(s.charAt(i))) {
                    i++;
                }
            }
            if (i < n && s.charAt(i) == '.') {
                i++;
                if (i >= n || !digit(s.charAt(i))) {
                    throw new IllegalArgumentException("number");
                }
                while (i < n && digit(s.charAt(i))) {
                    i++;
                }
            }
            if (i < n && (s.charAt(i) == 'e' || s.charAt(i) == 'E')) {
                i++;
                if (i < n && (s.charAt(i) == '+' || s.charAt(i) == '-')) {
                    i++;
                }
                if (i >= n || !digit(s.charAt(i))) {
                    throw new IllegalArgumentException("number");
                }
                while (i < n && digit(s.charAt(i))) {
                    i++;
                }
            }
            return new Num(s.substring(start, i));
        }

        Json literal(String word, Json value) {
            if (!s.regionMatches(i, word, 0, word.length())) {
                throw new IllegalArgumentException(word);
            }
            i += word.length();
            return value;
        }

        void skip() {
            while (i < n) {
                char c = s.charAt(i);
                if (c != ' ' && c != '\n' && c != '\r' && c != '\t') {
                    return;
                }
                i++;
            }
        }

        boolean peek(char c) {
            return i < n && s.charAt(i) == c;
        }

        void expect(char c) {
            if (!peek(c)) {
                throw new IllegalArgumentException("expected " + c);
            }
            i++;
        }

        static boolean digit(char c) {
            return c >= '0' && c <= '9';
        }
    }
}
