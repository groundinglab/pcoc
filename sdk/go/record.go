package pcoc

// Section 14 verify record (SPEC §14).
//
// The record measures. It does not judge. Tokens are the section 4 set.
// There is no JSON null. A conforming CLI writes this object with no
// insignificant whitespace, keys in the order section 14 lists, and one
// trailing newline. These bytes are the record without that newline.

import (
	"strconv"
	"strings"
)

// shape item strings, table order (SPEC §14).
var shapeOrder = []string{
	"malformed cbor",
	"field set",
	"stage-0 path key",
	"W_17 path",
	"constant",
	"stages length",
	"leg_count",
	"checkpoint width",
	"leg result width",
	"seed beacon",
	"last_interrupt",
	"header height order",
	"header hash differ",
	"height_range",
}

// Index is the documents a verification reads. Get returns the body of
// GET {index-url}/index/{height} when the read returns a document.
// ok is false when the read does not return a document.
type Index interface {
	Get(height uint64) (body []byte, ok bool)
}

// VerifyInput is one section 14 verification.
// Draw, when non-nil, is the rig byte source for Rule A4.4. Production
// verify leaves it nil and reads the operating-system CSPRNG.
// SixRuns is how many Rule A4.4 draws the record holds. Zero means one.
type VerifyInput struct {
	Profile Profile
	Object  []byte
	Index   Index
	Draw    ByteSource
	SixRuns int
}

// Verify measures in under SPEC §14.
//
// A malformed CBOR value returns the section 14 record whose shape_item is
// exactly ["malformed cbor"]. A decode failure returns decFail true and no
// record. An unknown kind returns the Rule A5 record.
func Verify(in VerifyInput) (record []byte, decFail bool) {
	dr := DecodeObject(in.Object)
	switch dr.Class {
	case ClassMalformed:
		return malformedRecord(in.Profile), false
	case ClassDecodeFailure:
		return nil, true
	case ClassUnknownKind:
		return UnknownKindRecord(dr.Unknown.KindText), false
	default:
		return measure(in, dr.Object), false
	}
}

func malformedRecord(p Profile) []byte {
	return encodeRecord(p, recordBody{
		shape:     "mismatch",
		shapeItem: []string{"malformed cbor"},
		malformed: true,
	})
}

// encodeRecord writes the section 14 object, keys in the order section 14
// lists, with no insignificant whitespace. The caller adds the trailing
// newline on the CLI. These bytes are the record.
func encodeRecord(p Profile, rb recordBody) []byte {
	var b strings.Builder
	b.WriteByte('{')
	comma := false
	key := func(k, raw string) {
		if comma {
			b.WriteByte(',')
		}
		comma = true
		b.WriteByte('"')
		b.WriteString(k)
		b.WriteString(`":`)
		b.WriteString(raw)
	}
	str := func(s string) string {
		return `"` + s + `"`
	}
	key("profile", str(profileName(p)))
	if p.Name == "conformance-small" {
		key("sample_bytes", strconv.Itoa(p.SampleBytes))
		key("slot_bytes", strconv.Itoa(p.SlotBytes))
		key("terrain_bytes", strconv.Itoa(p.TerrainBytes))
		key("steps_per_leg", strconv.Itoa(p.StepsPerLeg))
	}
	if rb.shape == "" {
		rb.shape = "mismatch"
	}
	key("shape", str(rb.shape))
	key("shape_item", jsonStringArray(rb.shapeItem))

	if rb.malformed {
		key("headers", notDefHeaders())
		key("seed_value", str("mismatch"))
		key("checkpoints", notDefStages())
		key("waypoints", notDefWaypoints())
		key("paths", notDefPaths())
		key("anchors", repeatToken("not anchored", 16))
		key("anchored_total", "0")
		key("skip_list", "[]")
		bookendTokens(&b, &comma, "not defined")
		key("six_leg", str("not defined"))
		key("six_leg_runs", "[]")
		key("analysis", analysisRaw(rb, true))
		b.WriteByte('}')
		return []byte(b.String())
	}

	key("headers", headersRaw(rb.headers))
	key("seed_value", str(tok(rb.seedValue, "mismatch")))
	key("checkpoints", checkpointsRaw(rb))
	key("waypoints", waypointsRaw(rb))
	key("paths", pathsRaw(rb))
	key("anchors", anchorsRaw(rb))
	key("anchored_total", strconv.Itoa(rb.anchoredTotal))
	key("skip_list", jsonIntArray(rb.skipList))
	bookendFields(&b, &comma, rb)
	key("six_leg", sixLegRaw(rb))
	key("six_leg_runs", sixRunsRaw(rb))
	key("analysis", analysisRaw(rb, false))
	b.WriteByte('}')
	return []byte(b.String())
}

