package pcoc.cli;

import java.net.URI;
import java.util.Locale;

/** Flag splitting shared by the two programs. Values may be --name=value or the next argument. */
final class Args {
    private Args() {}

    static boolean hasFlag(String[] args, String name) {
        String prefix = "--" + name;
        for (String arg : args) {
            if (arg.equals(prefix) || arg.startsWith(prefix + "=")) {
                return true;
            }
        }
        return false;
    }

    static String[] split(String arg) {
        if (!arg.startsWith("--") || arg.length() < 3) {
            return null;
        }
        String body = arg.substring(2);
        int eq = body.indexOf('=');
        if (eq >= 0) {
            return new String[] {body.substring(0, eq), body.substring(eq + 1), "inline"};
        }
        return new String[] {body, "", "next"};
    }

    /** Public zone. A test profile must not dial the apex or any host under it. */
    static boolean livePublicZone(String raw) {
        String host = hostOf(raw);
        if (host == null) {
            return false;
        }
        host = host.toLowerCase(Locale.ROOT);
        return host.equals("groundinglab.io") || host.endsWith(".groundinglab.io");
    }

    private static String hostOf(String raw) {
        try {
            String host = new URI(raw).getHost();
            if (host == null || host.isEmpty()) {
                return null;
            }
            return host;
        } catch (Exception ex) {
            return null;
        }
    }

    static boolean httpUrl(String raw) {
        if (raw.endsWith("/")) {
            return false;
        }
        try {
            URI uri = new URI(raw);
            if (uri.getHost() == null || uri.getHost().isEmpty()) {
                return false;
            }
            String scheme = uri.getScheme() == null ? "" : uri.getScheme().toLowerCase(Locale.ROOT);
            return scheme.equals("http") || scheme.equals("https");
        } catch (Exception ex) {
            return false;
        }
    }

    static byte[] hex32(String spelling) {
        if (spelling.length() != 64) {
            return null;
        }
        byte[] raw = pcoc.hike.Bytes.unhex(spelling);
        if (raw == null || raw.length != 32) {
            return null;
        }
        return raw;
    }
}
