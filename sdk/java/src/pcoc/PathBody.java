package pcoc;

/** §14.3 canonical path body. */
public final class PathBody {
    public final byte[] leaf;
    public final long leafIndex;
    public final long treeSize;
    public final byte[][] siblings;
    public final byte[] root;
    public final byte[] txid;
    public final long vout;
    public final byte[] blockHash;
    public final long blockHeight;

    private PathBody(byte[] leaf, long leafIndex, long treeSize, byte[][] siblings, byte[] root,
                     byte[] txid, long vout, byte[] blockHash, long blockHeight) {
        this.leaf = leaf;
        this.leafIndex = leafIndex;
        this.treeSize = treeSize;
        this.siblings = siblings;
        this.root = root;
        this.txid = txid;
        this.vout = vout;
        this.blockHash = blockHash;
        this.blockHeight = blockHeight;
    }

    /** §14.3. Null when the bytes are not a canonical path body. */
    public static PathBody decode(byte[] raw) {
        if (raw == null || raw.length < 146 || raw[0] != 0x01) {
            return null;
        }
        int siblingCount = raw[1 + 32 + 4 + 4] & 0xff;
        if (siblingCount > 32) {
            return null;
        }
        if (raw.length != 146 + 32 * siblingCount) {
            return null;
        }
        int p = 1;
        byte[] leaf = copy(raw, p, 32);
        p += 32;
        long leafIndex = u32(raw, p);
        p += 4;
        long treeSize = u32(raw, p);
        p += 4;
        p += 1;
        if (treeSize < 1 || leafIndex >= treeSize) {
            return null;
        }
        if (siblingCount != canonicalSiblingCount(treeSize)) {
            return null;
        }
        byte[][] siblings = new byte[siblingCount][];
        for (int i = 0; i < siblingCount; i++) {
            siblings[i] = copy(raw, p, 32);
            p += 32;
        }
        byte[] root = copy(raw, p, 32);
        p += 32;
        byte[] txid = copy(raw, p, 32);
        p += 32;
        long vout = u32(raw, p);
        p += 4;
        byte[] blockHash = copy(raw, p, 32);
        p += 32;
        long blockHeight = u32(raw, p);
        return new PathBody(leaf, leafIndex, treeSize, siblings, root, txid, vout, blockHash, blockHeight);
    }

    /** §14.3. Leaf equals the waypoint and the Merkle path resolves to {@code root}. */
    public boolean validateFor(Sha256 sha, byte[] waypoint) {
        if (waypoint == null || waypoint.length != 32 || !Bytes.eq(leaf, waypoint)) {
            return false;
        }
        byte[] node = sha.hash(new byte[] {0x1a}, leaf);
        long index = leafIndex;
        long width = treeSize;
        for (byte[] sibling : siblings) {
            if ((width & 1L) == 1L && index == width - 1L) {
                if (!Bytes.eq(sibling, node)) {
                    return false;
                }
                node = parent(sha, node, node);
            } else if ((index & 1L) == 0L) {
                node = parent(sha, node, sibling);
            } else {
                node = parent(sha, sibling, node);
            }
            index = index / 2L;
            width = width / 2L + (width & 1L);
        }
        return Bytes.eq(node, root);
    }

    /** Count of levels while {@code width > 1}, then {@code width = floor(width/2) + (width mod 2)}. */
    public static int canonicalSiblingCount(long treeSize) {
        long width = treeSize;
        int count = 0;
        while (width > 1L) {
            count++;
            width = width / 2L + (width % 2L);
        }
        return count;
    }

    private static byte[] parent(Sha256 sha, byte[] left, byte[] right) {
        return sha.hash(new byte[] {0x1b}, left, right);
    }

    private static byte[] copy(byte[] raw, int off, int n) {
        byte[] o = new byte[n];
        System.arraycopy(raw, off, o, 0, n);
        return o;
    }

    private static long u32(byte[] raw, int off) {
        return ((raw[off] & 0xffL) << 24) | ((raw[off + 1] & 0xffL) << 16)
                | ((raw[off + 2] & 0xffL) << 8) | (raw[off + 3] & 0xffL);
    }
}
