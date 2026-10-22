package pcoc;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

/**
 * FIPS 180-4 SHA-256 via {@link MessageDigest}. One instance is not shared across threads.
 * This host has no SHA-NI; the JDK implementation is the permitted software function (§11.1).
 */
public final class Sha256 {
    private final MessageDigest md;

    public Sha256() {
        try {
            md = MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("SHA-256", e);
        }
    }

    public void start() {
        md.reset();
    }

    public void update(byte[] b) {
        md.update(b);
    }

    public void update(byte[] b, int off, int len) {
        md.update(b, off, len);
    }

    public byte[] finish() {
        return md.digest();
    }

    public byte[] hash(byte[] a) {
        md.reset();
        md.update(a);
        return md.digest();
    }

    public byte[] hash(byte[] a, byte[] b) {
        md.reset();
        md.update(a);
        md.update(b);
        return md.digest();
    }

    public byte[] hash(byte[] a, byte[] b, byte[] c) {
        md.reset();
        md.update(a);
        md.update(b);
        md.update(c);
        return md.digest();
    }

    public byte[] hash(byte[] a, byte[] b, byte[] c, byte[] d) {
        md.reset();
        md.update(a);
        md.update(b);
        md.update(c);
        md.update(d);
        return md.digest();
    }
}
