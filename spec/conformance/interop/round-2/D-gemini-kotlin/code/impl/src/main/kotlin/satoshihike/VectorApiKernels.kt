package satoshihike

import jdk.incubator.vector.LongVector
import jdk.incubator.vector.VectorOperators
import java.security.MessageDigest
import java.util.stream.IntStream

/**
 * Explicit JDK 21 Vector API (LongVector.SPECIES_512) kernels for AVX-512 benchmarking comparison.
 */
object VectorApiKernels {
    private const val VLEN_512 = 8

    fun fillTerrainVector512(profile: Profile, seed: ByteArray, parallel: Boolean = true): Terrain {
        if (profile.wordsPerSlot < VLEN_512 || profile.wordsPerSlot % VLEN_512 != 0) {
            return Construction.fillTerrainScalar(profile, seed)
        }
        val lanes = Construction.extractLanes(seed)
        val nSlots = profile.nSlots
        val wordsPerSlot = profile.wordsPerSlot
        val words = LongArray(profile.totalWords)
        val p2Offsets = IntArray(nSlots)
        val p3Offsets = IntArray(nSlots)
        for (i in 1 until nSlots) {
            p2Offsets[i] = Construction.bitrevParent(i) * wordsPerSlot
            p3Offsets[i] = Construction.mulParent(i) * wordsPerSlot
        }
        val rot1Table = LongArray(wordsPerSlot) { j -> ((j + 1) and 63).toLong() }
        val rot2Table = LongArray(wordsPerSlot) { j -> ((j + 2) and 63).toLong() }
        val rot3Table = LongArray(wordsPerSlot) { j -> ((j + 3) and 63).toLong() }
        val jXorTable = LongArray(wordsPerSlot) { j -> lanes[j and 3] xor SpecConstants.WORD_XOR xor j.toLong() }

        val cores = Runtime.getRuntime().availableProcessors().coerceAtLeast(1)
        val totalVecs = wordsPerSlot / VLEN_512
        if (parallel && totalVecs >= cores && totalVecs % cores == 0) {
            val wordsPerCore = (totalVecs / cores) * VLEN_512
            IntStream.range(0, cores).parallel().forEach { coreIdx ->
                val jStart = coreIdx * wordsPerCore
                fillColumnSlice512(
                    words, lanes, p2Offsets, p3Offsets,
                    jXorTable, rot1Table, rot2Table, rot3Table,
                    nSlots, wordsPerSlot, jStart, jStart + wordsPerCore
                )
            }
        } else {
            fillColumnSlice512(
                words, lanes, p2Offsets, p3Offsets,
                jXorTable, rot1Table, rot2Table, rot3Table,
                nSlots, wordsPerSlot, 0, wordsPerSlot
            )
        }
        return Terrain(profile, words)
    }

