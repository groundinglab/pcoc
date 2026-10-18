package hike

import (
	"encoding/binary"
	"errors"
	"unicode/utf8"
)

// CBOR for section 7.3. Produce writes definite lengths and shortest
// unsigned integers. Verify accepts a non-shortest definite unsigned integer.
// Indefinite lengths, tags, floats, simple values, negative integers, a
// non-text key, and a duplicate key are malformed. decodeItem rejects a
// trailing byte. decodeHead reports that byte so a v2.0 record can still
// measure the item.

// MaxCBORNesting is an implementation resource limit: the most arrays and
// maps one item may sit inside, counting itself. A section 8 hike object
// nests at most 4 deep (top-level map, stages array, stage map,
// checkpoints/leg_results array; their elements are byte strings). 64 is 16
// times that and is the same value in the Rust, Go and Java SDKs. A deeper
// item is malformed CBOR.
const MaxCBORNesting = 64

var (
	errDepth      = errors.New("nesting depth")
	errCbor       = errors.New("malformed cbor")
	errIndefinite = errors.New("indefinite")
	errFloat      = errors.New("float-or-simple")
	errTag        = errors.New("tag")
	errDuplicate  = errors.New("duplicate")
	errNonText    = errors.New("non-text")
	errTruncated  = errors.New("truncated")
	errNegative   = errors.New("negative")
	errTrailing   = errors.New("trailing")
)

type cborVal struct {
	kind byte // 0 uint, 2 bytes, 3 text, 4 array, 5 map
	u    uint64
	b    []byte
	s    string
	arr  []cborVal
	keys []string
	vals []cborVal
}

func encodeUint(n uint64) []byte {
	switch {
	case n < 24:
		return []byte{byte(n)}
	case n < 256:
		return []byte{24, byte(n)}
	case n < 65536:
		out := []byte{25, 0, 0}
		binary.BigEndian.PutUint16(out[1:], uint16(n))
		return out
	case n < 1<<32:
		out := []byte{26, 0, 0, 0, 0}
		binary.BigEndian.PutUint32(out[1:], uint32(n))
		return out
	default:
		out := []byte{27, 0, 0, 0, 0, 0, 0, 0, 0}
		binary.BigEndian.PutUint64(out[1:], n)
		return out
	}
}

func head(major byte, n uint64) []byte {
	h := encodeUint(n)
	h[0] |= major << 5
	return h
}

func encodeBytes(b []byte) []byte {
	return append(head(2, uint64(len(b))), b...)
}

func encodeText(s string) []byte {
	raw := []byte(s)
	return append(head(3, uint64(len(raw))), raw...)
}

func encodeArray(items [][]byte) []byte {
	out := head(4, uint64(len(items)))
	for _, item := range items {
		out = append(out, item...)
	}
	return out
}

