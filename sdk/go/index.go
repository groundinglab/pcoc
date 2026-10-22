package pcoc

// Portable-index documents as section 14 reads them (SPEC §9, §14.2, §14.3).
// A missing document is a read that does not return a document.

import (
	"bytes"
	"encoding/hex"
	"encoding/json"
	"strconv"
	"strings"
)

// IndexDoc is one successful portable-index body, parsed under section 9.
// HeaderOK is true only when header.hash and header.height follow section 9.
// TimeOK is true only when header.time follows section 9.
type IndexDoc struct {
	HeaderOK    bool
	HashNatural []byte // 32 bytes, display reversed once
	HashDisplay string // lowercase display hex
	Height      uint64
	TimeOK      bool
	Time        uint64
	Network     string
	HasNetwork  bool
	Plants      []Plant
	Raw         []byte
}

// Plant is one recorded plant (SPEC §14.3).
type Plant struct {
	Root        []byte // natural order
	Txid        []byte // natural order (display reversed once)
	Vout        uint64
	BlockHash   []byte // natural order
	BlockHeight uint64
	OK          bool
}

// Index is the documents a verifier was given, keyed by the height it would
// GET. A height with no entry is a read that does not return a document.
// Header and plant are separate reads of the same URL (SPEC §9): a rig may
// supply different bodies. When PlantAt is nil, the plant read uses the
// header document.
type Index struct {
	At      map[uint64]*IndexDoc
	PlantAt map[uint64]*IndexDoc
}

// Doc returns the header document at height, or nil when the read does not
// return a document.
func (x *Index) Doc(height uint64) *IndexDoc {
	if x == nil || x.At == nil {
		return nil
	}
	return x.At[height]
}

// PlantDoc returns the plant document at height.
func (x *Index) PlantDoc(height uint64) *IndexDoc {
	if x == nil {
		return nil
	}
	if x.PlantAt != nil {
		if d, ok := x.PlantAt[height]; ok {
			return d
		}
		return nil
	}
	return x.Doc(height)
}

// ParseIndexBody parses one section 9 body. A body that is not one JSON
// object, has a BOM, is not UTF-8, or repeats a member name returns ok=false.
// A body that is one object but has no usable header still returns ok=true
// with HeaderOK false: that is a returned document (SPEC §9).
func ParseIndexBody(body []byte) (*IndexDoc, bool) {
	if len(body) >= 3 && body[0] == 0xef && body[1] == 0xbb && body[2] == 0xbf {
		return nil, false
	}
	if !utf8OK(body) {
		return nil, false
	}
	if !jsonOneObjectNoDup(body) {
		return nil, false
	}
	dec := json.NewDecoder(bytes.NewReader(body))
	dec.UseNumber()
	var raw map[string]json.RawMessage
	if err := dec.Decode(&raw); err != nil {
		return nil, false
	}
	if dec.More() {
		return nil, false
	}
	doc := &IndexDoc{Raw: append([]byte(nil), body...)}
	if nw, ok := raw["network"]; ok {
		var s string
		if json.Unmarshal(nw, &s) == nil {
			doc.Network = s
			doc.HasNetwork = true
		}
	}
	if hraw, ok := raw["header"]; ok {
		parseHeader(doc, hraw)
	}
	if p, ok := raw["plant"]; ok {
		if pl, ok := parsePlant(p); ok {
			doc.Plants = append(doc.Plants, pl)
		}
	}
	if ps, ok := raw["plants"]; ok {
		var arr []json.RawMessage
		if json.Unmarshal(ps, &arr) == nil {
			for _, el := range arr {
				if pl, ok := parsePlant(el); ok {
					doc.Plants = append(doc.Plants, pl)
				}
			}
		}
	}
	return doc, true
}

