package pcoc.cli;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import pcoc.hike.Bytes;
import pcoc.hike.Drive;
import pcoc.hike.Profile;
import pcoc.json.Json;

/**
 * satoshi-hike-produce. The locked rig passes --tips. Without that sequence
 * this port does not Observe and does not write an object.
 */
public final class Produce {
    public static void main(String[] args) {
        System.exit(run(args));
    }

    static int run(String[] args) {
        if (Args.hasFlag(args, "demo-mnemonic")) {
            System.err.println("satoshi-hike-produce: demo mnemonic is outside this port");
            usage();
            return 2;
        }
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
        if (parsed.tips == null) {
            System.err.println("satoshi-hike-produce: observe unavailable");
            writeReport(parsed.report, Drive.report(parsed.output, false));
            return 1;
        }
        List<Drive.Tip> tips;
        try {
            tips = readTips(parsed.tips);
        } catch (Exception ex) {
            System.err.println("satoshi-hike-produce: observe unavailable");
            writeReport(parsed.report, Drive.report(parsed.output, false));
            return 1;
        }
        Drive.Result result;
        try {
            result = Drive.run(parsed.publicKey, profile, tips);
        } catch (Drive.Clock ex) {
            System.err.println("satoshi-hike-produce: observe unavailable");
            writeReport(parsed.report, Drive.report(parsed.output, false));
            return 1;
        }
        System.err.print(result.stderr);
        try {
            Files.write(Path.of(parsed.output), result.object);
        } catch (Exception ex) {
            System.err.println("satoshi-hike-produce: output path");
            writeReport(parsed.report, Drive.report(parsed.output, false));
            return 1;
        }
        String report = Drive.report(parsed.output, true);
        System.out.print(report);
        if (parsed.report != null) {
            try {
                Files.writeString(Path.of(parsed.report), report);
            } catch (Exception ex) {
                System.err.println("satoshi-hike-produce: report path");
                return 1;
            }
        }
        return 0;
    }

    private static void writeReport(String path, String report) {
        System.out.print(report);
        if (path == null) {
            return;
        }
        try {
            Files.writeString(Path.of(path), report);
        } catch (Exception ignored) {
            // The stdout report is the one the caller already has.
        }
    }

    private static Profile profile(String name) {
        return Profile.forProduce(name);
    }

    private static List<Drive.Tip> readTips(String path) throws Exception {
        Json.Obj doc = Json.asObj(Json.parse(Files.readString(Path.of(path))));
        Json.Arr rows = Json.asArr(doc.fields().get("tip_sequence"));
        List<Drive.Tip> tips = new ArrayList<>();
        for (Json item : rows.items()) {
            Json.Obj row = Json.asObj(item);
            byte[] natural = Bytes.unhex(Json.asString(row.fields().get("natural_hex")));
            if (natural == null || natural.length != 32) {
                throw new IllegalArgumentException("tip");
            }
            tips.add(new Drive.Tip(natural, Long.parseUnsignedLong(Json.asNumber(row.fields().get("height")))));
        }
        if (tips.isEmpty()) {
            throw new IllegalArgumentException("tip");
        }
        return tips;
    }

    private static Parsed parse(String[] args) {
        Parsed out = new Parsed();
        out.profile = "test";
        for (int i = 0; i < args.length; i++) {
            String[] parts = Args.split(args[i]);
            if (parts == null) {
                return null;
            }
            String name = parts[0];
            boolean inline = "inline".equals(parts[2]);
            String value = parts[1];
            final int index = i;
            class Need {
                String take() {
                    if (inline) {
                        return value.isEmpty() ? null : value;
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
                case "pubkey-hex" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    byte[] raw = v == null ? null : Args.hex32(v);
                    if (raw == null) {
                        return null;
                    }
                    out.publicKey = raw;
                }
                case "output" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    if (v == null) {
                        return null;
                    }
                    out.output = v;
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
                case "tips" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    if (v == null) {
                        return null;
                    }
                    out.tips = v;
                }
                case "gossip-peer", "bari-url", "report", "gossip-network", "clock-source" -> {
                    String v = need.take();
                    if (!inline) {
                        i++;
                    }
                    if (v == null) {
                        return null;
                    }
                    if (name.equals("gossip-peer")) {
                        out.peer = v;
                    }
                    if (name.equals("report")) {
                        out.report = v;
                    }
                    if (name.equals("bari-url")) {
                        if (!Args.httpUrl(v)) {
                            return null;
                        }
                        out.bari = v;
                    }
                    if (name.equals("clock-source") && !v.equals("gossip")) {
                        return null;
                    }
                }
                case "tip-url" -> {
                    return null;
                }
                default -> {
                    return null;
                }
            }
        }
        if (out.publicKey == null || out.output == null) {
            return null;
        }
        // conformance-small is a verifier input. A --tips rig does not select it.
        if ("conformance-small".equals(out.profile)) {
            return null;
        }
        if (out.tips != null) {
            return out;
        }
        if (out.profile.equals("live") || out.profile.equals("production")) {
            return out;
        }
        if (!out.profile.equals("test") && !out.profile.equals("slow-lan")) {
            return null;
        }
        if (out.peer == null) {
            out.peer = System.getenv("PCOC_TEST_GOSSIP_PEER");
        }
        if (out.bari == null) {
            out.bari = System.getenv("PCOC_TEST_BARI_URL");
        }
        if (out.peer == null || out.bari == null) {
            return null;
        }
        if ("test".equals(out.profile) && Args.livePublicZone(out.bari)) {
            return null;
        }
        return out;
    }

    private static void usage() {
        System.err.println("usage: satoshi-hike-produce --pubkey-hex HEX --output PATH --gossip-peer HOST:PORT --bari-url URL [--profile test|live|production] [--tips PATH]");
    }

    private static final class Parsed {
        byte[] publicKey;
        String output;
        String profile;
        String tips;
        String peer;
        String bari;
        String report;
    }
}
