package io.groundinglab.pcoc.cli;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import io.groundinglab.pcoc.BuildInfo;
import io.groundinglab.pcoc.Bytes;
import io.groundinglab.pcoc.Draw;
import io.groundinglab.pcoc.Index;
import io.groundinglab.pcoc.Profile;
import io.groundinglab.pcoc.Sha256;
import io.groundinglab.pcoc.Verify;
import io.groundinglab.pcoc.produce.Cases;

/**
 * §18 verify CLI, plus {@code selftest}, {@code produce --rig}, and {@code --version}.
 * Stdout for verify is the section 14 record. Exit 0 is a record, including
 * {@code not read}, {@code mismatch}, and malformed CBOR. Exit 1 is an unreadable
 * file or a §7.3 decode failure. Exit 2 is usage. Streams carry no admission wording.
 */
public final class Main {
    private Main() {}

    public static void main(String[] args) {
        System.exit(run(args));
    }

    public static int run(String[] args) {
        if (args.length == 0) {
            usage();
            return 2;
        }
        if ("--version".equals(args[0]) || "version".equals(args[0])) {
            return version();
        }
        if ("selftest".equals(args[0])) {
            return selftest(args);
        }
        if ("quickstart".equals(args[0])) {
            return quickstart();
        }
        if ("verify".equals(args[0])) {
            return verify(args);
        }
        if ("produce".equals(args[0])) {
            return produce(args);
        }
        usage();
        return 2;
    }

    private static int quickstart() {
        try {
            Path sdk = BuildInfo.sdkDir();
            if (sdk == null) {
                System.err.println("SPEC_PIN not found");
                return 1;
            }
            Path root = Files.isRegularFile(sdk.resolve("SPEC_PIN")) ? sdk.getParent() : sdk;
            if (root == null) {
                System.err.println("SPEC_PIN not found");
                return 1;
            }
            String pinText = Files.readString(root.resolve("sdk").resolve("SPEC_PIN"), StandardCharsets.UTF_8);
            byte[] manifest = Files.readAllBytes(root.resolve("spec").resolve("conformance").resolve("pack").resolve("manifest.json"));
            String vector = pinField(pinText, "vector-sha256");
            String got = Bytes.hex(new Sha256().hash(manifest));
            if (!got.equals(vector) || !"2".equals(pinField(pinText, "erratum")) || vector.length() < 16) {
                System.err.println("pin mismatch");
                return 1;
            }
            byte[] object = Files.readAllBytes(root.resolve("sdk").resolve("java").resolve("objects").resolve("O1-S1.cbor"));
            String o1 = "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246";
            if (!o1.equals(Bytes.hex(new Sha256().hash(object)))) {
                System.err.println("O1-S1 sha256");
                return 1;
            }
            Profile profile = Profile.conformanceSmall(64, 32, 65536, 100);
            Sha256 sha = new Sha256();
            byte[] b0 = sha.hash("pcoc-pack-2.2/draw".getBytes(StandardCharsets.US_ASCII));
            Verify.Report report = Verify.verifyRig(object, profile, Index.EMPTY, new Draw.HashChain(sha, b0));
            if (!report.decoded || report.json == null || !measuredOk(report.json)) {
                System.err.println("O1-S1 six_leg, checkpoints, or waypoints differ");
                return 1;
            }
            String neg = Files.readString(root.resolve("sdk").resolve("java").resolve("candidates").resolve("negatives.txt"), StandardCharsets.UTF_8);
            if (count(neg, "matches_honest\tfalse") != 13) {
                System.err.println("negatives");
                return 1;
            }
            String prod = Files.readString(root.resolve("sdk").resolve("java").resolve("candidates").resolve("prod.txt"), StandardCharsets.UTF_8);
            if (!prod.contains("prod.PROD.stage.0.leg.0.legs_le_hex\t88a04ac5beacb8a7")) {
                System.err.println("prod leg");
                return 1;
            }
            System.out.println("pcoc " + BuildInfo.VERSION + " (java) " + pinField(pinText, "tag")
                    + " erratum " + pinField(pinText, "erratum") + " pack " + vector.substring(0, 16) + ": pin match");
            System.out.println("kernel scalar, sha256 " + BuildInfo.shaBackend());
            System.out.println("O1-S1 object sha256 " + o1);
            System.out.println("O1-S1 six_leg match, checkpoints match, waypoints match");
            System.out.println("negatives 13 non-match");
            System.out.println("PROD leg 0 legs_le_hex 88a04ac5beacb8a7");
            return 0;
        } catch (IOException | RuntimeException e) {
            System.err.println("quickstart not read");
            return 1;
        }
    }

