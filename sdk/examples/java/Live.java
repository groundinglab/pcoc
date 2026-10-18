// Verify a real hike object against the public index. Usage: Live OBJECT_FILE
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.time.Duration;
import pcoc.hike.Measure;
import pcoc.hike.Profile;

public class Live {
    public static void main(String[] args) throws Exception {
        byte[] object = Files.readAllBytes(Path.of(args[0]));
        HttpClient client = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(5)).build();
        Measure.Reader index = height -> {
            try {
                HttpResponse<String> r = client.send(
                        HttpRequest.newBuilder(URI.create("https://index.groundinglab.io/index/" + height)).GET().build(),
                        HttpResponse.BodyHandlers.ofString());
                return r.statusCode() == 200 ? r.body() : null;
            } catch (Exception e) {
                return null; // a failed read is recorded, not thrown
            }
        };
        byte[] stream = new byte[4096];
        new SecureRandom().nextBytes(stream);
        String record = Measure.record(object, Profile.PRODUCTION, index, stream, false);
        System.out.print(record == null ? "" : record); // null: CBOR that is not a map
    }
}