func parseHeader(doc *IndexDoc, raw json.RawMessage) {
	if !jsonOneObjectNoDup(raw) {
		return
	}
	var m map[string]json.RawMessage
	if json.Unmarshal(raw, &m) != nil {
		return
	}
	hs, ok := jsonString(m["hash"])
	if !ok || !hex64(hs) {
		return
	}
	ht, ok := jsonUint(m["height"], maxJSONInt)
	if !ok {
		return
	}
	disp, err := hex.DecodeString(hs)
	if err != nil || len(disp) != 32 {
		return
	}
	nat := make([]byte, 32)
	for i := 0; i < 32; i++ {
		nat[i] = disp[31-i]
	}
	doc.HeaderOK = true
	doc.HashNatural = nat
	doc.HashDisplay = strings.ToLower(hs)
	doc.Height = ht
	if t, ok := jsonUint(m["time"], uint64(^uint32(0))); ok {
		doc.TimeOK = true
		doc.Time = t
	}
}

func parsePlant(raw json.RawMessage) (Plant, bool) {
	if !jsonOneObjectNoDup(raw) {
		return Plant{}, false
	}
	var m map[string]json.RawMessage
	if json.Unmarshal(raw, &m) != nil {
		return Plant{}, false
	}
	rootS, ok1 := jsonString(m["root"])
	txidS, ok2 := jsonString(m["txid"])
	bhS, ok3 := jsonString(m["block_hash"])
	vout, ok4 := jsonUint(m["vout"], uint64(^uint32(0)))
	bh, ok5 := jsonUint(m["block_height"], uint64(^uint32(0)))
	if !ok1 || !ok2 || !ok3 || !ok4 || !ok5 || !hex64(rootS) || !hex64(txidS) || !hex64(bhS) {
		return Plant{}, false
	}
	root, _ := hex.DecodeString(rootS)
	txidD, _ := hex.DecodeString(txidS)
	bhD, _ := hex.DecodeString(bhS)
	txid := make([]byte, 32)
	block := make([]byte, 32)
	for i := 0; i < 32; i++ {
		txid[i] = txidD[31-i]
		block[i] = bhD[31-i]
	}
	return Plant{Root: root, Txid: txid, Vout: vout, BlockHash: block, BlockHeight: bh, OK: true}, true
}

func jsonString(raw json.RawMessage) (string, bool) {
	if len(raw) == 0 || raw[0] != '"' {
		return "", false
	}
	var s string
	if json.Unmarshal(raw, &s) != nil {
		return "", false
	}
	return s, true
}

// jsonUint accepts a JSON number with no fractional part and no exponent,
// in 0 … max (SPEC §9).
func jsonUint(raw json.RawMessage, max uint64) (uint64, bool) {
	if len(raw) == 0 || raw[0] == '"' {
		return 0, false
	}
	s := string(raw)
	if s == "" || s[0] == '-' || strings.ContainsAny(s, ".eE+") {
		return 0, false
	}
	if len(s) > 1 && s[0] == '0' {
		return 0, false
	}
	n, err := strconv.ParseUint(s, 10, 64)
	if err != nil || n > max {
		return 0, false
	}
	return n, true
}

func hex64(s string) bool {
	if len(s) != 64 {
		return false
	}
	for i := 0; i < 64; i++ {
		c := s[i]
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) {
			return false
		}
	}
	return true
}

const maxJSONInt = uint64(1<<53 - 1)

// jsonOneObjectNoDup reports whether b is one JSON object with no duplicate
// member name at any depth (SPEC §9).
func jsonOneObjectNoDup(b []byte) bool {
	dec := json.NewDecoder(bytes.NewReader(b))
	dec.UseNumber()
	if !walkJSON(dec, true) {
		return false
	}
	return !dec.More()
}

func walkJSON(dec *json.Decoder, wantObject bool) bool {
	tok, err := dec.Token()
	if err != nil {
		return false
	}
	d, ok := tok.(json.Delim)
	if !ok {
		return !wantObject
	}
	switch d {
	case '{':
		seen := map[string]struct{}{}
		for dec.More() {
			kt, err := dec.Token()
			if err != nil {
				return false
			}
			ks, ok := kt.(string)
			if !ok {
				return false
			}
			if _, dup := seen[ks]; dup {
				return false
			}
			seen[ks] = struct{}{}
			if !walkJSON(dec, false) {
				return false
			}
		}
		end, err := dec.Token()
		return err == nil && end == json.Delim('}')
	case '[':
		if wantObject {
			return false
		}
		for dec.More() {
			if !walkJSON(dec, false) {
				return false
			}
		}
		end, err := dec.Token()
		return err == nil && end == json.Delim(']')
	default:
		return false
	}
}
