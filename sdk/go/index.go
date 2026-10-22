package pcoc

// Portable-index document reads (SPEC §9).
//
// A successful body is UTF-8 JSON: one JSON object and no second value.
// A byte order mark is not a successful read. A repeated member name in
// any object, including a nested object, is not a successful read.
//
// A header read is successful only when header is a JSON object and
// header.hash and header.height follow section 9. header.time follows
// section 9 on its own: a missing or unusable time leaves T not read and
// does not by itself make the header read unsuccessful.

import (
	"bytes"
	"encoding/json"
	"unicode/utf8"
)

// HeaderDoc is one successful header read (SPEC §9).
type HeaderDoc struct {
	HashNatural []byte
	Height      uint64
	Time        uint64
	HasTime     bool
}

// PlantFact is one recorded plant (SPEC §14.3), already reversed to natural
// order on txid and block_hash. root stays natural.
type PlantFact struct {
	Root        []byte
	Txid        []byte
	Vout        uint64
	BlockHash   []byte
	BlockHeight uint64
	OK          bool
}

// IndexDoc is one returned index document (SPEC §9, §14.3).
// HeaderOK is a successful header read. Plants are the recorded plants.
// Network is the top-level network string when it is a string.
type IndexDoc struct {
	Present  bool
	Network  string
	HasNet   bool
	HeaderOK bool
	Header   HeaderDoc
	Plants   []PlantFact
}

// ParseIndexBody parses one index body (SPEC §9).
// present is false when the body is not a successful read: not UTF-8, a
// BOM, not one JSON object, a repeated member, or a second value.
func ParseIndexBody(body []byte) (IndexDoc, bool) {
	if len(body) >= 3 && body[0] == 0xef && body[1] == 0xbb && body[2] == 0xbf {
		return IndexDoc{}, false
	}
	if !utf8.Valid(body) {
		return IndexDoc{}, false
	}
	if dupKeys(body) {
		return IndexDoc{}, false
	}
	dec := json.NewDecoder(bytes.NewReader(body))
	dec.UseNumber()
	var raw map[string]json.RawMessage
	if err := dec.Decode(&raw); err != nil {
		return IndexDoc{}, false
	}
	if dec.More() {
		return IndexDoc{}, false
	}
	// A trailing non-space byte after one value is a second value.
	var extra any
	if err := dec.Decode(&extra); err == nil {
		return IndexDoc{}, false
	}
	doc := IndexDoc{Present: true}
	if nb, ok := raw["network"]; ok {
		var s string
		if json.Unmarshal(nb, &s) == nil {
			doc.Network = s
			doc.HasNet = true
		}
	}
	if hb, ok := raw["header"]; ok {
		doc.Header, doc.HeaderOK = parseHeader(hb)
	}
	doc.Plants = collectPlants(raw)
	return doc, true
}

func parseHeader(b []byte) (HeaderDoc, bool) {
	var raw map[string]json.RawMessage
	dec := json.NewDecoder(bytes.NewReader(b))
	dec.UseNumber()
	if err := dec.Decode(&raw); err != nil {
		return HeaderDoc{}, false
	}
	hs, ok := raw["hash"]
	if !ok {
		return HeaderDoc{}, false
	}
	var hashStr string
	if json.Unmarshal(hs, &hashStr) != nil {
		return HeaderDoc{}, false
	}
	display, ok := ParseHex32(hashStr)
	if !ok {
		return HeaderDoc{}, false
	}
	ht, ok := raw["height"]
	if !ok {
		return HeaderDoc{}, false
	}
	height, ok := jsonUint(ht, MaxJSONInt)
	if !ok {
		return HeaderDoc{}, false
	}
	h := HeaderDoc{HashNatural: Reverse32(display), Height: height}
	if tb, ok := raw["time"]; ok {
		if t, ok := jsonUint(tb, uint64(1<<32-1)); ok {
			h.Time = t
			h.HasTime = true
		}
	}
	return h, true
}

// jsonUint accepts a JSON number that is an integer in 0 … max, with no
// fractional part and no exponent (SPEC §9).
func jsonUint(b []byte, max uint64) (uint64, bool) {
	var n json.Number
	if json.Unmarshal(b, &n) != nil {
		return 0, false
	}
	s := n.String()
	if s == "" || s[0] == '-' || s[0] == '+' {
		return 0, false
	}
	for _, c := range s {
		if c < '0' || c > '9' {
			return 0, false
		}
	}
	if len(s) > 1 && s[0] == '0' {
		return 0, false
	}
	var v uint64
	for _, c := range s {
		d := uint64(c - '0')
		if v > (1<<64-1)/10 || (v == (1<<64-1)/10 && d > (1<<64-1)%10) {
			return 0, false
		}
		v = v*10 + d
	}
	if v > max {
		return 0, false
	}
	return v, true
}

func collectPlants(raw map[string]json.RawMessage) []PlantFact {
	var out []PlantFact
	if pb, ok := raw["plants"]; ok {
		var arr []json.RawMessage
		if json.Unmarshal(pb, &arr) == nil {
			for _, e := range arr {
				if p, ok := onePlant(e); ok {
					out = append(out, p)
				}
			}
		}
	}
	if pb, ok := raw["plant"]; ok {
		trim := bytes.TrimSpace(pb)
		if !bytes.Equal(trim, []byte("null")) {
			if p, ok := onePlant(pb); ok {
				out = append(out, p)
			}
		}
	}
	return out
}

