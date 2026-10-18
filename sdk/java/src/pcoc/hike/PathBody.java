package pcoc.hike;

import java.util.ArrayList;
import java.util.List;

/**
 * Canonical path body. The layout matches the reference path body copied
 * from services/bari. This class does not define a second grammar.
 */
public final class PathBody {
    static final int VERSION = 0x01;
    static final int MAX_SIBLINGS = 32;
    static final int FIXED_LEN = 146;
    static final byte LEAF_DOMAIN = 0x1A;
    static final byte PARENT_DOMAIN = 0x1B;

    /** Locators a chain bind receives after the body validates. */
    public static final class Locators {
        public final byte[] root;
        public final byte[] txid;
        public final long vout;
        public final byte[] blockHash;
        public final long blockHeight;

        Locators(byte[] root, byte[] txid, long vout, byte[] blockHash, long blockHeight) {
            this.root = root;
            this.txid = txid;
            this.vout = vout;
            this.blockHash = blockHash;
            this.blockHeight = blockHeight;
        }
    }

    private final byte[] leaf;
    private final long leafIndex;
    private final long treeSize;
    private final List<byte[]> siblings;
    private final byte[] root;
    private final byte[] txid;
    private final long vout;
    private final byte[] blockHash;
    private final long blockHeight;

    private PathBody(
            byte[] leaf,
            long leafIndex,
            long treeSize,
            List<byte[]> siblings,
            byte[] root,
            byte[] txid,
            long vout,
            byte[] blockHash,
            long blockHeight) {
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

    /** One-leaf fixture. block_hash is 32 bytes of 0x55, txid 0x44, vout 2. */
    public static byte[] encodePathInBlock(byte[] waypoint, long blockHeight) {
        byte[] blockHash = new byte[32];
        for (int i = 0; i < blockHash.length; i++) {
            blockHash[i] = 0x55;
        }
        return encodeForBlock(waypoint, blockHash, blockHeight);
    }

    /**
     * One-leaf fixture. {@code blockHash} is 32 natural-order bytes, the order
     * the path body stores. The verify record reverses them (SPEC §9).
     */
    public static byte[] encodeForBlock(byte[] waypoint, byte[] blockHash, long blockHeight) {
        if (waypoint == null || waypoint.length != 32 || blockHash == null || blockHash.length != 32) {
            return null;
        }
        byte[] root = leafNode(waypoint);
        byte[] out = new byte[FIXED_LEN];
        out[0] = VERSION;
        System.arraycopy(waypoint, 0, out, 1, 32);
        putBe32(out, 37, 1);
        System.arraycopy(root, 0, out, 42, 32);
        for (int i = 0; i < 32; i++) {
            out[74 + i] = 0x44;
        }
        putBe32(out, 106, 2);
        System.arraycopy(blockHash, 0, out, 110, 32);
        putBe32(out, 142, blockHeight);
        return out;
    }

    /** block_height when the body validates for its own leaf. */
    public static Long inclusionHeight(byte[] path) {
        PathBody body = decode(path);
        if (body == null) {
            return null;
        }
        return body.blockHeight;
    }

    /** Locators when the body validates for waypoint. Null when it does not. */
    public static Locators locatorsFor(byte[] path, byte[] waypoint) {
        PathBody body = decode(path);
        if (body == null || !body.validFor(waypoint)) {
            return null;
        }
        return new Locators(body.root, body.txid, body.vout, body.blockHash, body.blockHeight);
    }

    private static PathBody decode(byte[] bytes) {
        if (bytes == null || bytes.length < FIXED_LEN || (bytes[0] & 0xff) != VERSION) {
            return null;
        }
        int count = bytes[41] & 0xff;
        if (count > MAX_SIBLINGS || bytes.length != FIXED_LEN + count * 32) {
            return null;
        }
        byte[] leaf = copy(bytes, 1, 32);
        long leafIndex = be32(bytes, 33);
        long treeSize = be32(bytes, 37);
        List<byte[]> siblings = new ArrayList<>();
        int at = 42;
        for (int i = 0; i < count; i++) {
            siblings.add(copy(bytes, at, 32));
            at += 32;
        }
        byte[] root = copy(bytes, at, 32);
        at += 32;
        byte[] txid = copy(bytes, at, 32);
        at += 32;
        long vout = be32(bytes, at);
        at += 4;
        byte[] blockHash = copy(bytes, at, 32);
        at += 32;
        long blockHeight = be32(bytes, at);
        PathBody body = new PathBody(leaf, leafIndex, treeSize, siblings, root, txid, vout, blockHash, blockHeight);
        if (!body.validFor(leaf)) {
            return null;
        }
        return body;
    }

    private boolean validFor(byte[] requested) {
        if (!Bytes.eq(leaf, requested)) {
            return false;
        }
        byte[] got = resolve();
        return got != null && Bytes.eq(got, root);
    }

    private byte[] resolve() {
        if (treeSize == 0 || Long.compareUnsigned(leafIndex, treeSize) >= 0) {
            return null;
        }
        int expected = siblingCount(treeSize);
        if (expected < 0 || siblings.size() != expected) {
            return null;
        }
        byte[] node = leafNode(leaf);
        long index = leafIndex;
        long width = treeSize;
        for (byte[] sibling : siblings) {
            if (Long.remainderUnsigned(width, 2) == 1 && index == width - 1) {
                if (!Bytes.eq(sibling, node)) {
                    return null;
                }
                node = parentNode(node, node);
            } else if (Long.remainderUnsigned(index, 2) == 0) {
                node = parentNode(node, sibling);
            } else {
                node = parentNode(sibling, node);
            }
            index = Long.divideUnsigned(index, 2);
            width = Long.divideUnsigned(width, 2) + Long.remainderUnsigned(width, 2);
        }
        return node;
    }

    private static int siblingCount(long width) {
        if (width == 0) {
            return -1;
        }
        int count = 0;
        while (Long.compareUnsigned(width, 1) > 0) {
            width = Long.divideUnsigned(width, 2) + Long.remainderUnsigned(width, 2);
            count++;
        }
        return count;
    }

    private static byte[] leafNode(byte[] leaf) {
        byte[] pre = new byte[33];
        pre[0] = LEAF_DOMAIN;
        System.arraycopy(leaf, 0, pre, 1, 32);
        return Bytes.sha256(pre);
    }

    private static byte[] parentNode(byte[] left, byte[] right) {
        byte[] pre = new byte[65];
        pre[0] = PARENT_DOMAIN;
        System.arraycopy(left, 0, pre, 1, 32);
        System.arraycopy(right, 0, pre, 33, 32);
        return Bytes.sha256(pre);
    }

    private static byte[] copy(byte[] src, int off, int len) {
        byte[] out = new byte[len];
        System.arraycopy(src, off, out, 0, len);
        return out;
    }

    private static long be32(byte[] src, int off) {
        return ((src[off] & 255L) << 24)
                | ((src[off + 1] & 255L) << 16)
                | ((src[off + 2] & 255L) << 8)
                | (src[off + 3] & 255L);
    }

    private static void putBe32(byte[] dest, int off, long value) {
        dest[off] = (byte) (value >>> 24);
        dest[off + 1] = (byte) (value >>> 16);
        dest[off + 2] = (byte) (value >>> 8);
        dest[off + 3] = (byte) value;
    }
}
