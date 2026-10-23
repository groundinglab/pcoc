package satoshihike

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import java.util.stream.IntStream

/**
 * Terrain array wrapper (SPEC section 10).
 * Holds all N_SLOTS * WORDS_PER_SLOT 64-bit words in little-endian word order.
 */
class Terrain(
    val profile: Profile,
    val words: LongArray
) {
    init {
        require(words.size == profile.totalWords) {
            "Terrain words size ${words.size} != ${profile.totalWords}"
        }
    }

    fun word(slotIndex: Int, wordIndex: Int): Long {
        return words[slotIndex * profile.wordsPerSlot + wordIndex]
    }

    /**
     * Reads [length] bytes starting at unaligned byte offset [byteOffset] (SPEC section 11.3).
     */
    fun readBytes(byteOffset: Int, length: Int): ByteArray {
        val out = ByteArray(length)
        var pos = byteOffset
        var outIdx = 0
        while (outIdx < length) {
            val wordIdx = pos ushr 3
            val byteInWord = pos and 7
            val w = words[wordIdx]
            if (byteInWord == 0 && outIdx + 8 <= length) {
                Encoding.writeLe64(out, outIdx, w)
                pos += 8
                outIdx += 8
            } else {
                out[outIdx] = ((w ushr (byteInWord shl 3)) and 0xFFL).toByte()
                pos++
                outIdx++
            }
        }
        return out
    }

    /**
     * Terrain sample at previous checkpoint (SPEC section 11.3).
     */
    fun terrainSample(previousCheckpoint: ByteArray): ByteArray {
        val u = Encoding.u64Le(previousCheckpoint, 0)
        val offset = java.lang.Long.remainderUnsigned(u, profile.offsetModulus).toInt()
        return readBytes(offset, profile.sampleBytes)
    }

    /**
     * Slot sample for a given slot index (SPEC section 12.2).
     */
    fun slotSample(slotIndex: Int): ByteArray {
        val out = ByteArray(profile.slotBytes)
        val baseWord = slotIndex * profile.wordsPerSlot
        for (j in 0 until profile.wordsPerSlot) {
            Encoding.writeLe64(out, j * 8, words[baseWord + j])
        }
        return out
    }

    /**
     * Computes SHA-256 of the entire terrain byte array (SPEC section 10 / 16).
     */
    fun sha256(): ByteArray {
        val md = MessageDigest.getInstance("SHA-256")
        val chunkWords = minOf(profile.totalWords, 8192) // 64 KiB chunk
        val byteChunk = ByteArray(chunkWords * 8)
        val longBuf = ByteBuffer.wrap(byteChunk).order(ByteOrder.LITTLE_ENDIAN).asLongBuffer()
        var offset = 0
        while (offset < profile.totalWords) {
            val len = minOf(chunkWords, profile.totalWords - offset)
            longBuf.clear()
            longBuf.put(words, offset, len)
            md.update(byteChunk, 0, len * 8)
            offset += len
        }
        return md.digest()
    }
}

/**
 * Step trace for the fold vector (SPEC section 11.1, CASES.md case 4).
 */
data class StepTrace(
    val stepIndex: Int,
    val inputS: Long,
    val slotIndex: Int,
    val acc: Long,
    val outputS: Long
)

/**
 * Hashcash solve result (SPEC section 13.1).
 */
data class HashcashResult(
    val challenge: ByteArray,
    val nonce: Long,
    val digest: ByteArray,
    val leadingZeroBits: Int
)

/**
 * Satoshi Hike 2.2 core construction (SPEC sections 6-13, 14.6, 15.4, 15.5, 16.1).
 */
object Construction {

    // Section 10: Seed and lanes
    fun seedPreimage(publicKey: ByteArray, seedBeacon: ByteArray): ByteArray {
        require(publicKey.size == SpecConstants.PUBLIC_KEY_BYTES)
        require(seedBeacon.size == SpecConstants.BEACON_BYTES)
        return publicKey + seedBeacon
    }

    fun computeSeed(publicKey: ByteArray, seedBeacon: ByteArray): ByteArray {
        return Encoding.sha256(publicKey, seedBeacon)
    }

