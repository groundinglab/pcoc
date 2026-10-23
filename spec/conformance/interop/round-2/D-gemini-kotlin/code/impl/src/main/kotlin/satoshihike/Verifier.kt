package satoshihike

import java.security.SecureRandom
import java.util.stream.IntStream

sealed class VerifyResult {
    object DecodeFailure : VerifyResult()
    data class Record(val json: String) : VerifyResult()
}

/**
 * Section 14 Verifier (SPEC sections 14, 16.1, 18).
 */
object Verifier {
    private val MAX_HEIGHT_ULONG: ULong = (1uL shl 53) - 1uL

    private val TOP_LEVEL_KEYS = setOf(
        "kind", "fill", "slot_bytes", "n_slots", "sample_bytes", "steps_per_leg",
        "public_key", "seed_beacon", "stages", "last_interrupt", "last_interrupt_height"
    )
    private val STAGE_0_KEYS = setOf(
        "beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results"
    )
    private val STAGE_N_KEYS = setOf(
        "path", "beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results"
    )

    fun csprngDrawReader(): Construction.DrawByteReader {
        val sr = SecureRandom()
        return Construction.DrawByteReader {
            val bytes = ByteArray(64)
            sr.nextBytes(bytes)
            bytes
        }
    }

    fun verify(
        objectBytes: ByteArray,
        profile: Profile = Profile.PRODUCTION,
        indexSource: IndexSource = CasesFixtures.SYNTHETIC_INDEX_SOURCE,
        drawReader: Construction.DrawByteReader = csprngDrawReader()
    ): VerifyResult {
        return when (val classified = CborCodec.classifyAndDecode(objectBytes)) {
            is CborDecodeClassification.DecodeFailure -> VerifyResult.DecodeFailure
            is CborDecodeClassification.UnknownKind -> {
                VerifyResult.Record(CborCodec.formatUnknownKindRecord(classified.kindText))
            }
            is CborDecodeClassification.MalformedCbor -> {
                VerifyResult.Record(buildMalformedCborRecord(profile))
            }
            is CborDecodeClassification.ValidRootMap -> {
                VerifyResult.Record(verifyRootMap(classified.map, profile, indexSource, drawReader))
            }
        }
    }

    private data class HeaderRecord(
        val storedHashHex: String?, // null -> "not defined"
        val storedHashBytes: ByteArray?,
        val storedHeight: Long?, // null -> "not defined"
        val indexComparison: String, // "match", "mismatch", "not read"
        val confirmedHeight: Long?,
        val tValue: Long? // null -> "not read"
    )

    private data class BlockRef(val blockHashDisplay: String, val blockHeight: Long)

    private data class PathRecord1To16(
        val g: Int,
        val pathState: String,
        val plant: String,
        val pg: BlockRef?,
        val heightPg: Long?,
        val dg: Long?,
        val claimedBlock: BlockRef?,
        val isValidPath: Boolean,
        val issuedPlantGet: Boolean,
        val plantGetReturnedDoc: Boolean
    )

    private data class ParsedHeaderDoc(
        val hashNatural: ByteArray,
        val hashDisplayBytes: ByteArray,
        val height: Long,
        val time: Long?
    )

    private fun parseHeaderFromDoc(docObj: JsonVal.Obj?): ParsedHeaderDoc? {
        if (docObj == null) return null
        val headerVal = docObj.members["header"] as? JsonVal.Obj ?: return null
        val hashStr = (headerVal.members["hash"] as? JsonVal.Str)?.value ?: return null
        if (!Encoding.isValidHex64(hashStr)) return null
        val height = StrictJson.extractHeight(headerVal.members["height"]) ?: return null
        val displayBytes = Encoding.fromHex(hashStr)
        val naturalBytes = Encoding.reverseBytes32(displayBytes)
        val time = StrictJson.extractU32(headerVal.members["time"])
        return ParsedHeaderDoc(naturalBytes, displayBytes, height, time)
    }

