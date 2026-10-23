package satoshihike

import java.io.File
import kotlin.system.exitProcess

/**
 * CLI entrypoint supporting:
 * - `verify [options] <object>` (SPEC section 18)
 * - `gen-objects <output-dir>` (writes O1-S1.cbor, O2-S2.cbor, O3-PROD.cbor)
 * - `benchmark` (measures scalar vs optimised terrain fill and production leg)
 */
fun main(args: Array<String>) {
    if (args.isEmpty()) {
        exitProcess(2)
    }
    when (args[0]) {
        "verify" -> {
            val code = runVerifyCli(args.sliceArray(1 until args.size))
            exitProcess(code)
        }
        "gen-objects" -> {
            val outDir = if (args.size >= 2) File(args[1]) else File("objects")
            generateObjects(outDir)
        }
        "benchmark" -> {
            runBenchmark()
        }
        else -> {
            // Allow invoking directly with flags/file if binary is already named `verify`
            val code = runVerifyCli(args)
            exitProcess(code)
        }
    }
}

/**
 * Runs the section 18 verify CLI and returns the exit code (0, 1, or 2).
 */
fun runVerifyCli(args: Array<String>): Int {
    var profileName: String? = null
    var sampleBytes: Int? = null
    var slotBytes: Int? = null
    var terrainBytes: Int? = null
    var stepsPerLeg: Int? = null
    var indexUrl: String? = null
    var drawLabel: String? = null
    var objectPath: String? = null

    var i = 0
    while (i < args.size) {
        val arg = args[i]
        when {
            arg == "--profile" -> {
                if (i + 1 >= args.size) return 2
                profileName = args[++i]
            }
            arg.startsWith("--profile=") -> {
                profileName = arg.substringAfter("=")
            }
            arg == "--sample-bytes" || arg == "--sample_bytes" -> {
                if (i + 1 >= args.size) return 2
                sampleBytes = args[++i].toIntOrNull() ?: return 2
            }
            arg.startsWith("--sample-bytes=") || arg.startsWith("--sample_bytes=") -> {
                sampleBytes = arg.substringAfter("=").toIntOrNull() ?: return 2
            }
            arg == "--slot-bytes" || arg == "--slot_bytes" -> {
                if (i + 1 >= args.size) return 2
                slotBytes = args[++i].toIntOrNull() ?: return 2
            }
            arg.startsWith("--slot-bytes=") || arg.startsWith("--slot_bytes=") -> {
                slotBytes = arg.substringAfter("=").toIntOrNull() ?: return 2
            }
            arg == "--terrain-bytes" || arg == "--terrain_bytes" -> {
                if (i + 1 >= args.size) return 2
                terrainBytes = args[++i].toIntOrNull() ?: return 2
            }
            arg.startsWith("--terrain-bytes=") || arg.startsWith("--terrain_bytes=") -> {
                terrainBytes = arg.substringAfter("=").toIntOrNull() ?: return 2
            }
            arg == "--steps-per-leg" || arg == "--steps_per_leg" -> {
                if (i + 1 >= args.size) return 2
                stepsPerLeg = args[++i].toIntOrNull() ?: return 2
            }
            arg.startsWith("--steps-per-leg=") || arg.startsWith("--steps_per_leg=") -> {
                stepsPerLeg = arg.substringAfter("=").toIntOrNull() ?: return 2
            }
            arg == "--index-url" || arg == "--index_url" -> {
                if (i + 1 >= args.size) return 2
                indexUrl = args[++i]
            }
            arg.startsWith("--index-url=") || arg.startsWith("--index_url=") -> {
                indexUrl = arg.substringAfter("=")
            }
            arg == "--draw-label" || arg == "--draw_label" -> {
                if (i + 1 >= args.size) return 2
                drawLabel = args[++i]
            }
            arg.startsWith("--draw-label=") || arg.startsWith("--draw_label=") -> {
                drawLabel = arg.substringAfter("=")
            }
            arg.startsWith("-") -> {
                return 2
            }
            else -> {
                if (objectPath != null) return 2
                objectPath = arg
            }
        }
        i++
    }

    val targetPath = objectPath ?: return 2

    val profile: Profile = when (profileName) {
        null, "production", "test", "live" -> {
            if (sampleBytes != null || slotBytes != null || terrainBytes != null || stepsPerLeg != null) {
                return 2
            }
            Profile.PRODUCTION
        }
        "conformance-small" -> {
            val sb = sampleBytes ?: return 2
            val slb = slotBytes ?: return 2
            val tb = terrainBytes ?: return 2
            val spl = stepsPerLeg ?: return 2
            try {
                Profile(
                    name = "conformance-small",
                    slotBytes = slb,
                    sampleBytes = sb,
                    terrainBytes = tb,
                    stepsPerLeg = spl
                )
            } catch (e: IllegalArgumentException) {
                return 2
            }
        }
        else -> return 2
    }

    val file = File(targetPath)
    if (!file.exists() || !file.isFile || !file.canRead()) {
        return 1
    }
    val bytes = try {
        file.readBytes()
    } catch (e: Exception) {
        return 1
    }

    val indexSource: IndexSource = when {
        indexUrl != null -> HttpIndexSource(indexUrl)
        else -> CasesFixtures.SYNTHETIC_INDEX_SOURCE
    }

    val drawReader = if (drawLabel != null) {
        Construction.DrawByteReader(Construction.labelDrawByteSource(drawLabel))
    } else {
        Verifier.csprngDrawReader()
    }

    return when (val res = Verifier.verify(bytes, profile, indexSource, drawReader)) {
        is VerifyResult.DecodeFailure -> 1
        is VerifyResult.Record -> {
            print(res.json + "\n")
            0
        }
    }
}

