package satoshihike

import java.net.URI
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.time.Duration

/**
 * Portable index document source for section 14 verification.
 */
fun interface IndexSource {
    /**
     * Performs one GET of /index/{height} (SPEC section 9 & 14.2).
     * Returns raw UTF-8 response body bytes on HTTP 200, or null on any other status / error.
     */
    fun getIndexDocument(height: Long): ByteArray?
}

/**
 * HTTP portable index reader following SPEC section 9 (no redirects, status 200 only).
 */
class HttpIndexSource(rawBaseUrl: String) : IndexSource {
    private val baseUrl: String = rawBaseUrl.trimEnd('/')
    private val client: HttpClient = HttpClient.newBuilder()
        .followRedirects(HttpClient.Redirect.NEVER)
        .connectTimeout(Duration.ofSeconds(10))
        .build()

    override fun getIndexDocument(height: Long): ByteArray? {
        if (height < 0L) return null
        return try {
            val uri = URI.create("$baseUrl/index/$height")
            val req = HttpRequest.newBuilder(uri)
                .GET()
                .build()
            val resp = client.send(req, HttpResponse.BodyHandlers.ofByteArray())
            if (resp.statusCode() == 200) resp.body() else null
        } catch (e: Exception) {
            null
        }
    }
}

/**
 * Fixed inputs and synthetic test-plane index defined in CASES.md.
 */
object CasesFixtures {
    const val PK_A_HEX = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
    const val PK_B_HEX = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"

    val PK_A: ByteArray = Encoding.fromHex(PK_A_HEX)
    val PK_B: ByteArray = Encoding.fromHex(PK_B_HEX)

    val SEED_BEACON: ByteArray = Encoding.sha256("pcoc-pack-2.2/beacon/seed".toByteArray(Charsets.US_ASCII))
    const val SEED_BEACON_HEIGHT: Long = 1000L

    val INTERRUPTS: List<ByteArray> = List(18) { k ->
        Encoding.sha256("pcoc-pack-2.2/beacon/$k".toByteArray(Charsets.US_ASCII))
    }

    fun interruptHeight(k: Int): Long = 1001L + k

    /**
     * Natural 32-byte hash at height [h] in the CASES.md synthetic test-plane index (h = 989 .. 1018).
     */
    fun naturalBytesAtHeight(h: Long): ByteArray? {
        return when (h) {
            in 989L..999L -> Encoding.sha256("pcoc-pack-2.2/index/filler/$h".toByteArray(Charsets.US_ASCII))
            1000L -> SEED_BEACON
            in 1001L..1018L -> INTERRUPTS[(h - 1001L).toInt()]
            else -> null
        }
    }

    /**
     * Synthetic test-plane index serving GET /index/<h> for h = 989 .. 1018 (CASES.md).
     */
    val SYNTHETIC_INDEX_SOURCE: IndexSource = IndexSource { h ->
        val natural = naturalBytesAtHeight(h) ?: return@IndexSource null
        val displayHex = Encoding.toHex(Encoding.reverseBytes32(natural))
        val time = 1700000000L + 600L * h
        val json = """{"network":"synthetic","header":{"hash":"$displayHex","height":$h,"time":$time}}"""
        json.toByteArray(Charsets.UTF_8)
    }
}
