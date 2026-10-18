// Verify the kat-small conformance object and reproduce the vector's stored
// section 14 record byte for byte.
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;
import pcoc.hike.Bytes;
import pcoc.hike.Measure;
import pcoc.hike.Profile;
import pcoc.hike.RecordHash;
import pcoc.json.Json;

public class Quickstart {
    public static void main(String[] args) throws Exception {
        Path pack = Path.of(args[0]); // spec/conformance/v2.0
        Json.Obj kat = (Json.Obj) Json.parse(Files.readString(pack.resolve("kat-small.json")));
        byte[] object = Bytes.unhex(((Json.Str) kat.fields().get("object_hex")).value);

        // The index: height -> section 9 document body. A real verifier GETs
        // <index>/index/<height>; here the vector carries the replies.
        Map<Long, String> docs = new HashMap<>();
        for (Json d : ((Json.Arr) kat.fields().get("documents")).items()) {
            Json.Obj doc = (Json.Obj) d;
            docs.put(Long.parseLong(((Json.Num) doc.fields().get("height")).digits),
                    ((Json.Str) doc.fields().get("body")).value);
        }
        Measure.Reader index = docs::get; // null = no body for that height

        // The six-leg draw bytes. kat-small's draw_bytes_hex is the pinned stream
        // SHA-256(seed || le64(0)) || SHA-256(seed || le64(1)) || ... for this seed.
        // Production passes 4096 bytes from SecureRandom.
        byte[] stream = Bytes.unhex(((Json.Str) kat.fields().get("draw_bytes_hex")).value);
        byte[] seed = new byte[32];
        Arrays.fill(seed, (byte) 0x5A);
        byte[] pinned = new byte[stream.length];
        for (int i = 0; i * 32 < pinned.length; i++) {
            byte[] block = Bytes.sha256(seed, Bytes.le64(i));
            System.arraycopy(block, 0, pinned, i * 32, Math.min(32, pinned.length - i * 32));
        }
        System.out.println("draw = draw_bytes_hex: " + Arrays.equals(pinned, stream));

        // Profile.PRODUCTION for real objects. null means CBOR that is not a map.
        String record = Measure.record(object, Profile.CONFORMANCE_SMALL_V2, index, stream, false);
        Json.Obj parsed = (Json.Obj) Json.parse(record);
        System.out.println("shape: " + ((Json.Str) parsed.fields().get("shape")).value);
        System.out.println("matches kat-small.json record: "
                + record.equals(((Json.Str) kat.fields().get("record")).value));
        System.out.println("record bytes sha256: " + Bytes.hex(Bytes.sha256(record.getBytes(StandardCharsets.UTF_8))));
        System.out.println("verify_record_sha256: " + Bytes.hex(RecordHash.verifyRecordSha256(record)));
    }
}