func encodeMap(pairs [][2][]byte) []byte {
	// pairs are (encoded key, encoded value). Sort by encoded key bytes.
	sorted := append([][2][]byte(nil), pairs...)
	for i := 1; i < len(sorted); i++ {
		j := i
		for j > 0 && bytesLess(sorted[j][0], sorted[j-1][0]) {
			sorted[j], sorted[j-1] = sorted[j-1], sorted[j]
			j--
		}
	}
	out := head(5, uint64(len(sorted)))
	for _, p := range sorted {
		out = append(out, p[0]...)
		out = append(out, p[1]...)
	}
	return out
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

func textPair(key string, value []byte) [2][]byte {
	return [2][]byte{encodeText(key), value}
}

func readHead(data []byte, i int) (major byte, arg *uint64, next int, err error) {
	if i >= len(data) {
		return 0, nil, i, errTruncated
	}
	initial := data[i]
	i++
	major = initial >> 5
	additional := initial & 31
	if additional < 24 {
		v := uint64(additional)
		return major, &v, i, nil
	}
	width := 0
	switch additional {
	case 24:
		width = 1
	case 25:
		width = 2
	case 26:
		width = 4
	case 27:
		width = 8
	case 31:
		return major, nil, i, errIndefinite
	default:
		return 0, nil, i, errFloat
	}
	if i+width > len(data) {
		return 0, nil, i, errTruncated
	}
	var v uint64
	switch width {
	case 1:
		v = uint64(data[i])
	case 2:
		v = uint64(binary.BigEndian.Uint16(data[i : i+2]))
	case 4:
		v = uint64(binary.BigEndian.Uint32(data[i : i+4]))
	default:
		v = binary.BigEndian.Uint64(data[i : i+8])
	}
	return major, &v, i + width, nil
}

// capCount is the reservation for one array or map. The decoded count is
// not used as a size: nested heads can each name a count as large as the
// bytes still in the buffer.
func capCount(n uint64) int {
	if n > 64 {
		return 64
	}
	return int(n)
}

func decodeOne(data []byte, i int) (cborVal, int, error) {
	return decodeNested(data, i, 0)
}

// decodeNested reads one item. depth counts the arrays and maps around it.
func decodeNested(data []byte, i int, depth int) (cborVal, int, error) {
	major, arg, i, err := readHead(data, i)
	if err != nil {
		return cborVal{}, i, err
	}
	if arg == nil {
		return cborVal{}, i, errIndefinite
	}
	switch major {
	case 0:
		return cborVal{kind: 0, u: *arg}, i, nil
	case 1:
		return cborVal{}, i, errNegative
	case 2:
		// Compare the declared length as uint64 against the bytes still in
		// the buffer. i+int(*arg) overflows for a length near 2^63.
		if *arg > uint64(len(data)-i) {
			return cborVal{}, i, errTruncated
		}
		n := int(*arg)
		b := append([]byte(nil), data[i:i+n]...)
		return cborVal{kind: 2, b: b}, i + n, nil
	case 3:
		if *arg > uint64(len(data)-i) {
			return cborVal{}, i, errTruncated
		}
		n := int(*arg)
		raw := data[i : i+n]
		if !utf8.Valid(raw) {
			return cborVal{}, i, errCbor
		}
		return cborVal{kind: 3, s: string(raw)}, i + n, nil
	case 4:
		if depth >= MaxCBORNesting {
			return cborVal{}, i, errDepth
		}
		// One item is at least one byte, so a count past the bytes left
		// cannot be a well-formed array. The reservation is capped; the
		// count is not a size.
		if *arg > uint64(len(data)-i) {
			return cborVal{}, i, errTruncated
		}
		arr := make([]cborVal, 0, capCount(*arg))
		for n := uint64(0); n < *arg; n++ {
			item, next, err := decodeNested(data, i, depth+1)
			if err != nil {
				return cborVal{}, next, err
			}
			arr = append(arr, item)
			i = next
		}
		return cborVal{kind: 4, arr: arr}, i, nil
	case 5:
		if depth >= MaxCBORNesting {
			return cborVal{}, i, errDepth
		}
		// A map entry is a key and a value, at least two bytes.
		// The reservation is capped; the count is not a size.
		if *arg > uint64(len(data)-i)/2 {
			return cborVal{}, i, errTruncated
		}
		keys := make([]string, 0, capCount(*arg))
		vals := make([]cborVal, 0, capCount(*arg))
		seen := map[string]struct{}{}
		for n := uint64(0); n < *arg; n++ {
			key, next, err := decodeNested(data, i, depth+1)
			if err != nil {
				return cborVal{}, next, err
			}
			if key.kind != 3 {
				return cborVal{}, next, errNonText
			}
			if _, ok := seen[key.s]; ok {
				return cborVal{}, next, errDuplicate
			}
			seen[key.s] = struct{}{}
			val, next, err := decodeNested(data, next, depth+1)
			if err != nil {
				return cborVal{}, next, err
			}
			keys = append(keys, key.s)
			vals = append(vals, val)
			i = next
		}
		return cborVal{kind: 5, keys: keys, vals: vals}, i, nil
	case 6:
		return cborVal{}, i, errTag
	default:
		return cborVal{}, i, errFloat
	}
}

// CBORIssue names why data is not one section 7.3 item.
// An empty issue means the buffer is one well-formed item.
func CBORIssue(data []byte) string {
	_, err := decodeItem(data)
	if err == nil {
		return ""
	}
	return err.Error()
}

// UintEncoding is the shortest definite unsigned integer.
func UintEncoding(n uint64) []byte {
	return encodeUint(n)
}

// StageKeys returns one stage map's keys in encoded order.
func StageKeys(data []byte, index int) ([]string, error) {
	v, err := decodeItem(data)
	if err != nil {
		return nil, err
	}
	stages, ok := v.field("stages")
	if !ok || stages.kind != 4 || index < 0 || index >= len(stages.arr) {
		return nil, errCbor
	}
	st := stages.arr[index]
	if st.kind != 5 {
		return nil, errCbor
	}
	return append([]string(nil), st.keys...), nil
}

func decodeItem(data []byte) (cborVal, error) {
	v, end, err := decodeOne(data, 0)
	if err != nil {
		return cborVal{}, err
	}
	if end != len(data) {
		return cborVal{}, errTrailing
	}
	return v, nil
}

// decodeHead returns the first item. trailing is true when bytes follow it.
func decodeHead(data []byte) (cborVal, bool, error) {
	v, end, err := decodeOne(data, 0)
	if err != nil {
		return cborVal{}, false, err
	}
	return v, end != len(data), nil
}

func (v cborVal) field(name string) (cborVal, bool) {
	for i, key := range v.keys {
		if key == name {
			return v.vals[i], true
		}
	}
	return cborVal{}, false
}

// MapKeys returns definite-length text keys in encoded order.
func MapKeys(data []byte) ([]string, error) {
	v, err := decodeItem(data)
	if err != nil {
		return nil, err
	}
	if v.kind != 5 {
		return nil, errCbor
	}
	return append([]string(nil), v.keys...), nil
}

// DecodeUint decodes one unsigned integer, including a non-shortest form.
func DecodeUint(data []byte) (uint64, bool) {
	v, end, err := decodeOne(data, 0)
	if err != nil || end != len(data) || v.kind != 0 {
		return 0, false
	}
	return v.u, true
}
