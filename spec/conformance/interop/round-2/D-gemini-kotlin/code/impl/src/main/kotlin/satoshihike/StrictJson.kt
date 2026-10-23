package satoshihike

import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction

/**
 * Strict RFC 8259 JSON value representation for section 9 portable index parsing.
 * Distinguishes exact integers from fractional/exponent numbers and rejects duplicate object keys.
 */
sealed class JsonVal {
    data class Obj(val members: LinkedHashMap<String, JsonVal>) : JsonVal()
    data class Arr(val items: List<JsonVal>) : JsonVal()
    data class Str(val value: String) : JsonVal()
    data class IntNum(val raw: String, val value: Long?) : JsonVal() // value is null if outside signed 64-bit range
    data class FloatNum(val raw: String) : JsonVal()
    data class Bool(val value: Boolean) : JsonVal()
    object Null : JsonVal()
}

object StrictJson {
    private const val MAX_HEIGHT: Long = (1L shl 53) - 1L
    private const val MAX_U32: Long = 0xFFFFFFFFL

    /**
     * Parses a UTF-8 JSON byte array according to SPEC section 9:
     * - No UTF-8 BOM (EF BB BF)
     * - Valid UTF-8
     * - Exactly one top-level JSON object and no second value
     * - No duplicate member names in any JSON object (including nested objects)
     */
    fun parseObjectBytes(bytes: ByteArray): JsonVal.Obj? {
        if (bytes.size >= 3 &&
            (bytes[0].toInt() and 0xFF) == 0xEF &&
            (bytes[1].toInt() and 0xFF) == 0xBB &&
            (bytes[2].toInt() and 0xFF) == 0xBF
        ) {
            return null // BOM forbidden
        }
        val text = try {
            Charsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(bytes))
                .toString()
        } catch (e: Exception) {
            return null
        }
        return parseObjectString(text)
    }

    fun parseObjectString(text: String): JsonVal.Obj? {
        val parser = Parser(text)
        return try {
            parser.skipWs()
            val root = parser.parseValue()
            parser.skipWs()
            if (!parser.isAtEnd()) return null
            root as? JsonVal.Obj
        } catch (e: JsonParseException) {
            null
        }
    }

    fun extractHeight(v: JsonVal?): Long? {
        if (v !is JsonVal.IntNum) return null
        val n = v.value ?: return null
        if (n !in 0L..MAX_HEIGHT) return null
        return n
    }

    fun extractU32(v: JsonVal?): Long? {
        if (v !is JsonVal.IntNum) return null
        val n = v.value ?: return null
        if (n !in 0L..MAX_U32) return null
        return n
    }

    private class JsonParseException : Exception()

    private class Parser(private val s: String) {
        private var pos: Int = 0

        fun isAtEnd(): Boolean = pos == s.length

        fun skipWs() {
            while (pos < s.length) {
                when (s[pos]) {
                    ' ', '\t', '\r', '\n' -> pos++
                    else -> break
                }
            }
        }

        fun parseValue(): JsonVal {
            skipWs()
            if (pos >= s.length) throw JsonParseException()
            return when (s[pos]) {
                '{' -> parseObject()
                '[' -> parseArray()
                '"' -> JsonVal.Str(parseString())
                't' -> {
                    expectLiteral("true")
                    JsonVal.Bool(true)
                }
                'f' -> {
                    expectLiteral("false")
                    JsonVal.Bool(false)
                }
                'n' -> {
                    expectLiteral("null")
                    JsonVal.Null
                }
                '-', in '0'..'9' -> parseNumber()
                else -> throw JsonParseException()
            }
        }

        private fun parseObject(): JsonVal.Obj {
            pos++ // '{'
            skipWs()
            val map = LinkedHashMap<String, JsonVal>()
            if (pos < s.length && s[pos] == '}') {
                pos++
                return JsonVal.Obj(map)
            }
            while (true) {
                skipWs()
                if (pos >= s.length || s[pos] != '"') throw JsonParseException()
                val key = parseString()
                if (map.containsKey(key)) throw JsonParseException() // duplicate key forbidden
                skipWs()
                if (pos >= s.length || s[pos] != ':') throw JsonParseException()
                pos++
                val value = parseValue()
                map[key] = value
                skipWs()
                if (pos >= s.length) throw JsonParseException()
                when (s[pos]) {
                    ',' -> pos++
                    '}' -> {
                        pos++
                        return JsonVal.Obj(map)
                    }
                    else -> throw JsonParseException()
                }
            }
        }

        private fun parseArray(): JsonVal.Arr {
            pos++ // '['
            skipWs()
            val list = ArrayList<JsonVal>()
            if (pos < s.length && s[pos] == ']') {
                pos++
                return JsonVal.Arr(list)
            }
            while (true) {
                list.add(parseValue())
                skipWs()
                if (pos >= s.length) throw JsonParseException()
                when (s[pos]) {
                    ',' -> pos++
                    ']' -> {
                        pos++
                        return JsonVal.Arr(list)
                    }
                    else -> throw JsonParseException()
                }
            }
        }

        private fun parseString(): String {
            pos++ // '"'
            val sb = StringBuilder()
            while (pos < s.length) {
                val c = s[pos++]
                when {
                    c == '"' -> return sb.toString()
                    c == '\\' -> {
                        if (pos >= s.length) throw JsonParseException()
                        when (val esc = s[pos++]) {
                            '"' -> sb.append('"')
                            '\\' -> sb.append('\\')
                            '/' -> sb.append('/')
                            'b' -> sb.append('\b')
                            'f' -> sb.append('\u000C')
                            'n' -> sb.append('\n')
                            'r' -> sb.append('\r')
                            't' -> sb.append('\t')
                            'u' -> {
                                if (pos + 4 > s.length) throw JsonParseException()
                                val hex = s.substring(pos, pos + 4)
                                pos += 4
                                val code = hex.toIntOrNull(16) ?: throw JsonParseException()
                                sb.append(code.toChar())
                            }
                            else -> throw JsonParseException()
                        }
                    }
                    c.code < 0x20 -> throw JsonParseException()
                    else -> sb.append(c)
                }
            }
            throw JsonParseException()
        }

        private fun parseNumber(): JsonVal {
            val start = pos
            if (s[pos] == '-') pos++
            if (pos >= s.length || s[pos] !in '0'..'9') throw JsonParseException()
            if (s[pos] == '0') {
                pos++
                if (pos < s.length && s[pos] in '0'..'9') throw JsonParseException() // leading zero
            } else {
                while (pos < s.length && s[pos] in '0'..'9') pos++
            }
            var isFloat = false
            if (pos < s.length && s[pos] == '.') {
                isFloat = true
                pos++
                if (pos >= s.length || s[pos] !in '0'..'9') throw JsonParseException()
                while (pos < s.length && s[pos] in '0'..'9') pos++
            }
            if (pos < s.length && (s[pos] == 'e' || s[pos] == 'E')) {
                isFloat = true
                pos++
                if (pos < s.length && (s[pos] == '+' || s[pos] == '-')) pos++
                if (pos >= s.length || s[pos] !in '0'..'9') throw JsonParseException()
                while (pos < s.length && s[pos] in '0'..'9') pos++
            }
            val raw = s.substring(start, pos)
            return if (isFloat) {
                JsonVal.FloatNum(raw)
            } else {
                JsonVal.IntNum(raw, raw.toLongOrNull())
            }
        }

        private fun expectLiteral(lit: String) {
            if (!s.startsWith(lit, pos)) throw JsonParseException()
            pos += lit.length
        }
    }
}
