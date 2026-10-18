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

    public static int run(String[] args) {
        Parsed parsed = parse(args);
        if (parsed == null) {
            usage();
            return 2;
        }
        Profile profile = profile(parsed);
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
        // Exit 1 only when no top-level map decodes: the record is not written
        // here, though the library record still exists for that buffer. A
        // trailing byte after a map still opens it.
        boolean opened = true;
        try {
            opened = Cbor.decodeHead(object).value.kind == Cbor.Val.Kind.MAP;
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

    private static Profile profile(Parsed parsed) {
        int given = 0;
        for (Integer n : new Integer[] {parsed.sampleBytes, parsed.slotBytes, parsed.terrainBytes, parsed.stepsPerLeg}) {
            if (n != null) {
                given++;
            }
        }
        if ("conformance-small".equals(parsed.profile)) {
            if (given != 4) {
                return null;
            }
            return Profile.conformanceSmall(parsed.sampleBytes, parsed.slotBytes, parsed.terrainBytes, parsed.stepsPerLeg);
        }
        // The four integers exist for conformance-small. Any of them here is usage.
        if (given != 0) {
            return null;
        }
        return Profile.forVerify(parsed.profile);
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
                case "sample-bytes", "slot-bytes", "terrain-bytes", "steps-per-leg" -> {
                    Integer v = decimal(need.take());
                    if (!inline) {
                        i++;
                    }
                    if (v == null) {
                        return null;
                    }
                    switch (name) {
                        case "sample-bytes" -> {
                            if (out.sampleBytes != null) {
                                return null;
                            }
                            out.sampleBytes = v;
                        }
                        case "slot-bytes" -> {
                            if (out.slotBytes != null) {
                                return null;
                            }
                            out.slotBytes = v;
                        }
                        case "terrain-bytes" -> {
                            if (out.terrainBytes != null) {
                                return null;
                            }
                            out.terrainBytes = v;
                        }
                        default -> {
                            if (out.stepsPerLeg != null) {
                                return null;
                            }
                            out.stepsPerLeg = v;
                        }
                    }
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

    /** A decimal integer with no sign and no leading zero. */
    private static Integer decimal(String text) {
        if (text == null || text.isEmpty() || (text.length() > 1 && text.charAt(0) == '0')) {
            return null;
        }
        int n = 0;
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            if (c < '0' || c > '9' || n > (Integer.MAX_VALUE - (c - '0')) / 10) {
                return null;
            }
            n = n * 10 + (c - '0');
        }
        return n;
    }

    private static void usage() {
        System.err.println("usage: satoshi-hike-verify --input PATH --index-url URL [--audit-seed HEX] [--report PATH] [--profile production|conformance-small|test|live] [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N] [--chain-rpc URL] [--log quiet|verbose]");
        System.err.println("       default profile is test. Omitted --index-url is PCOC_TEST_INDEX_URL. Unset is a usage error. This tree does not fill that host. It does not fill https://index.groundinglab.io.");
        System.err.println("       --profile conformance-small requires all four integers. Any of them with another profile is a usage error.");
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
        Integer sampleBytes;
        Integer slotBytes;
        Integer terrainBytes;
        Integer stepsPerLeg;
    }
}
