package satoshihike

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.CharBuffer
import java.nio.charset.CodingErrorAction

/**
 * Stage record in a Hike Object (SPEC section 8 Rule A3.2).
 */
data class StageRecord(
    val stageIndex: Int,
    val beacon: ByteArray,
    val beaconHeight: ULong,
    val waypoint: ByteArray,
    val path: ByteArray?, // null for stage 0 (omitted), non-null for stages 1..17
    val legCount: ULong,
    val checkpoints: List<ByteArray>,
    val legResults: List<ByteArray>
)

/**
 * Complete Hike Object (SPEC section 8 Rule A3).
 */
data class HikeObject(
    val kind: String = SpecConstants.KIND,
    val fill: String = SpecConstants.FILL,
    val slotBytes: ULong,
    val nSlots: ULong,
    val sampleBytes: ULong,
    val stepsPerLeg: ULong,
    val publicKey: ByteArray,
    val seedBeacon: ByteArray,
    val stages: List<StageRecord>,
    val lastInterrupt: ByteArray,
    val lastInterruptHeight: ULong
)

/**
 * Generic definite-length CBOR AST node (SPEC section 7.3).
 */
sealed class CborValue {
    data class UIntVal(val value: ULong) : CborValue()
    data class BytesVal(val bytes: ByteArray) : CborValue()
    data class TextVal(val text: String) : CborValue()
    data class ArrayVal(val items: List<CborValue>) : CborValue()
    data class MapVal(val entries: LinkedHashMap<String, CborValue>) : CborValue()
}

/**
 * Section 7.3 four-way classification of input bytes for verification.
 */
sealed class CborDecodeClassification {
    object MalformedCbor : CborDecodeClassification()
    object DecodeFailure : CborDecodeClassification()
    data class UnknownKind(val kindText: String) : CborDecodeClassification()
    data class ValidRootMap(val map: CborValue.MapVal) : CborDecodeClassification()
}

/**
 * Deterministic CBOR encoder and section 7.3 strict definite-length decoder.
 */
object CborCodec {

    /**
     * Encodes a [HikeObject] in core deterministic CBOR encoding (SPEC section 7.3).
     */
    fun encode(obj: HikeObject): ByteArray {
        val out = ByteArrayOutputStream(8192)
        // Top-level map has 11 keys in exact section 7.3 order
        writeHead(out, 5, 11uL)

        // 1. fill
        writeText(out, "fill")
        writeText(out, obj.fill)

        // 2. kind
        writeText(out, "kind")
        writeText(out, obj.kind)

        // 3. stages
        writeText(out, "stages")
        writeHead(out, 4, obj.stages.size.toULong())
        for (stage in obj.stages) {
            if (stage.stageIndex == 0) {
                require(stage.path == null) { "Stage 0 must omit path" }
                writeHead(out, 5, 6uL)
                // 1. beacon
                writeText(out, "beacon")
                writeBytes(out, stage.beacon)
                // 2. waypoint
                writeText(out, "waypoint")
                writeBytes(out, stage.waypoint)
                // 3. leg_count
                writeText(out, "leg_count")
                writeUInt(out, stage.legCount)
                // 4. checkpoints
                writeText(out, "checkpoints")
                writeHead(out, 4, stage.checkpoints.size.toULong())
                for (cp in stage.checkpoints) {
                    writeBytes(out, cp)
                }
                // 5. leg_results
                writeText(out, "leg_results")
                writeHead(out, 4, stage.legResults.size.toULong())
                for (lr in stage.legResults) {
                    writeBytes(out, lr)
                }
                // 6. beacon_height
                writeText(out, "beacon_height")
                writeUInt(out, stage.beaconHeight)
            } else {
                val pathBytes = requireNotNull(stage.path) { "Stage ${stage.stageIndex} must include path" }
                writeHead(out, 5, 7uL)
                // 1. path
                writeText(out, "path")
                writeBytes(out, pathBytes)
                // 2. beacon
                writeText(out, "beacon")
                writeBytes(out, stage.beacon)
                // 3. waypoint
                writeText(out, "waypoint")
                writeBytes(out, stage.waypoint)
                // 4. leg_count
                writeText(out, "leg_count")
                writeUInt(out, stage.legCount)
                // 5. checkpoints
                writeText(out, "checkpoints")
                writeHead(out, 4, stage.checkpoints.size.toULong())
                for (cp in stage.checkpoints) {
                    writeBytes(out, cp)
                }
                // 6. leg_results
                writeText(out, "leg_results")
                writeHead(out, 4, stage.legResults.size.toULong())
                for (lr in stage.legResults) {
                    writeBytes(out, lr)
                }
                // 7. beacon_height
                writeText(out, "beacon_height")
                writeUInt(out, stage.beaconHeight)
            }
        }

        // 4. n_slots
        writeText(out, "n_slots")
        writeUInt(out, obj.nSlots)

        // 5. public_key
        writeText(out, "public_key")
        writeBytes(out, obj.publicKey)

        // 6. slot_bytes
        writeText(out, "slot_bytes")
        writeUInt(out, obj.slotBytes)

        // 7. seed_beacon
        writeText(out, "seed_beacon")
        writeBytes(out, obj.seedBeacon)

        // 8. sample_bytes
        writeText(out, "sample_bytes")
        writeUInt(out, obj.sampleBytes)

        // 9. steps_per_leg
        writeText(out, "steps_per_leg")
        writeUInt(out, obj.stepsPerLeg)

        // 10. last_interrupt
        writeText(out, "last_interrupt")
        writeBytes(out, obj.lastInterrupt)

        // 11. last_interrupt_height
        writeText(out, "last_interrupt_height")
        writeUInt(out, obj.lastInterruptHeight)

        return out.toByteArray()
    }