    private static String pinField(String text, String key) {
        String prefix = key + "=";
        for (String line : text.split("\n")) {
            line = line.trim();
            if (line.startsWith(prefix)) {
                return line.substring(prefix.length());
            }
        }
        return "";
    }

    private static int count(String text, String lit) {
        int n = 0;
        int i = 0;
        while (true) {
            int j = text.indexOf(lit, i);
            if (j < 0) {
                return n;
            }
            n++;
            i = j + lit.length();
        }
    }

    private static String between(String text, String start, String end) {
        int i = text.indexOf(start);
        if (i < 0) {
            return null;
        }
        int a = i + start.length();
        int b = text.indexOf(end, a);
        if (b < 0) {
            return null;
        }
        return text.substring(a, b);
    }

    private static boolean resultsMatch(String body) {
        int n = 0;
        int i = 0;
        String mark = "\"result\":\"";
        while (true) {
            int j = body.indexOf(mark, i);
            if (j < 0) {
                return n > 0;
            }
            if (!body.startsWith("match\"", j + mark.length())) {
                return false;
            }
            n++;
            i = j + mark.length();
        }
    }

    private static boolean measuredOk(String record) {
        String six = between(record, "\"six_leg\":", ",\"six_leg_runs\":");
        if (six == null || six.contains("mismatch") || count(six, "\"match\"") != 6) {
            return false;
        }
        String checkpoints = between(record, "\"checkpoints\":", ",\"waypoints\":");
        String waypoints = between(record, "\"waypoints\":", ",\"paths\":");
        return checkpoints != null && waypoints != null && resultsMatch(checkpoints) && resultsMatch(waypoints);
    }

    private static int version() {
        try {
            System.out.print(BuildInfo.versionText());
            return 0;
        } catch (IOException e) {
            System.err.println("pin not read");
            return 1;
        }
    }

    private static int selftest(String[] args) {
        if (args.length != 1) {
            usage();
            return 2;
        }
        try {
            String text = BuildInfo.selftestText();
            System.out.print(text);
            return text.contains("mismatch") ? 1 : 0;
        } catch (IOException e) {
            System.err.println("pin not read");
            return 1;
        }
    }

    private static int produce(String[] args) {
        boolean rig = false;
        Path out = null;
        for (int i = 1; i < args.length; i++) {
            String a = args[i];
            if ("--rig".equals(a)) {
                rig = true;
            } else if ("--out".equals(a)) {
                String v = need(args, ++i);
                if (v.isEmpty()) {
                    usage();
                    return 2;
                }
                out = Path.of(v);
            } else {
                usage();
                return 2;
            }
        }
        if (!rig || out == null) {
            usage();
            return 2;
        }
        try {
            Cases.writeObjects(out);
            return 0;
        } catch (IOException e) {
            System.err.println("objects not written");
            return 1;
        }
    }

