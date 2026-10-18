// Produce a hike object offline from the stage0-start.json tip script at
// conformance-small sizes, then verify it.
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import pcoc.hike.Bytes;
import pcoc.hike.Drive;
import pcoc.hike.Measure;
import pcoc.hike.Profile;
import pcoc.json.Json;

public class ProduceExample {
    public static void main(String[] args) throws Exception {
        Path pack = Path.of(args[0]); // spec/conformance/v2.0
        Json.Obj v = (Json.Obj) Json.parse(Files.readString(pack.resolve("stage0-start.json")));
        byte[] publicKey = Bytes.unhex(((Json.Str) v.fields().get("public_key_hex")).value);
        // The tip sequence. Live produce reads it from Bitcoin header gossip;
        // each new header ends a stage.
        List<Drive.Tip> tips = new ArrayList<>();
        for (Json t : ((Json.Arr) v.fields().get("tips")).items()) {
            Json.Obj tip = (Json.Obj) t;
            tips.add(new Drive.Tip(
                    Bytes.unhex(((Json.Str) tip.fields().get("natural_hex")).value),
                    Long.parseLong(((Json.Num) tip.fields().get("height")).digits)));
        }

        // Drive has no plant service: every path is empty bytes.
        // Throws Drive.Clock when the key is not 32 bytes or the tips run out.
        Drive.Result result = Drive.run(publicKey, Profile.CONFORMANCE_SMALL_V2, tips); // Profile.PRODUCTION for a real PCOC
        System.out.println("object bytes: " + result.object.length);
        System.out.println("object sha256: " + Bytes.hex(Bytes.sha256(result.object)));

        // Verify what was produced. There is no index here, so header reads are "not read".
        String record = Measure.record(result.object, Profile.CONFORMANCE_SMALL_V2, height -> null, new byte[4096], false);
        Json.Obj parsed = (Json.Obj) Json.parse(record);
        System.out.println("shape: " + ((Json.Str) parsed.fields().get("shape")).value);
    }
}
