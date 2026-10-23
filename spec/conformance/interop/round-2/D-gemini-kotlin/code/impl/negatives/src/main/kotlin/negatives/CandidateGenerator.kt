package negatives

import satoshihike.CasesFixtures
import satoshihike.CborCodec
import satoshihike.Construction
import satoshihike.Encoding
import satoshihike.Producer
import satoshihike.Profile
import satoshihike.Terrain
import satoshihike.VerifyResult
import satoshihike.Verifier
import java.io.File

/**
 * Deterministic candidate generator writing all 14 files in CASES.md to `candidates/`.
 */
object CandidateGenerator {

    fun generateAll(outDir: File) {
        outDir.mkdirs()

        val seedA = Construction.computeSeed(CasesFixtures.PK_A, CasesFixtures.SEED_BEACON)
        val seedB = Construction.computeSeed(CasesFixtures.PK_B, CasesFixtures.SEED_BEACON)

        // 1. seed.txt
        writeCandidateFile(outDir, "seed.txt", generateSeedCases(seedA, seedB))

        // Fill S1 terrain once for S1 cases
        val terrainS1 = Construction.fillTerrain(Profile.S1, seedA)
        val terrainS1Sha256Hex = Encoding.toHex(terrainS1.sha256())

        // 2. nl4.txt
        writeCandidateFile(outDir, "nl4.txt", generateNl4Cases(seedA, terrainS1))

        // Fill S2 and PROD terrains
        val terrainS2 = Construction.fillTerrain(Profile.S2, seedA)
        val terrainS2Sha256Hex = Encoding.toHex(terrainS2.sha256())

        val terrainProd = Construction.fillTerrain(Profile.PRODUCTION, seedA)
        val terrainProdSha256Hex = Encoding.toHex(terrainProd.sha256())

        // 3. terrain.txt
        writeCandidateFile(
            outDir,
            "terrain.txt",
            mapOf(
                "terrain.S1.terrain_sha256" to terrainS1Sha256Hex,
                "terrain.S2.terrain_sha256" to terrainS2Sha256Hex,
                "terrain.PROD.terrain_sha256" to terrainProdSha256Hex
            )
        )

        // 4. fold.txt
        writeCandidateFile(outDir, "fold.txt", generateFoldCases(terrainS1, seedA))

        // 5. kat-small.txt (also gives O1 for unknown-kind)
        val (katSmallMap, hikeO1) = generateKatHikeCases("kat-small.S1", Profile.S1, terrainS1, terrainS1Sha256Hex, 1)
        writeCandidateFile(outDir, "kat-small.txt", katSmallMap)

        // 6. test-3leg.txt
        val (test3LegMap, _) = generateKatHikeCases("test-3leg.S2", Profile.S2, terrainS2, terrainS2Sha256Hex, 3)
        writeCandidateFile(outDir, "test-3leg.txt", test3LegMap)

        // 7. prod.txt
        writeCandidateFile(outDir, "prod.txt", generateProdCases(terrainProd, terrainProdSha256Hex, seedA))

        // 8. wes.txt
        val s0WaypointHex = Encoding.toHex(hikeO1.hikeObject.stages[0].waypoint)
        val dHex = Encoding.toHex(seedA)
        writeCandidateFile(
            outDir,
            "wes.txt",
            mapOf(
                "wes.S1.D" to dHex,
                "wes.S1.stage.0.waypoint" to s0WaypointHex,
                "wes.S1.equal" to (dHex == s0WaypointHex).toString()
            )
        )

        // 9. rate.txt
        writeCandidateFile(outDir, "rate.txt", generateRateCases())

        // 10. anchor.txt
        writeCandidateFile(outDir, "anchor.txt", generateAnchorCases())

        // 11. draw.txt
        writeCandidateFile(outDir, "draw.txt", generateDrawCases())

        // 12. unknown-kind.txt
        writeCandidateFile(outDir, "unknown-kind.txt", generateUnknownKindCases(hikeO1.hikeObject))

        // 13. hashcash.txt
        writeCandidateFile(outDir, "hashcash.txt", generateHashcashCases())

        // 14. negatives.txt
        writeCandidateFile(outDir, "negatives.txt", generateNegativesCases(terrainS1, terrainS1Sha256Hex, seedA))
    }