fun generateObjects(outDir: File) {
    outDir.mkdirs()

    println("Generating O1-S1.cbor (S1, pk_A, 18 x 1)...")
    val o1 = Producer.produceOffline(
        profile = Profile.S1,
        legsPerStage = IntArray(18) { 1 }
    )
    val o1File = File(outDir, "O1-S1.cbor")
    o1File.writeBytes(o1.cborBytes)
    println("Wrote ${o1File.path} (${o1.cborBytes.size} bytes, sha256=${Encoding.toHex(Encoding.sha256(o1.cborBytes))})")

    println("Generating O2-S2.cbor (S2, pk_A, 18 x 3)...")
    val o2 = Producer.produceOffline(
        profile = Profile.S2,
        legsPerStage = IntArray(18) { 3 }
    )
    val o2File = File(outDir, "O2-S2.cbor")
    o2File.writeBytes(o2.cborBytes)
    println("Wrote ${o2File.path} (${o2.cborBytes.size} bytes, sha256=${Encoding.toHex(Encoding.sha256(o2.cborBytes))})")

    println("Generating O3-PROD.cbor (PROD, pk_A, 18 x 1)...")
    val t0 = System.nanoTime()
    val o3 = Producer.produceOffline(
        profile = Profile.PRODUCTION,
        legsPerStage = IntArray(18) { 1 }
    )
    val elapsedMs = (System.nanoTime() - t0) / 1_000_000.0
    val o3File = File(outDir, "O3-PROD.cbor")
    o3File.writeBytes(o3.cborBytes)
    println(
        "Wrote ${o3File.path} (${o3.cborBytes.size} bytes, sha256=${Encoding.toHex(Encoding.sha256(o3.cborBytes))}) in %.2f ms".format(
            elapsedMs
        )
    )
}