    private fun writeUInt(out: ByteArrayOutputStream, value: ULong) {
        writeHead(out, 0, value)
    }

    private fun writeBytes(out: ByteArrayOutputStream, bytes: ByteArray) {
        writeHead(out, 2, bytes.size.toULong())
        out.write(bytes)
    }

    private fun writeText(out: ByteArrayOutputStream, text: String) {
        val utf8 = text.toByteArray(Charsets.UTF_8)
        writeHead(out, 3, utf8.size.toULong())
        out.write(utf8)
    }

    private fun writeHead(out: ByteArrayOutputStream, majorType: Int, arg: ULong) {
        val prefix = majorType shl 5
        when {
            arg < 24uL -> {
                out.write(prefix or arg.toInt())
            }
            arg <= 0xFFuL -> {
                out.write(prefix or 24)
                out.write(arg.toInt())
            }
            arg <= 0xFFFFuL -> {
                out.write(prefix or 25)
                val v = arg.toInt()
                out.write((v ushr 8) and 0xFF)
                out.write(v and 0xFF)
            }
            arg <= 0xFFFFFFFFuL -> {
                out.write(prefix or 26)
                val v = arg.toLong()
                out.write(((v ushr 24) and 0xFF).toInt())
                out.write(((v ushr 16) and 0xFF).toInt())
                out.write(((v ushr 8) and 0xFF).toInt())
                out.write((v and 0xFF).toInt())
            }
            else -> {
                out.write(prefix or 27)
                val v = arg.toLong()
                for (shift in 56 downTo 0 step 8) {
                    out.write(((v ushr shift) and 0xFF).toInt())
                }
            }
        }
    }

    /**
     * Decodes and classifies input bytes according to SPEC section 7.3 order.
     */
    fun classifyAndDecode(input: ByteArray): CborDecodeClassification {
        if (input.isEmpty()) return CborDecodeClassification.MalformedCbor
        val reader = CborReader(input)
        val item = try {
            reader.readItem()
        } catch (e: MalformedCborException) {
            return CborDecodeClassification.MalformedCbor
        }
        if (!reader.isAtEnd()) {
            return CborDecodeClassification.MalformedCbor
        }
        if (item !is CborValue.MapVal) {
            return CborDecodeClassification.DecodeFailure
        }
        val kindVal = item.entries["kind"]
        if (kindVal !is CborValue.TextVal) {
            return CborDecodeClassification.DecodeFailure
        }
        if (kindVal.text != SpecConstants.KIND) {
            return CborDecodeClassification.UnknownKind(kindVal.text)
        }
        return CborDecodeClassification.ValidRootMap(item)
    }

    private class MalformedCborException : Exception()

    private class CborReader(private val data: ByteArray) {
        private var pos: Int = 0

