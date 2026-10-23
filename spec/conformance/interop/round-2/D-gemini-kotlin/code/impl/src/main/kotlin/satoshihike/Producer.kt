package satoshihike

/**
 * Offline and asynchronous test/live plane producer (SPEC section 13).
 */
object Producer {

    data class ProducedHike(
        val terrain: Terrain,
        val hikeObject: HikeObject,
        val cborBytes: ByteArray,
        val stderrLines: List<String>
    )

    /**
     * Produces a HikeObject for a given profile, public key, beacons, and legs_per_stage schedule (SPEC section 13.2).
     */
    fun produceOffline(
        profile: Profile,
        publicKey: ByteArray = CasesFixtures.PK_A,
        seedBeacon: ByteArray = CasesFixtures.SEED_BEACON,
        seedBeaconHeight: ULong = CasesFixtures.SEED_BEACON_HEIGHT.toULong(),
        interrupts: List<ByteArray> = CasesFixtures.INTERRUPTS,
        interruptHeights: List<ULong> = List(18) { k -> CasesFixtures.interruptHeight(k).toULong() },
        legsPerStage: IntArray = IntArray(18) { 1 },
        pathsByStage1To16: Map<Int, ByteArray> = emptyMap(),
        precomputedTerrain: Terrain? = null,
        emitStderr: Boolean = false
    ): ProducedHike {
        require(interrupts.size == SpecConstants.STAGES)
        require(interruptHeights.size == SpecConstants.STAGES)
        require(legsPerStage.size == SpecConstants.STAGES)

        val seed = Construction.computeSeed(publicKey, seedBeacon)
        val terrain = precomputedTerrain ?: Construction.fillTerrain(profile, seed)

        var waypoint = seed
        var beacon = seedBeacon
        var beaconHeight = seedBeaconHeight

        val stages = ArrayList<StageRecord>(SpecConstants.STAGES)

        for (g in 0 until SpecConstants.STAGES) {
            var previous = waypoint
            val numLegs = legsPerStage[g]
            require(numLegs >= 1) { "leg_count must be >= 1" }
            val checkpoints = ArrayList<ByteArray>(numLegs)
            val legResults = ArrayList<ByteArray>(numLegs)

            for (j in 0 until numLegs) {
                val lr = Construction.legResult(terrain, previous)
                val lrBytes = Encoding.le64(lr)
                val sample = terrain.terrainSample(previous)
                val cp = Construction.computeCheckpoint(previous, waypoint, sample, lrBytes)
                checkpoints.add(cp)
                legResults.add(lrBytes)
                previous = cp
            }

            val stagePath: ByteArray? = when {
                g == 0 -> null
                g in 1..16 -> pathsByStage1To16[g] ?: ByteArray(0)
                else -> ByteArray(0) // stage 17 path is always empty ("not fetched by design")
            }

            stages.add(
                StageRecord(
                    stageIndex = g,
                    beacon = beacon,
                    beaconHeight = beaconHeight,
                    waypoint = waypoint,
                    path = stagePath,
                    legCount = checkpoints.size.toULong(),
                    checkpoints = checkpoints,
                    legResults = legResults
                )
            )

            val interrupt = interrupts[g]
            val interruptHeight = interruptHeights[g]
            if (g < 17) {
                waypoint = Construction.computeWaypoint(interrupt, terrain, waypoint, checkpoints)
                beacon = interrupt
                beaconHeight = interruptHeight
            }
        }

        val lastInterrupt = interrupts[17]
        val lastInterruptHeight = interruptHeights[17]

        val hikeObject = HikeObject(
            kind = SpecConstants.KIND,
            fill = SpecConstants.FILL,
            slotBytes = profile.slotBytes.toULong(),
            nSlots = profile.nSlots.toULong(),
            sampleBytes = profile.sampleBytes.toULong(),
            stepsPerLeg = profile.stepsPerLeg.toULong(),
            publicKey = publicKey,
            seedBeacon = seedBeacon,
            stages = stages,
            lastInterrupt = lastInterrupt,
            lastInterruptHeight = lastInterruptHeight
        )

        val cborBytes = CborCodec.encode(hikeObject)

        // Rule A3.4: stderr step at close
        val stderrLines = ArrayList<String>(17)
        for (g in 1..16) {
            val p = stages[g].path
            if (p == null || p.isEmpty()) {
                stderrLines.add("path_skip $g")
            }
        }
        stderrLines.add("not fetched by design 17")

        if (emitStderr) {
            val sb = StringBuilder()
            for (line in stderrLines) {
                sb.append(line).append('\n')
            }
            System.err.print(sb.toString())
        }

        return ProducedHike(
            terrain = terrain,
            hikeObject = hikeObject,
            cborBytes = cborBytes,
            stderrLines = stderrLines
        )
    }
}
