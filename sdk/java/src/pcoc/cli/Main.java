package pcoc.cli;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import pcoc.Draw;
import pcoc.Index;
import pcoc.Profile;
import pcoc.Verify;
import pcoc.Version;

/** §18 verify CLI, plus {@code version}. No admission wording. */
public final class Main {
    private Main() {}

    public static void main(String[] args) {
        int code = run(args);
        System.exit(code);
    }

    public static int run(String[] args) {
        if (args.length == 0) {
            usage();
            return 2;
        }
        if ("version".equals(args[0])) {
            System.out.println(Version.version());
            return 0;
        }
        if (!"verify".equals(args[0])) {
            usage();
            return 2;
        }
        String profileName = null;
        Integer slot = null;
        Integer sample = null;
        Integer terrain = null;
        Integer steps = null;
        Path indexDir = null;
        String drawHex = null;
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
                indexDir = Path.of(need(args, ++i));
            } else if ("--draw-bytes".equals(a)) {
                drawHex = need(args, ++i);
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
        if (object == null || (slot != null && slot < 0) || (sample != null && sample < 0)
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
        Draw.Source draw = null;
        if (drawHex != null) {
            try {
                draw = new Draw.Finite(pcoc.Bytes.parseHex(drawHex));
            } catch (IllegalArgumentException e) {
                usage();
                return 2;
            }
        }
        Index index = indexDir == null ? Index.EMPTY : new DirIndex(indexDir);
        Verify.Report report = Verify.verify(bytes, profile, index, draw);
        if (!report.decoded) {
            System.err.println("object not decoded");
            return 1;
        }
        System.out.println(report.json);
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
        System.err.println("usage: version");
        System.err.println("usage: verify [--profile production|conformance-small|test|live]"
                + " [--slot-bytes N --sample-bytes N --terrain-bytes N --steps-per-leg N]"
                + " [--index dir] [--draw-bytes hex] object");
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
