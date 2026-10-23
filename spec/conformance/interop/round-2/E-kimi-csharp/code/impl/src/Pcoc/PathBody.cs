using System;
using System.Security.Cryptography;

namespace Pcoc;

/// <summary>
/// The canonical path body of section 14.3: version 0x01, then leaf (32),
/// leaf_index (BE4), tree_size (BE4), sibling_count (1), siblings
/// (32 × count), root (32), txid (32), vout (BE4), block_hash (32),
/// block_height (BE4). Length is exactly 146 + 32 × sibling_count.
/// </summary>
public sealed class PathBody
{
    public byte[] Leaf = Array.Empty<byte>();
    public uint LeafIndex;
    public uint TreeSize;
    public byte[][] Siblings = Array.Empty<byte[]>();
    public byte[] Root = Array.Empty<byte>();
    public byte[] Txid = Array.Empty<byte>();
    public uint Vout;
    public byte[] BlockHash = Array.Empty<byte>();
    public uint BlockHeight;

    public const int MinLength = 146;
    public const int MaxSiblings = 32;

    /// <summary>Decodes a canonical path body; null when the bytes are not one.</summary>
    public static PathBody? Decode(ReadOnlySpan<byte> bytes)
    {
        if (bytes.Length < MinLength) return null;
        if (bytes[0] != 0x01) return null;
        int siblingCount = bytes[41];
        if (siblingCount > MaxSiblings) return null;
        if (bytes.Length != MinLength + 32 * siblingCount) return null;
        var body = new PathBody
        {
            Leaf = bytes.Slice(1, 32).ToArray(),
            LeafIndex = ReadBe32(bytes.Slice(33, 4)),
            TreeSize = ReadBe32(bytes.Slice(37, 4)),
        };
        if (body.TreeSize < 1) return null;
        if (body.LeafIndex >= body.TreeSize) return null;
        var sibs = new byte[siblingCount][];
        for (int k = 0; k < siblingCount; k++)
            sibs[k] = bytes.Slice(42 + 32 * k, 32).ToArray();
        body.Siblings = sibs;
        int off = 42 + 32 * siblingCount;
        body.Root = bytes.Slice(off, 32).ToArray();
        body.Txid = bytes.Slice(off + 32, 32).ToArray();
        body.Vout = ReadBe32(bytes.Slice(off + 64, 4));
        body.BlockHash = bytes.Slice(off + 68, 32).ToArray();
        body.BlockHeight = ReadBe32(bytes.Slice(off + 100, 4));
        // canonical_sibling_count(tree_size) must equal the body's count
        if (CanonicalSiblingCount(body.TreeSize) != siblingCount) return null;
        return body;
    }

    /// <summary>
    /// Reads only block_hash and block_height out of would-be path bytes, for
    /// the W_17 claimed_block rule: "decode it only far enough to read
    /// block_hash and block_height … using the canonical path layout".
    /// </summary>
    public static bool TryReadClaimedBlock(ReadOnlySpan<byte> bytes, out byte[]? blockHash, out uint blockHeight)
    {
        blockHash = null;
        blockHeight = 0;
        if (bytes.Length < MinLength) return false;
        int siblingCount = bytes[41];
        if (siblingCount > MaxSiblings) return false;
        if (bytes.Length < MinLength + 32 * siblingCount) return false;
        int off = 42 + 32 * siblingCount;
        blockHash = bytes.Slice(off + 68, 32).ToArray();
        blockHeight = ReadBe32(bytes.Slice(off + 100, 4));
        return true;
    }

    /// <summary>canonical_sibling_count: one level while width > 1, width → floor(width/2) + width mod 2.</summary>
    public static int CanonicalSiblingCount(uint treeSize)
    {
        int count = 0;
        uint width = treeSize;
        while (width > 1)
        {
            count++;
            width = width / 2 + width % 2;
        }
        return count;
    }

    /// <summary>
    /// validate_for(waypoint): leaf equals the waypoint and the Merkle path
    /// resolves to root (leaf = SHA-256(0x1A ‖ leaf), parent =
    /// SHA-256(0x1B ‖ left ‖ right), odd-width last-node duplication).
    /// </summary>
    public bool ValidateFor(ReadOnlySpan<byte> waypoint32)
    {
        if (!Leaf.AsSpan().SequenceEqual(waypoint32)) return false;
        Span<byte> node = stackalloc byte[32];
        Span<byte> msg = stackalloc byte[65];
        msg[0] = 0x1A;
        Leaf.AsSpan().CopyTo(msg.Slice(1));
        SHA256.TryHashData(msg.Slice(0, 33), node, out _);
        uint index = LeafIndex;
        uint width = TreeSize;
        msg[0] = 0x1B;
        foreach (var sibling in Siblings)
        {
            if ((width & 1) == 1 && index == width - 1)
            {
                // the sibling must equal the current node; parent of the node with itself
                if (!sibling.AsSpan().SequenceEqual(node)) return false;
                node.CopyTo(msg.Slice(1));
                node.CopyTo(msg.Slice(33));
            }
            else if ((index & 1) == 0)
            {
                node.CopyTo(msg.Slice(1));
                sibling.AsSpan().CopyTo(msg.Slice(33));
            }
            else
            {
                sibling.AsSpan().CopyTo(msg.Slice(1));
                node.CopyTo(msg.Slice(33));
            }
            SHA256.TryHashData(msg, node, out _);
            index /= 2;
            width = width / 2 + width % 2;
        }
        return Root.AsSpan().SequenceEqual(node);
    }

    private static uint ReadBe32(ReadOnlySpan<byte> b) =>
        System.Buffers.Binary.BinaryPrimitives.ReadUInt32BigEndian(b);
}