        fun isAtEnd(): Boolean = pos == data.size

        fun readItem(): CborValue {
            if (pos >= data.size) throw MalformedCborException()
            val ib = data[pos++].toInt() and 0xFF
            val major = ib ushr 5
            val ai = ib and 0x1F

            val arg: ULong = when {
                ai < 24 -> ai.toULong()
                ai == 24 -> readBytesAsULong(1)
                ai == 25 -> readBytesAsULong(2)
                ai == 26 -> readBytesAsULong(4)
                ai == 27 -> readBytesAsULong(8)
                else -> throw MalformedCborException() // 28..31 (indefinite or reserved)
            }

            return when (major) {
                0 -> CborValue.UIntVal(arg)
                1 -> throw MalformedCborException() // negative integers forbidden
                2 -> {
                    val len = argToLength(arg)
                    val slice = readSlice(len)
                    CborValue.BytesVal(slice)
                }
                3 -> {
                    val len = argToLength(arg)
                    val slice = readSlice(len)
                    val text = decodeStrictUtf8(slice)
                    CborValue.TextVal(text)
                }
                4 -> {
                    val len = argToLength(arg)
                    val list = ArrayList<CborValue>(minOf(len, 1024))
                    for (i in 0 until len) {
                        list.add(readItem())
                    }
                    CborValue.ArrayVal(list)
                }
                5 -> {
                    val len = argToLength(arg)
                    val map = LinkedHashMap<String, CborValue>(minOf(len * 2, 1024))
                    for (i in 0 until len) {
                        val keyItem = readItem()
                        if (keyItem !is CborValue.TextVal) {
                            throw MalformedCborException()
                        }
                        if (map.containsKey(keyItem.text)) {
                            throw MalformedCborException() // duplicate key
                        }
                        val valueItem = readItem()
                        map[keyItem.text] = valueItem
                    }
                    CborValue.MapVal(map)
                }
                else -> throw MalformedCborException() // tags (6) and floats/simple (7) forbidden
            }
        }

        private fun readBytesAsULong(count: Int): ULong {
            if (pos + count > data.size) throw MalformedCborException()
            var res = 0uL
            for (i in 0 until count) {
                res = (res shl 8) or (data[pos++].toInt() and 0xFF).toULong()
            }
            return res
        }

        private fun argToLength(arg: ULong): Int {
            if (arg > (data.size - pos).toULong() || arg > Int.MAX_VALUE.toULong()) {
                throw MalformedCborException()
            }
            return arg.toInt()
        }

        private fun readSlice(len: Int): ByteArray {
            if (pos + len > data.size) throw MalformedCborException()
            val out = data.copyOfRange(pos, pos + len)
            pos += len
            return out
        }

        private fun decodeStrictUtf8(bytes: ByteArray): String {
            val decoder = Charsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
            return try {
                decoder.decode(ByteBuffer.wrap(bytes)).toString()
            } catch (e: Exception) {
                throw MalformedCborException()
            }
        }
    }

    /**
     * Formats Rule A5 unknown-kind JSON record (without trailing newline) per RFC 8785 (SPEC section 14.7).
     */
    fun formatUnknownKindRecord(kindText: String): String {
        return "{\"kind\":\"unknown kind\",\"kind_text\":${escapeJcsString(kindText)}}"
    }

    /**
     * Escapes a string as RFC 8785 (JSON Canonicalization Scheme) specifies.
     */
    fun escapeJcsString(s: String): String {
        val sb = StringBuilder(s.length + 2)
        sb.append('"')
        for (c in s) {
            when (c) {
                '"' -> sb.append("\\\"")
                '\\' -> sb.append("\\\\")
                '\b' -> sb.append("\\b")
                '\t' -> sb.append("\\t")
                '\n' -> sb.append("\\n")
                '\u000C' -> sb.append("\\f")
                '\r' -> sb.append("\\r")
                in '\u0000'..'\u001F' -> {
                    sb.append("\\u00")
                    val v = c.code
                    val hex = "0123456789abcdef"
                    sb.append(hex[(v ushr 4) and 0xF])
                    sb.append(hex[v and 0xF])
                }
                else -> sb.append(c)
            }
        }
        sb.append('"')
        return sb.toString()
    }
}
