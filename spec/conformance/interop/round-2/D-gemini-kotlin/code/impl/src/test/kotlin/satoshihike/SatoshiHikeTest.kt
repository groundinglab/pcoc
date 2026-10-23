package satoshihike

import com.sun.net.httpserver.HttpServer
import negatives.CandidateGenerator
import java.io.File
import java.net.InetSocketAddress
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SatoshiHikeTest {

    private val expectedCandidateFiles = listOf(
        "seed.txt",
        "nl4.txt",
        "terrain.txt",
        "fold.txt",
        "kat-small.txt",
        "test-3leg.txt",
        "prod.txt",
        "wes.txt",
        "rate.txt",
        "anchor.txt",
        "draw.txt",
        "unknown-kind.txt",
        "hashcash.txt",
        "negatives.txt"
    )

    @Test
    fun testSpecSection7_4InformativeChecks() {
        // le64(1) = 01 00 00 00 00 00 00 00
        assertEquals("0100000000000000", Encoding.toHex(Encoding.le64(1L)))
        // le64(0x0102030405060708) = 08 07 06 05 04 03 02 01
        assertEquals("0807060504030201", Encoding.toHex(Encoding.le64(0x0102030405060708L)))

        // FIPS 180-4 SHA-256("abc")
        assertEquals(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            Encoding.toHex(Encoding.sha256("abc".toByteArray(Charsets.US_ASCII)))
        )

        // SHA-256(SEAL_DOMAIN || le64(0))
        val seal0 = Encoding.sha256(SpecConstants.SEAL_DOMAIN, Encoding.le64(0L))
        assertEquals(
            "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533",
            Encoding.toHex(seal0)
        )
        val sPrime = Encoding.u64Le(seal0, 0)
        assertEquals(0x76042423afbd1648L, sPrime)

        // Seed preimage of 32 bytes of 0x11 followed by 32 bytes of 0x22
        val pk11 = ByteArray(32) { 0x11 }
        val beacon22 = ByteArray(32) { 0x22 }
        val d = Construction.computeSeed(pk11, beacon22)
        assertEquals(
            "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c",
            Encoding.toHex(d)
        )
        val lanes = Construction.extractLanes(d)
        assertEquals(0x545dfe297dc78951uL, lanes[0].toULong())
        assertEquals(0x27858669c45e046auL, lanes[1].toULong())
        assertEquals(0x9cdac73ac1a5fe85uL, lanes[2].toULong())
        assertEquals(0x7c81df6efbf55f11uL, lanes[3].toULong())

        val nl4_0_0 = Construction.nl4BaseWord(lanes, 0, 0)
        assertEquals(0xc7582082a632c2c8uL, nl4_0_0.toULong())
    }

    @Test
    fun testScalarAndVectorEquivalence() {
        val seed = Construction.computeSeed(CasesFixtures.PK_A, CasesFixtures.SEED_BEACON)
        for (profile in listOf(Profile.S1, Profile.S2)) {
            val tScalar = Construction.fillTerrainScalar(profile, seed)
            val tVec = VectorApiKernels.fillTerrainVector512(profile, seed, parallel = true)
            val tOpt = Construction.fillTerrain(profile, seed, parallel = true)

            assertContentEquals(tScalar.words, tVec.words, "Vector512 terrain words mismatch on $profile")
            assertContentEquals(tScalar.words, tOpt.words, "Optimised terrain words mismatch on $profile")

            val lrScalar = Construction.legResultScalar(tScalar, seed)
            val lrVec = VectorApiKernels.legResultVector512(tScalar, seed)
            val lrOpt = Construction.legResult(tScalar, seed)

            assertEquals(lrScalar, lrVec, "Vector512 legResult mismatch on $profile")
            assertEquals(lrScalar, lrOpt, "Optimised legResult mismatch on $profile")
        }
    }

    @Test
    fun testCandidateGeneratorMatchesCommittedCandidates() {
        val committedDir = File("candidates")
        assertTrue(committedDir.exists() && committedDir.isDirectory, "Committed candidates/ directory must exist")

        val tempDir = Files.createTempDirectory("candidates-test").toFile()
        try {
            CandidateGenerator.generateAll(tempDir)

            val generatedNames = tempDir.list()?.sorted() ?: emptyList()
            assertEquals(expectedCandidateFiles.sorted(), generatedNames)

            for (fileName in expectedCandidateFiles) {
                val committedFile = File(committedDir, fileName)
                val generatedFile = File(tempDir, fileName)
                assertTrue(committedFile.exists(), "Committed file candidates/$fileName missing")
                assertTrue(generatedFile.exists(), "Generated file $fileName missing")

                val committedBytes = committedFile.readBytes()
                val generatedBytes = generatedFile.readBytes()
                assertContentEquals(
                    committedBytes,
                    generatedBytes,
                    "Generated candidate file $fileName differs from committed candidates/$fileName"
                )

                // Verify format rules from CASES.md: ASCII, key<TAB>value<LF>, sorted by key in byte order
                val text = String(committedBytes, Charsets.US_ASCII)
                assertTrue(text.endsWith("\n"), "$fileName must end with LF")
                val lines = text.dropLast(1).split('\n')
                assertTrue(lines.isNotEmpty(), "$fileName must not be empty")
                var prevKey: ByteArray? = null
                for (line in lines) {
                    val tabIdx = line.indexOf('\t')
                    assertTrue(tabIdx > 0, "Line in $fileName missing TAB: $line")
                    assertEquals(-1, line.indexOf('\t', tabIdx + 1), "Line in $fileName has multiple TABs: $line")
                    val keyBytes = line.substring(0, tabIdx).toByteArray(Charsets.US_ASCII)
                    if (prevKey != null) {
                        assertTrue(compareUnsignedBytes(prevKey, keyBytes) < 0, "Keys not sorted in $fileName: $line")
                    }
                    prevKey = keyBytes
                }
            }
        } finally {
            tempDir.deleteRecursively()
        }
    }

    @Test
    fun testCommittedObjectsAndHttpIndexVerification() {
        val o1File = File("objects/O1-S1.cbor")
        val o2File = File("objects/O2-S2.cbor")
        val o3File = File("objects/O3-PROD.cbor")
        assertTrue(o1File.exists(), "objects/O1-S1.cbor must exist")
        assertTrue(o2File.exists(), "objects/O2-S2.cbor must exist")
        assertTrue(o3File.exists(), "objects/O3-PROD.cbor must exist")

        // Re-produce O1 and O2 and check byte-identity
        val reproO1 = Producer.produceOffline(Profile.S1, legsPerStage = IntArray(18) { 1 })
        assertContentEquals(o1File.readBytes(), reproO1.cborBytes, "O1-S1.cbor must be byte-identical")
        assertEquals(17, reproO1.stderrLines.size)
        assertEquals("path_skip 1", reproO1.stderrLines.first())
        assertEquals("not fetched by design 17", reproO1.stderrLines.last())

        val reproO2 = Producer.produceOffline(Profile.S2, legsPerStage = IntArray(18) { 3 })
        assertContentEquals(o2File.readBytes(), reproO2.cborBytes, "O2-S2.cbor must be byte-identical")

        // Start a real local HTTP server serving the CASES.md synthetic index
        val server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        server.createContext("/index/") { exchange ->
            val path = exchange.requestURI.path
            val heightStr = path.removePrefix("/index/")
            val h = heightStr.toLongOrNull()
            val body = if (h != null && h.toString() == heightStr) {
                CasesFixtures.SYNTHETIC_INDEX_SOURCE.getIndexDocument(h)
            } else {
                null
            }
            if (body != null) {
                exchange.sendResponseHeaders(200, body.size.toLong())
                exchange.responseBody.use { it.write(body) }
            } else {
                exchange.sendResponseHeaders(404, -1)
                exchange.close()
            }
        }
        server.start()
        try {
            val port = server.address.port
            val httpSource = HttpIndexSource("http://127.0.0.1:$port")

            // Verify O1 via HTTP index source
            val drawReaderO1 = Construction.DrawByteReader(Construction.labelDrawByteSource("pcoc-pack-2.2/draw"))
            val resO1 = Verifier.verify(o1File.readBytes(), Profile.S1, httpSource, drawReaderO1)
            assertTrue(resO1 is VerifyResult.Record)
            val jsonO1 = StrictJson.parseObjectString(resO1.json)
            assertNotNull(jsonO1)
            assertEquals("conformance-small", (jsonO1.members["profile"] as JsonVal.Str).value)
            assertEquals("match", (jsonO1.members["shape"] as JsonVal.Str).value)
            assertEquals("match", (jsonO1.members["seed_value"] as JsonVal.Str).value)
            val sixLegO1 = jsonO1.members["six_leg"] as JsonVal.Obj
            val sixResultsO1 = (sixLegO1.members["results"] as JsonVal.Arr).items.map { (it as JsonVal.Str).value }
            assertEquals(List(6) { "match" }, sixResultsO1)
            val analysisO1 = jsonO1.members["analysis"] as JsonVal.Obj
            assertEquals(18L, (analysisO1.members["hike_legs"] as JsonVal.IntNum).value)
            assertEquals(10800L, (analysisO1.members["hike_span"] as JsonVal.IntNum).value)

            // Verify O2 via HTTP index source
            val drawReaderO2 = Construction.DrawByteReader(Construction.labelDrawByteSource("pcoc-pack-2.2/draw"))
            val resO2 = Verifier.verify(o2File.readBytes(), Profile.S2, httpSource, drawReaderO2)
            assertTrue(resO2 is VerifyResult.Record)
            val jsonO2 = StrictJson.parseObjectString(resO2.json)
            assertNotNull(jsonO2)
            assertEquals("match", (jsonO2.members["shape"] as JsonVal.Str).value)
            val analysisO2 = jsonO2.members["analysis"] as JsonVal.Obj
            assertEquals(54L, (analysisO2.members["hike_legs"] as JsonVal.IntNum).value)
            assertEquals(10800L, (analysisO2.members["hike_span"] as JsonVal.IntNum).value)
        } finally {
            server.stop(0)
        }
    }

    @Test
    fun testDecodeAndShapeEdgeCases() {
        // 1. Empty input -> malformed cbor
        val emptyRes = Verifier.verify(ByteArray(0), Profile.S1)
        assertTrue(emptyRes is VerifyResult.Record)
        val emptyObj = StrictJson.parseObjectString(emptyRes.json)!!
        assertEquals("mismatch", (emptyObj.members["shape"] as JsonVal.Str).value)
        val emptyShapeItems = (emptyObj.members["shape_item"] as JsonVal.Arr).items.map { (it as JsonVal.Str).value }
        assertEquals(listOf("malformed cbor"), emptyShapeItems)

        // 2. Non-map CBOR item (unsigned int 1 -> 0x01) -> DecodeFailure (exit code 1)
        val nonMapRes = Verifier.verify(byteArrayOf(0x01), Profile.S1)
        assertTrue(nonMapRes is VerifyResult.DecodeFailure)

        // 3. Trailing byte after valid O1 object -> malformed cbor
        val o1Bytes = File("objects/O1-S1.cbor").readBytes()
        val trailingRes = Verifier.verify(o1Bytes + byteArrayOf(0x00), Profile.S1)
        assertTrue(trailingRes is VerifyResult.Record)
        val trailingObj = StrictJson.parseObjectString(trailingRes.json)!!
        assertEquals(
            listOf("malformed cbor"),
            (trailingObj.members["shape_item"] as JsonVal.Arr).items.map { (it as JsonVal.Str).value }
        )

        // 4. CLI exit code 2 when --profile conformance-small is missing any of the 4 integers
        assertEquals(2, runVerifyCli(arrayOf("--profile", "conformance-small", "objects/O1-S1.cbor")))
        assertEquals(1, runVerifyCli(arrayOf("objects/non-existent-file.cbor")))
    }

    @Test
    fun testCanonicalPathBodyAndBariPaidRule() {
        val waypoint = Encoding.sha256("test-waypoint".toByteArray(Charsets.US_ASCII))
        val leafNode = Encoding.sha256(byteArrayOf(0x1A), waypoint)
        val sibling0 = Encoding.sha256("sibling-0".toByteArray(Charsets.US_ASCII))
        // treeSize = 2, leafIndex = 0 -> root = SHA-256(0x1B || leafNode || sibling0)
        val root = Encoding.sha256(byteArrayOf(0x1B), leafNode, sibling0)
        val txid = Encoding.sha256("txid".toByteArray(Charsets.US_ASCII))
        val blockHash = CasesFixtures.INTERRUPTS[2] // height 1003

        val pathBytes = ByteArray(146 + 32)
        pathBytes[0] = 0x01
        System.arraycopy(waypoint, 0, pathBytes, 1, 32)
        // leafIndex = 0 (bytes 33..36)
        // treeSize = 2 (bytes 37..40)
        pathBytes[40] = 2
        // siblingCount = 1 (byte 41)
        pathBytes[41] = 1
        System.arraycopy(sibling0, 0, pathBytes, 42, 32)
        System.arraycopy(root, 0, pathBytes, 74, 32)
        System.arraycopy(txid, 0, pathBytes, 106, 32)
        pathBytes[141] = 0 // vout = 0
        System.arraycopy(blockHash, 0, pathBytes, 142, 32)
        // blockHeight = 1003 = 0x03EB (bytes 174..177)
        pathBytes[176] = 0x03
        pathBytes[177] = 0xEB.toByte()

        val decoded = CanonicalPathBody.decode(pathBytes)
        assertNotNull(decoded)
        assertTrue(decoded.validateFor(waypoint))
        assertFalse(decoded.validateFor( ByteArray(32)))

        // Bari paid rule BR1 check
        val bariOrigin = "https://bari.example.com"
        val feeHeader = ByteArray(80) { (it + 1).toByte() }
        val feeHeaderHash = Encoding.sha256(Encoding.sha256(feeHeader))
        val prevTipHash = Encoding.sha256("prev-tip".toByteArray(Charsets.US_ASCII))
        val hc = Construction.solveHashcash(bariOrigin, feeHeaderHash, waypoint)

        // Current tip match -> 204
        assertEquals(
            204,
            BariPaidRule.checkWaypointPost(
                body = waypoint,
                nonceHeader = hc.nonce.toULong().toString(),
                feeHeaderHex = Encoding.toHex(feeHeader),
                bariOrigin = bariOrigin,
                bariTipHashNatural = feeHeaderHash,
                bariTipPrevHashNatural = prevTipHash
            )
        )

        // Reversed (display-order) tip hash -> 402 (SPEC P9.17)
        assertEquals(
            402,
            BariPaidRule.checkWaypointPost(
                body = waypoint,
                nonceHeader = hc.nonce.toULong().toString(),
                feeHeaderHex = Encoding.toHex(feeHeader),
                bariOrigin = bariOrigin,
                bariTipHashNatural = Encoding.reverseBytes32(feeHeaderHash),
                bariTipPrevHashNatural = prevTipHash
            )
        )

        // Wrong origin -> 402
        assertEquals(
            402,
            BariPaidRule.checkWaypointPost(
                body = waypoint,
                nonceHeader = hc.nonce.toULong().toString(),
                feeHeaderHex = Encoding.toHex(feeHeader),
                bariOrigin = "https://other.example.com",
                bariTipHashNatural = feeHeaderHash,
                bariTipPrevHashNatural = prevTipHash
            )
        )
    }

    private fun compareUnsignedBytes(a: ByteArray, b: ByteArray): Int {
        val minLen = minOf(a.size, b.size)
        for (i in 0 until minLen) {
            val va = a[i].toInt() and 0xFF
            val vb = b[i].toInt() and 0xFF
            if (va != vb) return va.compareTo(vb)
        }
        return a.size.compareTo(b.size)
    }
}