    private fun verifyRootMap(
        rootMap: CborValue.MapVal,
        profile: Profile,
        indexSource: IndexSource,
        drawReader: Construction.DrawByteReader
    ): String {
        val entries = rootMap.entries

        // Check height_range across all integers in the object's own bytes
        var heightRangeViolated = false
        fun checkUIntRange(v: CborValue?): ULong? {
            if (v !is CborValue.UIntVal) return null
            if (v.value > MAX_HEIGHT_ULONG) {
                heightRangeViolated = true
                return null
            }
            return v.value
        }

        val kindStr = (entries["kind"] as? CborValue.TextVal)?.text
        val fillStr = (entries["fill"] as? CborValue.TextVal)?.text
        val slotBytesVal = checkUIntRange(entries["slot_bytes"])
        val nSlotsVal = checkUIntRange(entries["n_slots"])
        val sampleBytesVal = checkUIntRange(entries["sample_bytes"])
        val stepsPerLegVal = checkUIntRange(entries["steps_per_leg"])
        val publicKeyBytes = (entries["public_key"] as? CborValue.BytesVal)?.bytes
        val seedBeaconBytes = (entries["seed_beacon"] as? CborValue.BytesVal)?.bytes
        val lastInterruptBytes = (entries["last_interrupt"] as? CborValue.BytesVal)?.bytes
        val lastInterruptHeightVal = checkUIntRange(entries["last_interrupt_height"])
        val stagesArr = (entries["stages"] as? CborValue.ArrayVal)?.items

        var fieldSetViolated = false
        if (entries.keys != TOP_LEVEL_KEYS) {
            fieldSetViolated = true
        }
        if (kindStr == null || fillStr == null ||
            entries["slot_bytes"] !is CborValue.UIntVal ||
            entries["n_slots"] !is CborValue.UIntVal ||
            entries["sample_bytes"] !is CborValue.UIntVal ||
            entries["steps_per_leg"] !is CborValue.UIntVal ||
            publicKeyBytes == null || publicKeyBytes.size != 32 ||
            seedBeaconBytes == null || seedBeaconBytes.size != 32 ||
            lastInterruptBytes == null || lastInterruptBytes.size != 32 ||
            entries["last_interrupt_height"] !is CborValue.UIntVal ||
            stagesArr == null || stagesArr.any { it !is CborValue.MapVal }
        ) {
            fieldSetViolated = true
        }

        val stagesLengthViolated = stagesArr == null || stagesArr.size != SpecConstants.STAGES

        var stage0PathKeyViolated = false
        var w17PathViolated = false
        val constantViolated = kindStr != SpecConstants.KIND ||
            fillStr != SpecConstants.FILL ||
            slotBytesVal != profile.slotBytes.toULong() ||
            nSlotsVal != profile.nSlots.toULong() ||
            sampleBytesVal != profile.sampleBytes.toULong() ||
            stepsPerLegVal != profile.stepsPerLeg.toULong()

        var legCountViolated = false
        var checkpointWidthViolated = false
        var legResultWidthViolated = false

        data class ExtractedStage(
            val present: Boolean,
            val beacon: ByteArray?, // 32B if valid
            val beaconHeight: Long?,
            val waypoint: ByteArray?, // 32B if valid
            val path: ByteArray?,
            val checkpoints: List<ByteArray>?, // non-null if array of 32B byte strings
            val rawCheckpoints: List<CborValue>?,
            val rawLegResults: List<CborValue>?
        )

        // Inspect all present stage maps (even if stages.size > 18) for field set / height_range
        if (stagesArr != null) {
            for ((idx, item) in stagesArr.withIndex()) {
                val sMap = item as? CborValue.MapVal
                if (sMap == null) {
                    fieldSetViolated = true
                    continue
                }
                val sEntries = sMap.entries
                for (sv in sEntries.values) {
                    if (sv is CborValue.UIntVal && sv.value > MAX_HEIGHT_ULONG) {
                        heightRangeViolated = true
                    }
                }
                if (idx == 0) {
                    if (sEntries.containsKey("path")) {
                        stage0PathKeyViolated = true
                        fieldSetViolated = true
                    }
                    if (sEntries.keys != STAGE_0_KEYS) {
                        fieldSetViolated = true
                    }
                } else {
                    if (sEntries.keys != STAGE_N_KEYS) {
                        fieldSetViolated = true
                    }
                    val pVal = sEntries["path"]
                    if (pVal !is CborValue.BytesVal) {
                        fieldSetViolated = true
                    }
                }
                val bVal = (sEntries["beacon"] as? CborValue.BytesVal)?.bytes
                val wVal = (sEntries["waypoint"] as? CborValue.BytesVal)?.bytes
                val bhVal = sEntries["beacon_height"]
                val lcVal = sEntries["leg_count"]
                val cpVal = (sEntries["checkpoints"] as? CborValue.ArrayVal)?.items
                val lrVal = (sEntries["leg_results"] as? CborValue.ArrayVal)?.items
                if (bVal == null || bVal.size != 32 ||
                    wVal == null || wVal.size != 32 ||
                    bhVal !is CborValue.UIntVal ||
                    lcVal !is CborValue.UIntVal ||
                    cpVal == null || cpVal.any { it !is CborValue.BytesVal } ||
                    lrVal == null || lrVal.any { it !is CborValue.BytesVal }
                ) {
                    fieldSetViolated = true
                }
            }
        }

        val extractedStages = ArrayList<ExtractedStage>(SpecConstants.STAGES)
        for (g in 0 until SpecConstants.STAGES) {
            val sMap = if (stagesArr != null && g < stagesArr.size) stagesArr[g] as? CborValue.MapVal else null
            if (sMap == null) {
                legCountViolated = true
                checkpointWidthViolated = true
                legResultWidthViolated = true
                if (g == 17) w17PathViolated = true
                extractedStages.add(
                    ExtractedStage(false, null, null, null, null, null, null, null)
                )
                continue
            }
            val sEntries = sMap.entries
            val bBytes = (sEntries["beacon"] as? CborValue.BytesVal)?.bytes?.takeIf { it.size == 32 }
            val bhULong = checkUIntRange(sEntries["beacon_height"])
            val lcULong = checkUIntRange(sEntries["leg_count"])
            val wBytes = (sEntries["waypoint"] as? CborValue.BytesVal)?.bytes?.takeIf { it.size == 32 }
            val pBytes = if (g >= 1) (sEntries["path"] as? CborValue.BytesVal)?.bytes else null

            if (g == 17) {
                if (pBytes == null || pBytes.isNotEmpty()) {
                    w17PathViolated = true
                }
            }

            val cpItems = (sEntries["checkpoints"] as? CborValue.ArrayVal)?.items
            val lrItems = (sEntries["leg_results"] as? CborValue.ArrayVal)?.items

            if (lcULong == null || cpItems == null || lrItems == null ||
                lcULong < 1uL ||
                lcULong != cpItems.size.toULong() ||
                lrItems.size != cpItems.size
            ) {
                legCountViolated = true
            }

            var cpAll32 = true
            val validCps = ArrayList<ByteArray>()
            if (cpItems == null) {
                checkpointWidthViolated = true
                cpAll32 = false
            } else {
                for (item in cpItems) {
                    val b = (item as? CborValue.BytesVal)?.bytes
                    if (b == null || b.size != 32) {
                        checkpointWidthViolated = true
                        cpAll32 = false
                    } else {
                        validCps.add(b)
                    }
                }
            }

            if (lrItems == null) {
                legResultWidthViolated = true
            } else {
                for (item in lrItems) {
                    val b = (item as? CborValue.BytesVal)?.bytes
                    if (b == null || b.size != 8) {
                        legResultWidthViolated = true
                    }
                }
            }

            extractedStages.add(
                ExtractedStage(
                    present = true,
                    beacon = bBytes,
                    beaconHeight = bhULong?.toLong(),
                    waypoint = wBytes,
                    path = pBytes,
                    checkpoints = if (cpAll32) validCps else null,
                    rawCheckpoints = cpItems,
                    rawLegResults = lrItems
                )
            )
        }

        val s0Beacon = extractedStages[0].beacon
        val seedBeaconViolated = s0Beacon == null || seedBeaconBytes == null || seedBeaconBytes.size != 32 ||
            !s0Beacon.contentEquals(seedBeaconBytes)

        val s17Beacon = extractedStages[17].beacon
        val s17Height = extractedStages[17].beaconHeight
        val validLastInterruptBytes = lastInterruptBytes?.takeIf { it.size == 32 }
        val validLastInterruptHeight = lastInterruptHeightVal?.toLong()

        val lastInterruptViolated = validLastInterruptBytes == null ||
            s17Beacon == null ||
            validLastInterruptBytes.contentEquals(s17Beacon) ||
            validLastInterruptHeight == null ||
            s17Height == null ||
            validLastInterruptHeight <= s17Height

        // 19 headers in order: stages[0..17].beacon and last_interrupt
        val headerHashes = Array<ByteArray?>(19) { k ->
            if (k < 18) extractedStages[k].beacon else validLastInterruptBytes
        }
        val headerHeights = Array<Long?>(19) { k ->
            if (k < 18) extractedStages[k].beaconHeight else validLastInterruptHeight
        }

        var headerHeightOrderViolated = false
        var headerHashDifferViolated = false
        for (k in 1..18) {
            val hPrev = headerHeights[k - 1]
            val hCurr = headerHeights[k]
            if (hPrev == null || hCurr == null || hCurr <= hPrev) {
                headerHeightOrderViolated = true
            }
            val bPrev = headerHashes[k - 1]
            val bCurr = headerHashes[k]
            if (bPrev == null || bCurr == null || bCurr.contentEquals(bPrev)) {
                headerHashDifferViolated = true
            }
        }

        val shapeItems = ArrayList<String>()
        if (fieldSetViolated) shapeItems.add("field set")
        if (stage0PathKeyViolated) shapeItems.add("stage-0 path key")
        if (w17PathViolated) shapeItems.add("W_17 path")
        if (constantViolated) shapeItems.add("constant")
        if (stagesLengthViolated) shapeItems.add("stages length")
        if (legCountViolated) shapeItems.add("leg_count")
        if (checkpointWidthViolated) shapeItems.add("checkpoint width")
        if (legResultWidthViolated) shapeItems.add("leg result width")
        if (seedBeaconViolated) shapeItems.add("seed beacon")
        if (lastInterruptViolated) shapeItems.add("last_interrupt")
        if (headerHeightOrderViolated) shapeItems.add("header height order")
        if (headerHashDifferViolated) shapeItems.add("header hash differ")
        if (heightRangeViolated) shapeItems.add("height_range")

        val shapeToken = if (shapeItems.isEmpty()) "match" else "mismatch"

        // Section 14.2: Headers (19 GETs in order)
        val headerRecords = ArrayList<HeaderRecord>(19)
        for (k in 0..18) {
            val hBytes = headerHashes[k]
            val hHex = hBytes?.let { Encoding.toHex(it) }
            val hHeight = headerHeights[k]
            if (hHeight == null) {
                headerRecords.add(HeaderRecord(hHex, hBytes, null, "not read", null, null))
            } else {
                val rawDoc = indexSource.getIndexDocument(hHeight)
                val parsedDoc = parseHeaderFromDoc(rawDoc?.let { StrictJson.parseObjectBytes(it) })
                if (hBytes == null || parsedDoc == null) {
                    headerRecords.add(HeaderRecord(hHex, hBytes, hHeight, "not read", null, null))
                } else if (parsedDoc.height == hHeight && parsedDoc.hashNatural.contentEquals(hBytes)) {
                    headerRecords.add(
                        HeaderRecord(hHex, hBytes, hHeight, "match", parsedDoc.height, parsedDoc.time)
                    )
                } else {
                    headerRecords.add(HeaderRecord(hHex, hBytes, hHeight, "mismatch", null, null))
                }
            }
        }

        // Section 14.3: Rule A4.3 Chain (seed, terrain, checkpoints, waypoints)
        val validPk = publicKeyBytes?.takeIf { it.size == 32 }
        val validSeedBeacon = seedBeaconBytes?.takeIf { it.size == 32 }
        val computedSeed = if (validPk != null && validSeedBeacon != null) {
            Construction.computeSeed(validPk, validSeedBeacon)
        } else {
            null
        }
        val terrain = if (computedSeed != null) {
            Construction.fillTerrain(profile, computedSeed)
        } else {
            null
        }

        val s0Waypoint = extractedStages[0].waypoint
        val seedValueToken = if (computedSeed != null && s0Waypoint != null) {
            if (s0Waypoint.contentEquals(computedSeed)) "match" else "mismatch"
        } else {
            "not defined"
        }

        // Checkpoints: 18 stage objects
        val checkpointsStageResults = Array<List<String>?>(18) { null }
        for (g in 0..17) {
            val st = extractedStages[g]
            val stageLimb = if (g == 0) computedSeed else st.waypoint
            val cps = st.checkpoints
            val rawLrs = st.rawLegResults
            val lrsAllValidOrMissing = rawLrs != null && rawLrs.all {
                it is CborValue.BytesVal && it.bytes.size == 8
            }
            if (terrain != null && stageLimb != null && cps != null && rawLrs != null && lrsAllValidOrMissing) {
                val legTokens = ArrayList<String>(cps.size)
                for (j in cps.indices) {
                    val prev = if (j == 0) stageLimb else cps[j - 1]
                    val lrItem = if (j < rawLrs.size) {
                        (rawLrs[j] as? CborValue.BytesVal)?.bytes?.takeIf { it.size == 8 }
                    } else {
                        null
                    }
                    if (lrItem == null) {
                        legTokens.add("mismatch")
                    } else {
                        val sample = terrain.terrainSample(prev)
                        val expectedCp = Construction.computeCheckpoint(prev, stageLimb, sample, lrItem)
                        legTokens.add(if (cps[j].contentEquals(expectedCp)) "match" else "mismatch")
                    }
                }
                checkpointsStageResults[g] = legTokens
            } else {
                checkpointsStageResults[g] = null // "not defined"
            }
        }

        // Waypoints: 17 objects for g = 1..17
        val waypointResults = Array(17) { "not defined" }
        for (g in 0..16) {
            val stG = extractedStages[g]
            val stNext = extractedStages[g + 1]
            val bInterrupt = stNext.beacon
            val wCurrent = stG.waypoint
            val cpsG = stG.checkpoints
            val wTarget = stNext.waypoint
            if (terrain != null && bInterrupt != null && wCurrent != null && cpsG != null && wTarget != null) {
                val recomputedW = Construction.computeWaypoint(bInterrupt, terrain, wCurrent, cpsG)
                waypointResults[g] = if (recomputedW.contentEquals(wTarget)) "match" else "mismatch"
            }
        }

        // Section 14.3 Rule A4.5: Paths (g = 1..17)
        val pathRecords1To16 = ArrayList<PathRecord1To16>(16)
        val anchors = Array(16) { "not anchored" }
        val skipList = ArrayList<Int>()
        val issuedPlantRawDocs = HashMap<Long, ByteArray?>()

        for (g in 1..16) {
            val st = extractedStages[g]
            val pBytes = st.path
            if (pBytes == null) {
                pathRecords1To16.add(
                    PathRecord1To16(g, "not defined", "not defined", null, null, null, null, false, false, false)
                )
                continue
            }
            if (pBytes.isEmpty()) {
                skipList.add(g)
                pathRecords1To16.add(
                    PathRecord1To16(g, "skip", "n/a", null, null, null, null, false, false, false)
                )
                continue
            }

            // Non-empty path bytes -> present
            val decodedBody = CanonicalPathBody.decode(pBytes)
            val stWaypoint = st.waypoint
            if (decodedBody == null || stWaypoint == null || !decodedBody.validateFor(stWaypoint)) {
                pathRecords1To16.add(
                    PathRecord1To16(g, "present", "mismatch", null, null, null, null, false, false, false)
                )
                continue
            }

            val claimed = BlockRef(
                blockHashDisplay = Encoding.toHex(Encoding.reverseBytes32(decodedBody.blockHash)),
                blockHeight = decodedBody.blockHeight
            )

            // Check window (H + 2, 3, 4) and stored interrupt header check (headers[1..18])
            val stageH = st.beaconHeight
            val inWindow = stageH != null &&
                (decodedBody.blockHeight - stageH) in listOf(2L, 3L, 4L)
            var matchesStoredInterrupt = false
            if (inWindow) {
                for (k in 1..18) {
                    val hRec = headerRecords[k]
                    if (hRec.storedHeight == decodedBody.blockHeight &&
                        hRec.storedHashBytes != null &&
                        hRec.storedHashBytes.contentEquals(decodedBody.blockHash)
                    ) {
                        matchesStoredInterrupt = true
                        break
                    }
                }
            }

            if (!inWindow || !matchesStoredInterrupt) {
                // Plant GET is not issued
                pathRecords1To16.add(
                    PathRecord1To16(
                        g = g,
                        pathState = "present",
                        plant = "not read",
                        pg = null,
                        heightPg = null,
                        dg = null,
                        claimedBlock = claimed,
                        isValidPath = true,
                        issuedPlantGet = false,
                        plantGetReturnedDoc = false
                    )
                )
                continue
            }

            // Issue plant GET for body.blockHeight
            val rawPlantDoc = indexSource.getIndexDocument(decodedBody.blockHeight)
            issuedPlantRawDocs[decodedBody.blockHeight] = rawPlantDoc
            val plantDocObj = rawPlantDoc?.let { StrictJson.parseObjectBytes(it) }
            if (plantDocObj == null) {
                pathRecords1To16.add(
                    PathRecord1To16(
                        g = g,
                        pathState = "present",
                        plant = "not read",
                        pg = null,
                        heightPg = null,
                        dg = null,
                        claimedBlock = claimed,
                        isValidPath = true,
                        issuedPlantGet = true,
                        plantGetReturnedDoc = false
                    )
                )
                continue
            }

            val plantMatches = checkPlantDocumentMatch(plantDocObj, decodedBody)
            if (plantMatches) {
                anchors[g - 1] = "anchored"
                val hdrG = headerRecords[g]
                val dgVal = if (hdrG.indexComparison == "match" && hdrG.confirmedHeight != null) {
                    decodedBody.blockHeight - hdrG.confirmedHeight
                } else {
                    null
                }
                pathRecords1To16.add(
                    PathRecord1To16(
                        g = g,
                        pathState = "present",
                        plant = "match",
                        pg = claimed,
                        heightPg = decodedBody.blockHeight,
                        dg = dgVal,
                        claimedBlock = claimed,
                        isValidPath = true,
                        issuedPlantGet = true,
                        plantGetReturnedDoc = true
                    )
                )
            } else {
                pathRecords1To16.add(
                    PathRecord1To16(
                        g = g,
                        pathState = "present",
                        plant = "mismatch",
                        pg = null,
                        heightPg = null,
                        dg = null,
                        claimedBlock = claimed,
                        isValidPath = true,
                        issuedPlantGet = true,
                        plantGetReturnedDoc = true
                    )
                )
            }
        }

        // Stage 17 path record
        val st17Path = extractedStages[17].path
        val w17PathToken = if (st17Path != null && st17Path.isEmpty()) "not fetched by design" else "not defined"
        val w17ClaimedBlock = if (st17Path != null && st17Path.isNotEmpty()) {
            CanonicalPathBody.extractClaimedBlockLayout(st17Path)?.let { (hash, height) ->
                BlockRef(hash, height)
            }
        } else {
            null
        }

        val anchoredTotal = anchors.count { it == "anchored" }

        // Section 14.4: Bookends and Rule A4.11
        val validPathCount = pathRecords1To16.count { it.isValidPath }
        val anyIssuedPlantGetMissingDoc = pathRecords1To16.any { it.issuedPlantGet && !it.plantGetReturnedDoc }

        var aField: Any = "not defined"
        var bField: Any = "not defined"
        var nField: Any = "not defined"
        var sField: Any = "not defined"
        var deltaHField: Any = "not defined"
        var fixedStartN: Any = "not defined"
        var fixedStartS: Any = "not defined"
        var fixedStartDeltaH: Any = "not defined"
        var pbField: Any = "not defined"
        var heightPbField: Any = "not defined"
        var indexCompPbField: String = "not defined"
        var tPbField: Any = "not defined"
        var heightIaMinus1Field: Any = "not defined"
        var tIaMinus1Field: Any = "not defined"
        var legsOutsideField: Any = "not defined"

        fun setAllBookends(token: String) {
            aField = token
            bField = token
            nField = token
            sField = token
            deltaHField = token
            fixedStartN = token
            fixedStartS = token
            fixedStartDeltaH = token
            pbField = token
            heightPbField = token
            indexCompPbField = token
            tPbField = token
            heightIaMinus1Field = token
            tIaMinus1Field = token
            legsOutsideField = token
        }

        if (validPathCount < 2) {
            // Rule A4.11 step 1
            setAllBookends("not defined")
        } else if (anyIssuedPlantGetMissingDoc) {
            // Rule A4.11 step 2
            setAllBookends("not read")
        } else if (anchoredTotal < 2) {
            // Rule A4.11 step 3
            setAllBookends("not defined")
        } else {
            // Rule A4.11 step 4
            val anchoredIndices = (1..16).filter { anchors[it - 1] == "anchored" }
            val a = anchoredIndices.first()
            val b = anchoredIndices.last()
            val wbRec = pathRecords1To16[b - 1]
            val pbBlock = wbRec.pg

            // Reuse section 14.3 document if already read for P_b.block_height
            val rawPbDoc = if (pbBlock != null) {
                if (issuedPlantRawDocs.containsKey(pbBlock.blockHeight)) {
                    issuedPlantRawDocs[pbBlock.blockHeight]
                } else {
                    indexSource.getIndexDocument(pbBlock.blockHeight)
                }
            } else {
                null
            }
            val parsedPbHeader = parseHeaderFromDoc(rawPbDoc?.let { StrictJson.parseObjectBytes(it) })
            val pbComp = if (pbBlock == null || parsedPbHeader == null) {
                "not read"
            } else {
                val pbDisplayBytes = Encoding.fromHex(pbBlock.blockHashDisplay)
                if (parsedPbHeader.height == pbBlock.blockHeight &&
                    parsedPbHeader.hashDisplayBytes.contentEquals(pbDisplayBytes)
                ) {
                    "match"
                } else {
                    "mismatch"
                }
            }

            val hdrA = headerRecords[a]
            if (hdrA.indexComparison == "not read" || pbBlock == null || pbComp == "not read") {
                setAllBookends("not read")
            } else {
                aField = a.toLong()
                bField = b.toLong()

                // N: checkpoints in stages a .. b-1
                val canCountN = (a until b).all { extractedStages[it].rawCheckpoints != null }
                val nVal = if (canCountN) {
                    (a until b).sumOf { extractedStages[it].rawCheckpoints!!.size.toLong() }
                } else {
                    null
                }
                nField = nVal ?: "not defined"

                pbField = pbBlock
                indexCompPbField = pbComp
                val confirmedHeightPb = if (pbComp == "match") parsedPbHeader!!.height else null
                val confirmedTPb = if (pbComp == "match") parsedPbHeader!!.time else null
                heightPbField = confirmedHeightPb ?: "not defined"
                tPbField = confirmedTPb ?: "not read"

                val confirmedHeightIa = if (hdrA.indexComparison == "match") hdrA.confirmedHeight else null
                val confirmedTIa = if (hdrA.indexComparison == "match") hdrA.tValue else null
                heightIaMinus1Field = confirmedHeightIa ?: "not defined"
                tIaMinus1Field = confirmedTIa ?: "not read"

                sField = if (confirmedTPb != null && confirmedTIa != null) {
                    confirmedTPb - confirmedTIa
                } else {
                    "not read"
                }
                deltaHField = if (confirmedHeightPb != null && confirmedHeightIa != null) {
                    confirmedHeightPb - confirmedHeightIa
                } else {
                    "not defined"
                }

                // fixed_start: stages 1 .. b-1
                val canCountFixedN = (1 until b).all { extractedStages[it].rawCheckpoints != null }
                fixedStartN = if (canCountFixedN) {
                    (1 until b).sumOf { extractedStages[it].rawCheckpoints!!.size.toLong() }
                } else {
                    "not defined"
                }
                val hdr1 = headerRecords[1]
                fixedStartS = if (confirmedTPb != null && hdr1.tValue != null) {
                    confirmedTPb - hdr1.tValue
                } else {
                    "not read"
                }
                fixedStartDeltaH = when {
                    hdr1.indexComparison == "not read" || pbComp == "not read" -> "not read"
                    confirmedHeightPb != null && hdr1.confirmedHeight != null -> {
                        confirmedHeightPb - hdr1.confirmedHeight
                    }
                    else -> "not defined"
                }

                val canCountAll = (0..17).all { extractedStages[it].rawCheckpoints != null }
                legsOutsideField = if (canCountAll && nVal != null) {
                    val total = (0..17).sumOf { extractedStages[it].rawCheckpoints!!.size.toLong() }
                    total - nVal
                } else {
                    "not defined"
                }
            }
        }

        // Section 14.6: Six-leg check
        val allStagesHaveCheckpoints = (0..17).all { extractedStages[it].rawCheckpoints != null }
        val legTotal = if (allStagesHaveCheckpoints) {
            (0..17).sumOf { extractedStages[it].rawCheckpoints!!.size }
        } else {
            0
        }

        var sixLegJson: String = "\"not defined\""
        var sixLegRunsJson: String = "[]"

        if (allStagesHaveCheckpoints && legTotal >= 6) {
            val drawnIndices = Construction.drawSixLegIndices(legTotal, drawReader)
            if (drawnIndices == null) {
                sixLegJson = "\"not read\""
                sixLegRunsJson = "\"not read\""
            } else {
                // Map global leg index q to (stage g, local leg j)
                val stageStarts = IntArray(19)
                for (g in 0..17) {
                    stageStarts[g + 1] = stageStarts[g] + extractedStages[g].rawCheckpoints!!.size
                }
                val results = Array(6) { "mismatch" }
                IntStream.range(0, 6).parallel().forEach { idxPos ->
                    val q = drawnIndices[idxPos]
                    var g = 0
                    while (g < 17 && q >= stageStarts[g + 1]) {
                        g++
                    }
                    val j = q - stageStarts[g]
                    val st = extractedStages[g]
                    val prev = if (j == 0) {
                        if (g == 0) computedSeed else st.waypoint
                    } else {
                        (st.rawCheckpoints!![j - 1] as? CborValue.BytesVal)?.bytes?.takeIf { it.size == 32 }
                    }
                    val storedLr = if (st.rawLegResults != null && j < st.rawLegResults.size) {
                        (st.rawLegResults[j] as? CborValue.BytesVal)?.bytes?.takeIf { it.size == 8 }
                    } else {
                        null
                    }
                    if (terrain != null && prev != null && storedLr != null) {
                        val recomputed = Construction.legResult(terrain, prev)
                        val storedU64 = Encoding.u64Le(storedLr, 0)
                        results[idxPos] = if (recomputed == storedU64) "match" else "mismatch"
                    } else {
                        results[idxPos] = "mismatch"
                    }
                }
                val indicesStr = drawnIndices.joinToString(",")
                val resultsStr = results.joinToString(",") { "\"$it\"" }
                sixLegJson = """{"indices":[$indicesStr],"results":[$resultsStr]}"""
                sixLegRunsJson = "[$sixLegJson]"
            }
        }

        // Section 14.4 Rule A4.10: Analysis
        val wholeHikeLegs: Any = if ((1..17).all { extractedStages[it].rawCheckpoints != null }) {
            (1..17).sumOf { extractedStages[it].rawCheckpoints!!.size.toLong() }
        } else {
            "not defined"
        }
        val wholeHikeSpan: Any = if (headerRecords[18].tValue != null && headerRecords[1].tValue != null) {
            headerRecords[18].tValue!! - headerRecords[1].tValue!!
        } else {
            "not read"
        }
        val hikeLegs: Any = if (allStagesHaveCheckpoints) {
            legTotal.toLong()
        } else {
            "not defined"
        }

        val mtpFirst = computeMtp(headerRecords[0], indexSource)
        val mtpLast = computeMtp(headerRecords[18], indexSource)
        val hikeSpan: Any = if (mtpFirst != null && mtpLast != null) {
            mtpLast - mtpFirst
        } else {
            "not read"
        }

        // Build final JSON string with exact key ordering
        val sb = StringBuilder(4096)
        sb.append('{')
        sb.append("\"profile\":\"").append(profile.name).append('"')
        if (profile.name == "conformance-small") {
            sb.append(",\"sample_bytes\":").append(profile.sampleBytes)
            sb.append(",\"slot_bytes\":").append(profile.slotBytes)
            sb.append(",\"terrain_bytes\":").append(profile.terrainBytes)
            sb.append(",\"steps_per_leg\":").append(profile.stepsPerLeg)
        }
        sb.append(",\"shape\":\"").append(shapeToken).append('"')
        sb.append(",\"shape_item\":[")
        for (i in shapeItems.indices) {
            if (i > 0) sb.append(',')
            sb.append('"').append(shapeItems[i]).append('"')
        }
        sb.append(']')

        // headers
        sb.append(",\"headers\":[")
        for (k in 0..18) {
            if (k > 0) sb.append(',')
            val hr = headerRecords[k]
            sb.append('{')
            sb.append("\"stored_hash\":")
            appendJsonScalar(sb, hr.storedHashHex ?: "not defined")
            sb.append(",\"stored_height\":")
            appendJsonScalar(sb, hr.storedHeight ?: "not defined")
            sb.append(",\"index_comparison\":\"").append(hr.indexComparison).append('"')
            sb.append(",\"T\":")
            appendJsonScalar(sb, hr.tValue ?: "not read")
            sb.append('}')
        }
        sb.append(']')

        // seed_value
        sb.append(",\"seed_value\":\"").append(seedValueToken).append('"')

        // checkpoints
        sb.append(",\"checkpoints\":[")
        for (g in 0..17) {
            if (g > 0) sb.append(',')
            sb.append("{\"g\":").append(g).append(",\"legs\":")
            val legs = checkpointsStageResults[g]
            if (legs == null) {
                sb.append("\"not defined\"")
            } else {
                sb.append('[')
                for (j in legs.indices) {
                    if (j > 0) sb.append(',')
                    sb.append("{\"j\":").append(j).append(",\"result\":\"").append(legs[j]).append("\"}")
                }
                sb.append(']')
            }
            sb.append('}')
        }
        sb.append(']')

        // waypoints
        sb.append(",\"waypoints\":[")
        for (idx in 0..16) {
            if (idx > 0) sb.append(',')
            val g = idx + 1
            sb.append("{\"g\":").append(g).append(",\"result\":\"").append(waypointResults[idx]).append("\"}")
        }
        sb.append(']')

        // paths
        sb.append(",\"paths\":[")
        for (idx in 0..15) {
            if (idx > 0) sb.append(',')
            val pr = pathRecords1To16[idx]
            sb.append("{\"g\":").append(pr.g)
            sb.append(",\"path_state\":\"").append(pr.pathState).append('"')
            sb.append(",\"plant\":\"").append(pr.plant).append('"')
            sb.append(",\"P_g\":")
            appendBlockRefOrToken(sb, pr.pg)
            sb.append(",\"height_P_g\":")
            appendJsonScalar(sb, pr.heightPg ?: "not defined")
            sb.append(",\"d_g\":")
            appendJsonScalar(sb, pr.dg ?: "not defined")
            sb.append(",\"claimed_block\":")
            appendBlockRefOrToken(sb, pr.claimedBlock)
            sb.append('}')
        }
        sb.append(",{\"g\":17,\"w17_path\":\"").append(w17PathToken).append("\",\"claimed_block\":")
        appendBlockRefOrToken(sb, w17ClaimedBlock)
        sb.append("}]")

        // anchors
        sb.append(",\"anchors\":[")
        for (idx in 0..15) {
            if (idx > 0) sb.append(',')
            sb.append('"').append(anchors[idx]).append('"')
        }
        sb.append(']')

        // anchored_total & skip_list
        sb.append(",\"anchored_total\":").append(anchoredTotal)
        sb.append(",\"skip_list\":[").append(skipList.joinToString(",")).append(']')

        // bookends
        sb.append(",\"a\":"); appendJsonScalar(sb, aField)
        sb.append(",\"b\":"); appendJsonScalar(sb, bField)
        sb.append(",\"N\":"); appendJsonScalar(sb, nField)
        sb.append(",\"S\":"); appendJsonScalar(sb, sField)
        sb.append(",\"delta_h\":"); appendJsonScalar(sb, deltaHField)

        sb.append(",\"fixed_start\":{\"N\":")
        appendJsonScalar(sb, fixedStartN)
        sb.append(",\"S\":")
        appendJsonScalar(sb, fixedStartS)
        sb.append(",\"delta_h\":")
        appendJsonScalar(sb, fixedStartDeltaH)
        sb.append('}')

        sb.append(",\"P_b\":")
        if (pbField is BlockRef) {
            appendBlockRefOrToken(sb, pbField as BlockRef)
        } else {
            appendJsonScalar(sb, pbField)
        }
        sb.append(",\"height_P_b\":"); appendJsonScalar(sb, heightPbField)
        sb.append(",\"index_comparison_P_b\":\"").append(indexCompPbField).append('"')
        sb.append(",\"T_P_b\":"); appendJsonScalar(sb, tPbField)
        sb.append(",\"height_I_a_minus_1\":"); appendJsonScalar(sb, heightIaMinus1Field)
        sb.append(",\"T_I_a_minus_1\":"); appendJsonScalar(sb, tIaMinus1Field)

        // six_leg & six_leg_runs
        sb.append(",\"six_leg\":").append(sixLegJson)
        sb.append(",\"six_leg_runs\":").append(sixLegRunsJson)

        // analysis
        sb.append(",\"analysis\":{\"whole_hike_legs\":")
        appendJsonScalar(sb, wholeHikeLegs)
        sb.append(",\"whole_hike_span\":")
        appendJsonScalar(sb, wholeHikeSpan)
        sb.append(",\"per_stage\":[")
        for (g in 0..17) {
            if (g > 0) sb.append(',')
            val cpCount: Any = extractedStages[g].rawCheckpoints?.size?.toLong() ?: "not defined"
            val span: Any = if (headerRecords[g + 1].tValue != null && headerRecords[g].tValue != null) {
                headerRecords[g + 1].tValue!! - headerRecords[g].tValue!!
            } else {
                "not read"
            }
            sb.append("{\"g\":").append(g).append(",\"checkpoint_count\":")
            appendJsonScalar(sb, cpCount)
            sb.append(",\"span\":")
            appendJsonScalar(sb, span)
            sb.append('}')
        }
        sb.append("],\"legs_outside\":")
        appendJsonScalar(sb, legsOutsideField)
        sb.append(",\"hike_legs\":")
        appendJsonScalar(sb, hikeLegs)
        sb.append(",\"hike_span\":")
        appendJsonScalar(sb, hikeSpan)
        sb.append("}}")

        return sb.toString()
    }