func onePlant(b []byte) (PlantFact, bool) {
	var raw map[string]json.RawMessage
	if json.Unmarshal(b, &raw) != nil {
		return PlantFact{}, false
	}
	rootS, ok1 := jsonString(raw["root"])
	txidS, ok2 := jsonString(raw["txid"])
	bhS, ok3 := jsonString(raw["block_hash"])
	vout, ok4 := jsonUint(raw["vout"], uint64(1<<32-1))
	bh, ok5 := jsonUint(raw["block_height"], uint64(1<<32-1))
	if !ok1 || !ok2 || !ok3 || !ok4 || !ok5 {
		return PlantFact{OK: false}, true
	}
	root, rOK := ParseHex32(rootS)
	txidD, tOK := ParseHex32(txidS)
	blockD, bOK := ParseHex32(bhS)
	if !rOK || !tOK || !bOK {
		return PlantFact{OK: false}, true
	}
	return PlantFact{
		Root: root, Txid: Reverse32(txidD), Vout: vout,
		BlockHash: Reverse32(blockD), BlockHeight: bh, OK: true,
	}, true
}

func jsonString(b []byte) (string, bool) {
	if len(b) == 0 {
		return "", false
	}
	var s string
	if json.Unmarshal(b, &s) != nil {
		return "", false
	}
	return s, true
}

// dupKeys reports a repeated member name in any JSON object of body.
// The scan is a structural walk: strings are skipped, and object keys at
// each nesting level are compared as decoded text.
func dupKeys(body []byte) bool {
	i := 0
	n := len(body)
	skipWS := func() {
		for i < n {
			c := body[i]
			if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
				i++
				continue
			}
			return
		}
	}
	readStr := func() ([]byte, bool) {
		return scanString(&i, body)
	}
	var walk func() bool
	walk = func() bool {
		skipWS()
		if i >= n {
			return false
		}
		switch body[i] {
		case '{':
			i++
			seen := map[string]struct{}{}
			skipWS()
			if i < n && body[i] == '}' {
				i++
				return false
			}
			for {
				skipWS()
				if i >= n || body[i] != '"' {
					return false
				}
				key, ok := readStr()
				if !ok {
					return true
				}
				if _, dup := seen[string(key)]; dup {
					return true
				}
				seen[string(key)] = struct{}{}
				skipWS()
				if i >= n || body[i] != ':' {
					return false
				}
				i++
				if walk() {
					return true
				}
				skipWS()
				if i >= n {
					return false
				}
				if body[i] == ',' {
					i++
					continue
				}
				if body[i] == '}' {
					i++
					return false
				}
				return false
			}
		case '[':
			i++
			skipWS()
			if i < n && body[i] == ']' {
				i++
				return false
			}
			for {
				if walk() {
					return true
				}
				skipWS()
				if i >= n {
					return false
				}
				if body[i] == ',' {
					i++
					continue
				}
				if body[i] == ']' {
					i++
					return false
				}
				return false
			}
		case '"':
			_, ok := readStr()
			return !ok
		default:
			for i < n {
				c := body[i]
				if c == ',' || c == '}' || c == ']' || c == ' ' || c == '\t' || c == '\n' || c == '\r' {
					break
				}
				i++
			}
			return false
		}
	}
	return walk()
}

func scanString(i *int, body []byte) ([]byte, bool) {
	if *i >= len(body) || body[*i] != '"' {
		return nil, false
	}
	*i++
	start := *i
	var buf []byte
	for *i < len(body) {
		c := body[*i]
		if c == '"' {
			*i++
			if buf == nil {
				return body[start : *i-1], true
			}
			return buf, true
		}
		if c == '\\' {
			if buf == nil {
				buf = append([]byte(nil), body[start:*i]...)
			}
			*i++
			if *i >= len(body) {
				return nil, false
			}
			esc := body[*i]
			*i++
			switch esc {
			case '"', '\\', '/':
				buf = append(buf, esc)
			case 'b':
				buf = append(buf, '\b')
			case 'f':
				buf = append(buf, '\f')
			case 'n':
				buf = append(buf, '\n')
			case 'r':
				buf = append(buf, '\r')
			case 't':
				buf = append(buf, '\t')
			case 'u':
				if *i+4 > len(body) {
					return nil, false
				}
				r, ok := hex4(body[*i : *i+4])
				if !ok {
					return nil, false
				}
				*i += 4
				buf = append(buf, string(rune(r))...)
			default:
				return nil, false
			}
			continue
		}
		if c < 0x20 {
			return nil, false
		}
		if buf != nil {
			buf = append(buf, c)
		}
		*i++
	}
	return nil, false
}

func hex4(b []byte) (uint32, bool) {
	var v uint32
	for _, c := range b {
		h, ok := hexVal(c)
		if !ok {
			return 0, false
		}
		v = v<<4 | uint32(h)
	}
	return v, true
}
