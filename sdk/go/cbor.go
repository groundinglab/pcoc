package pcoc

// Definite-length CBOR for the hike object (SPEC §7.3, RFC 8949).
//
// Produce emits the core deterministic encoding: definite lengths, shortest
// unsigned integers, no floating-point values, no tags, map keys sorted by
// the bytewise order of their definite-length encodings.
//
// Decode accepts any definite-length encoding of the same values, including
// a non-shortest unsigned integer. Indefinite lengths, tags, floating-point
// values, simple values, negative integers, a map key that is not a text
// string, a duplicate key, and any byte after the one data item are a
// malformed CBOR value (SPEC §7.3).

import "encoding/binary"

type cborType int

const (
	cborUint cborType = iota
	cborBytes
	cborText
	cborArray
	cborMap
)

type cborValue struct {
	typ   cborType
	uint  uint64
	bytes []byte
	array []cborValue
	pairs []cborPair
}

type cborPair struct {
	key string
	val cborValue
}

func cborEncodeUint(x uint64) []byte {
	switch {
	case x < 24:
		return []byte{byte(x)}
	case x < 256:
		return []byte{0x18, byte(x)}
	case x < 65536:
		b := []byte{0x19, 0, 0}
		binary.BigEndian.PutUint16(b[1:], uint16(x))
		return b
	case x < 1<<32:
		b := []byte{0x1a, 0, 0, 0, 0}
		binary.BigEndian.PutUint32(b[1:], uint32(x))
		return b
	default:
		b := []byte{0x1b, 0, 0, 0, 0, 0, 0, 0, 0}
		binary.BigEndian.PutUint64(b[1:], x)
		return b
	}
}

func cborHead(major byte, n uint64) []byte {
	body := cborEncodeUint(n)
	body[0] = major | (body[0] & 0x1f)
	return body
}

func cborEncodeText(s string) []byte {
	b := []byte(s)
	return append(cborHead(0x60, uint64(len(b))), b...)
}

func cborEncodeBytes(b []byte) []byte {
	return append(cborHead(0x40, uint64(len(b))), b...)
}

// encodeValue emits one definite-length item. Map keys are sorted by the
// bytewise order of their encodings (SPEC §7.3).
func encodeValue(v cborValue) []byte {
	switch v.typ {
	case cborUint:
		return cborEncodeUint(v.uint)
	case cborBytes:
		return cborEncodeBytes(v.bytes)
	case cborText:
		return cborEncodeText(string(v.bytes))
	case cborArray:
		out := cborHead(0x80, uint64(len(v.array)))
		for _, e := range v.array {
			out = append(out, encodeValue(e)...)
		}
		return out
	case cborMap:
		type kv struct {
			enc []byte
			val []byte
		}
		items := make([]kv, len(v.pairs))
		for i, p := range v.pairs {
			items[i] = kv{enc: cborEncodeText(p.key), val: encodeValue(p.val)}
		}
		for i := 1; i < len(items); i++ {
			j := i
			for j > 0 && bytesLess(items[j].enc, items[j-1].enc) {
				items[j], items[j-1] = items[j-1], items[j]
				j--
			}
		}
		out := cborHead(0xa0, uint64(len(items)))
		for _, it := range items {
			out = append(out, it.enc...)
			out = append(out, it.val...)
		}
		return out
	default:
		return nil
	}
}

func bytesLess(a, b []byte) bool {
	n := len(a)
	if len(b) < n {
		n = len(b)
	}
	for i := 0; i < n; i++ {
		if a[i] != b[i] {
			return a[i] < b[i]
		}
	}
	return len(a) < len(b)
}

func cborUintV(x uint64) cborValue { return cborValue{typ: cborUint, uint: x} }
func cborTextV(s string) cborValue { return cborValue{typ: cborText, bytes: []byte(s)} }
func cborBytesV(b []byte) cborValue {
	c := make([]byte, len(b))
	copy(c, b)
	return cborValue{typ: cborBytes, bytes: c}
}

type decodeError string

func (e decodeError) Error() string { return "malformed cbor: " + string(e) }

func cborMalformed(why string) error { return decodeError(why) }