    fun extractLanes(seed: ByteArray): LongArray {
        require(seed.size == SpecConstants.HASH_BYTES)
        return LongArray(4) { k -> Encoding.u64Le(seed, k * 8) }
    }

    // Section 10.1: mix64
    @Suppress("NOTHING_TO_INLINE")
    inline fun mix64(x: Long): Long {
        var y = x + SpecConstants.MIX_ADD
        y = (y xor (y ushr 30)) * SpecConstants.MIX_MUL1
        y = (y xor (y ushr 27)) * SpecConstants.MIX_MUL2
        return y xor (y ushr 31)
    }

    // Section 10.3: Word rotation and parent indices
    fun floorLog2(n: Int): Int {
        require(n >= 1)
        return 31 - Integer.numberOfLeadingZeros(n)
    }

    fun reverse32(v: Int): Long {
        return Integer.reverse(v).toLong() and 0xFFFFFFFFL
    }

    fun bitrevParent(i: Int): Int {
        require(i >= 1)
        val bits = maxOf(floorLog2(i), 1)
        val shifted = reverse32(i) ushr (32 - bits)
        return (shifted % i.toLong()).toInt()
    }

    fun mulParent(i: Int): Int {
        require(i >= 1)
        val h = i.toLong() * SpecConstants.MIX_ADD
        val high32 = h ushr 32
        return (high32 % i.toLong()).toInt()
    }

    // Section 10.4: NL4 base word
    fun nl4BaseWord(lanes: LongArray, i: Int, j: Int): Long {
        val c = (i.toLong() shl 32) xor j.toLong() xor SpecConstants.WORD_XOR
        var t = mix64(lanes[j and 3] xor c)
        t = mix64(t xor java.lang.Long.rotateLeft(lanes[(j + 1) and 3], 17))
        t = mix64(t xor java.lang.Long.rotateLeft(lanes[(j + 2) and 3], 31))
        t = mix64(t xor java.lang.Long.rotateLeft(lanes[(j + 3) and 3], 47))
        return t
    }

    /**
     * Plain scalar terrain fill (SPEC section 10.4).
     */
    fun fillTerrainScalar(profile: Profile, seed: ByteArray): Terrain {
        val lanes = extractLanes(seed)
        val nSlots = profile.nSlots
        val wordsPerSlot = profile.wordsPerSlot
        val words = LongArray(profile.totalWords)

        val r1 = LongArray(4) { k -> java.lang.Long.rotateLeft(lanes[(k + 1) and 3], 17) }
        val r2 = LongArray(4) { k -> java.lang.Long.rotateLeft(lanes[(k + 2) and 3], 31) }
        val r3 = LongArray(4) { k -> java.lang.Long.rotateLeft(lanes[(k + 3) and 3], 47) }

        // Slot 0
        for (j in 0 until wordsPerSlot) {
            val laneIdx = j and 3
            val c = j.toLong() xor SpecConstants.WORD_XOR
            var t = mix64(lanes[laneIdx] xor c)
            t = mix64(t xor r1[laneIdx])
            t = mix64(t xor r2[laneIdx])
            t = mix64(t xor r3[laneIdx])
            words[j] = t
        }

        // Slots 1 .. nSlots - 1
        for (i in 1 until nSlots) {
            val p1Base = (i - 1) * wordsPerSlot
            val p2Base = bitrevParent(i) * wordsPerSlot
            val p3Base = mulParent(i) * wordsPerSlot
            val slotBase = i * wordsPerSlot
            val iShift = (i.toLong() shl 32) xor SpecConstants.WORD_XOR
            val slotMix = i.toLong() * SpecConstants.SLOT_MUL

            for (j in 0 until wordsPerSlot) {
                val laneIdx = j and 3
                val c = iShift xor j.toLong()
                var w = mix64(lanes[laneIdx] xor c)
                w = mix64(w xor r1[laneIdx])
                w = mix64(w xor r2[laneIdx])
                w = mix64(w xor r3[laneIdx])

                val pw1 = words[p1Base + j]
                w = mix64((w xor java.lang.Long.rotateLeft(pw1, (j + 1) and 63)) xor SpecConstants.PARENT_MUL_1)

                val pw2 = words[p2Base + j]
                w = mix64((w xor java.lang.Long.rotateLeft(pw2, (j + 2) and 63)) xor SpecConstants.PARENT_MUL_2)

                val pw3 = words[p3Base + j]
                w = mix64((w xor java.lang.Long.rotateLeft(pw3, (j + 3) and 63)) xor SpecConstants.PARENT_MUL_3)

                w = mix64(w xor slotMix)
                words[slotBase + j] = w
            }
        }

        return Terrain(profile, words)
    }

