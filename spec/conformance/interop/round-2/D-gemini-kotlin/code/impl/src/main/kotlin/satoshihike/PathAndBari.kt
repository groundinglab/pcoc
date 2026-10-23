package satoshihike

import java.math.BigInteger

/**
 * Canonical path body (SPEC section 14.3).
 */
data class CanonicalPathBody(
    val version: Int,
    val leaf: ByteArray,
    val leafIndex: Long,
    val treeSize: Long,
    val siblingCount: Int,
    val siblings: List<ByteArray>,
    val root: ByteArray,
    val txid: ByteArray,
    val vout: Long,
    val blockHash: ByteArray, // 32 bytes in natural order
    val blockHeight: Long
) {
    /**
     * Validates that [leaf] equals [waypoint] and the Merkle path resolves to [root] (SPEC section 14.3).
     */
    fun validateFor(waypoint: ByteArray): Boolean {
        if (!leaf.contentEquals(waypoint)) return false
        var current = Encoding.sha256( byteArrayOf(0x1A), leaf)
        var index = leafIndex
        var width = treeSize
        for (sibling in siblings) {
            current = if (width % 2L == 1L && index == width - 1L) {
                if (!sibling.contentEquals(current)) return false
                Encoding.sha256(byteArrayOf(0x1B), current, current)
            } else if (index % 2L == 0L) {
                Encoding.sha256(byteArrayOf(0x1B), current, sibling)
            } else {
                Encoding.sha256(byteArrayOf(0x1B), sibling, current)
            }
            index /= 2L
            width = (width / 2L) + (width % 2L)
        }
        return current.contentEquals(root)
    }

    companion object {
        fun canonicalSiblingCount(treeSize: Long): Int {
            var width = treeSize
            var count = 0
            while (width > 1L) {
                count++
                width = (width / 2L) + (width % 2L)
            }
            return count
        }

        /**
         * Decodes a canonical path body according to SPEC section 14.3.
         * Returns null if any structural rule fails.
         */
        fun decode(bytes: ByteArray): CanonicalPathBody? {
            if (bytes.size < 146) return null
            val version = bytes[0].toInt() and 0xFF
            if (version != 0x01) return null

            val leaf = bytes.copyOfRange(1, 33)
            val leafIndex = Encoding.u32Be(bytes, 33)
            val treeSize = Encoding.u32Be(bytes, 37)
            val siblingCount = bytes[41].toInt() and 0xFF

            if (siblingCount > 32) return null
            if (bytes.size != 146 + 32 * siblingCount) return null
            if (treeSize < 1L) return null
            if (leafIndex !in 0L until treeSize) return null
            if (siblingCount != canonicalSiblingCount(treeSize)) return null

            val siblings = ArrayList<ByteArray>(siblingCount)
            var pos = 42
            for (i in 0 until siblingCount) {
                siblings.add(bytes.copyOfRange(pos, pos + 32))
                pos += 32
            }

            val root = bytes.copyOfRange(pos, pos + 32)
            pos += 32
            val txid = bytes.copyOfRange(pos, pos + 32)
            pos += 32
            val vout = Encoding.u32Be(bytes, pos)
            pos += 4
            val blockHash = bytes.copyOfRange(pos, pos + 32)
            pos += 4 * 8 // 32
            val blockHeight = Encoding.u32Be(bytes, pos)

            return CanonicalPathBody(
                version = version,
                leaf = leaf,
                leafIndex = leafIndex,
                treeSize = treeSize,
                siblingCount = siblingCount,
                siblings = siblings,
                root = root,
                txid = txid,
                vout = vout,
                blockHash = blockHash,
                blockHeight = blockHeight
            )
        }

        /**
         * Reads claimed_block (block_hash in display order and block_height) from non-empty W_17 path bytes
         * using the canonical path layout without running validateFor (SPEC section 14.3).
         */
        fun extractClaimedBlockLayout(bytes: ByteArray): Pair<String, Long>? {
            if (bytes.size < 146) return null
            val siblingCount = bytes[41].toInt() and 0xFF
            if (siblingCount > 32) return null
            if (bytes.size != 146 + 32 * siblingCount) return null
            val blockHashPos = 42 + 32 * siblingCount + 32 + 32 + 4
            val blockHashNatural = bytes.copyOfRange(blockHashPos, blockHashPos + 32)
            val blockHeight = Encoding.u32Be(bytes, blockHashPos + 32)
            val displayHex = Encoding.toHex(Encoding.reverseBytes32(blockHashNatural))
            return displayHex to blockHeight
        }
    }
}

