package io.groundinglab.pcoc;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Runs the release jar and captures stdout and stderr. */
public final class Capture {
    public final int code;
    public final String out;
    public final String err;

    private Capture(int code, String out, String err) {
        this.code = code;
        this.out = out;
        this.err = err;
    }

    public static Capture run(String... args) throws Exception {
        Path root = Repo.javaDir();
        Path jar = root.resolve("build/libs/pcoc-2.2.0.jar");
        if (!Files.isRegularFile(jar)) {
            throw new IllegalStateException("missing " + jar);
        }
        List<String> cmd = new ArrayList<String>();
        cmd.add(System.getProperty("java.home") + "/bin/java");
        cmd.add("-XX:+UnlockDiagnosticVMOptions");
        cmd.add("-Xmx512m");
        cmd.add("-jar");
        cmd.add(jar.toString());
        for (String a : args) {
            cmd.add(a);
        }
        Process p = new ProcessBuilder(cmd).directory(root.toFile()).start();
        byte[] out = p.getInputStream().readAllBytes();
        byte[] err = p.getErrorStream().readAllBytes();
        int code = p.waitFor();
        return new Capture(code, new String(out, StandardCharsets.UTF_8), new String(err, StandardCharsets.UTF_8));
    }

    public static void assertNoAdmissionWords(String text) {
        String[] banned = {"pass", "fail", "accept", "refuse"};
        for (String word : banned) {
            if (containsWord(text, word)) {
                throw new AssertionError(word + " in: " + text);
            }
        }
    }

    private static boolean containsWord(String text, String word) {
        String low = text.toLowerCase();
        int i = 0;
        while (i <= low.length() - word.length()) {
            int at = low.indexOf(word, i);
            if (at < 0) {
                return false;
            }
            boolean before = at == 0 || !Character.isLetter(low.charAt(at - 1));
            int after = at + word.length();
            boolean afterOk = after == low.length() || !Character.isLetter(low.charAt(after));
            if (before && afterOk) {
                return true;
            }
            i = at + 1;
        }
        return false;
    }
}
