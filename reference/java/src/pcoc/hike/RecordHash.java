package pcoc.hike;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import pcoc.json.Json;

/** SHA-256 of a section 14 record under RFC 8785. The trailing newline is not an input. */
public final class RecordHash {
    private RecordHash() {}

    public static byte[] verifyRecordSha256(String record) {
        String body = record.endsWith("\n") ? record.substring(0, record.length() - 1) : record;
        return Bytes.sha256(jcs(Json.parse(body)).getBytes(java.nio.charset.StandardCharsets.UTF_8));
    }

    static String jcs(Json value) {
        if (value instanceof Json.Obj obj) {
            List<String> keys = new ArrayList<>(obj.fields().keySet());
            keys.sort(Comparator.naturalOrder());
            StringBuilder out = new StringBuilder();
            out.append('{');
            boolean first = true;
            for (String key : keys) {
                if (!first) {
                    out.append(',');
                }
                first = false;
                out.append(Json.str(key).compact()).append(':').append(jcs(obj.fields().get(key)));
            }
            out.append('}');
            return out.toString();
        }
        if (value instanceof Json.Arr arr) {
            StringBuilder out = new StringBuilder();
            out.append('[');
            boolean first = true;
            for (Json item : arr.items()) {
                if (!first) {
                    out.append(',');
                }
                first = false;
                out.append(jcs(item));
            }
            out.append(']');
            return out.toString();
        }
        return value.compact();
    }
}