    private fun fillColumnSlice512(
        words: LongArray,
        lanes: LongArray,
        p2Offsets: IntArray,
        p3Offsets: IntArray,
        jXorTable: LongArray,
        rot1Table: LongArray,
        rot2Table: LongArray,
        rot3Table: LongArray,
        nSlots: Int,
        wordsPerSlot: Int,
        jStart: Int,
        jEnd: Int
    ) {
        val sp = LongVector.SPECIES_512
        val r1Arr = LongArray(VLEN_512) { k -> java.lang.Long.rotateLeft(lanes[(k + 1) and 3], 17) }
        val r2Arr = LongArray(VLEN_512) { k -> java.lang.Long.rotateLeft(lanes[(k + 2) and 3], 31) }
        val r3Arr = LongArray(VLEN_512) { k -> java.lang.Long.rotateLeft(lanes[(k + 3) and 3], 47) }

        val r1Vec = LongVector.fromArray(sp, r1Arr, 0)
        val r2Vec = LongVector.fromArray(sp, r2Arr, 0)
        val r3Vec = LongVector.fromArray(sp, r3Arr, 0)

        val mixAddVec = LongVector.broadcast(sp, SpecConstants.MIX_ADD)
        val mixMul1Vec = LongVector.broadcast(sp, SpecConstants.MIX_MUL1)
        val mixMul2Vec = LongVector.broadcast(sp, SpecConstants.MIX_MUL2)
        val pm1Vec = LongVector.broadcast(sp, SpecConstants.PARENT_MUL_1)
        val pm2Vec = LongVector.broadcast(sp, SpecConstants.PARENT_MUL_2)
        val pm3Vec = LongVector.broadcast(sp, SpecConstants.PARENT_MUL_3)

        var j = jStart
        while (j < jEnd) {
            val p0XorJ = LongVector.fromArray(sp, jXorTable, j)
            var w = vectorMix64(p0XorJ, mixAddVec, mixMul1Vec, mixMul2Vec)
            w = vectorMix64(w.lanewise(VectorOperators.XOR, r1Vec), mixAddVec, mixMul1Vec, mixMul2Vec)
            w = vectorMix64(w.lanewise(VectorOperators.XOR, r2Vec), mixAddVec, mixMul1Vec, mixMul2Vec)
            w = vectorMix64(w.lanewise(VectorOperators.XOR, r3Vec), mixAddVec, mixMul1Vec, mixMul2Vec)
            w.intoArray(words, j)
            j += VLEN_512
        }

        for (i in 1 until nSlots) {
            val iShiftVec = LongVector.broadcast(sp, i.toLong() shl 32)
            val slotMixVec = LongVector.broadcast(sp, i.toLong() * SpecConstants.SLOT_MUL)
            val p1Off = (i - 1) * wordsPerSlot
            val p2Off = p2Offsets[i]
            val p3Off = p3Offsets[i]
            val outOff = i * wordsPerSlot

            var col = jStart
            while (col < jEnd) {
                val p0XorJ = LongVector.fromArray(sp, jXorTable, col)
                val rot1 = LongVector.fromArray(sp, rot1Table, col)
                val rot2 = LongVector.fromArray(sp, rot2Table, col)
                val rot3 = LongVector.fromArray(sp, rot3Table, col)

                var v = vectorMix64(p0XorJ.lanewise(VectorOperators.XOR, iShiftVec), mixAddVec, mixMul1Vec, mixMul2Vec)
                v = vectorMix64(v.lanewise(VectorOperators.XOR, r1Vec), mixAddVec, mixMul1Vec, mixMul2Vec)
                v = vectorMix64(v.lanewise(VectorOperators.XOR, r2Vec), mixAddVec, mixMul1Vec, mixMul2Vec)
                v = vectorMix64(v.lanewise(VectorOperators.XOR, r3Vec), mixAddVec, mixMul1Vec, mixMul2Vec)

                val pw1 = LongVector.fromArray(sp, words, p1Off + col).lanewise(VectorOperators.ROL, rot1)
                v = vectorMix64(
                    v.lanewise(VectorOperators.XOR, pw1).lanewise(VectorOperators.XOR, pm1Vec),
                    mixAddVec, mixMul1Vec, mixMul2Vec
                )

                val pw2 = LongVector.fromArray(sp, words, p2Off + col).lanewise(VectorOperators.ROL, rot2)
                v = vectorMix64(
                    v.lanewise(VectorOperators.XOR, pw2).lanewise(VectorOperators.XOR, pm2Vec),
                    mixAddVec, mixMul1Vec, mixMul2Vec
                )

                val pw3 = LongVector.fromArray(sp, words, p3Off + col).lanewise(VectorOperators.ROL, rot3)
                v = vectorMix64(
                    v.lanewise(VectorOperators.XOR, pw3).lanewise(VectorOperators.XOR, pm3Vec),
                    mixAddVec, mixMul1Vec, mixMul2Vec
                )

                v = vectorMix64(v.lanewise(VectorOperators.XOR, slotMixVec), mixAddVec, mixMul1Vec, mixMul2Vec)
                v.intoArray(words, outOff + col)

                col += VLEN_512
            }
        }
    }

    @Suppress("NOTHING_TO_INLINE")
    private inline fun vectorMix64(
        x: LongVector,
        mixAddVec: LongVector,
        mixMul1Vec: LongVector,
        mixMul2Vec: LongVector
    ): LongVector {
        var y = x.add(mixAddVec)
        y = y.lanewise(VectorOperators.XOR, y.lanewise(VectorOperators.LSHR, 30)).mul(mixMul1Vec)
        y = y.lanewise(VectorOperators.XOR, y.lanewise(VectorOperators.LSHR, 27)).mul(mixMul2Vec)
        return y.lanewise(VectorOperators.XOR, y.lanewise(VectorOperators.LSHR, 31))
    }