fun runBenchmark() {
    val seed = Construction.computeSeed(CasesFixtures.PK_A, CasesFixtures.SEED_BEACON)

    // Warmup with wordsPerSlot = 2048 so HotSpot C2 compiles the exact 2048-word loop profile
    println("Warming up JIT...")
    val warmupProfile = Profile(
        name = "conformance-small",
        slotBytes = 16384,
        sampleBytes = 1024,
        terrainBytes = 16384 * 64,
        stepsPerLeg = 5000
    )
    repeat(3) {
        Construction.fillTerrainScalar(warmupProfile, seed)
        VectorApiKernels.fillTerrainVector512(warmupProfile, seed, parallel = true)
        val wt = Construction.fillTerrain(warmupProfile, seed, parallel = true)
        Construction.legResultScalar(wt, seed)
        VectorApiKernels.legResultVector512(wt, seed)
        Construction.legResult(wt, seed)
    }

    // 1. Scalar Terrain Fill (PROD: 256 MiB)
    println("\n--- Terrain Fill (PROD: 256 MiB, 16384 slots x 2048 words) ---")
    val scalarFillMs = (1..3).minOf {
        val t0 = System.nanoTime()
        val t = Construction.fillTerrainScalar(Profile.PRODUCTION, seed)
        val ms = (System.nanoTime() - t0) / 1_000_000.0
        ms
    }
    val prodTerrainScalar = Construction.fillTerrainScalar(Profile.PRODUCTION, seed)
    val scalarHash = Encoding.toHex(prodTerrainScalar.sha256())
    println("Plain scalar terrain fill (1 thread): %8.2f ms (sha256=%s)".format(scalarFillMs, scalarHash))

    // 2. Explicit Vector API Terrain Fill (4 threads)
    val vec4FillMs = (1..3).minOf {
        val t0 = System.nanoTime()
        val t = VectorApiKernels.fillTerrainVector512(Profile.PRODUCTION, seed, parallel = true)
        val ms = (System.nanoTime() - t0) / 1_000_000.0
        check(Encoding.toHex(t.sha256()) == scalarHash)
        ms
    }
    println("Explicit Vector API (4 threads) fill: %8.2f ms (%.2fx speedup vs scalar)".format(
        vec4FillMs, scalarFillMs / vec4FillMs
    ))

    // 3. Optimised Unrolled + 4-Thread Column-Partitioned Terrain Fill
    val bestOptFillMs = (1..3).minOf {
        val t0 = System.nanoTime()
        val t = Construction.fillTerrain(Profile.PRODUCTION, seed, parallel = true)
        val ms = (System.nanoTime() - t0) / 1_000_000.0
        check(Encoding.toHex(t.sha256()) == scalarHash)
        ms
    }
    println("Optimised unrolled (4 threads) fill:  %8.2f ms (%.2fx speedup vs scalar)".format(
        bestOptFillMs, scalarFillMs / bestOptFillMs
    ))

    // 4. Production Leg (2,000,000 steps x 2048 words = 4.096B mix64 + 2M SHA-256 seals)
    println("\n--- One Production Leg (PROD: 2,000,000 steps x 16 KiB slot) ---")
    var legScalar = 0L
    val scalarLegMs = (1..2).minOf {
        val t0 = System.nanoTime()
        legScalar = Construction.legResultScalar(prodTerrainScalar, seed)
        (System.nanoTime() - t0) / 1_000_000.0
    }
    println("Plain scalar production leg:          %8.2f ms (leg_result_le=%s)".format(
        scalarLegMs, Encoding.toHex(Encoding.le64(legScalar))
    ))

    val vecLegMs = (1..2).minOf {
        val t0 = System.nanoTime()
        val lr = VectorApiKernels.legResultVector512(prodTerrainScalar, seed)
        val ms = (System.nanoTime() - t0) / 1_000_000.0
        check(lr == legScalar) { "Leg result mismatch in legResultVector512!" }
        ms
    }
    println("Explicit Vector API (AVX-512) leg:    %8.2f ms (%.2fx speedup vs scalar)".format(
        vecLegMs, scalarLegMs / vecLegMs
    ))

    val optLegMs = (1..2).minOf {
        val t0 = System.nanoTime()
        val lr = Construction.legResult(prodTerrainScalar, seed)
        val ms = (System.nanoTime() - t0) / 1_000_000.0
        check(lr == legScalar) { "Leg result mismatch in legResult!" }
        ms
    }
    println("Optimised C2 SuperWord AVX-512 leg:   %8.2f ms (%.2fx speedup vs scalar)".format(
        optLegMs, scalarLegMs / optLegMs
    ))
}
