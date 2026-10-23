package negatives

import satoshihike.CasesFixtures
import satoshihike.Construction
import satoshihike.Encoding
import satoshihike.Profile
import satoshihike.SpecConstants
import satoshihike.Terrain
import java.security.MessageDigest

/**
 * Permanent negatives (SPEC section 16 & CASES.md case 14).
 * Lives strictly in the `negatives` generator module, never on an SDK code path.
 */
object Negatives {

    private fun mix64NoFinal(x: Long): Long {
        var y = x + SpecConstants.MIX_ADD
        y = (y xor (y ushr 30)) * SpecConstants.MIX_MUL1
        y = (y xor (y ushr 27)) * SpecConstants.MIX_MUL2
        return y
    }

    private fun runCustomStepLeg(
        terrain: Terrain,
        seed: ByteArray,
        foldStep: (slotIndex: Int, s: Long) -> Long
    ): Long {
        val profile = terrain.profile
        val nSlots = profile.nSlots
        val steps = profile.stepsPerLeg
        val md = MessageDigest.getInstance("SHA-256")
        val sealBuf = ByteArray(20)
        System.arraycopy(SpecConstants.SEAL_DOMAIN, 0, sealBuf, 0, 12)
        val digestBuf = ByteArray(32)

        var s = Encoding.u64Le(seed, 0)
        for (step in 0 until steps) {
            val slotIdx = java.lang.Long.remainderUnsigned(s, nSlots.toLong()).toInt()
            val sFolded = foldStep(slotIdx, s)
            Encoding.writeLe64(sealBuf, 12, sFolded)
            md.update(sealBuf, 0, 20)
            md.digest(digestBuf, 0, 32)
            s = Encoding.u64Le(digestBuf, 0)
        }
        return s
    }

    fun slotSummaryFold(terrain: Terrain, seed: ByteArray): Long {
        val words = terrain.words
        val wps = terrain.profile.wordsPerSlot
        return runCustomStepLeg(terrain, seed) { slotIdx, s ->
            val base = slotIdx * wps
            var xorSum = 0L
            for (w in 0 until wps) {
                xorSum = xorSum xor (words[base + w] xor w.toLong())
            }
            s xor xorSum
        }
    }

    fun xorAccumulatorFold(terrain: Terrain, seed: ByteArray): Long {
        val words = terrain.words
        val wps = terrain.profile.wordsPerSlot
        return runCustomStepLeg(terrain, seed) { slotIdx, s ->
            val base = slotIdx * wps
            var acc = 0L
            for (w in 0 until wps) {
                acc = acc xor Construction.mix64(words[base + w] xor w.toLong() xor s)
            }
            acc
        }
    }

    fun xorWFold(terrain: Terrain, seed: ByteArray): Long {
        val words = terrain.words
        val wps = terrain.profile.wordsPerSlot
        return runCustomStepLeg(terrain, seed) { slotIdx, s ->
            val base = slotIdx * wps
            var acc = 0L
            for (w in 0 until wps) {
                acc += Construction.mix64(words[base + w] xor w.toLong() xor s)
            }
            acc
        }
    }

    fun sPlusWFold(terrain: Terrain, seed: ByteArray): Long {
        val words = terrain.words
        val wps = terrain.profile.wordsPerSlot
        return runCustomStepLeg(terrain, seed) { slotIdx, s ->
            val base = slotIdx * wps
            var acc = 0L
            for (w in 0 until wps) {
                acc += Construction.mix64(words[base + w] xor (s + w.toLong()))
            }
            acc
        }
    }

    fun prefoldHoist(terrain: Terrain, seed: ByteArray): Long {
        val words = terrain.words
        val wps = terrain.profile.wordsPerSlot
        val nSlots = terrain.profile.nSlots
        val pre = LongArray(nSlots) { i ->
            val base = i * wps
            var sum = 0L
            for (w in 0 until wps) {
                sum += Construction.mix64(words[base + w] xor w.toLong())
            }
            sum
        }
        return runCustomStepLeg(terrain, seed) { slotIdx, s ->
            pre[slotIdx] + s
        }
    }

    fun finalStageHoist(terrain: Terrain, seed: ByteArray): Long {
        val words = terrain.words
        val wps = terrain.profile.wordsPerSlot
        return runCustomStepLeg(terrain, seed) { slotIdx, s ->
            val base = slotIdx * wps
            var x = 0L
            for (w in 0 until wps) {
                x = x xor mix64NoFinal(words[base + w] xor s)
            }
            x xor (x ushr 31)
        }
    }

    fun partialSlotRead(terrain: Terrain, seed: ByteArray): Long {
        val words = terrain.words
        val wps = terrain.profile.wordsPerSlot
        return runCustomStepLeg(terrain, seed) { slotIdx, s ->
            val base = slotIdx * wps
            Construction.mix64(words[base] xor s)
        }
    }

