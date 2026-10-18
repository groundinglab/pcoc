package pcoc.hike;

/**
 * 64-deep containers whose declared count equals the bytes left.
 * Run with {@code -Xmx128m}. A reservation taken from that count does not fit.
 */
public final class F10Reserve {
    private F10Reserve() {}

    public static void main(String[] args) {
        check(1 << 20, 0x9a, "arr-1MiB");
        check(2 << 20, 0x9a, "arr-2MiB");
        check(2 << 20, 0xba, "map-2MiB");
        check(4096, 0x9a, "arr-4KiB");
        System.out.println("f10 reserve ok");
    }

    static byte[] amp(int size, int levels, int major) {
        byte[] out = new byte[size];
        int n = 0;
        for (int k = 0; k < levels; k++) {
            int rem = size - n - 5;
            int cnt = major == 0xba ? rem / 2 : rem;
            out[n++] = (byte) major;
            out[n++] = (byte) (cnt >>> 24);
            out[n++] = (byte) (cnt >>> 16);
            out[n++] = (byte) (cnt >>> 8);
            out[n++] = (byte) cnt;
            if (major == 0xba) {
                out[n++] = 0x60;
            }
        }
        return out;
    }

    private static void check(int size, int major, String name) {
        String issue = Cbor.issue(amp(size, 64, major));
        if (issue == null || issue.isEmpty()) {
            throw new AssertionError(name + " decoded");
        }
        System.out.println(name + " " + issue);
    }
}