    /**
     * Multi-threaded column-partitioned terrain fill using 4-way unrolled primitive loops
     * (enabling HotSpot C2 SuperWord auto-vectorisation + zero-synchronization 4-core parallelism).
     */
    fun fillTerrain(profile: Profile, seed: ByteArray, parallel: Boolean = true): Terrain {
        val wordsPerSlot = profile.wordsPerSlot
        if (!parallel || wordsPerSlot < 32 || wordsPerSlot % 32 != 0) {
            return fillTerrainScalar(profile, seed)
        }

        val lanes = extractLanes(seed)
        val nSlots = profile.nSlots
        val words = LongArray(profile.totalWords)

        val p2Offsets = IntArray(nSlots)
        val p3Offsets = IntArray(nSlots)
        for (i in 1 until nSlots) {
            p2Offsets[i] = bitrevParent(i) * wordsPerSlot
            p3Offsets[i] = mulParent(i) * wordsPerSlot
        }

        val r1 = LongArray(4) { k -> java.lang.Long.rotateLeft(lanes[(k + 1) and 3], 17) }
        val r2 = LongArray(4) { k -> java.lang.Long.rotateLeft(lanes[(k + 2) and 3], 31) }
        val r3 = LongArray(4) { k -> java.lang.Long.rotateLeft(lanes[(k + 3) and 3], 47) }
        val jXorTable = LongArray(wordsPerSlot) { j -> lanes[j and 3] xor SpecConstants.WORD_XOR xor j.toLong() }

        val cores = 4
        val wordsPerCore = wordsPerSlot / cores
        IntStream.range(0, cores).parallel().forEach { coreIdx ->
            val jStart = coreIdx * wordsPerCore
            val jEnd = jStart + wordsPerCore
            fillColumnRangeScalarUnrolled(
                words, p2Offsets, p3Offsets, jXorTable, r1, r2, r3,
                nSlots, wordsPerSlot, jStart, jEnd
            )
        }
        return Terrain(profile, words)
    }

