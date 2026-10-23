package satoshihike

import java.lang.invoke.MethodHandles
import java.lang.invoke.VarHandle
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest

/**
 * Satoshi Hike 2.2 section 6 constants and section 16.1 profile definition.
 */
object SpecConstants {
    const val SLOT_BYTES: Int = 16384
    const val WORDS_PER_SLOT: Int = 2048
    const val TERRAIN_BYTES: Int = 268435456
    const val N_SLOTS: Int = 16384
    const val SAMPLE_BYTES: Int = 1024
    const val OFFSET_MODULUS: Long = 268434433L
    const val STEPS_PER_LEG: Int = 2_000_000
    const val STAGES: Int = 18
    const val HASH_BYTES: Int = 32
    const val LEG_RESULT_BYTES: Int = 8
    const val PUBLIC_KEY_BYTES: Int = 32
    const val BEACON_BYTES: Int = 32

    const val SEAL_DOMAIN_STR: String = "pcoc-walk-v1"
    val SEAL_DOMAIN: ByteArray = SEAL_DOMAIN_STR.toByteArray(Charsets.US_ASCII)

    const val KIND: String = "v2.2-pcoc-object"
    const val FILL: String = "dag2"

    const val MIX_ADD: Long = -0x61c8864680b583ebL      // 0x9E3779B97F4A7C15
    const val MIX_MUL1: Long = -0x40a7b892e31b1a47L     // 0xBF58476D1CE4E5B9
    const val MIX_MUL2: Long = -0x6b2fb644ecceee15L     // 0x94D049BB133111EB
    const val WORD_XOR: Long = 0xC0FFEEL                // 0x0000000000C0FFEE
    const val PARENT_MUL: Long = -0x2e4ab5cd2e6d12fdL   // 0xD1B54A32D192ED03
    const val SLOT_MUL: Long = -0x5db4512b69c11bf9L     // 0xA24BAED4963EE407

    const val PARENT_MUL_1: Long = 1L * PARENT_MUL
    const val PARENT_MUL_2: Long = 2L * PARENT_MUL
    const val PARENT_MUL_3: Long = 3L * PARENT_MUL

    val ANCHOR_WINDOW_OFFSETS: IntArray = intArrayOf(2, 3, 4)
    const val HASHCASH_DIFFICULTY_BITS: Int = 21
    val HASHCASH_PREFIX: ByteArray = "bari-hc-v2".toByteArray(Charsets.US_ASCII)
    val SIGNIN_DOMAIN: ByteArray = "pcoc-gate-signin-v1".toByteArray(Charsets.US_ASCII)
}

/**
 * Profile in force (SPEC sections 6, 14, and 16.1).
 */
data class Profile(
    val name: String,
    val slotBytes: Int,
    val sampleBytes: Int,
    val terrainBytes: Int,
    val stepsPerLeg: Int
) {
    init {
        require(slotBytes >= 8 && slotBytes % 8 == 0 && slotBytes <= SpecConstants.SLOT_BYTES) {
            "Invalid slotBytes: $slotBytes"
        }
        require(sampleBytes in 1..SpecConstants.SAMPLE_BYTES) {
            "Invalid sampleBytes: $sampleBytes"
        }
        require(
            terrainBytes <= SpecConstants.TERRAIN_BYTES &&
                terrainBytes >= sampleBytes &&
                terrainBytes % slotBytes == 0
        ) {
            "Invalid terrainBytes: $terrainBytes"
        }
        require(stepsPerLeg in 1..SpecConstants.STEPS_PER_LEG) {
            "Invalid stepsPerLeg: $stepsPerLeg"
        }
    }

    val wordsPerSlot: Int = slotBytes / 8
    val nSlots: Int = terrainBytes / slotBytes
    val offsetModulus: Long = (terrainBytes - sampleBytes + 1).toLong()
    val totalWords: Int = terrainBytes / 8

    companion object {
        val PRODUCTION = Profile(
            name = "production",
            slotBytes = SpecConstants.SLOT_BYTES,
            sampleBytes = SpecConstants.SAMPLE_BYTES,
            terrainBytes = SpecConstants.TERRAIN_BYTES,
            stepsPerLeg = SpecConstants.STEPS_PER_LEG
        )

        val S1 = Profile(
            name = "conformance-small",
            slotBytes = 64,
            sampleBytes = 32,
            terrainBytes = 65536,
            stepsPerLeg = 100
        )

        val S2 = Profile(
            name = "conformance-small",
            slotBytes = 256,
            sampleBytes = 64,
            terrainBytes = 1048576,
            stepsPerLeg = 1000
        )
    }
}