    private fun computeMtp(beaconHeader: HeaderRecord, indexSource: IndexSource): Long? {
        val h = beaconHeader.storedHeight ?: return null
        if (beaconHeader.indexComparison != "match" || beaconHeader.tValue == null) return null
        val n = minOf(11L, h + 1L).toInt()
        val times = LongArray(n)
        times[n - 1] = beaconHeader.tValue
        val startH = h - n + 1L
        for (idx in 0 until n - 1) {
            val reqH = startH + idx
            val raw = indexSource.getIndexDocument(reqH) ?: return null
            val parsed = parseHeaderFromDoc(StrictJson.parseObjectBytes(raw)) ?: return null
            if (parsed.height != reqH || parsed.time == null) return null
            times[idx] = parsed.time
        }
        times.sort()
        return times[n / 2]
    }

    private fun checkPlantDocumentMatch(docObj: JsonVal.Obj, body: CanonicalPathBody): Boolean {
        val network = (docObj.members["network"] as? JsonVal.Str)?.value ?: return false
        if (network != "main" && network != "synthetic") return false

        val candidatePlants = ArrayList<JsonVal.Obj>()
        val plantsArr = docObj.members["plants"]
        if (plantsArr is JsonVal.Arr) {
            for (item in plantsArr.items) {
                if (item is JsonVal.Obj) candidatePlants.add(item)
            }
        }
        val singlePlant = docObj.members["plant"]
        if (singlePlant is JsonVal.Obj) {
            candidatePlants.add(singlePlant)
        }
        if (candidatePlants.isEmpty()) return false

        for (pObj in candidatePlants) {
            val rootStr = (pObj.members["root"] as? JsonVal.Str)?.value ?: continue
            val txidStr = (pObj.members["txid"] as? JsonVal.Str)?.value ?: continue
            val blockHashStr = (pObj.members["block_hash"] as? JsonVal.Str)?.value ?: continue
            val vout = StrictJson.extractU32(pObj.members["vout"]) ?: continue
            val blockHeight = StrictJson.extractU32(pObj.members["block_height"]) ?: continue

            if (!Encoding.isValidHex64(rootStr) ||
                !Encoding.isValidHex64(txidStr) ||
                !Encoding.isValidHex64(blockHashStr)
            ) {
                continue
            }

            val rootNatural = Encoding.fromHex(rootStr)
            val txidNatural = Encoding.reverseBytes32(Encoding.fromHex(txidStr))
            val blockHashNatural = Encoding.reverseBytes32(Encoding.fromHex(blockHashStr))

            if (rootNatural.contentEquals(body.root) &&
                txidNatural.contentEquals(body.txid) &&
                vout == body.vout &&
                blockHashNatural.contentEquals(body.blockHash) &&
                blockHeight == body.blockHeight
            ) {
                return true
            }
        }
        return false
    }