func profileName(p Profile) string {
	if p.Name == "conformance-small" {
		return "conformance-small"
	}
	return "production"
}

func tok(s, def string) string {
	if s == "" {
		return def
	}
	return s
}

func jsonStringArray(ss []string) string {
	var b strings.Builder
	b.WriteByte('[')
	for i, s := range ss {
		if i > 0 {
			b.WriteByte(',')
		}
		b.WriteByte('"')
		b.WriteString(s)
		b.WriteByte('"')
	}
	b.WriteByte(']')
	return b.String()
}

func jsonIntArray(ns []int) string {
	var b strings.Builder
	b.WriteByte('[')
	for i, n := range ns {
		if i > 0 {
			b.WriteByte(',')
		}
		b.WriteString(strconv.Itoa(n))
	}
	b.WriteByte(']')
	return b.String()
}

func repeatToken(token string, n int) string {
	ss := make([]string, n)
	for i := range ss {
		ss[i] = token
	}
	return jsonStringArray(ss)
}

func notDefHeaders() string {
	one := `{"stored_hash":"not defined","stored_height":"not defined","index_comparison":"not read","T":"not read"}`
	var b strings.Builder
	b.WriteByte('[')
	for i := 0; i < 19; i++ {
		if i > 0 {
			b.WriteByte(',')
		}
		b.WriteString(one)
	}
	b.WriteByte(']')
	return b.String()
}

func notDefStages() string {
	var b strings.Builder
	b.WriteByte('[')
	for g := 0; g < Stages; g++ {
		if g > 0 {
			b.WriteByte(',')
		}
		b.WriteString(`{"g":`)
		b.WriteString(strconv.Itoa(g))
		b.WriteString(`,"legs":"not defined"}`)
	}
	b.WriteByte(']')
	return b.String()
}

func notDefWaypoints() string {
	var b strings.Builder
	b.WriteByte('[')
	for g := 1; g <= 17; g++ {
		if g > 1 {
			b.WriteByte(',')
		}
		b.WriteString(`{"g":`)
		b.WriteString(strconv.Itoa(g))
		b.WriteString(`,"result":"not defined"}`)
	}
	b.WriteByte(']')
	return b.String()
}

func notDefPaths() string {
	var b strings.Builder
	b.WriteByte('[')
	for g := 1; g <= 17; g++ {
		if g > 1 {
			b.WriteByte(',')
		}
		if g < 17 {
			b.WriteString(`{"g":`)
			b.WriteString(strconv.Itoa(g))
			b.WriteString(`,"path_state":"skip","plant":"n/a","P_g":"not defined","height_P_g":"not defined","d_g":"not defined","claimed_block":"not defined"}`)
		} else {
			b.WriteString(`{"g":17,"w17_path":"not defined","claimed_block":"not defined"}`)
		}
	}
	b.WriteByte(']')
	return b.String()
}

func headersRaw(hs []hdr) string {
	var b strings.Builder
	b.WriteByte('[')
	for i := 0; i < 19; i++ {
		if i > 0 {
			b.WriteByte(',')
		}
		h := hdr{storedHash: "not defined", storedHeight: "not defined", indexCmp: "not read", t: "not read"}
		if i < len(hs) {
			h = hs[i]
		}
		b.WriteString(`{"stored_hash":`)
		b.WriteString(jsonStr(h.storedHash))
		b.WriteString(`,"stored_height":`)
		b.WriteString(numOrStr(h.storedHeight))
		b.WriteString(`,"index_comparison":`)
		b.WriteString(jsonStr(tok(h.indexCmp, "not read")))
		b.WriteString(`,"T":`)
		b.WriteString(numOrStr(tok(h.t, "not read")))
		b.WriteByte('}')
	}
	b.WriteByte(']')
	return b.String()
}

func jsonStr(s string) string {
	return `"` + s + `"`
}

// numOrStr writes a plain decimal when s is an integer spelling, and a
// JSON string otherwise.
func numOrStr(s string) string {
	if isPlainInt(s) {
		return s
	}
	return jsonStr(s)
}

func isPlainInt(s string) bool {
	if s == "" || s == "-" || s == "-0" {
		return false
	}
	i := 0
	if s[0] == '-' {
		i = 1
	}
	if i >= len(s) {
		return false
	}
	if s[i] == '0' && len(s) > i+1 {
		return false
	}
	for ; i < len(s); i++ {
		if s[i] < '0' || s[i] > '9' {
			return false
		}
	}
	return true
}