    private static int verify(String[] args) {
        String profileName = null;
        Integer slot = null;
        Integer sample = null;
        Integer terrain = null;
        Integer steps = null;
        Path indexDir = null;
        String drawLabel = null;
        Path object = null;
        for (int i = 1; i < args.length; i++) {
            String a = args[i];
            if ("--profile".equals(a)) {
                profileName = need(args, ++i);
            } else if ("--slot-bytes".equals(a)) {
                slot = integer(need(args, ++i));
            } else if ("--sample-bytes".equals(a)) {
                sample = integer(need(args, ++i));
            } else if ("--terrain-bytes".equals(a)) {
                terrain = integer(need(args, ++i));
            } else if ("--steps-per-leg".equals(a)) {
                steps = integer(need(args, ++i));
            } else if ("--index".equals(a)) {
                String v = need(args, ++i);
                if (v.isEmpty()) {
                    usage();
                    return 2;
                }
                indexDir = Path.of(v);
            } else if ("--draw-label".equals(a)) {
                drawLabel = need(args, ++i);
                if (drawLabel.isEmpty() && i >= args.length) {
                    usage();
                    return 2;
                }
            } else if (a.startsWith("--")) {
                usage();
                return 2;
            } else if (object == null) {
                object = Path.of(a);
            } else {
                usage();
                return 2;
            }
        }
        if (object == null) {
            usage();
            return 2;
        }
        if ((slot != null && slot < 0) || (sample != null && sample < 0)
                || (terrain != null && terrain < 0) || (steps != null && steps < 0)) {
            usage();
            return 2;
        }
        boolean small = "conformance-small".equals(profileName);
        if (profileName != null && !small && !"production".equals(profileName)
                && !"test".equals(profileName) && !"live".equals(profileName)) {
            usage();
            return 2;
        }
        Profile profile;
        if (small) {
            if (slot == null || sample == null || terrain == null || steps == null) {
                usage();
                return 2;
            }
            try {
                profile = Profile.conformanceSmall(slot, sample, terrain, steps);
            } catch (IllegalArgumentException e) {
                usage();
                return 2;
            }
        } else {
            profile = Profile.production();
        }
        byte[] bytes;
        try {
            bytes = Files.readAllBytes(object);
        } catch (IOException e) {
            System.err.println("object not read");
            return 1;
        }
        Index index = indexDir == null ? Index.EMPTY : new DirIndex(indexDir);
        Verify.Report report;
        if (drawLabel != null) {
            System.err.println("draw: injected label (rig input)");
            Sha256 sha = new Sha256();
            byte[] b0 = sha.hash(drawLabel.getBytes(java.nio.charset.StandardCharsets.US_ASCII));
            report = Verify.verifyRig(bytes, profile, index, new Draw.HashChain(sha, b0));
        } else {
            report = Verify.verify(bytes, profile, index);
        }
        if (!report.decoded) {
            System.err.println("object not decoded");
            return 1;
        }
        System.out.print(report.json);
        System.out.print('\n');
        return 0;
    }

    private static String need(String[] args, int i) {
        if (i >= args.length) {
            return "";
        }
        return args[i];
    }

    private static Integer integer(String s) {
        if (s.isEmpty() || (s.length() > 1 && s.charAt(0) == '0')) {
            return -1;
        }
        try {
            return Integer.valueOf(s);
        } catch (NumberFormatException e) {
            return -1;
        }
    }

    private static void usage() {
        System.err.println("usage: pcoc --version");
        System.err.println("usage: pcoc selftest");
        System.err.println("usage: pcoc quickstart");
        System.err.println("usage: pcoc produce --rig --out dir");
        System.err.println("usage: pcoc verify [--profile production|conformance-small|test|live]"
                + " [--slot-bytes N --sample-bytes N --terrain-bytes N --steps-per-leg N]"
                + " [--index dir] [--draw-label text] object");
    }

    private static final class DirIndex implements Index {
        final Path dir;

        DirIndex(Path dir) {
            this.dir = dir;
        }

        public byte[] get(long height) {
            Path file = dir.resolve(Long.toString(height) + ".json");
            if (!Files.isRegularFile(file)) {
                return null;
            }
            try {
                return Files.readAllBytes(file);
            } catch (IOException e) {
                return null;
            }
        }
    }
}
