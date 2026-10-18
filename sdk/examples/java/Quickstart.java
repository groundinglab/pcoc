// Verify the kat-small conformance object against its recorded index replies.
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HashMap;
import java.util.Map;
import pcoc.hike.Bytes;
import pcoc.hike.Measure;
import pcoc.hike.Profile;
import pcoc.json.Json;

public class Quickstart {
    public static void main(String[] args) throws Exception {
        Path pack = Path.of(args[0]); // spec/conformance/v2.0
        Json.Obj kat = (Json.Obj) Json.parse(Files.readString(pack.resolve("kat-small.json")));
        byte[] object = Bytes.unhex(((Json.Str) kat.fields().get("object_hex")).value);

        // The index: height -> document body. A real verifier GETs <index>/index/<height>.
        Map<Long, String> docs = new HashMap<>();
        for (Json d : ((Json.Arr) kat.fields().get("documents")).items()) {
            Json.Obj doc = (Json.Obj) d;
            docs.put(Long.parseLong(((Json.Num) doc.fields().get("height")).digits),
                    ((Json.Str) doc.fields().get("body")).value);
        }
        Measure.Reader index = docs::get; // null = no body for that height

        // Draw bytes: SHA-256(seed || le64(i)) blocks, as the CLIs' --audit-seed.
        // Use 4096 bytes from SecureRandom in production.
        byte[] seed = new byte[32];
        java.util.Arrays.fill(seed, (byte) 0x11);
        byte[] stream = new byte[4096];
        for (int i = 0, filled = 0; filled < stream.length; i++) {
            byte[] block = Bytes.sha256(seed, Bytes.le64(i));
            int n = Math.min(block.length, stream.length - filled);
            System.arraycopy(block, 0, stream, filled, n);
            filled += n;
        }

        // Profile.PRODUCTION for real objects. null means the object did not decode.
        String record = Measure.record(object, Profile.CONFORMANCE_SMALL_V2, index, stream, false);
        byte[] digest = MessageDigest.getInstance("SHA-256").digest(record.getBytes(StandardCharsets.UTF_8));
        System.out.println("record sha256: " + Bytes.hex(digest));
    }
}
