package pcoc

// Verify is SPEC §14. It records measurements. It does not judge them.
//
// Rule A5 runs first, before any terrain fill. An unknown kind returns the
// A5 record and nothing else.

import (
	"encoding/hex"
)

// shapeItemOrder is the closed set, table order (SPEC §14 verify record).
var shapeItemOrder = []string{
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

// JSON is a verify-record value: a token string, a number, an array, or an object.
type JSON interface{}

// VerifyInput is one section 14 verification.
//
// Profile is the profile in force. Index supplies the portable-index
// documents; a missing height is a read that does not return a document.
// Draw is the Rule A4.4 byte source. Nil means the draw is not read (the
// source cannot supply bytes). Runs is how many Rule A4.4 draws to record;
// zero records one draw when a draw is made.
//
// RecomputeLegs controls the six-leg leg recompute. The default records the
// draw and does not recompute legs (the caller passes stored results it has
// already checked, or sets Recompute). When Recompute is true, each drawn
// leg is recomputed by section 11.2 against the terrain built for this check.
type VerifyInput struct {
	Object    []byte
	Profile   Profile
	Index     *Index
	Draw      []byte
	Runs      int
	Recompute bool
	// MTPTimes, when non-nil, supplies header.time for the median-time-past
	// windows of the first and last beacons, keyed by height. A missing
	// height leaves that beacon's MTP not read. The beacon's own time still
	// comes from the Rule A4.2 document.
	MTPTimes map[uint64]*IndexDoc
}

// Verify runs section 14 and returns the record as a JSON-shaped map.
// When the kind is unknown, the map is the Rule A5 object and okA5 is true.
func Verify(in VerifyInput) (record map[string]JSON, a5 bool) {
	dec := DecodeObject(in.Object)
	if dec.Unknown != nil {
		return map[string]JSON{"kind": "unknown kind", "kind_text": dec.Unknown.KindText}, true
	}
	return verifyDecoded(dec, in), false
}

func verifyDecoded(dec DecodeResult, in VerifyInput) map[string]JSON {
	p := in.Profile
	rec := map[string]JSON{}
	rec["profile"] = p.Name
	if p.Name == "conformance-small" {
		rec["sample_bytes"] = p.SampleBytes
		rec["slot_bytes"] = p.SlotBytes
		rec["terrain_bytes"] = p.TerrainBytes
		rec["steps_per_leg"] = p.StepsPerLeg
	}

	items := map[string]bool{}
	if dec.Malformed {
		items["malformed cbor"] = true
	}
	o := dec.Object
	if o == nil {
		o = &Object{}
	}

	// Field set, from the raw map when the object decoded.
	if !dec.Malformed {
		checkFieldSet(in.Object, items)
	}

	if len(o.Stages) != Stages {
		items["stages length"] = true
	}
	if len(o.Stages) > 0 && o.Stages[0].HasPath {
		items["stage-0 path key"] = true
	}
	if len(o.Stages) > 17 {
		if !o.Stages[17].HasPath || len(o.Stages[17].Path) != 0 {
			items["W_17 path"] = true
		}
	} else if !dec.Malformed {
		items["W_17 path"] = true
	}

	// Constants against the profile in force (A4.1.2).
	if !dec.Malformed {
		if o.Kind != Kind || o.Fill != Fill ||
			o.SlotBytes != uint64(p.SlotBytes) ||
			o.NSlots != uint64(p.NSlots()) ||
			o.SampleBytes != uint64(p.SampleBytes) ||
			o.StepsPerLeg != uint64(p.StepsPerLeg) {
			items["constant"] = true
		}
	}

	// Per-stage widths and counts.
	for g := 0; g < len(o.Stages) && g < Stages; g++ {
		st := o.Stages[g]
		if st.LegCount < 1 || st.LegCount != uint64(len(st.Checkpoints)) || len(st.LegResults) != len(st.Checkpoints) {
			items["leg_count"] = true
		}
		for _, c := range st.Checkpoints {
			if len(c) != 32 {
				items["checkpoint width"] = true
			}
		}
		for _, lr := range st.LegResults {
			if len(lr) != 8 {
				items["leg result width"] = true
			}
		}
		if g == 0 && !bytesEqual(st.Beacon, o.SeedBeacon) {
			items["seed beacon"] = true
		}
		if heightAbove(st.BeaconHeight) {
			items["height_range"] = true
		}
	}
	if len(o.Stages) == 0 && !dec.Malformed {
		items["leg_count"] = true
		items["seed beacon"] = true
	}

	// last_interrupt.
	lastOK := o.HasLastInterrupt && o.HasLastInterruptHeight && len(o.LastInterrupt) == 32
	if len(o.Stages) > 17 {
		if !lastOK || bytesEqual(o.LastInterrupt, o.Stages[17].Beacon) || !(o.LastInterruptHeight > o.Stages[17].BeaconHeight) {
			items["last_interrupt"] = true
		}
	} else if !dec.Malformed {
		items["last_interrupt"] = true
	}
	if o.HasLastInterruptHeight && heightAbove(o.LastInterruptHeight) {
		items["height_range"] = true
	}

	// Header order across the stored headers.
	type hdr struct {
		hash   []byte
		height uint64
		hasH   bool
		hasHt  bool
	}
	headers := make([]hdr, 19)
	for k := 0; k < 19; k++ {
		headers[k].hash = nil
		headers[k].hasH = false
	}
	for g := 0; g < len(o.Stages) && g < 18; g++ {
		st := o.Stages[g]
		if len(st.Beacon) == 32 {
			headers[g].hash = st.Beacon
			headers[g].hasH = true
		}
		headers[g].height = st.BeaconHeight
		headers[g].hasHt = true
	}
	if o.HasLastInterrupt && len(o.LastInterrupt) == 32 {
		headers[18].hash = o.LastInterrupt
		headers[18].hasH = true
	}
	if o.HasLastInterruptHeight {
		headers[18].height = o.LastInterruptHeight
		headers[18].hasHt = true
	}
	for k := 1; k < 19; k++ {
		if !headers[k].hasHt || !headers[k-1].hasHt || !(headers[k].height > headers[k-1].height) {
			// Absent last height with a present previous height is a violation
			// unless only the hash is absent and the height is present and greater
			// (SPEC §14, the last_interrupt-hash-absent sentence). The generic
			// absent-field rule still marks the condition.
			if headers[k].hasHt && headers[k-1].hasHt && headers[k].height > headers[k-1].height {
				continue
			}
			items["header height order"] = true
		}
		if !headers[k].hasH || !headers[k-1].hasH || bytesEqual(headers[k].hash, headers[k-1].hash) {
			items["header hash differ"] = true
		}
	}

	shape := "match"
	shapeItem := []JSON{}
	if dec.Malformed {
		shape = "mismatch"
		shapeItem = []JSON{"malformed cbor"}
	} else if len(items) > 0 {
		shape = "mismatch"
		for _, name := range shapeItemOrder {
			if items[name] {
				shapeItem = append(shapeItem, name)
			}
		}
	}
	rec["shape"] = shape
	rec["shape_item"] = shapeItem

	// Headers (Rule A4.2).
	recHeaders := make([]JSON, 19)
	for k := 0; k < 19; k++ {
		hobj := map[string]JSON{}
		if !headers[k].hasH {
			hobj["stored_hash"] = "not defined"
		} else {
			hobj["stored_hash"] = hex.EncodeToString(headers[k].hash)
		}
		if !headers[k].hasHt || (headers[k].hasHt && heightAbove(headers[k].height)) {
			hobj["stored_height"] = "not defined"
		} else {
			hobj["stored_height"] = headers[k].height
		}
		hobj["index_comparison"] = "not read"
		hobj["T"] = "not read"
		if headers[k].hasH && headers[k].hasHt && !heightAbove(headers[k].height) && in.Index != nil {
			doc := in.Index.Doc(headers[k].height)
			if doc != nil && doc.HeaderOK {
				if bytesEqual(doc.HashNatural, headers[k].hash) && doc.Height == headers[k].height {
					hobj["index_comparison"] = "match"
					if doc.TimeOK {
						hobj["T"] = doc.Time
					}
				} else {
					hobj["index_comparison"] = "mismatch"
				}
			}
		}
		recHeaders[k] = hobj
	}
	rec["headers"] = recHeaders

	// Seed value (Rule A4.3).
	var computedSeed []byte
	if len(o.PublicKey) == 32 && len(o.SeedBeacon) == 32 {
		computedSeed = Seed(o.PublicKey, o.SeedBeacon)
	}
	seedValue := "mismatch"
	if computedSeed != nil && len(o.Stages) > 0 && bytesEqual(o.Stages[0].Waypoint, computedSeed) {
		seedValue = "match"
	}
	if dec.Malformed || len(o.Stages) == 0 || len(o.Stages[0].Waypoint) != 32 || computedSeed == nil {
		if dec.Malformed || len(o.PublicKey) != 32 || len(o.SeedBeacon) != 32 {
			seedValue = "mismatch"
		}
	}
	rec["seed_value"] = seedValue

	// Terrain for checkpoint samples and waypoint slot samples. Built once
	// for this check (SPEC §10). A malformed object or a missing key skips
	// the fill; those measurements are not defined from usable bytes.
	var terrain *Terrain
	if !dec.Malformed && computedSeed != nil && p.Check() == nil {
		terrain, _ = FillTerrain(p, o.PublicKey, o.SeedBeacon)
	}

	// Checkpoints and waypoints (Rule A4.3).
	cps := make([]JSON, 18)
	wps := make([]JSON, 17)
	for g := 0; g < 18; g++ {
		usable := !dec.Malformed && g < len(o.Stages) && stageBytesUsable(o, g)
		if !usable {
			cps[g] = map[string]JSON{"g": g, "legs": "not defined"}
			if g < 17 {
				// W_{g+1} needs stages g and g+1.
			}
			continue
		}
		st := o.Stages[g]
		legs := make([]JSON, len(st.Checkpoints))
		var prev []byte
		limb := stageLimb(o, g, computedSeed)
		for j := 0; j < len(st.Checkpoints); j++ {
			if j == 0 {
				prev = limb
			} else {
				prev = st.Checkpoints[j-1]
			}
			result := "mismatch"
			if j < len(st.LegResults) && len(st.LegResults[j]) == 8 && terrain != nil && len(prev) == 32 && len(limb) == 32 && len(st.Checkpoints[j]) == 32 {
				want := Checkpoint(terrain, prev, limb, u64le(st.LegResults[j]))
				if bytesEqual(want, st.Checkpoints[j]) {
					result = "match"
				}
			}
			legs[j] = map[string]JSON{"j": j, "result": result}
		}
		cps[g] = map[string]JSON{"g": g, "legs": legs}
	}
	for g := 0; g < 17; g++ {
		pairUsable := !dec.Malformed && g+1 < len(o.Stages) && stageBytesUsable(o, g) && len(o.Stages[g+1].Beacon) == 32 && len(o.Stages[g+1].Waypoint) == 32 && terrain != nil
		if !pairUsable {
			wps[g] = map[string]JSON{"g": g + 1, "result": "not defined"}
			continue
		}
		got := Waypoint(terrain, o.Stages[g+1].Beacon, o.Stages[g].Waypoint, o.Stages[g].Checkpoints)
		result := "mismatch"
		if bytesEqual(got, o.Stages[g+1].Waypoint) {
			result = "match"
		}
		wps[g] = map[string]JSON{"g": g + 1, "result": result}
	}
	rec["checkpoints"] = cps
	rec["waypoints"] = wps

	// Paths and anchors (Rule A4.5). The rest of the record is filled by
	// verifyPaths so this function stays readable.
	verifyPaths(rec, o, dec.Malformed, in, terrain)
	return rec
}

func stageBytesUsable(o *Object, g int) bool {
	if g >= len(o.Stages) {
		return false
	}
	st := o.Stages[g]
	if len(st.Beacon) != 32 || len(st.Waypoint) != 32 {
		return false
	}
	return true
}

func stageLimb(o *Object, g int, computedSeed []byte) []byte {
	if g == 0 {
		return computedSeed
	}
	if g < len(o.Stages) {
		return o.Stages[g].Waypoint
	}
	return nil
}

func heightAbove(h uint64) bool { return h > maxJSONInt }

func checkFieldSet(raw []byte, items map[string]bool) {
	v, rest, err := decodeOne(raw)
	if err != nil || len(rest) != 0 || v.typ != cborMap {
		return
	}
	// Top-level keys.
	allowed := map[string]bool{}
	for _, k := range topLevelKeys {
		allowed[k] = true
	}
	for _, k := range v.keys() {
		if !allowed[k] {
			items["field set"] = true
		}
	}
	for _, k := range topLevelKeys {
		if _, ok := v.lookup(k); !ok {
			// last_interrupt may be the absent pair the SPEC names. Still a
			// field-set violation. The shape_item list records it once.
			items["field set"] = true
		}
	}
	typeOK := map[string]cborType{
		"fill": cborText, "kind": cborText, "stages": cborArray,
		"n_slots": cborUint, "public_key": cborBytes, "slot_bytes": cborUint,
		"seed_beacon": cborBytes, "sample_bytes": cborUint, "steps_per_leg": cborUint,
		"last_interrupt": cborBytes, "last_interrupt_height": cborUint,
	}
	for k, typ := range typeOK {
		if fv, ok := v.lookup(k); ok && fv.typ != typ {
			items["field set"] = true
		}
	}
	sv, ok := v.lookup("stages")
	if !ok || sv.typ != cborArray {
		return
	}
	for g, st := range sv.array {
		if st.typ != cborMap {
			items["field set"] = true
			continue
		}
		var want []string
		if g == 0 {
			want = stage0Keys()
		} else {
			want = stageNKeys()
		}
		wset := map[string]bool{}
		for _, k := range want {
			wset[k] = true
		}
		for _, k := range st.keys() {
			if !wset[k] {
				items["field set"] = true
			}
		}
		for _, k := range want {
			if _, ok := st.lookup(k); !ok {
				items["field set"] = true
			}
		}
	}
}

// DrawSix is SPEC §14.6. It draws six distinct global indices from src.
// A source that cannot supply the next 8 bytes returns notRead.
// legTotal < 6 returns notDefined and consumes nothing.
func DrawSix(legTotal int, src *ByteSource) (indices []int, notDefined, notRead bool) {
	if legTotal < 6 {
		return nil, true, false
	}
	M := uint64(0) // 2^64 wraps; limit is computed in wide arithmetic below.
	_ = M
	chosen := map[int]bool{}
	for len(chosen) < 6 {
		q, ok := unbiased(legTotal, src)
		if !ok {
			return nil, false, true
		}
		if chosen[q] {
			continue
		}
		chosen[q] = true
		indices = append(indices, q)
	}
	return indices, false, false
}

// unbiased is one SPEC §14.6 draw from 0 … legTotal−1.
func unbiased(legTotal int, src *ByteSource) (int, bool) {
	if legTotal <= 0 {
		return 0, false
	}
	// limit = 2^64 − (2^64 mod legTotal). Computed without a big int:
	// 2^64 mod n = (2^64 − n * floor(2^64 / n)) but 2^64 does not fit.
	// floor(2^64 / n) = (2^64 − 1) / n, and 2^64 mod n = (2^64 − 1) % n + 1,
	// except when n divides 2^64, in which case the mod is 0.
	mod := mod2pow64(uint64(legTotal))
	var limit uint64
	if mod == 0 {
		limit = 0 // means "no rejection": every u is < limit in the wrapping sense. Handled below.
	} else {
		limit = 0 - mod // 2^64 − mod
	}
	for {
		b, ok := src.Next(8)
		if !ok {
			return 0, false
		}
		u := u64le(b)
		if mod == 0 || u < limit {
			return int(u % uint64(legTotal)), true
		}
	}
}

// mod2pow64 returns 2^64 mod n for n ≥ 1. n that divide 2^64 return 0.
func mod2pow64(n uint64) uint64 {
	if n&(n-1) == 0 {
		return 0
	}
	// (2^64 − 1) % n + 1, which equals 2^64 % n since n does not divide 2^64.
	return (^uint64(0) % n) + 1
}

// ByteSource is the section 14.6 byte source.
type ByteSource struct {
	b []byte
	i int
}

// NewByteSource wraps injected draw bytes (SPEC §16.1).
func NewByteSource(b []byte) *ByteSource { return &ByteSource{b: b} }

// Next returns the next n bytes, or false when the source is short.
func (s *ByteSource) Next(n int) ([]byte, bool) {
	if s == nil || s.i+n > len(s.b) {
		return nil, false
	}
	out := s.b[s.i : s.i+n]
	s.i += n
	return out, true
}

// LegRateFloor is SPEC §15.4 / Rule B2. The comparison is integer-only:
//
//	q · hike_legs ≥ p · hike_span, when hike_span > 0
//
// The outcome words are the ones Rule B2 names. A span that is not read is
// "held" (the state; there is no numeric rate). A span ≤ 0, and a numeric
// rate that falls short, are "refuse". A ratio that meets the floor is
// "meet": Rule B2 names that comparison and does not give it a second word.
func LegRateFloor(hikeLegs int, hikeSpan *int, p, q int) string {
	if hikeSpan == nil {
		return "held"
	}
	if *hikeSpan <= 0 {
		return "refuse"
	}
	if int64(q)*int64(hikeLegs) >= int64(p)*int64(*hikeSpan) {
		return "meet"
	}
	return "refuse"
}

// AnchorWindow is SPEC §13.1 / §14.3 for one stored stage height H and one
// header height. The outcome word is "anchored" when headerHeight is
// H + an offset in {2, 3, 4}, and "not anchored" otherwise. This is the
// window half of the anchor check. The interrupt-header and plant halves
// are the full Rule A4.5 measurement.
func AnchorWindow(h, headerHeight int) string {
	d := headerHeight - h
	if d == 2 || d == 3 || d == 4 {
		return "anchored"
	}
	return "not anchored"
}