    private fun fillColumnRangeScalarUnrolled(
        words: LongArray,
        p2Offsets: IntArray,
        p3Offsets: IntArray,
        jXorTable: LongArray,
        r1: LongArray,
        r2: LongArray,
        r3: LongArray,
        nSlots: Int,
        wordsPerSlot: Int,
        jStart: Int,
        jEnd: Int
    ) {
        val r1_0 = r1[0]; val r1_1 = r1[1]; val r1_2 = r1[2]; val r1_3 = r1[3]
        val r2_0 = r2[0]; val r2_1 = r2[1]; val r2_2 = r2[2]; val r2_3 = r2[3]
        val r3_0 = r3[0]; val r3_1 = r3[1]; val r3_2 = r3[2]; val r3_3 = r3[3]

        // Slot 0
        var j = jStart
        while (j < jEnd) {
            var w0 = mix64(jXorTable[j])
            w0 = mix64(w0 xor r1_0); w0 = mix64(w0 xor r2_0); w0 = mix64(w0 xor r3_0)
            words[j] = w0

            var w1 = mix64(jXorTable[j + 1])
            w1 = mix64(w1 xor r1_1); w1 = mix64(w1 xor r2_1); w1 = mix64(w1 xor r3_1)
            words[j + 1] = w1

            var w2 = mix64(jXorTable[j + 2])
            w2 = mix64(w2 xor r1_2); w2 = mix64(w2 xor r2_2); w2 = mix64(w2 xor r3_2)
            words[j + 2] = w2

            var w3 = mix64(jXorTable[j + 3])
            w3 = mix64(w3 xor r1_3); w3 = mix64(w3 xor r2_3); w3 = mix64(w3 xor r3_3)
            words[j + 3] = w3

            j += 4
        }

        // Slots 1 .. nSlots - 1
        for (i in 1 until nSlots) {
            val p1Base = (i - 1) * wordsPerSlot
            val p2Base = p2Offsets[i]
            val p3Base = p3Offsets[i]
            val slotBase = i * wordsPerSlot
            val iShift = i.toLong() shl 32
            val slotMix = i.toLong() * SpecConstants.SLOT_MUL

            var col = jStart
            while (col < jEnd) {
                var w0 = mix64(jXorTable[col] xor iShift)
                w0 = mix64(w0 xor r1_0); w0 = mix64(w0 xor r2_0); w0 = mix64(w0 xor r3_0)
                w0 = mix64((w0 xor java.lang.Long.rotateLeft(words[p1Base + col], col + 1)) xor SpecConstants.PARENT_MUL_1)
                w0 = mix64((w0 xor java.lang.Long.rotateLeft(words[p2Base + col], col + 2)) xor SpecConstants.PARENT_MUL_2)
                w0 = mix64((w0 xor java.lang.Long.rotateLeft(words[p3Base + col], col + 3)) xor SpecConstants.PARENT_MUL_3)
                words[slotBase + col] = mix64(w0 xor slotMix)

                val c1 = col + 1
                var w1 = mix64(jXorTable[c1] xor iShift)
                w1 = mix64(w1 xor r1_1); w1 = mix64(w1 xor r2_1); w1 = mix64(w1 xor r3_1)
                w1 = mix64((w1 xor java.lang.Long.rotateLeft(words[p1Base + c1], c1 + 1)) xor SpecConstants.PARENT_MUL_1)
                w1 = mix64((w1 xor java.lang.Long.rotateLeft(words[p2Base + c1], c1 + 2)) xor SpecConstants.PARENT_MUL_2)
                w1 = mix64((w1 xor java.lang.Long.rotateLeft(words[p3Base + c1], c1 + 3)) xor SpecConstants.PARENT_MUL_3)
                words[slotBase + c1] = mix64(w1 xor slotMix)

                val c2 = col + 2
                var w2 = mix64(jXorTable[c2] xor iShift)
                w2 = mix64(w2 xor r1_2); w2 = mix64(w2 xor r2_2); w2 = mix64(w2 xor r3_2)
                w2 = mix64((w2 xor java.lang.Long.rotateLeft(words[p1Base + c2], c2 + 1)) xor SpecConstants.PARENT_MUL_1)
                w2 = mix64((w2 xor java.lang.Long.rotateLeft(words[p2Base + c2], c2 + 2)) xor SpecConstants.PARENT_MUL_2)
                w2 = mix64((w2 xor java.lang.Long.rotateLeft(words[p3Base + c2], c2 + 3)) xor SpecConstants.PARENT_MUL_3)
                words[slotBase + c2] = mix64(w2 xor slotMix)

                val c3 = col + 3
                var w3 = mix64(jXorTable[c3] xor iShift)
                w3 = mix64(w3 xor r1_3); w3 = mix64(w3 xor r2_3); w3 = mix64(w3 xor r3_3)
                w3 = mix64((w3 xor java.lang.Long.rotateLeft(words[p1Base + c3], c3 + 1)) xor SpecConstants.PARENT_MUL_1)
                w3 = mix64((w3 xor java.lang.Long.rotateLeft(words[p2Base + c3], c3 + 2)) xor SpecConstants.PARENT_MUL_2)
                w3 = mix64((w3 xor java.lang.Long.rotateLeft(words[p3Base + c3], c3 + 3)) xor SpecConstants.PARENT_MUL_3)
                words[slotBase + c3] = mix64(w3 xor slotMix)

                col += 4
            }
        }
    }

