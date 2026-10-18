// Library-level record, for sdk/tools/crosscheck.py.
//
// Usage: LibRecord OBJECT DOCS_JSON SEED_HEX
// Calls Measure.record at conformance-small sizes with the pinned draw stream
// SHA-256(seed || le64(i)), i = 0, 1, ..., and prints the record, or the line
// "<no record>" when Measure.record returns null.
import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import pcoc.json.Json;
import pcoc.hike.Measure;
import pcoc.hike.Profile;

public class LibRecord {
    public static void main(String[] args) throws Exception {
        byte[] object = Files.readAllBytes(Path.of(args[0]));
        Json.Obj docs = (Json.Obj) Json.parse(Files.readString(Path.of(args[1])));
        byte[] seed = java.util.HexFormat.of().parseHex(args[2]);
        ByteArrayOutputStream stream = new ByteArrayOutputStream();
        for (long i = 0; i < 64; i++) {
            MessageDigest sha = MessageDigest.getInstance("SHA-256");
            sha.update(seed);
            sha.update(ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(i).array());
            stream.write(sha.digest());
        }
        Measure.Reader index = height -> {
            Json value = docs.fields().get(Long.toString(height));
            return value == null ? null : ((Json.Str) value).value;
        };
        String record = Measure.record(object, Profile.CONFORMANCE_SMALL_V2, index, stream.toByteArray(), false);
        System.out.print(record == null ? "<no record>\n" : record);
    }
}
