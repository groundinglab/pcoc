package pcoc;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

/** Rule I5. Reads {@code sdk/SPEC_PIN} and the SHA-256 of the pack manifest. */
public final class Pin {
    public final String tag;
    public final String vectorSha256;

    public Pin(String tag, String vectorSha256) {
        this.tag = tag;
        this.vectorSha256 = vectorSha256;
    }

    /** Lines {@code tag=} and {@code vector-sha256=}. A {@code #} line is a comment. */
    public static Pin read(Path file) throws IOException {
        String tag = null;
        String hash = null;
        for (String line : Files.readAllLines(file, StandardCharsets.UTF_8)) {
            if (line.isEmpty() || line.charAt(0) == '#') {
                continue;
            }
            int eq = line.indexOf('=');
            if (eq < 0) {
                continue;
            }
            String key = line.substring(0, eq);
            String value = line.substring(eq + 1);
            if ("tag".equals(key)) {
                tag = value;
            } else if ("vector-sha256".equals(key)) {
                hash = value;
            }
        }
        if (tag == null || hash == null) {
            throw new IOException("SPEC_PIN");
        }
        return new Pin(tag, hash);
    }

    public static String sha256(Path file) throws IOException {
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            return Bytes.hex(md.digest(Files.readAllBytes(file)));
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("SHA-256", e);
        }
    }

    /** True when {@code vector-sha256} equals the SHA-256 of the manifest bytes. */
    public boolean matches(Path manifest) throws IOException {
        return vectorSha256.equals(sha256(manifest));
    }
}