    /**
     * Plain scalar leg computation baseline (SPEC section 11.1 & 11.2, unvectorised scalar loop).
     */
    fun legResultScalar(terrain: Terrain, previousCheckpoint: ByteArray): Long {
        val profile = terrain.profile
        val words = terrain.words
        val wordsPerSlot = profile.wordsPerSlot
        val wordsPerSlotLong = wordsPerSlot.toLong()
        val nSlots = profile.nSlots
        val steps = profile.stepsPerLeg

        val md = MessageDigest.getInstance("SHA-256")
        val sealBuf = ByteArray(20)
        System.arraycopy(SpecConstants.SEAL_DOMAIN, 0, sealBuf, 0, 12)
        val digestBuf = ByteArray(32)

        var s = Encoding.u64Le(previousCheckpoint, 0)
        for (step in 0 until steps) {
            val slotIdx = java.lang.Long.remainderUnsigned(s, nSlots.toLong()).toInt()
            val baseOffset = slotIdx * wordsPerSlot
            var acc = 0L
            var w = 0L
            while (w < wordsPerSlotLong) {
                acc += mix64(words[baseOffset + w.toInt()] xor s)
                w++
            }
            Encoding.writeLe64(sealBuf, 12, acc)
            md.update(sealBuf, 0, 20)
            md.digest(digestBuf, 0, 32)
            s = Encoding.u64Le(digestBuf, 0)
        }
        return s
    }

    /**
     * Optimised leg computation (SPEC section 11.1 & 11.2).
     * Combines compile-time constant folding (`const val` mix64 constants unlocking HotSpot C2 SuperWord
     * AVX-512 `vpmullq`/`vpxorq`/`vpsrlq`/`vpaddq` vectorisation), power-of-two slot masking,
     * VarHandle single-instruction LE64 memory access, and hardware SHA-NI intrinsic digest sealing.
     */
    fun legResult(terrain: Terrain, previousCheckpoint: ByteArray): Long {
        val profile = terrain.profile
        val words = terrain.words
        val wordsPerSlot = profile.wordsPerSlot
        val nSlots = profile.nSlots
        val isPow2Slots = (nSlots and (nSlots - 1)) == 0
        val slotMask = nSlots - 1
        val steps = profile.stepsPerLeg

        val md = MessageDigest.getInstance("SHA-256")
        val sealBuf = ByteArray(20)
        System.arraycopy(SpecConstants.SEAL_DOMAIN, 0, sealBuf, 0, 12)
        val digestBuf = ByteArray(32)

        var s = Encoding.u64Le(previousCheckpoint, 0)
        for (step in 0 until steps) {
            val slotIdx = if (isPow2Slots) {
                s.toInt() and slotMask
            } else {
                java.lang.Long.remainderUnsigned(s, nSlots.toLong()).toInt()
            }
            val baseOffset = slotIdx * wordsPerSlot
            var acc = 0L
            for (w in 0 until wordsPerSlot) {
                acc += mix64(words[baseOffset + w] xor s)
            }
            Encoding.writeLe64(sealBuf, 12, acc)
            md.update(sealBuf, 0, 20)
            md.digest(digestBuf, 0, 32)
            s = Encoding.u64Le(digestBuf, 0)
        }
        return s
    }

    /**
     * Traces the first [traceSteps] steps and returns both the traces and the final leg_result (CASES.md case 4).
     */
    fun legWithTrace(terrain: Terrain, previousCheckpoint: ByteArray, traceSteps: Int): Pair<List<StepTrace>, Long> {
        val profile = terrain.profile
        val words = terrain.words
        val wordsPerSlot = profile.wordsPerSlot
        val nSlots = profile.nSlots
        val steps = profile.stepsPerLeg

        val md = MessageDigest.getInstance("SHA-256")
        val sealBuf = ByteArray(20)
        System.arraycopy(SpecConstants.SEAL_DOMAIN, 0, sealBuf, 0, 12)
        val digestBuf = ByteArray(32)

        val traces = ArrayList<StepTrace>(traceSteps)
        var s = Encoding.u64Le(previousCheckpoint, 0)
        for (step in 0 until steps) {
            val inputS = s
            val slotIdx = java.lang.Long.remainderUnsigned(s, nSlots.toLong()).toInt()
            val baseOffset = slotIdx * wordsPerSlot
            var acc = 0L
            for (w in 0 until wordsPerSlot) {
                acc += mix64(words[baseOffset + w] xor s)
            }
            Encoding.writeLe64(sealBuf, 12, acc)
            md.update(sealBuf, 0, 20)
            md.digest(digestBuf, 0, 32)
            val nextS = Encoding.u64Le(digestBuf, 0)
            if (step < traceSteps) {
                traces.add(StepTrace(step, inputS, slotIdx, acc, nextS))
            }
            s = nextS
        }
        return traces to s
    }

