package pcoc.cli;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.time.Duration;
import pcoc.hike.Bytes;
import pcoc.hike.Cbor;
import pcoc.hike.Measure;
import pcoc.hike.Profile;

/**
 * satoshi-hike-verify. Exit 0 writes a section 14 record.
 * Exit 1 is an unreadable or undecodable file. Exit 2 is usage.
 * Admission is not applied to the exit status.
 */
public final class Verify {
    public static void main(String[] args) {
        System.exit(run(args));
    }

    static int run(String[] args) {
        Parsed parsed = parse(args);
        if (parsed == null) {
            usage();
            return 2;
        }
        Profile profile = profile(parsed.profile);
        if (profile == null) {
            usage();
            return 2;
        }
        String indexUrl = resolveIndex(parsed);
        if (indexUrl == null) {
            usage();
            return 2;
        }
        if (parsed.chainRpc != null && !parsed.profile.equals("live")) {
            usage();
            return 2;
        }
        if (parsed.profile.equals("live") && parsed.chainRpc != null && !Args.httpUrl(parsed.chainRpc)) {
            usage();
            return 2;
        }
        byte[] object;
        try {
            object = Files.readAllBytes(Path.of(parsed.input));
        } catch (Exception ex) {
            System.err.println("satoshi-hike-verify: input path");
            return 1;
        }
        byte[] stream;
        if (parsed.audit != null) {
            stream = auditStream(parsed.audit, 4096);
        } else {
            stream = new byte[4096];
            try {
                new SecureRandom().nextBytes(stream);
            } catch (RuntimeException ex) {
                stream = null;
            }
        }
        // A buffer that does not open is not written here. The library
        // record still exists for that buffer. A trailing byte opens the item.
        boolean opened = true;
        try {
            Cbor.decodeHead(object);
        } catch (Cbor.Problem problem) {
            opened = false;
        }
        String record = Measure.record(object, profile, fetch(indexUrl), stream, false);
        if (!opened || record == null) {
            System.err.println("satoshi-hike-verify: undecodable");
            return 1;
        }
        try {
            System.out.write(record.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        } catch (Exception ex) {
            System.err.println("satoshi-hike-verify: stdout");
            return 1;
        }
        if ("verbose".equals(parsed.log)) {
            System.err.println(record);
        }
        if (parsed.report != null) {
            try {
                Files.writeString(Path.of(parsed.report), record);
            } catch (Exception ex) {
                System.err.println("satoshi-hike-verify: report path");
                return 1;
            }
        }
        return 0;
    }

    private static Profile profile(String name) {
        return Profile.forVerify(name);
    }

    private static String resolveIndex(Parsed parsed) {
        if (parsed.indexUrl != null) {
            if (!"live".equals(parsed.profile) && Args.livePublicZone(parsed.indexUrl)) {
                return null;
            }
            return parsed.indexUrl;
        }
        if ("live".equals(parsed.profile)) {
            return "https://index.groundinglab.io";
        }
        String env = System.getenv("PCOC_TEST_INDEX_URL");
        if (env == null || !Args.httpUrl(env) || Args.livePublicZone(env)) {
            return null;
        }
        return env;
    }

    private static Measure.Reader fetch(String base) {
        HttpClient client = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(2))
                .followRedirects(HttpClient.Redirect.NEVER)
                .build();
        return height -> {
            try {
                HttpRequest request = HttpRequest.newBuilder()
                        .uri(URI.create(base + "/index/" + height))
                        .timeout(Duration.ofSeconds(2))
                        .GET()
                        .build();
                HttpResponse<byte[]> response = client.send(request, HttpResponse.BodyHandlers.ofByteArray());
                if (response.statusCode() != 200) {
                    return null;
                }
                byte[] body = response.body();
                if (body.length > (1 << 20)) {
                    body = java.util.Arrays.copyOf(body, 1 << 20);
                }
                return new String(body, java.nio.charset.StandardCharsets.UTF_8);
            } catch (Exception ex) {
                return null;
            }
        };
    }

    static byte[] auditStream(byte[] seed, int bytes) {
        byte[] out = new byte[bytes];
        int filled = 0;
        int i = 0;
        MessageDigest md = Bytes.digest();
        byte[] counter = new byte[8];
        while (filled < bytes) {
            Bytes.putLe64(counter, 0, i);
            md.update(seed);
            md.update(counter);
            byte[] block;
            try {
                block = md.digest();
            } catch (RuntimeException ex) {
                break;
            }
            int n = Math.min(block.length, bytes - filled);
            System.arraycopy(block, 0, out, filled, n);
            filled += n;
            i++;
        }
        return out;
    }

    private static Parsed parse(String[] args) {
        Parsed out = new Parsed();
        out.log = "quiet";
        out.profile = "test";
        for (int i = 0; i < args.length; i++) {
            String[] parts = Args.split(args[i]);
            if (parts == null) {
                return null;
            }
            String name = parts[0];
            boolean inline = "inline".equals(parts[2]);
            String inlineValue = parts[1];
            final int index = i;
            class Need {
                String take() {
                    if (inline) {
                        return inlineValue.isEmpty() ? null : inlineValue;
                    }
                    int next = index + 1;
                    if (next >= args.length || args[next].startsWith("-")) {
                        return null;
                    }
                    return args[next];
                }
            }
            Need need = new Need();
            switch (name) {
                case "input" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    if (v == null) {
                        return null;
                    }
                    out.input = v;
                }
                case "index-url" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    if (v == null || !Args.httpUrl(v)) {
                        return null;
                    }
                    out.indexUrl = v;
                }
                case "profile" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    if (v == null) {
                        return null;
                    }
                    out.profile = v;
                }
                case "audit-seed" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    byte[] raw = v == null ? null : Args.hex32(v);
                    if (raw == null) {
                        return null;
                    }
                    out.audit = raw;
                }
                case "report" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    if (v == null) {
                        return null;
                    }
                    out.report = v;
                }
                case "chain-rpc" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    if (v == null) {
                        return null;
                    }
                    out.chainRpc = v;
                }
                case "log" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    if (v == null || (!v.equals("quiet") && !v.equals("verbose"))) {
                        return null;
                    }
                    out.log = v;
                }
                default -> {
                    return null;
                }
            }
        }
        if (out.input == null) {
            return null;
        }
        return out;
    }

    private static void usage() {
        System.err.println("usage: satoshi-hike-verify --input PATH --index-url URL [--audit-seed HEX] [--report PATH] [--profile production|conformance-small|test|live] [--chain-rpc URL] [--log quiet|verbose]");
        System.err.println("       default profile is test. Omitted --index-url is PCOC_TEST_INDEX_URL. Unset is a usage error. This tree does not fill that host. It does not fill https://index.groundinglab.io.");
        System.err.println("       --profile live fills that index. --chain-rpc is live-only.");
        System.err.println("       --log defaults to quiet. verbose writes the measurement record to stderr. stdout is the measurement record when the object decodes.");
    }

    private static final class Parsed {
        String input;
        String indexUrl;
        String profile;
        byte[] audit;
        String report;
        String chainRpc;
        String log;
    }
}