    fun legResultVector512(terrain: Terrain, previousCheckpoint: ByteArray): Long {
        val profile = terrain.profile
        val words = terrain.words
        val wordsPerSlot = profile.wordsPerSlot
        if (wordsPerSlot < VLEN_512 || wordsPerSlot % VLEN_512 != 0) {
            return Construction.legResultScalar(terrain, previousCheckpoint)
        }

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
            val acc = foldSlotVector512(words, baseOffset, wordsPerSlot, s)
            Encoding.writeLe64(sealBuf, 12, acc)
            md.update(sealBuf, 0, 20)
            md.digest(digestBuf, 0, 32)
            s = Encoding.u64Le(digestBuf, 0)
        }
        return s
    }

    private fun foldSlotVector512(words: LongArray, baseOffset: Int, wordsPerSlot: Int, s: Long): Long {
        if (wordsPerSlot < 32) {
            return foldSlotVector512Small(words, baseOffset, wordsPerSlot, s)
        }
        val mixAddVec = LongVector.broadcast(LongVector.SPECIES_512, SpecConstants.MIX_ADD)
        val mixMul1Vec = LongVector.broadcast(LongVector.SPECIES_512, SpecConstants.MIX_MUL1)
        val mixMul2Vec = LongVector.broadcast(LongVector.SPECIES_512, SpecConstants.MIX_MUL2)
        val sVec = LongVector.broadcast(LongVector.SPECIES_512, s)
        var acc0 = LongVector.zero(LongVector.SPECIES_512)
        var acc1 = LongVector.zero(LongVector.SPECIES_512)
        var acc2 = LongVector.zero(LongVector.SPECIES_512)
        var acc3 = LongVector.zero(LongVector.SPECIES_512)
        var idx = baseOffset
        val end = baseOffset + wordsPerSlot
        while (idx < end) {
            var y0 = LongVector.fromArray(LongVector.SPECIES_512, words, idx)
                .lanewise(VectorOperators.XOR, sVec).add(mixAddVec)
            y0 = y0.lanewise(VectorOperators.XOR, y0.lanewise(VectorOperators.LSHR, 30)).mul(mixMul1Vec)
            y0 = y0.lanewise(VectorOperators.XOR, y0.lanewise(VectorOperators.LSHR, 27)).mul(mixMul2Vec)
            y0 = y0.lanewise(VectorOperators.XOR, y0.lanewise(VectorOperators.LSHR, 31))
            acc0 = acc0.add(y0)

            var y1 = LongVector.fromArray(LongVector.SPECIES_512, words, idx + 8)
                .lanewise(VectorOperators.XOR, sVec).add(mixAddVec)
            y1 = y1.lanewise(VectorOperators.XOR, y1.lanewise(VectorOperators.LSHR, 30)).mul(mixMul1Vec)
            y1 = y1.lanewise(VectorOperators.XOR, y1.lanewise(VectorOperators.LSHR, 27)).mul(mixMul2Vec)
            y1 = y1.lanewise(VectorOperators.XOR, y1.lanewise(VectorOperators.LSHR, 31))
            acc1 = acc1.add(y1)

            var y2 = LongVector.fromArray(LongVector.SPECIES_512, words, idx + 16)
                .lanewise(VectorOperators.XOR, sVec).add(mixAddVec)
            y2 = y2.lanewise(VectorOperators.XOR, y2.lanewise(VectorOperators.LSHR, 30)).mul(mixMul1Vec)
            y2 = y2.lanewise(VectorOperators.XOR, y2.lanewise(VectorOperators.LSHR, 27)).mul(mixMul2Vec)
            y2 = y2.lanewise(VectorOperators.XOR, y2.lanewise(VectorOperators.LSHR, 31))
            acc2 = acc2.add(y2)

            var y3 = LongVector.fromArray(LongVector.SPECIES_512, words, idx + 24)
                .lanewise(VectorOperators.XOR, sVec).add(mixAddVec)
            y3 = y3.lanewise(VectorOperators.XOR, y3.lanewise(VectorOperators.LSHR, 30)).mul(mixMul1Vec)
            y3 = y3.lanewise(VectorOperators.XOR, y3.lanewise(VectorOperators.LSHR, 27)).mul(mixMul2Vec)
            y3 = y3.lanewise(VectorOperators.XOR, y3.lanewise(VectorOperators.LSHR, 31))
            acc3 = acc3.add(y3)

            idx += 32
        }
        return acc0.add(acc1).add(acc2.add(acc3)).reduceLanes(VectorOperators.ADD)
    }

    private fun foldSlotVector512Small(words: LongArray, baseOffset: Int, wordsPerSlot: Int, s: Long): Long {
        val mixAddVec = LongVector.broadcast(LongVector.SPECIES_512, SpecConstants.MIX_ADD)
        val mixMul1Vec = LongVector.broadcast(LongVector.SPECIES_512, SpecConstants.MIX_MUL1)
        val mixMul2Vec = LongVector.broadcast(LongVector.SPECIES_512, SpecConstants.MIX_MUL2)
        val sVec = LongVector.broadcast(LongVector.SPECIES_512, s)
        var acc0 = LongVector.zero(LongVector.SPECIES_512)
        var idx = baseOffset
        val end = baseOffset + wordsPerSlot
        while (idx < end) {
            var y0 = LongVector.fromArray(LongVector.SPECIES_512, words, idx)
                .lanewise(VectorOperators.XOR, sVec).add(mixAddVec)
            y0 = y0.lanewise(VectorOperators.XOR, y0.lanewise(VectorOperators.LSHR, 30)).mul(mixMul1Vec)
            y0 = y0.lanewise(VectorOperators.XOR, y0.lanewise(VectorOperators.LSHR, 27)).mul(mixMul2Vec)
            y0 = y0.lanewise(VectorOperators.XOR, y0.lanewise(VectorOperators.LSHR, 31))
            acc0 = acc0.add(y0)
            idx += 8
        }
        return acc0.reduceLanes(VectorOperators.ADD)
    }
}