    // Section 11.4: Checkpoint
    fun computeCheckpoint(
        previousCheckpoint: ByteArray,
        stageLimb: ByteArray,
        terrainSample: ByteArray,
        legResultLe64: ByteArray
    ): ByteArray {
        require(previousCheckpoint.size == 32)
        require(stageLimb.size == 32)
        require(legResultLe64.size == 8)
        return Encoding.sha256(previousCheckpoint, stageLimb, terrainSample, legResultLe64)
    }

    // Section 12.2: Checkpoint fold and Waypoint
    fun foldCheckpoints(checkpoints: List<ByteArray>): ByteArray {
        var f = ByteArray(32)
        if (checkpoints.isEmpty()) return f
        val md = MessageDigest.getInstance("SHA-256")
        for (c in checkpoints) {
            md.update(f)
            md.update(c)
            f = md.digest()
        }
        return f
    }

    fun computeWaypoint(
        interrupt: ByteArray,
        terrain: Terrain,
        stageWaypoint: ByteArray,
        checkpoints: List<ByteArray>
    ): ByteArray {
        require(interrupt.size == 32)
        require(stageWaypoint.size == 32)
        val slotIndex = java.lang.Long.remainderUnsigned(
            Encoding.u64Le(interrupt, 0),
            terrain.profile.nSlots.toLong()
        ).toInt()
        val slotSample = terrain.slotSample(slotIndex)
        val fold = foldCheckpoints(checkpoints)
        return Encoding.sha256(interrupt, slotSample, stageWaypoint, fold)
    }

    // Section 13.1: Hashcash
    fun hashcashChallenge(bariOrigin: String, headerHash: ByteArray): ByteArray {
        require(headerHash.size == 32)
        return Encoding.sha256(
            SpecConstants.HASHCASH_PREFIX,
            bariOrigin.toByteArray(Charsets.US_ASCII),
            headerHash
        )
    }

    fun hashcashDigest(challenge: ByteArray, waypoint: ByteArray, nonce: Long): ByteArray {
        return Encoding.sha256(challenge, waypoint, Encoding.le64(nonce))
    }

    fun leadingZeroBits(digest: ByteArray): Int {
        var bits = 0
        for (b in digest) {
            val v = b.toInt() and 0xFF
            if (v == 0) {
                bits += 8
            } else {
                bits += Integer.numberOfLeadingZeros(v) - 24
                break
            }
        }
        return bits
    }

    fun solveHashcash(
        bariOrigin: String,
        headerHash: ByteArray,
        waypoint: ByteArray,
        difficultyBits: Int = SpecConstants.HASHCASH_DIFFICULTY_BITS
    ): HashcashResult {
        val challenge = hashcashChallenge(bariOrigin, headerHash)
        val preimage = ByteArray(72)
        System.arraycopy(challenge, 0, preimage, 0, 32)
        System.arraycopy(waypoint, 0, preimage, 32, 32)
        val md = MessageDigest.getInstance("SHA-256")
        val digestBuf = ByteArray(32)

        var nonce = 0L
        while (true) {
            Encoding.writeLe64(preimage, 64, nonce)
            md.update(preimage, 0, 72)
            md.digest(digestBuf, 0, 32)
            val lzb = leadingZeroBits(digestBuf)
            if (lzb >= difficultyBits) {
                return HashcashResult(challenge, nonce, digestBuf.copyOf(), lzb)
            }
            nonce++
        }
    }