/**
 * Bari paid-waypoint rule BR1 (SPEC section 13.6).
 */
object BariPaidRule {
    const val SYNTH_NBITS: UInt = 0x207fffffu

    /**
     * Checks a waypoint POST against Rule BR1 (SPEC section 13.6).
     * Returns HTTP status code 204 (accepted) or 402 (denied).
     */
    fun checkWaypointPost(
        body: ByteArray,
        nonceHeader: String,
        feeHeaderHex: String,
        bariOrigin: String,
        bariTipHashNatural: ByteArray,
        bariTipPrevHashNatural: ByteArray,
        requiredAfterNBits: UInt = SYNTH_NBITS
    ): Int {
        // 1. Body must be 32 bytes, nonce shortest decimal u64, feeHeaderHex 160 hex digits
        if (body.size != 32) return 402
        val nonce = parseShortestDecimalU64(nonceHeader) ?: return 402
        if (feeHeaderHex.length != 160) return 402
        for (c in feeHeaderHex) {
            if (Encoding.hexDigit(c) < 0) return 402
        }
        val feeHeader = Encoding.fromHex(feeHeaderHex)
        if (feeHeader.size != 80) return 402

        // 2. Fee header must be current, before, or after Bari's tip (all comparisons in natural order)
        val headerHash = Encoding.sha256(Encoding.sha256(feeHeader))
        val isCurrent = headerHash.contentEquals(bariTipHashNatural)
        val isBefore = headerHash.contentEquals(bariTipPrevHashNatural)
        val isAfter = if (!isCurrent && !isBefore) {
            val prevBlockField = feeHeader.copyOfRange(4, 36)
            val feeNBits = readU32Le(feeHeader, 72)
            prevBlockField.contentEquals(bariTipHashNatural) &&
                feeNBits == requiredAfterNBits &&
                meetsCompactTarget(headerHash, requiredAfterNBits)
        } else {
            false
        }

        if (!isCurrent && !isBefore && !isAfter) return 402

        // 3. Hashcash check (difficulty 21 leading zero bits)
        val challenge = Construction.hashcashChallenge(bariOrigin, headerHash)
        val digest = Construction.hashcashDigest(challenge, body, nonce.toLong())
        if (Construction.leadingZeroBits(digest) < SpecConstants.HASHCASH_DIFFICULTY_BITS) {
            return 402
        }

        return 204
    }

    private fun parseShortestDecimalU64(s: String): ULong? {
        if (s.isEmpty()) return null
        if (s.length > 1 && s[0] == '0') return null
        for (c in s) {
            if (c !in '0'..'9') return null
        }
        return s.toULongOrNull()
    }

    private fun readU32Le(b: ByteArray, offset: Int): UInt {
        return ((b[offset].toInt() and 0xFF) or
            ((b[offset + 1].toInt() and 0xFF) shl 8) or
            ((b[offset + 2].toInt() and 0xFF) shl 16) or
            ((b[offset + 3].toInt() and 0xFF) shl 24)).toUInt()
    }

    /**
     * Decodes compact target nBits and compares natural-order SHA-256(SHA-256(header))
     * as a 256-bit little-endian integer against target (SPEC section 9 & 13.6).
     */
    fun meetsCompactTarget(headerHashNatural: ByteArray, nBits: UInt): Boolean {
        val exponent = ((nBits shr 24) and 0xFFu).toInt()
        val mantissa = (nBits and 0x007FFFFFu).toLong()
        if ((nBits and 0x00800000u) != 0u || mantissa == 0L) return false
        val target = if (exponent <= 3) {
            BigInteger.valueOf(mantissa ushr (8 * (3 - exponent)))
        } else {
            BigInteger.valueOf(mantissa).shiftLeft(8 * (exponent - 3))
        }
        val hashLeBigInt = BigInteger(1, headerHashNatural.reversedArray())
        return hashLeBigInt <= target
    }
}