    private fun writeCandidateFile(outDir: File, fileName: String, entries: Map<String, String>) {
        // Sort lines by key in ASCII byte order
        val sortedKeys = entries.keys.sortedWith { a, b ->
            val ba = a.toByteArray(Charsets.US_ASCII)
            val bb = b.toByteArray(Charsets.US_ASCII)
            val minLen = minOf(ba.size, bb.size)
            var cmp = 0
            for (i in 0 until minLen) {
                val va = ba[i].toInt() and 0xFF
                val vb = bb[i].toInt() and 0xFF
                if (va != vb) {
                    cmp = va.compareTo(vb)
                    break
                }
            }
            if (cmp != 0) cmp else ba.size.compareTo(bb.size)
        }

        val sb = StringBuilder()
        for (k in sortedKeys) {
            val v = entries.getValue(k)
            sb.append(k).append('\t').append(v).append('\n')
        }
        File(outDir, fileName).writeText(sb.toString(), Charsets.US_ASCII)
    }

    private fun generateSeedCases(seedA: ByteArray, seedB: ByteArray): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        val pairs = listOf(
            "pk_A" to (CasesFixtures.PK_A to seedA),
            "pk_B" to (CasesFixtures.PK_B to seedB)
        )
        for ((name, pair) in pairs) {
            val (pk, seed) = pair
            val preimage = Construction.seedPreimage(pk, CasesFixtures.SEED_BEACON)
            val lanes = Construction.extractLanes(seed)
            map["seed.$name.preimage"] = Encoding.toHex(preimage)
            map["seed.$name.D"] = Encoding.toHex(seed)
            for (k in 0..3) {
                map["seed.$name.P${k}_u64"] = Encoding.u64ToHexBe(lanes[k])
            }
        }
        return map
    }

    private fun generateNl4Cases(seedA: ByteArray, terrainS1: Terrain): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        val lanes = Construction.extractLanes(seedA)
        for (i in listOf(0, 1, 1023)) {
            for (j in 0..7) {
                val baseW = Construction.nl4BaseWord(lanes, i, j)
                val storedW = terrainS1.word(i, j)
                map["nl4.S1.slot.$i.word.$j.base_u64"] = Encoding.u64ToHexBe(baseW)
                map["nl4.S1.slot.$i.word.$j.stored_u64"] = Encoding.u64ToHexBe(storedW)
            }
        }
        return map
    }

    private fun generateFoldCases(terrainS1: Terrain, seedA: ByteArray): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        val (traces, legResult) = Construction.legWithTrace(terrainS1, seedA, 4)
        for (tr in traces) {
            val n = tr.stepIndex
            map["fold.S1.step.$n.S_u64"] = Encoding.u64ToHexBe(tr.inputS)
            map["fold.S1.step.$n.slot_index"] = tr.slotIndex.toString()
            map["fold.S1.step.$n.acc_u64"] = Encoding.u64ToHexBe(tr.acc)
            map["fold.S1.step.$n.S_next_u64"] = Encoding.u64ToHexBe(tr.outputS)
        }
        map["fold.S1.leg_result_u64"] = Encoding.u64ToHexBe(legResult)
        return map
    }

    private fun generateKatHikeCases(
        prefix: String,
        profile: Profile,
        terrain: Terrain,
        terrainSha256Hex: String,
        legsPerStageCount: Int
    ): Pair<Map<String, String>, Producer.ProducedHike> {
        val map = LinkedHashMap<String, String>()
        val produced = Producer.produceOffline(
            profile = profile,
            legsPerStage = IntArray(18) { legsPerStageCount },
            precomputedTerrain = terrain
        )
        map["$prefix.terrain_sha256"] = terrainSha256Hex
        for (stage in produced.hikeObject.stages) {
            val g = stage.stageIndex
            map["$prefix.stage.$g.waypoint"] = Encoding.toHex(stage.waypoint)
            for (j in stage.checkpoints.indices) {
                map["$prefix.stage.$g.leg.$j.legs_le_hex"] = Encoding.toHex(stage.legResults[j])
                map["$prefix.stage.$g.leg.$j.checkpoint"] = Encoding.toHex(stage.checkpoints[j])
            }
        }
        return map to produced
    }

    private fun generateProdCases(
        terrainProd: Terrain,
        terrainProdSha256Hex: String,
        seedA: ByteArray
    ): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        val lr = Construction.legResult(terrainProd, seedA)
        val lrBytes = Encoding.le64(lr)
        val sample = terrainProd.terrainSample(seedA)
        val cp = Construction.computeCheckpoint(seedA, seedA, sample, lrBytes)
        map["prod.PROD.terrain_sha256"] = terrainProdSha256Hex
        map["prod.PROD.stage.0.leg.0.legs_le_hex"] = Encoding.toHex(lrBytes)
        map["prod.PROD.stage.0.leg.0.checkpoint"] = Encoding.toHex(cp)
        return map
    }

    private fun generateRateCases(): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        val legsGrid = listOf(0L, 1L, 5L, 6L, 1000L)
        val spansGrid = listOf(1L, 300L, 3600L)
        val floorsGrid = listOf(1L to 5L, 1L to 4L, 7L to 20L)
        for (l in legsGrid) {
            for (s in spansGrid) {
                for ((p, q) in floorsGrid) {
                    val res = Construction.compareLegRateFloor(l, s, p, q)
                    map["rate.legs.$l.span.$s.floor.${p}_${q}"] = res
                }
            }
        }
        return map
    }

    private fun generateAnchorCases(): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        for (h in 100L..106L) {
            map["anchor.H.100.height.$h"] = Construction.compareAnchorWindow(100L, h)
        }
        return map
    }

    private fun generateDrawCases(): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        val label = "pcoc-pack-2.2/draw"
        val thoroughRounds = 3
        map["draw.b0_label"] = label
        map["draw.thorough_rounds"] = thoroughRounds.toString()

        for (legCount in listOf(6, 18, 54)) {
            val sixReader = Construction.DrawByteReader(Construction.labelDrawByteSource(label))
            val sixIndices = requireNotNull(Construction.drawSixLegIndices(legCount, sixReader))
            map["draw.legs.$legCount.six_leg.indices"] = sixIndices.joinToString(",")

            val thoroughReader = Construction.DrawByteReader(Construction.labelDrawByteSource(label))
            val thoroughIndices = requireNotNull(
                Construction.drawThoroughIndices(legCount, thoroughRounds, thoroughReader)
            )
            map["draw.legs.$legCount.thorough.indices"] = thoroughIndices.joinToString(",")
        }
        return map
    }

    private fun generateUnknownKindCases(o1Object: satoshihike.HikeObject): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        val labels = listOf(
            "v2.1-pcoc-object",
            "satoshi-hike-v2.0-object",
            "",
            "v2.2-pcoc-objecT"
        )
        for ((n, label) in labels.withIndex()) {
            val modifiedObj = o1Object.copy(kind = label)
            val cborBytes = CborCodec.encode(modifiedObj)
            val verified = Verifier.verify(cborBytes, Profile.S1)
            check(verified is VerifyResult.Record) { "Expected Rule A5 record for unknown kind $label" }
            val labelUtf8Hex = Encoding.toHex(label.toByteArray(Charsets.UTF_8))
            val recordHex = Encoding.toHex(verified.json.toByteArray(Charsets.UTF_8))
            map["unknown-kind.$n.kind_text_hex"] = labelUtf8Hex
            map["unknown-kind.$n.record_hex"] = recordHex
        }
        return map
    }

    private fun generateHashcashCases(): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        val bariOrigin = "https://example.com"
        val fh0 = Encoding.sha256("pcoc-pack-2.2/hashcash/fee_header/0".toByteArray(Charsets.US_ASCII))
        val fh1 = Encoding.sha256("pcoc-pack-2.2/hashcash/fee_header/1".toByteArray(Charsets.US_ASCII))
        val fh2 = Encoding.sha256("pcoc-pack-2.2/hashcash/fee_header/2".toByteArray(Charsets.US_ASCII))
        val feeHeader = fh0 + fh1 + fh2.copyOfRange(0, 16)
        check(feeHeader.size == 80)

        val headerHash = Encoding.sha256(Encoding.sha256(feeHeader))
        val waypoint = Encoding.sha256("pcoc-pack-2.2/hashcash/waypoint".toByteArray(Charsets.US_ASCII))
        val difficultyBits = 21

        val solved = Construction.solveHashcash(bariOrigin, headerHash, waypoint, difficultyBits)

        map["hashcash.bari_origin"] = bariOrigin
        map["hashcash.fee_header"] = Encoding.toHex(feeHeader)
        map["hashcash.header_hash"] = Encoding.toHex(headerHash)
        map["hashcash.waypoint"] = Encoding.toHex(waypoint)
        map["hashcash.difficulty_bits"] = difficultyBits.toString()
        map["hashcash.challenge"] = Encoding.toHex(solved.challenge)
        map["hashcash.nonce"] = solved.nonce.toULong().toString()
        map["hashcash.digest"] = Encoding.toHex(solved.digest)
        map["hashcash.leading_zero_bits"] = solved.leadingZeroBits.toString()
        return map
    }

    private fun generateNegativesCases(
        terrainS1: Terrain,
        honestTerrainSha256Hex: String,
        seedA: ByteArray
    ): Map<String, String> {
        val map = LinkedHashMap<String, String>()
        val honestLeg = Construction.legResult(terrainS1, seedA)
        val honestLegHex = Encoding.toHex(Encoding.le64(honestLeg))

        map["negatives.honest.legs_le_hex"] = honestLegHex
        map["negatives.honest.terrain_sha256"] = honestTerrainSha256Hex

        val legNegatives = listOf(
            "slot_summary_fold" to Negatives.slotSummaryFold(terrainS1, seedA),
            "xor_accumulator_fold" to Negatives.xorAccumulatorFold(terrainS1, seedA),
            "xor_w_fold" to Negatives.xorWFold(terrainS1, seedA),
            "s_plus_w_fold" to Negatives.sPlusWFold(terrainS1, seedA),
            "prefold_hoist" to Negatives.prefoldHoist(terrainS1, seedA),
            "final_stage_hoist" to Negatives.finalStageHoist(terrainS1, seedA),
            "partial_slot_read" to Negatives.partialSlotRead(terrainS1, seedA),
            "partial_slot_read_2047" to Negatives.partialSlotRead2047(terrainS1, seedA),
            "f3_shared_terrain" to Negatives.f3SharedTerrain(
                Profile.S1,
                CasesFixtures.PK_A,
                CasesFixtures.PK_B,
                CasesFixtures.SEED_BEACON
            ),
            "cached_leg" to Negatives.cachedLeg(terrainS1, seedA)
        )

        for ((name, negLeg) in legNegatives) {
            val negHex = Encoding.toHex(Encoding.le64(negLeg))
            val matches = (negHex == honestLegHex)
            if (matches) {
                error("Permanent leg negative '$name' matched honest value $honestLegHex!")
            }
            map["negatives.$name.legs_le_hex"] = negHex
            map["negatives.$name.matches_honest"] = matches.toString()
        }

        val terrainNegatives = listOf(
            "one_lane_flip" to Negatives.oneLaneFlipTerrainSha256(Profile.S1, seedA),
            "all4_kernel" to Negatives.all4KernelTerrainSha256(Profile.S1, seedA),
            "squeezed_seed_terrain" to Negatives.squeezedSeedTerrainSha256(Profile.S1, seedA)
        )

        for ((name, negSha) in terrainNegatives) {
            val negHex = Encoding.toHex(negSha)
            val matches = (negHex == honestTerrainSha256Hex)
            if (matches) {
                error("Permanent terrain negative '$name' matched honest terrain digest $honestTerrainSha256Hex!")
            }
            map["negatives.$name.terrain_sha256"] = negHex
            map["negatives.$name.matches_honest"] = matches.toString()
        }

        check(map.size == 28) { "Expected 28 keys in negatives.txt, got ${map.size}" }
        return map
    }
}

fun main(args: Array<String>) {
    val outDir = if (args.isNotEmpty()) File(args[0]) else File("candidates")
    val t0 = System.nanoTime()
    CandidateGenerator.generateAll(outDir)
    val ms = (System.nanoTime() - t0) / 1_000_000.0
    println("Generated all 14 candidate files in ${outDir.path} in %.2f ms".format(ms))
}