// decodeOne decodes one definite-length data item (SPEC §7.3).
// A non-shortest unsigned integer is accepted. A major-type-3 string whose
// bytes are not well-formed UTF-8 is not a text string under this section.
func decodeOne(b []byte) (cborValue, []byte, error) {
	if len(b) == 0 {
		return cborValue{}, nil, cborMalformed("empty")
	}
	ib := b[0]
	major := ib >> 5
	ai := ib & 0x1f
	rest := b[1:]
	if ai == 31 {
		return cborValue{}, nil, cborMalformed("indefinite")
	}
	var n uint64
	switch ai {
	case 24:
		if len(rest) < 1 {
			return cborValue{}, nil, cborMalformed("short")
		}
		n = uint64(rest[0])
		rest = rest[1:]
	case 25:
		if len(rest) < 2 {
			return cborValue{}, nil, cborMalformed("short")
		}
		n = uint64(binary.BigEndian.Uint16(rest[:2]))
		rest = rest[2:]
	case 26:
		if len(rest) < 4 {
			return cborValue{}, nil, cborMalformed("short")
		}
		n = uint64(binary.BigEndian.Uint32(rest[:4]))
		rest = rest[4:]
	case 27:
		if len(rest) < 8 {
			return cborValue{}, nil, cborMalformed("short")
		}
		n = binary.BigEndian.Uint64(rest[:8])
		rest = rest[8:]
	default:
		if ai >= 28 {
			return cborValue{}, nil, cborMalformed("ai")
		}
		n = uint64(ai)
	}
	switch major {
	case 0:
		return cborValue{typ: cborUint, uint: n}, rest, nil
	case 1:
		return cborValue{}, nil, cborMalformed("negative")
	case 2, 3:
		if uint64(len(rest)) < n {
			return cborValue{}, nil, cborMalformed("short bytes")
		}
		payload := append([]byte(nil), rest[:n]...)
		rest = rest[n:]
		typ := cborBytes
		if major == 3 {
			if !utf8OK(payload) {
				return cborValue{}, nil, cborMalformed("utf8")
			}
			typ = cborText
		}
		return cborValue{typ: typ, bytes: payload}, rest, nil
	case 4:
		arr := make([]cborValue, 0, n)
		var err error
		for i := uint64(0); i < n; i++ {
			var e cborValue
			e, rest, err = decodeOne(rest)
			if err != nil {
				return cborValue{}, nil, err
			}
			arr = append(arr, e)
		}
		return cborValue{typ: cborArray, array: arr}, rest, nil
	case 5:
		pairs := make([]cborPair, 0, n)
		seen := map[string]struct{}{}
		var err error
		for i := uint64(0); i < n; i++ {
			if len(rest) == 0 {
				return cborValue{}, nil, cborMalformed("short map")
			}
			var key cborValue
			key, rest, err = decodeOne(rest)
			if err != nil {
				return cborValue{}, nil, err
			}
			if key.typ != cborText {
				return cborValue{}, nil, cborMalformed("map key")
			}
			ks := string(key.bytes)
			if _, ok := seen[ks]; ok {
				return cborValue{}, nil, cborMalformed("duplicate key")
			}
			seen[ks] = struct{}{}
			var val cborValue
			val, rest, err = decodeOne(rest)
			if err != nil {
				return cborValue{}, nil, err
			}
			pairs = append(pairs, cborPair{key: ks, val: val})
		}
		return cborValue{typ: cborMap, pairs: pairs}, rest, nil
	default:
		return cborValue{}, nil, cborMalformed("tag or simple")
	}
}

// utf8OK reports whether b is well-formed UTF-8 (RFC 3629).
// Overlong encodings and surrogate code points are rejected.
func utf8OK(b []byte) bool {
	i := 0
	for i < len(b) {
		c := b[i]
		if c < 0x80 {
			i++
			continue
		}
		var need int
		switch {
		case c&0xe0 == 0xc0:
			need = 2
			if c < 0xc2 {
				return false
			}
		case c&0xf0 == 0xe0:
			need = 3
		case c&0xf8 == 0xf0:
			need = 4
			if c > 0xf4 {
				return false
			}
		default:
			return false
		}
		if i+need > len(b) {
			return false
		}
		for k := 1; k < need; k++ {
			if b[i+k]&0xc0 != 0x80 {
				return false
			}
		}
		if need == 3 {
			cp := uint32(c&0x0f)<<12 | uint32(b[i+1]&0x3f)<<6 | uint32(b[i+2]&0x3f)
			if cp < 0x800 || (cp >= 0xd800 && cp <= 0xdfff) {
				return false
			}
		}
		if need == 4 {
			cp := uint32(c&0x07)<<18 | uint32(b[i+1]&0x3f)<<12 | uint32(b[i+2]&0x3f)<<6 | uint32(b[i+3]&0x3f)
			if cp < 0x10000 || cp > 0x10ffff {
				return false
			}
		}
		i += need
	}
	return true
}

func (v cborValue) lookup(key string) (cborValue, bool) {
	for _, p := range v.pairs {
		if p.key == key {
			return p.val, true
		}
	}
	return cborValue{}, false
}

func (v cborValue) keySet() map[string]struct{} {
	out := make(map[string]struct{}, len(v.pairs))
	for _, p := range v.pairs {
		out[p.key] = struct{}{}
	}
	return out
}