    fun partialSlotRead2047(terrain: Terrain, seed: ByteArray): Long {
        val words = terrain.words
        val wps = terrain.profile.wordsPerSlot
        return runCustomStepLeg(terrain, seed) { slotIdx, s ->
            val base = slotIdx * wps
            var acc = 0L
            for (w in 0 until (wps - 1)) {
                acc += Construction.mix64(words[base + w] xor s)
            }
            acc
        }
    }

    fun f3SharedTerrain(profile: Profile, pkA: ByteArray, pkB: ByteArray, seedBeacon: ByteArray): Long {
        val seedA = Construction.computeSeed(pkA, seedBeacon)
        val seedB = Construction.computeSeed(pkB, seedBeacon)
        val terrainB = Construction.fillTerrain(profile, seedB)
        return Construction.legResult(terrainB, seedA)
    }

    fun cachedLeg(terrain: Terrain, seed: ByteArray): Long {
        val words = terrain.words
        val wps = terrain.profile.wordsPerSlot
        val cache = HashMap<Int, Long>()
        var revisitedCount = 0
        val result = runCustomStepLeg(terrain, seed) { slotIdx, s ->
            val existing = cache[slotIdx]
            if (existing != null) {
                revisitedCount++
                existing
            } else {
                val base = slotIdx * wps
                var acc = 0L
                for (w in 0 until wps) {
                    acc += Construction.mix64(words[base + w] xor s)
                }
                cache[slotIdx] = acc
                acc
            }
        }
        check(revisitedCount > 0) {
            "cached_leg required at least one slot revisit at S1 pk_A, but revisitedCount was 0"
        }
        return result
    }

    private fun fillCustomBaseWordTerrain(
        profile: Profile,
        lanes: LongArray,
        baseWordFn: (lanes: LongArray, i: Int, j: Int) -> Long
    ): Terrain {
        val nSlots = profile.nSlots
        val wordsPerSlot = profile.wordsPerSlot
        val words = LongArray(profile.totalWords)

        for (j in 0 until wordsPerSlot) {
            words[j] = baseWordFn(lanes, 0, j)
        }

        val pm1 = SpecConstants.PARENT_MUL_1
        val pm2 = SpecConstants.PARENT_MUL_2
        val pm3 = SpecConstants.PARENT_MUL_3

        for (i in 1 until nSlots) {
            val p1Base = (i - 1) * wordsPerSlot
            val p2Base = Construction.bitrevParent(i) * wordsPerSlot
            val p3Base = Construction.mulParent(i) * wordsPerSlot
            val slotBase = i * wordsPerSlot
            val slotMix = i.toLong() * SpecConstants.SLOT_MUL

            for (j in 0 until wordsPerSlot) {
                var w = baseWordFn(lanes, i, j)
                val pw1 = words[p1Base + j]
                w = Construction.mix64((w xor java.lang.Long.rotateLeft(pw1, (j + 1) and 63)) xor pm1)
                val pw2 = words[p2Base + j]
                w = Construction.mix64((w xor java.lang.Long.rotateLeft(pw2, (j + 2) and 63)) xor pm2)
                val pw3 = words[p3Base + j]
                w = Construction.mix64((w xor java.lang.Long.rotateLeft(pw3, (j + 3) and 63)) xor pm3)
                w = Construction.mix64(w xor slotMix)
                words[slotBase + j] = w
            }
        }
        return Terrain(profile, words)
    }

    fun oneLaneFlipTerrainSha256(profile: Profile, seed: ByteArray): ByteArray {
        val lanes = Construction.extractLanes(seed)
        val terrain = fillCustomBaseWordTerrain(profile, lanes) { p, i, j ->
            val c = (i.toLong() shl 32) xor j.toLong() xor SpecConstants.WORD_XOR
            Construction.mix64(p[j and 3] xor c)
        }
        return terrain.sha256()
    }

    fun all4KernelTerrainSha256(profile: Profile, seed: ByteArray): ByteArray {
        val lanes = Construction.extractLanes(seed)
        val terrain = fillCustomBaseWordTerrain(profile, lanes) { p, i, j ->
            val c = (i.toLong() shl 32) xor j.toLong() xor SpecConstants.WORD_XOR
            val linear = p[j and 3] xor
                java.lang.Long.rotateLeft(p[(j + 1) and 3], 17) xor
                java.lang.Long.rotateLeft(p[(j + 2) and 3], 31) xor
                java.lang.Long.rotateLeft(p[(j + 3) and 3], 47) xor c
            Construction.mix64(linear)
        }
        return terrain.sha256()
    }

    fun squeezedSeedTerrainSha256(profile: Profile, seed: ByteArray): ByteArray {
        val lanes = Construction.extractLanes(seed)
        val squeezed = lanes[0] xor lanes[1] xor lanes[2] xor lanes[3]
        val sqLanes = LongArray(4) { squeezed }
        val terrain = fillCustomBaseWordTerrain(profile, sqLanes) { p, i, j ->
            Construction.nl4BaseWord(p, i, j)
        }
        return terrain.sha256()
    }
}
