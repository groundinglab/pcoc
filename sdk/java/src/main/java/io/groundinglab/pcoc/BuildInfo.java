package io.groundinglab.pcoc;

import java.io.IOException;
import java.lang.management.ManagementFactory;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;

import com.sun.management.HotSpotDiagnosticMXBean;

/**
 * Rule I5 and §11.1. Version, pack pin, and the SHA-256 backend.
 * {@code sha-ni} is reported only when the CPU has SHA-NI and the JVM reports
 * {@code UseSHA} and {@code UseSHA256Intrinsics} both on. {@code UseSHA256Intrinsics}
 * is a diagnostic flag; the process must be started with {@code -XX:+UnlockDiagnosticVMOptions}
 * for the JVM to report it. The flag's default is already on; unlocking only makes it readable.
 */
public final class BuildInfo {
    public static final String VERSION = "2.2.0";
    public static final String LANGUAGE = "java";
    public static final String KERNEL = "scalar";

    private BuildInfo() {}

    /** Walk from the working directory to {@code sdk/SPEC_PIN}. */
    public static Path sdkDir() {
        String prop = System.getProperty("pcoc.sdk.dir");
        if (prop != null && !prop.isEmpty()) {
            return Path.of(prop);
        }
        Path d = Path.of("").toAbsolutePath();
        for (int i = 0; i < 8 && d != null; i++) {
            if (Files.isRegularFile(d.resolve("SPEC_PIN"))) {
                return d;
            }
            Path nested = d.resolve("sdk").resolve("SPEC_PIN");
            if (Files.isRegularFile(nested)) {
                return d.resolve("sdk");
            }
            d = d.getParent();
        }
        return null;
    }

    public static Pin pin() throws IOException {
        Path dir = sdkDir();
        if (dir == null) {
            throw new IOException("SPEC_PIN");
        }
        return Pin.read(dir.resolve("SPEC_PIN"));
    }

    public static boolean cpuHasShaNi() {
        try {
            String cpu = Files.readString(Path.of("/proc/cpuinfo"), StandardCharsets.UTF_8);
            return cpu.contains(" sha_ni") || cpu.contains("\tsha_ni") || cpu.startsWith("sha_ni");
        } catch (IOException e) {
            return false;
        }
    }

    /** True only when the JVM reports the named flag as {@code true}. */
    public static boolean jvmFlag(String name) {
        try {
            HotSpotDiagnosticMXBean bean = ManagementFactory.getPlatformMXBean(HotSpotDiagnosticMXBean.class);
            return "true".equals(bean.getVMOption(name).getValue());
        } catch (RuntimeException e) {
            return false;
        }
    }

    /**
     * §11.1 backend name. {@code sha-ni} only when the CPU has SHA-NI and both
     * JDK flags are reported on. Otherwise {@code software}.
     */
    public static String shaBackend() {
        if (cpuHasShaNi() && jvmFlag("UseSHA") && jvmFlag("UseSHA256Intrinsics")) {
            return "sha-ni";
        }
        return "software";
    }

    /** CPU model line, or empty when {@code /proc/cpuinfo} has none. No hostname. */
    public static String cpuModel() {
        try {
            for (String line : Files.readAllLines(Path.of("/proc/cpuinfo"), StandardCharsets.UTF_8)) {
                if (line.startsWith("model name")) {
                    int colon = line.indexOf(':');
                    return colon < 0 ? "" : line.substring(colon + 1).trim();
                }
            }
        } catch (IOException e) {
            return "";
        }
        return "";
    }

    /**
     * §7.4 known answers: FIPS SHA-256("abc") and the seal of {@code S_folded = 0}.
     * Returns null when both agree.
     */
    public static String katProblem() {
        Sha256 sha = new Sha256();
        byte[] abc = sha.hash("abc".getBytes(StandardCharsets.US_ASCII));
        String abcHex = Bytes.hex(abc);
        if (!"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".equals(abcHex)) {
            return "kat sha256 mismatch";
        }
        byte[] buf = new byte[20];
        System.arraycopy(Const.SEAL_DOMAIN, 0, buf, 0, Const.SEAL_DOMAIN.length);
        long s = Step.seal(sha, buf, 0L);
        if (s != 0x76042423afbd1648L) {
            return "kat seal mismatch";
        }
        return null;
    }

    /** Text of {@code pcoc selftest}. One trailing newline is added by the caller. */
    public static String selftestText() throws IOException {
        Pin pin = pin();
        String problem = katProblem();
        StringBuilder b = new StringBuilder();
        b.append("sdk ").append(LANGUAGE).append('\n');
        b.append("version ").append(VERSION).append('\n');
        b.append("spec ").append(pin.tag).append('\n');
        b.append("pack ").append(pin.vectorSha256).append('\n');
        b.append("kernel ").append(KERNEL).append('\n');
        b.append("sha ").append(shaBackend()).append('\n');
        b.append("sha-flag UseSHA=").append(jvmFlag("UseSHA")).append('\n');
        b.append("sha-flag UseSHA256Intrinsics=").append(jvmFlag("UseSHA256Intrinsics")).append('\n');
        b.append("cpu-sha-ni ").append(cpuHasShaNi()).append('\n');
        String model = cpuModel();
        if (!model.isEmpty()) {
            b.append("cpu ").append(model).append('\n');
        }
        b.append(problem == null ? "kat seal match" : problem).append('\n');
        b.append(problem == null ? "kat sha256 match" : problem).append('\n');
        return b.toString();
    }

    public static String versionText() throws IOException {
        Pin pin = pin();
        return VERSION + "\n" + pin.tag + "\n" + pin.vectorSha256.toLowerCase(Locale.ROOT) + "\n";
    }
}
