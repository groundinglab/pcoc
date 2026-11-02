namespace Pcoc;

/// §14.3
public sealed class PathBody
{
    public byte[] Leaf { get; }
    public uint LeafIndex { get; }
    public uint TreeSize { get; }
    public byte[][] Siblings { get; }
    public byte[] Root { get; }
    public byte[] Txid { get; }
    public uint Vout { get; }
    public byte[] BlockHash { get; }
    public uint BlockHeight { get; }

    private PathBody(byte[] leaf, uint leafIndex, uint treeSize, byte[][] siblings, byte[] root, byte[] txid, uint vout, byte[] blockHash, uint blockHeight)
    {
        Leaf = leaf;
        LeafIndex = leafIndex;
        TreeSize = treeSize;
        Siblings = siblings;
        Root = root;
        Txid = txid;
        Vout = vout;
        BlockHash = blockHash;
        BlockHeight = blockHeight;
    }

    /// §14.3
    public static bool TryDecode(byte[] bytes, out PathBody body)
    {
        body = null!;
        if (bytes.Length < 146 || bytes[0] != 0x01)
        {
            return false;
        }
        var siblingCount = bytes[73];
        if (siblingCount > 32)
        {
            return false;
        }
        if (bytes.Length != 146 + 32 * siblingCount)
        {
            return false;
        }
        var leaf = Slice(bytes, 1, 32);
        var leafIndex = Be32(bytes, 33);
        var treeSize = Be32(bytes, 37);
        if (treeSize < 1 || leafIndex >= treeSize)
        {
            return false;
        }
        if (siblingCount != CanonicalSiblingCount(treeSize))
        {
            return false;
        }
        var siblings = new byte[siblingCount][];
        var o = 74;
        for (var s = 0; s < siblingCount; s++)
        {
            siblings[s] = Slice(bytes, o, 32);
            o += 32;
        }
        var root = Slice(bytes, o, 32);
        o += 32;
        var txid = Slice(bytes, o, 32);
        o += 32;
        var vout = Be32(bytes, o);
        o += 4;
        var blockHash = Slice(bytes, o, 32);
        o += 32;
        var blockHeight = Be32(bytes, o);
        body = new PathBody(leaf, leafIndex, treeSize, siblings, root, txid, vout, blockHash, blockHeight);
        return true;
    }

    /// §14.3
    public bool ValidateFor(byte[] waypoint)
    {
        if (!Bytes.Equal(Leaf, waypoint))
        {
            return false;
        }
        var node = Sha256.Hash(Sha256.Concat(new byte[] { 0x1A }, Leaf));
        uint index = LeafIndex;
        uint width = TreeSize;
        foreach (var sibling in Siblings)
        {
            if ((width & 1) == 1 && index == width - 1)
            {
                if (!Bytes.Equal(sibling, node))
                {
                    return false;
                }
                node = Parent(node, node);
            }
            else if ((index & 1) == 0)
            {
                node = Parent(node, sibling);
            }
            else
            {
                node = Parent(sibling, node);
            }
            index /= 2;
            width = width / 2 + (width & 1);
        }
        return Bytes.Equal(node, Root);
    }

    /// §14.3
    public static int CanonicalSiblingCount(uint treeSize)
    {
        var width = treeSize;
        var count = 0;
        while (width > 1)
        {
            count++;
            width = width / 2 + (width & 1);
        }
        return count;
    }

    private static byte[] Parent(byte[] left, byte[] right)
    {
        var pre = new byte[65];
        pre[0] = 0x1B;
        System.Buffer.BlockCopy(left, 0, pre, 1, 32);
        System.Buffer.BlockCopy(right, 0, pre, 33, 32);
        return Sha256.Hash(pre);
    }

    private static byte[] Slice(byte[] b, int o, int n)
    {
        var d = new byte[n];
        System.Buffer.BlockCopy(b, o, d, 0, n);
        return d;
    }

    private static uint Be32(byte[] b, int o)
    {
        return ((uint)b[o] << 24) | ((uint)b[o + 1] << 16) | ((uint)b[o + 2] << 8) | b[o + 3];
    }
}