    private fun buildMalformedCborRecord(profile: Profile): String {
        val sb = StringBuilder(4096)
        sb.append('{')
        sb.append("\"profile\":\"").append(profile.name).append('"')
        if (profile.name == "conformance-small") {
            sb.append(",\"sample_bytes\":").append(profile.sampleBytes)
            sb.append(",\"slot_bytes\":").append(profile.slotBytes)
            sb.append(",\"terrain_bytes\":").append(profile.terrainBytes)
            sb.append(",\"steps_per_leg\":").append(profile.stepsPerLeg)
        }
        sb.append(",\"shape\":\"mismatch\"")
        sb.append(",\"shape_item\":[\"malformed cbor\"]")
        sb.append(",\"headers\":[")
        for (k in 0..18) {
            if (k > 0) sb.append(',')
            sb.append("{\"stored_hash\":\"not defined\",\"stored_height\":\"not defined\",\"index_comparison\":\"not read\",\"T\":\"not read\"}")
        }
        sb.append(']')
        sb.append(",\"seed_value\":\"not defined\"")
        sb.append(",\"checkpoints\":[")
        for (g in 0..17) {
            if (g > 0) sb.append(',')
            sb.append("{\"g\":").append(g).append(",\"legs\":\"not defined\"}")
        }
        sb.append(']')
        sb.append(",\"waypoints\":[")
        for (g in 1..17) {
            if (g > 1) sb.append(',')
            sb.append("{\"g\":").append(g).append(",\"result\":\"not defined\"}")
        }
        sb.append(']')
        sb.append(",\"paths\":[")
        for (g in 1..16) {
            if (g > 1) sb.append(',')
            sb.append("{\"g\":").append(g).append(",\"path_state\":\"not defined\",\"plant\":\"not defined\",\"P_g\":\"not defined\",\"height_P_g\":\"not defined\",\"d_g\":\"not defined\",\"claimed_block\":\"not defined\"}")
        }
        sb.append(",{\"g\":17,\"w17_path\":\"not defined\",\"claimed_block\":\"not defined\"}]")
        sb.append(",\"anchors\":[")
        for (g in 1..16) {
            if (g > 1) sb.append(',')
            sb.append("\"not anchored\"")
        }
        sb.append(']')
        sb.append(",\"anchored_total\":0,\"skip_list\":[]")
        sb.append(",\"a\":\"not defined\",\"b\":\"not defined\",\"N\":\"not defined\",\"S\":\"not defined\",\"delta_h\":\"not defined\"")
        sb.append(",\"fixed_start\":{\"N\":\"not defined\",\"S\":\"not defined\",\"delta_h\":\"not defined\"}")
        sb.append(",\"P_b\":\"not defined\",\"height_P_b\":\"not defined\",\"index_comparison_P_b\":\"not defined\",\"T_P_b\":\"not defined\",\"height_I_a_minus_1\":\"not defined\",\"T_I_a_minus_1\":\"not defined\"")
        sb.append(",\"six_leg\":\"not defined\",\"six_leg_runs\":[]")
        sb.append(",\"analysis\":{\"whole_hike_legs\":\"not defined\",\"whole_hike_span\":\"not read\",\"per_stage\":[")
        for (g in 0..17) {
            if (g > 0) sb.append(',')
            sb.append("{\"g\":").append(g).append(",\"checkpoint_count\":\"not defined\",\"span\":\"not read\"}")
        }
        sb.append("],\"legs_outside\":\"not defined\",\"hike_legs\":\"not defined\",\"hike_span\":\"not read\"}}")
        return sb.toString()
    }

    private fun appendJsonScalar(sb: StringBuilder, value: Any) {
        when (value) {
            is Number -> sb.append(value.toString())
            is String -> sb.append(CborCodec.escapeJcsString(value))
            else -> sb.append(CborCodec.escapeJcsString(value.toString()))
        }
    }

    private fun appendBlockRefOrToken(sb: StringBuilder, obj: BlockRef?) {
        if (obj == null) {
            sb.append("\"not defined\"")
            return
        }
        sb.append("{\"block_hash\":\"").append(obj.blockHashDisplay).append("\",\"block_height\":").append(obj.blockHeight).append('}')
    }
}