func checkpointsRaw(rb recordBody) string {
	var b strings.Builder
	b.WriteByte('[')
	for g := 0; g < Stages; g++ {
		if g > 0 {
			b.WriteByte(',')
		}
		b.WriteString(`{"g":`)
		b.WriteString(strconv.Itoa(g))
		b.WriteString(`,"legs":`)
		if !rb.cpDefined[g] {
			b.WriteString(`"not defined"`)
		} else {
			b.WriteByte('[')
			for j, r := range rb.cpLegs[g] {
				if j > 0 {
					b.WriteByte(',')
				}
				b.WriteString(`{"j":`)
				b.WriteString(strconv.Itoa(j))
				b.WriteString(`,"result":`)
				b.WriteString(jsonStr(r))
				b.WriteByte('}')
			}
			b.WriteByte(']')
		}
		b.WriteByte('}')
	}
	b.WriteByte(']')
	return b.String()
}

func waypointsRaw(rb recordBody) string {
	var b strings.Builder
	b.WriteByte('[')
	for g := 1; g <= 17; g++ {
		if g > 1 {
			b.WriteByte(',')
		}
		r := "not defined"
		if g-1 < len(rb.wp) && rb.wp[g-1] != "" {
			r = rb.wp[g-1]
		}
		b.WriteString(`{"g":`)
		b.WriteString(strconv.Itoa(g))
		b.WriteString(`,"result":`)
		b.WriteString(jsonStr(r))
		b.WriteByte('}')
	}
	b.WriteByte(']')
	return b.String()
}

func pathsRaw(rb recordBody) string {
	var b strings.Builder
	b.WriteByte('[')
	for g := 1; g <= 17; g++ {
		if g > 1 {
			b.WriteByte(',')
		}
		pr := rb.paths[g-1]
		if g < 17 {
			state := tok(pr.state, "skip")
			plant := pr.plant
			if plant == "" {
				if state == "skip" {
					plant = "n/a"
				} else {
					plant = "not read"
				}
			}
			b.WriteString(`{"g":`)
			b.WriteString(strconv.Itoa(g))
			b.WriteString(`,"path_state":`)
			b.WriteString(jsonStr(state))
			b.WriteString(`,"plant":`)
			b.WriteString(jsonStr(plant))
			b.WriteString(`,"P_g":`)
			if pr.pgObj {
				b.WriteString(blockObj(pr.pgHash, pr.claimH))
			} else {
				b.WriteString(`"not defined"`)
			}
			b.WriteString(`,"height_P_g":`)
			if pr.heightPg != "" {
				b.WriteString(numOrStr(pr.heightPg))
			} else {
				b.WriteString(`"not defined"`)
			}
			b.WriteString(`,"d_g":`)
			if pr.dg != "" {
				b.WriteString(numOrStr(pr.dg))
			} else {
				b.WriteString(`"not defined"`)
			}
			b.WriteString(`,"claimed_block":`)
			if pr.claimOK {
				b.WriteString(blockObj(pr.claimHash, pr.claimH))
			} else {
				b.WriteString(`"not defined"`)
			}
			b.WriteByte('}')
		} else {
			w := tok(pr.state, "not defined")
			b.WriteString(`{"g":17,"w17_path":`)
			b.WriteString(jsonStr(w))
			b.WriteString(`,"claimed_block":`)
			if pr.claimOK {
				b.WriteString(blockObj(pr.claimHash, pr.claimH))
			} else {
				b.WriteString(`"not defined"`)
			}
			b.WriteByte('}')
		}
	}
	b.WriteByte(']')
	return b.String()
}

func blockObj(hash string, height uint64) string {
	return `{"block_hash":"` + hash + `","block_height":` + strconv.FormatUint(height, 10) + `}`
}

func anchorsRaw(rb recordBody) string {
	ss := make([]string, 16)
	for i := 0; i < 16; i++ {
		ss[i] = tok(rb.anchors[i], "not anchored")
	}
	return jsonStringArray(ss)
}

func bookendTokens(b *strings.Builder, comma *bool, token string) {
	fields := []string{"a", "b", "N", "S", "delta_h"}
	q := `"` + token + `"`
	write := func(k, raw string) {
		if *comma {
			b.WriteByte(',')
		}
		*comma = true
		b.WriteByte('"')
		b.WriteString(k)
		b.WriteString(`":`)
		b.WriteString(raw)
	}
	for _, f := range fields {
		write(f, q)
	}
	write("fixed_start", `{"N":`+q+`,"S":`+q+`,"delta_h":`+q+`}`)
	write("P_b", q)
	write("height_P_b", q)
	write("index_comparison_P_b", q)
	write("T_P_b", q)
	write("height_I_a_minus_1", q)
	write("T_I_a_minus_1", q)
}