    // Section 15.4 Rule B2: Leg-rate floor comparison
    fun compareLegRateFloor(hikeLegs: Long, hikeSpan: Long, p: Long, q: Long): String {
        require(p >= 1L && q >= 1L)
        if (hikeSpan <= 0L) return "refuse"
        val lhs = java.math.BigInteger.valueOf(q).multiply(java.math.BigInteger.valueOf(hikeLegs))
        val rhs = java.math.BigInteger.valueOf(p).multiply(java.math.BigInteger.valueOf(hikeSpan))
        return if (lhs >= rhs) "meets" else "refuse"
    }

    // Section 16.1: Integer anchor comparison
    fun compareAnchorWindow(stageHeightH: Long, candidateInclusionHeight: Long): String {
        val diff = candidateInclusionHeight - stageHeightH
        for (off in SpecConstants.ANCHOR_WINDOW_OFFSETS) {
            if (diff == off.toLong()) return "anchored"
        }
        return "not anchored"
    }

    // Section 16.1: Deterministic draw byte source from label
    fun labelDrawByteSource(label: String): () -> ByteArray? {
        var current: ByteArray? = null
        return {
            val next = if (current == null) {
                Encoding.sha256(label.toByteArray(Charsets.US_ASCII))
            } else {
                Encoding.sha256(current!!)
            }
            current = next
            next
        }
    }

    /**
     * Wraps a block generator into an 8-byte reader for section 14.6 / 15.5 draws.
     */
    class DrawByteReader(private val nextChunk: () -> ByteArray?) {
        private var buf: ByteArray = ByteArray(0)
        private var pos: Int = 0

        fun read8Bytes(): ByteArray? {
            val out = ByteArray(8)
            var filled = 0
            while (filled < 8) {
                if (pos >= buf.size) {
                    val chunk = nextChunk() ?: return null
                    if (chunk.isEmpty()) return null
                    buf = chunk
                    pos = 0
                }
                val take = minOf(8 - filled, buf.size - pos)
                System.arraycopy(buf, pos, out, filled, take)
                pos += take
                filled += take
            }
            return out
        }
    }

    /**
     * One unbiased draw from 0 .. legTotal - 1 (SPEC section 14.6).
     * Returns null if the byte source is exhausted before a valid index is drawn.
     */
    fun drawOneIndex(legTotal: Int, reader: DrawByteReader): Int? {
        require(legTotal >= 1)
        val n = legTotal.toULong()
        val rem = (0uL - n) % n
        val checkLimit = rem != 0uL
        val limit = 0uL - rem

        while (true) {
            val bytes = reader.read8Bytes() ?: return null
            val u = Encoding.u64Le(bytes, 0).toULong()
            if (!checkLimit || u < limit) {
                return (u % n).toInt()
            }
        }
    }

    /**
     * Section 14.6 six-leg draw.
     * Returns List of 6 distinct indices, or null if byte source is short (`not read`).
     */
    fun drawSixLegIndices(legTotal: Int, reader: DrawByteReader): List<Int>? {
        require(legTotal >= 6)
        val chosen = ArrayList<Int>(6)
        while (chosen.size < 6) {
            val q = drawOneIndex(legTotal, reader) ?: return null
            if (q !in chosen) {
                chosen.add(q)
            }
        }
        return chosen
    }

    /**
     * Section 15.5 thorough-round draw.
     * Draws D = 6 * thoroughRounds indices in draw order.
     */
    fun drawThoroughIndices(legTotal: Int, thoroughRounds: Int, reader: DrawByteReader): List<Int>? {
        require(legTotal >= 6 && thoroughRounds >= 1)
        val d = 6 * thoroughRounds
        val phase = ArrayList<Int>(d)
        while (phase.size < d) {
            val q = drawOneIndex(legTotal, reader) ?: return null
            val groupStart = (phase.size / 6) * 6
            var inCurrentGroup = false
            for (k in groupStart until phase.size) {
                if (phase[k] == q) {
                    inCurrentGroup = true
                    break
                }
            }
            if (inCurrentGroup) continue

            if (q in phase && phase.distinct().size < legTotal) {
                continue
            }
            phase.add(q)
        }
        return phase
    }
}
