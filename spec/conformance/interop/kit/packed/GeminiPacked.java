// Thin packed-run driver over Gemini's UNCHANGED satoshihike library (fat jar).
// run <label> <engine opt|scalar> <stopfile> : one independent hike (own pk = sha256(label),
//   own 256 MiB terrain, chained 2M-step legs per SPEC 11.2-11.4 stage 0) until stopfile exists.
// check <pk_hex> <prev_hex> : recompute one leg result + checkpoint.
import satoshihike.*;
import java.security.MessageDigest;
import java.nio.file.*;
import java.util.HexFormat;

public class GeminiPacked {
    static final HexFormat H = HexFormat.of();
    static long now() { java.time.Instant i = java.time.Instant.now(); return i.getEpochSecond() * 1_000_000_000L + i.getNano(); }
    public static void main(String[] a) throws Exception {
        Construction C = Construction.INSTANCE;
        Profile P = Profile.Companion.getPRODUCTION();
        byte[] beacon = CasesFixtures.INSTANCE.getSEED_BEACON();
        if (a[0].equals("run")) {
            byte[] pk = MessageDigest.getInstance("SHA-256").digest(a[1].getBytes("US-ASCII"));
            boolean scalar = a[2].equals("scalar");
            System.out.println("pk," + H.formatHex(pk));
            long t0 = now();
            byte[] seed = C.computeSeed(pk, beacon);
            Terrain t = scalar ? C.fillTerrainScalar(P, seed) : C.fillTerrain(P, seed, true);
            System.out.println("fill," + t0 + "," + now());
            byte[] waypoint = seed, prev = seed;
            Path stop = Paths.get(a[3]);
            for (int j = 0; j < 100000; j++) {
                long ts = now();
                long r = scalar ? C.legResultScalar(t, prev) : C.legResult(t, prev);
                byte[] le = Encoding.INSTANCE.le64(r);
                byte[] cp = C.computeCheckpoint(prev, waypoint, t.terrainSample(prev), le);
                long te = now();
                System.out.println("leg," + j + "," + ts + "," + te + "," + H.formatHex(le) + "," + H.formatHex(prev) + "," + H.formatHex(cp));
                System.out.flush();
                prev = cp;
                if (Files.exists(stop)) break;
            }
        } else {
            byte[] pk = H.parseHex(a[1]), prev = H.parseHex(a[2]);
            byte[] seed = C.computeSeed(pk, beacon);
            Terrain t = C.fillTerrain(P, seed, true);
            long r = C.legResult(t, prev);
            byte[] le = Encoding.INSTANCE.le64(r);
            byte[] cp = C.computeCheckpoint(prev, seed, t.terrainSample(prev), le);
            System.out.println(H.formatHex(le) + "," + H.formatHex(cp));
        }
    }
}