/**
 * Encoding & hashing helpers (SPEC sections 3 and 7).
 */
object Encoding {
    @JvmField
    val LONG_LE: VarHandle = MethodHandles.byteArrayViewVarHandle(LongArray::class.java, ByteOrder.LITTLE_ENDIAN)

    fun le64(x: Long): ByteArray {
        val out = ByteArray(8)
        LONG_LE.set(out, 0, x)
        return out
    }

    @Suppress("NOTHING_TO_INLINE")
    inline fun writeLe64(buf: ByteArray, offset: Int, x: Long) {
        LONG_LE.set(buf, offset, x)
    }

    @Suppress("NOTHING_TO_INLINE")
    inline fun u64Le(b: ByteArray, offset: Int = 0): Long {
        return LONG_LE.get(b, offset) as Long
    }

    fun u32Be(b: ByteArray, offset: Int): Long {
        return ((b[offset].toLong() and 0xFFL) shl 24) or
            ((b[offset + 1].toLong() and 0xFFL) shl 16) or
            ((b[offset + 2].toLong() and 0xFFL) shl 8) or
            (b[offset + 3].toLong() and 0xFFL)
    }

    private val HEX_CHARS = "0123456789abcdef".toCharArray()

    fun toHex(bytes: ByteArray): String {
        val chars = CharArray(bytes.size * 2)
        for (i in bytes.indices) {
            val v = bytes[i].toInt() and 0xFF
            chars[i * 2] = HEX_CHARS[v ushr 4]
            chars[i * 2 + 1] = HEX_CHARS[v and 0x0F]
        }
        return String(chars)
    }

    fun u64ToHexBe(value: Long): String {
        val chars = CharArray(16)
        for (i in 0 until 16) {
            val shift = (15 - i) * 4
            chars[i] = HEX_CHARS[((value ushr shift) and 0xFL).toInt()]
        }
        return String(chars)
    }

    fun fromHex(hex: String): ByteArray {
        val clean = if (hex.startsWith("0x") || hex.startsWith("0X")) hex.substring(2) else hex
        require(clean.length % 2 == 0) { "Hex string must have even length" }
        val out = ByteArray(clean.length / 2)
        for (i in out.indices) {
            val hi = hexDigit(clean[i * 2])
            val lo = hexDigit(clean[i * 2 + 1])
            require(hi >= 0 && lo >= 0) { "Invalid hex character in $hex" }
            out[i] = ((hi shl 4) or lo).toByte()
        }
        return out
    }

    fun isValidHex64(s: String): Boolean {
        if (s.length != 64) return false
        for (i in 0 until 64) {
            if (hexDigit(s[i]) < 0) return false
        }
        return true
    }

    fun hexDigit(c: Char): Int = when (c) {
        in '0'..'9' -> c - '0'
        in 'a'..'f' -> c - 'a' + 10
        in 'A'..'F' -> c - 'A' + 10
        else -> -1
    }

    /**
     * SPEC section 9 single reversal rule between natural order and Bitcoin display order.
     */
    fun reverseBytes32(b: ByteArray): ByteArray {
        require(b.size == 32) { "Expected 32 bytes, got ${b.size}" }
        val out = ByteArray(32)
        for (i in 0 until 32) {
            out[i] = b[31 - i]
        }
        return out
    }

    fun sha256(vararg parts: ByteArray): ByteArray {
        val md = MessageDigest.getInstance("SHA-256")
        for (p in parts) {
            md.update(p)
        }
        return md.digest()
    }
}