func bookendFields(b *strings.Builder, comma *bool, rb recordBody) {
	write := func(k, raw string) {
		if *comma {
			b.WriteByte(',')
		}
		*comma = true
		b.WriteByte('"')
		b.WriteString(k)
		b.WriteString(`":`)
		b.WriteString(raw)
	}
	put := func(k, v, def string) {
		if v == "" {
			v = def
		}
		write(k, numOrStr(v))
	}
	put("a", rb.a, "not defined")
	put("b", rb.b, "not defined")
	put("N", rb.n, "not defined")
	put("S", rb.s, "not defined")
	put("delta_h", rb.deltaH, "not defined")
	fs := `{"N":` + numOrStr(tok(rb.fsN, "not defined")) + `,"S":` + numOrStr(tok(rb.fsS, "not defined")) + `,"delta_h":` + numOrStr(tok(rb.fsD, "not defined")) + `}`
	write("fixed_start", fs)
	if rb.pbObj {
		write("P_b", blockObj(rb.pbHash, rb.pbHeight))
	} else {
		put("P_b", "", "not defined")
	}
	put("height_P_b", rb.heightPb, "not defined")
	put("index_comparison_P_b", rb.indexCmpPb, "not defined")
	put("T_P_b", rb.tPb, "not defined")
	put("height_I_a_minus_1", rb.heightIa, "not defined")
	put("T_I_a_minus_1", rb.tIa, "not defined")
}

func sixLegRaw(rb recordBody) string {
	switch rb.sixLeg {
	case "object":
		return sixObjRaw(rb.sixIdx, rb.sixRes)
	case "not read":
		return `"not read"`
	default:
		return `"not defined"`
	}
}

func sixObjRaw(idx []int, res []string) string {
	var b strings.Builder
	b.WriteString(`{"indices":`)
	b.WriteString(jsonIntArray(idx))
	b.WriteString(`,"results":`)
	b.WriteString(jsonStringArray(res))
	b.WriteByte('}')
	return b.String()
}

func sixRunsRaw(rb recordBody) string {
	if rb.sixRuns == "not read" {
		return `"not read"`
	}
	var b strings.Builder
	b.WriteByte('[')
	for i, r := range rb.sixRunObj {
		if i > 0 {
			b.WriteByte(',')
		}
		b.WriteString(sixObjRaw(r.idx, r.res))
	}
	b.WriteByte(']')
	return b.String()
}

func analysisRaw(rb recordBody, bare bool) string {
	var b strings.Builder
	b.WriteString(`{"whole_hike_legs":`)
	if bare {
		b.WriteString("0")
	} else {
		b.WriteString(strconv.Itoa(rb.wholeLegs))
	}
	b.WriteString(`,"whole_hike_span":`)
	if bare || rb.wholeSpan == "" {
		b.WriteString(`"not read"`)
	} else {
		b.WriteString(numOrStr(rb.wholeSpan))
	}
	b.WriteString(`,"per_stage":[`)
	for g := 0; g < Stages; g++ {
		if g > 0 {
			b.WriteByte(',')
		}
		b.WriteString(`{"g":`)
		b.WriteString(strconv.Itoa(g))
		b.WriteString(`,"checkpoint_count":`)
		if bare {
			b.WriteByte('0')
		} else {
			b.WriteString(strconv.Itoa(rb.perCount[g]))
		}
		b.WriteString(`,"span":`)
		sp := "not read"
		if !bare && rb.perSpan[g] != "" {
			sp = rb.perSpan[g]
		}
		b.WriteString(numOrStr(sp))
		b.WriteByte('}')
	}
	b.WriteString(`],"legs_outside":`)
	lo := rb.legsOutside
	if bare || lo == "" {
		lo = "not defined"
	}
	b.WriteString(numOrStr(lo))
	b.WriteString(`,"hike_legs":`)
	if bare {
		b.WriteByte('0')
	} else {
		b.WriteString(strconv.Itoa(rb.hikeLegs))
	}
	b.WriteString(`,"hike_span":`)
	hs := rb.hikeSpan
	if bare || hs == "" {
		hs = "not read"
	}
	b.WriteString(numOrStr(hs))
	b.WriteByte('}')
	return b.String()
}
