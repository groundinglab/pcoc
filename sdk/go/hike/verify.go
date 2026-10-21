package hike

import (
	"bytes"
	"encoding/json"
	"math/bits"
	"sort"
	"strconv"
)

// Index is the portable index keyed by height. A missing key is an
// unsuccessful read. A present body is still unsuccessful when section 9
// rejects it.
type Index struct {
	Documents map[uint64]string
	// Fetch loads a body that is not already in Documents.
	// ok is false when that read did not return a body.
	Fetch func(height uint64) (body string, ok bool)
}

func (x Index) Read(height uint64) *HeaderRead {
	body, ok := x.hasBody(height)
	if !ok {
		return nil
	}
	return ReadIndexDocument(body)
}

// hasBody reports whether a document was returned for height.
// A refused connection, a missing key in a supplied map, and a non-200
// response are false. A body that section 9 rejects is still true: the
// document was read. An index with no map and no fetch was not asked, and
// the nil chain stays a successful bind.
func (x Index) hasBody(height uint64) (string, bool) {
	if x.Documents != nil {
		if body, ok := x.Documents[height]; ok {
			return body, true
		}
	}
	if x.Fetch != nil {
		return x.Fetch(height)
	}
	if x.Documents != nil {
		return "", false
	}
	return "", true
}

// Verify measures one hike object. It does not apply section 15.
// stream is the rig draw byte source. A nil stream with a decodable object
// that has at least six legs records the draw as not read.
// omitSix leaves six_leg and six_leg_runs off the record (earlier objects
// that publish no draw).
func Verify(data []byte, profile Profile, index Index, stream []byte, omitSix bool) (Record, bool) {
	return verifyWithChain(data, profile, index, nil, stream, omitSix)
}

// verifyWithChain is Verify. pathChain is the section 14.3 bind for a present
// path that sits in the anchor window and names a stored interrupt header.
// A nil callback is a successful bind when the index returned a document for
// that block. No document is plant `not read`. It is not called for an earlier kind.
func verifyWithChain(data []byte, profile Profile, index Index, pathChain func(PathLocators) error, stream []byte, omitSix bool) (Record, bool) {
	item, trailing, err := decodeHead(data)
	if err != nil {
		return undecodedV2(profile), false
	}
	// A top level that is not a map is no hike at all: the same undecoded
	// ["malformed cbor"] record as bytes that do not decode (rc.2; Rust, Go
	// and Java agree).
	if item.kind != 5 {
		return undecodedV2(profile), false
	}
	if kind, ok := textOf(item, "kind"); ok && IsEarlierKind(kind) {
		return earlierVersionRecord(kind), true
	}
	// A trailing byte on a map that is not a usable hike is still the
	// section 14 item ["malformed cbor"], on the undecoded record.
	if trailing && !v2Usable(item) {
		return undecodedV2(profile), true
	}
	rec := measure(item, profile, index, pathChain, stream, omitSix)
	if trailing {
		rec.Shape = "mismatch"
		rec.ShapeItem = []string{"malformed cbor"}
	}
	return rec, true
}

// undecodedBase is the bookend skeleton undecodedV2 starts from.
func undecodedBase(profile Profile) Record {
	header := Header{
		StoredHash:      jstr("not defined"),
		StoredHeight:    jstr("not defined"),
		IndexComparison: "not read",
		T:               jstr("not read"),
	}
	headers := make([]Header, 19)
	for i := range headers {
		headers[i] = header
	}
	cps := make([]CheckpointRow, 18)
	for g := 0; g < 18; g++ {
		cps[g] = CheckpointRow{G: g, Legs: jstr("not defined")}
	}
	rec := emptyBookends(profile, "mismatch", []string{"malformed cbor"}, headers, cps)
	rec.SeedValue = "mismatch"
	rec.SixLeg = jstr("not defined")
	return rec
}

// undecodedV2 is the section 14 record when no usable hike was obtained.
// shape_item is exactly ["malformed cbor"]. Counts and results are not
// defined. six_leg and six_leg_runs are not read.
func undecodedV2(profile Profile) Record {
	rec := undecodedBase(profile)
	rec.SeedValue = "not defined"
	for i := range rec.Waypoints {
		rec.Waypoints[i].Result = "not defined"
	}
	nd := jstr("not defined")
	for g := 1; g < Stages; g++ {
		if g == 17 {
			rec.Paths[g-1] = jraw(path17{G: 17, W17: "not defined", Claimed: nd})
			continue
		}
		rec.Paths[g-1] = jraw(pathSkip{
			G: g, PathState: "not defined", Plant: "not defined",
			PG: nd, HeightPG: nd, DG: nd, Claimed: nd,
		})
	}
	for i := range rec.Analysis.PerStage {
		rec.Analysis.PerStage[i].CheckpointCount = jstr("not defined")
	}
	rec.Analysis.WholeHikeLegs = jstr("not defined")
	rec.Analysis.HikeLegs = jstr("not defined")
	rec.Analysis.HikeSpan = jstr("not read")
	rec.SixLeg = jstr("not read")
	rec.SixLegRuns = jstr("not read")
	return rec
}

// v2Usable is the reference read_measured_object '?' boundary.
// A missing or mistyped required field is not a usable hike.
// A wrong value, a stage-0 path key, or leg_count below 1 is a shape note
// on a usable hike.
func v2Usable(item cborVal) bool {
	if _, ok := textOf(item, "kind"); !ok {
		return false
	}
	if _, ok := textOf(item, "fill"); !ok {
		return false
	}
	for _, name := range []string{"slot_bytes", "n_slots", "sample_bytes", "steps_per_leg"} {
		if _, ok, _ := uintField(item, name); !ok {
			return false
		}
	}
	pub, ok := bytesOf(item, "public_key")
	if !ok || len(pub) != 32 {
		return false
	}
	seed, ok := bytesOf(item, "seed_beacon")
	if !ok || len(seed) != 32 {
		return false
	}
	stages, ok := item.field("stages")
	if !ok || stages.kind != 4 {
		return false
	}
	for _, stage := range stages.arr {
		if !stageUsable(stage) {
			return false
		}
	}
	return true
}

func stageUsable(stage cborVal) bool {
	if stage.kind != 5 {
		return false
	}
	_, pathKey := stage.field("path")
	expect := stage0Keys
	if pathKey {
		expect = stageKeys
	}
	if !exactKeys(stage.keys, expect) {
		return false
	}
	beacon, ok := bytesOf(stage, "beacon")
	if !ok || len(beacon) != 32 {
		return false
	}
	waypoint, ok := bytesOf(stage, "waypoint")
	if !ok || len(waypoint) != 32 {
		return false
	}
	if pathKey {
		if _, ok := bytesOf(stage, "path"); !ok {
			return false
		}
	}
	legCount, ok, _ := uintField(stage, "leg_count")
	if !ok {
		return false
	}
	if _, ok, _ := uintField(stage, "beacon_height"); !ok {
		return false
	}
	checks, ok := bytesWidth(stage, "checkpoints", 32)
	if !ok {
		return false
	}
	legs, ok := bytesWidth(stage, "leg_results", 8)
	if !ok {
		return false
	}
	return uint64(len(checks)) == legCount && len(checks) == len(legs)
}

func bytesWidth(v cborVal, name string, width int) ([][]byte, bool) {
	f, ok := v.field(name)
	if !ok || f.kind != 4 {
		return nil, false
	}
	out := make([][]byte, len(f.arr))
	for i, item := range f.arr {
		if item.kind != 2 || len(item.b) != width {
			return nil, false
		}
		out[i] = item.b
	}
	return out, true
}

// earlierVersionRecord is Rule A5. Sections 14.1–14.6 are not applied.
// Marshal writes {"version":"earlier version","kind":"<kind>"}.
func earlierVersionRecord(kind string) Record {
	return Record{EarlierKind: kind}
}

// Record is the section 14 verify record.
type Record struct {
	Profile      string            `json:"profile"`
	SampleBytes  *int              `json:"sample_bytes,omitempty"`
	SlotBytes    *int              `json:"slot_bytes,omitempty"`
	TerrainBytes *int              `json:"terrain_bytes,omitempty"`
	StepsPerLeg  *int              `json:"steps_per_leg,omitempty"`
	Shape        string            `json:"shape"`
	ShapeItem    []string          `json:"shape_item"`
	Headers      []Header          `json:"headers"`
	SeedValue    string            `json:"seed_value"`
	Checkpoints  []CheckpointRow   `json:"checkpoints"`
	Waypoints    []WaypointRow     `json:"waypoints"`
	Paths        []json.RawMessage `json:"paths"`
	// Anchors is the 16-token list on a v2.0 record.
	Anchors           []string        `json:"anchors,omitempty"`
	AnchoredTotal     int             `json:"anchored_total"`
	SkipList          []int           `json:"skip_list"`
	A                 json.RawMessage `json:"a"`
	B                 json.RawMessage `json:"b"`
	N                 json.RawMessage `json:"N"`
	S                 json.RawMessage `json:"S"`
	DeltaH            json.RawMessage `json:"delta_h"`
	FixedStart        FixedStart      `json:"fixed_start"`
	PB                json.RawMessage `json:"P_b"`
	HeightPB          json.RawMessage `json:"height_P_b"`
	IndexComparisonPB json.RawMessage `json:"index_comparison_P_b"`
	TPB               json.RawMessage `json:"T_P_b"`
	HeightIAMinus1    json.RawMessage `json:"height_I_a_minus_1"`
	TIAMinus1         json.RawMessage `json:"T_I_a_minus_1"`
	SixLeg            json.RawMessage `json:"six_leg,omitempty"`
	SixLegRuns        json.RawMessage `json:"six_leg_runs,omitempty"`
	Analysis          Analysis        `json:"analysis"`
	// EarlierKind is Rule A5. When set, Marshal writes only the version line.
	EarlierKind string `json:"-"`
}

type Header struct {
	StoredHash      json.RawMessage `json:"stored_hash"`
	StoredHeight    json.RawMessage `json:"stored_height"`
	IndexComparison string          `json:"index_comparison"`
	T               json.RawMessage `json:"T"`
}

type CheckpointRow struct {
	G    int             `json:"g"`
	Legs json.RawMessage `json:"legs"`
}

type WaypointRow struct {
	G      int    `json:"g"`
	Result string `json:"result"`
}

type FixedStart struct {
	N      json.RawMessage `json:"N"`
	S      json.RawMessage `json:"S"`
	DeltaH json.RawMessage `json:"delta_h"`
}

type Analysis struct {
	WholeHikeLegs json.RawMessage `json:"whole_hike_legs"`
	WholeHikeSpan json.RawMessage `json:"whole_hike_span"`
	PerStage      []StageSpan     `json:"per_stage"`
	LegsOutside   json.RawMessage `json:"legs_outside"`
	HikeLegs      json.RawMessage `json:"hike_legs"`
	HikeSpan      json.RawMessage `json:"hike_span"`
}

type StageSpan struct {
	G               int             `json:"g"`
	CheckpointCount json.RawMessage `json:"checkpoint_count"`
	Span            json.RawMessage `json:"span"`
}

type SixDraw struct {
	Indices []uint64 `json:"indices"`
	Results []string `json:"results"`
}

func (r Record) Marshal() []byte {
	if r.EarlierKind != "" {
		raw, err := json.Marshal(struct {
			Version string `json:"version"`
			Kind    string `json:"kind"`
		}{Version: EarlierVersionToken, Kind: r.EarlierKind})
		if err != nil {
			return []byte("{}\n")
		}
		return append(raw, '\n')
	}
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	if err := enc.Encode(r); err != nil {
		return []byte("{}\n")
	}
	return buf.Bytes()
}

func jstr(s string) json.RawMessage {
	b, err := json.Marshal(s)
	if err != nil {
		return json.RawMessage(`""`)
	}
	return b
}

func jnum(n int64) json.RawMessage {
	return json.RawMessage(strconv.FormatInt(n, 10))
}

func jraw(v any) json.RawMessage {
	b, err := json.Marshal(v)
	if err != nil {
		return jstr("not defined")
	}
	return b
}

func measure(item cborVal, profile Profile, index Index, pathChain func(PathLocators) error, stream []byte, omitSix bool) Record {
	items := shapeItems(item, profile)
	shape := "match"
	if len(items) > 0 {
		shape = "mismatch"
	}
	pub, _ := bytesOf(item, "public_key")
	seedBeacon, _ := bytesOf(item, "seed_beacon")
	var computed []byte
	if len(pub) == 32 && len(seedBeacon) == 32 {
		computed = SeedValue(pub, seedBeacon)
	}
	stages, _ := item.field("stages")
	headers := make([]Header, 19)
	for g := 0; g < Stages; g++ {
		st, _ := stageAt(stages, g)
		headers[g] = headerFromStage(st, index)
	}
	headers[18] = headerFromLast(item, index)

	seedToken := "mismatch"
	if st, ok := stageAt(stages, 0); ok {
		if wp, ok := bytesField(st, "waypoint"); ok && computed != nil && bytes.Equal(wp, computed) {
			seedToken = "match"
		}
	}

	var terrain *Terrain
	if len(pub) == 32 && len(seedBeacon) == 32 {
		terrain = FillTerrain(pub, seedBeacon, profile)
	}

	counts := make([]int, Stages)
	cps := make([]CheckpointRow, Stages)
	for g := 0; g < Stages; g++ {
		st, ok := stageAt(stages, g)
		if !ok {
			// An absent stage has no checkpoint list. The count is not zero.
			cps[g] = CheckpointRow{G: g, Legs: jstr("not defined")}
			counts[g] = -1
			continue
		}
		checks := byteList(st, "checkpoints")
		results := byteList(st, "leg_results")
		counts[g] = len(checks)
		if checks == nil || !allLen(checks, 32) {
			cps[g] = CheckpointRow{G: g, Legs: jstr("not defined")}
			continue
		}
		var limb []byte
		if g == 0 {
			limb = computed
		} else if wp, ok := bytesField(st, "waypoint"); ok {
			limb = wp
		}
		legs := make([]LegOut, len(checks))
		for j, checkpoint := range checks {
			if j >= len(results) || len(results[j]) != 8 || terrain == nil || len(limb) != 32 {
				legs[j] = LegOut{J: j, Result: "mismatch"}
				continue
			}
			var previous []byte
			if j == 0 {
				previous = limb
			} else {
				previous = checks[j-1]
			}
			digest := CheckpointDigest(terrain.Blob, previous, limb, U64LE(results[j]), profile)
			token := "mismatch"
			if bytes.Equal(digest, checkpoint) {
				token = "match"
			}
			legs[j] = LegOut{J: j, Result: token}
		}
		cps[g] = CheckpointRow{G: g, Legs: jraw(legs)}
	}

	ways := make([]WaypointRow, 0, Stages-1)
	for g := 0; g < Stages-1; g++ {
		cur, okCur := stageAt(stages, g)
		next, okNext := stageAt(stages, g+1)
		token := "not defined"
		if okCur && okNext && terrain != nil {
			interrupt, okB := bytesField(next, "beacon")
			limb, okW := bytesField(cur, "waypoint")
			stored, okS := bytesField(next, "waypoint")
			checks := byteList(cur, "checkpoints")
			if okB && okW && okS && len(interrupt) == 32 && len(limb) == 32 && len(stored) == 32 && checks != nil && allLen(checks, 32) {
				digest := WaypointDigest(terrain.Blob, interrupt, limb, checks, profile)
				token = "mismatch"
				if bytes.Equal(digest, stored) {
					token = "match"
				}
			}
		}
		ways = append(ways, WaypointRow{G: g + 1, Result: token})
	}

	paths := make([]json.RawMessage, 0, Stages-1)
	anchored := 0
	present := 0
	var skips []int
	var anchors []string
	var facts []pathMeasure
	if profile.Kind == Kind {
		anchors = make([]string, 0, 16)
		facts = make([]pathMeasure, 0, 16)
	}
	for g := 1; g < Stages; g++ {
		st, ok := stageAt(stages, g)
		path := []byte{}
		var waypoint []byte
		var beaconHeight uint64
		if ok {
			if p, ok := bytesField(st, "path"); ok {
				path = p
			}
			if w, ok := bytesField(st, "waypoint"); ok {
				waypoint = w
			}
			if h, okH, over := uintField(st, "beacon_height"); okH && !over {
				beaconHeight = h
			}
		}
		var row any
		var pathState, plant, anchor string
		var measured pathMeasure
		if profile.Kind == Kind && !ok {
			// A missing stage is not an empty stored path. It is not `skip`.
			measured = absentStagePath(g)
			row, pathState, plant, anchor = measured.row, measured.state, measured.plant, measured.anchor
		} else if profile.Kind == Kind {
			measured = section14Path(g, path, waypoint, beaconHeight, headers[g], item, stages, pathChain, index)
			row, pathState, plant, anchor = measured.row, measured.state, measured.plant, measured.anchor
		} else {
			row, pathState, plant = pathRow(g, path)
		}
		paths = append(paths, jraw(row))
		if g < 17 {
			if profile.Kind == Kind {
				anchors = append(anchors, anchor)
				facts = append(facts, measured)
			}
			if pathState == "present" {
				present++
				if plant == "match" {
					anchored++
				}
			}
			if pathState == "skip" {
				skips = append(skips, g)
			}
		}
	}
	if skips == nil {
		skips = []int{}
	}

	book := bookendFields(present, anchored)
	if profile.Kind == Kind {
		book = v2Book(present, facts, anchors, headers, cps, counts, index)
	}
	times := make([]json.RawMessage, len(headers))
	for i, h := range headers {
		times[i] = h.T
	}
	per := make([]StageSpan, Stages)
	whole := 0
	wholeDefined := true
	for g := 0; g < Stages; g++ {
		var span json.RawMessage
		switch g {
		case 0:
			span = spanOf(times[1], times[0])
		case 17:
			span = spanOf(times[18], times[17])
		default:
			span = spanOf(times[g+1], times[g])
		}
		var count json.RawMessage
		if counts[g] < 0 {
			count = jstr("not defined")
			if g >= 1 {
				wholeDefined = false
			}
		} else {
			count = jnum(int64(counts[g]))
			if g >= 1 {
				whole += counts[g]
			}
		}
		per[g] = StageSpan{G: g, CheckpointCount: count, Span: span}
	}
	wholeLegs := jnum(int64(whole))
	if !wholeDefined {
		wholeLegs = jstr("not defined")
	}
	outside := jstr("not defined")
	if book.defined {
		outside = jnum(0)
	}
	if profile.Kind == Kind {
		outside = book.outside
	}
	hikeLegsN := 0
	hikeLegsOK := true
	for g := 0; g < Stages; g++ {
		if counts[g] < 0 {
			hikeLegsOK = false
			break
		}
		hikeLegsN += counts[g]
	}
	hikeLegs := jnum(int64(hikeLegsN))
	if !hikeLegsOK {
		hikeLegs = jstr("not defined")
	}

	rec := Record{
		Profile:           profile.Name,
		Shape:             shape,
		ShapeItem:         items,
		Headers:           headers,
		SeedValue:         seedToken,
		Checkpoints:       cps,
		Waypoints:         ways,
		Paths:             paths,
		Anchors:           anchors,
		AnchoredTotal:     anchored,
		SkipList:          skips,
		A:                 book.a,
		B:                 book.b,
		N:                 book.n,
		S:                 book.s,
		DeltaH:            book.delta,
		PB:                book.pb,
		HeightPB:          book.heightPB,
		IndexComparisonPB: book.cmpPB,
		TPB:               book.tpb,
		HeightIAMinus1:    book.heightI,
		TIAMinus1:         book.tI,
		FixedStart: FixedStart{
			N: book.fixedN, S: book.fixedS, DeltaH: book.fixedD,
		},
		Analysis: Analysis{
			WholeHikeLegs: wholeLegs,
			WholeHikeSpan: spanOf(times[18], times[1]),
			PerStage:      per,
			LegsOutside:   outside,
			HikeLegs:      hikeLegs,
			HikeSpan:      hikeSpanOf(index, headers[0], headers[18]),
		},
	}
	if profile.Name == ConformanceSmallV2.Name {
		s, sl, t, st := profile.SampleBytes, profile.SlotBytes, profile.TerrainBytes, profile.StepsPerLeg
		rec.SampleBytes, rec.SlotBytes, rec.TerrainBytes, rec.StepsPerLeg = &s, &sl, &t, &st
	}
	if !omitSix {
		rec.SixLeg, rec.SixLegRuns = sixLeg(terrain, stages, counts, computed, stream)
		// Fewer than six legs. six_leg is the string not defined. six_leg_runs
		// is [] only in that case. A short draw stays the string not read.
		if profile.Kind == Kind && rec.SixLegRuns == nil {
			rec.SixLegRuns = json.RawMessage(`[]`)
		}
	}
	if rec.ShapeItem == nil {
		rec.ShapeItem = []string{}
	}
	return rec
}

type LegOut struct {
	J      int    `json:"j"`
	Result string `json:"result"`
}

type book struct {
	defined                                                  bool
	a, b, n, s, delta, pb, heightPB, cmpPB, tpb, heightI, tI json.RawMessage
	fixedN, fixedS, fixedD                                   json.RawMessage
	outside                                                  json.RawMessage
}

// BookendToken is Rule A4.11.
// Fewer than two present paths, or two or more present paths whose reads
// all returned and fewer than two plants match, is "not defined".
// A short or missing dependent read with two or more present paths is "not read".
func BookendToken(present int, readsMissing bool, anchored int) string {
	if present >= 2 && readsMissing && anchored < 2 {
		return "not read"
	}
	if anchored < 2 {
		return "not defined"
	}
	return "defined"
}

func bookendFields(present, anchored int) book {
	token := BookendToken(present, false, anchored)
	if token == "defined" {
		// The pack does not construct a plant match from this SPEC text.
		// No locked object reaches numeric bookends.
		token = "not defined"
	}
	return tokenBook(token)
}

// v2Book is Rule A4.11. Fewer than two present paths is not defined.
// An issued plant GET that did not return a document is not read.
// An unread header that is not a bookend endpoint does not, by itself,
// make a, b, or N not read. Two anchored paths are the integers.
func v2Book(present int, facts []pathMeasure, anchors []string, headers []Header, cps []CheckpointRow, counts []int, index Index) book {
	if present < 2 {
		return tokenBook("not defined")
	}
	for _, fact := range facts {
		if fact.plantRead && fact.plant == "not read" {
			return tokenBook("not read")
		}
	}
	anchored := make([]int, 0, len(anchors))
	for i, token := range anchors {
		if token == "anchored" {
			anchored = append(anchored, i)
		}
	}
	if len(anchored) < 2 {
		return tokenBook("not defined")
	}
	a := anchored[0] + 1
	b := anchored[len(anchored)-1] + 1
	if a >= len(headers) || headers[a].IndexComparison == "not read" {
		return tokenBook("not read")
	}
	if b-1 >= len(facts) || !facts[b-1].hasBlock {
		return tokenBook("not read")
	}
	pathB := facts[b-1]
	cmp, heightPB, timePB := compareBlock(pathB.hash, pathB.height, index)
	if cmp == "not read" {
		return tokenBook("not read")
	}
	heightI, timeI := endpoint(headers[a])
	var heightPBJSON, timePBJSON json.RawMessage
	switch cmp {
	case "match":
		heightPBJSON = jsonU(heightPB)
		timePBJSON = jsonTime(timePB)
	default:
		heightPBJSON = jstr("not defined")
		timePBJSON = jstr("not read")
	}
	header1 := headers[1]
	_, time1 := endpoint(header1)
	fixedS := subRaw(timePBJSON, time1, "not read")
	fixedDelta := fixedDelta(header1, cmp, heightPBJSON)
	return book{
		a: jnum(int64(a)), b: jnum(int64(b)),
		n: countRange(counts, a, b), s: subRaw(timePBJSON, timeI, "not read"),
		delta: subRaw(heightPBJSON, heightI, "not defined"),
		pb:    blockRaw(pathB.hash, pathB.height), heightPB: heightPBJSON, cmpPB: jstr(cmp), tpb: timePBJSON,
		heightI: heightI, tI: timeI,
		fixedN: countRange(counts, 1, b), fixedS: fixedS, fixedD: fixedDelta,
		outside: legsOutside(counts, a, b),
	}
}

func compareBlock(hash []byte, height uint64, index Index) (string, uint64, *uint64) {
	if height > JSONIntMax {
		return "not read", 0, nil
	}
	read := index.Read(height)
	if read == nil {
		return "not read", 0, nil
	}
	if bytes.Equal(read.Natural, hash) && read.Height == height && read.Height <= JSONIntMax {
		return "match", read.Height, read.Time
	}
	return "mismatch", 0, nil
}

func endpoint(header Header) (json.RawMessage, json.RawMessage) {
	if header.IndexComparison != "match" {
		return jstr("not defined"), jstr("not read")
	}
	return header.StoredHeight, header.T
}

func jsonU(height uint64) json.RawMessage {
	if height > JSONIntMax {
		return jstr("not defined")
	}
	return jnum(int64(height))
}

func jsonTime(time *uint64) json.RawMessage {
	if time == nil || *time > JSONIntMax {
		return jstr("not read")
	}
	return jnum(int64(*time))
}

func subRaw(later, earlier json.RawMessage, missing string) json.RawMessage {
	left, okL := rawInt(later)
	right, okR := rawInt(earlier)
	if !okL || !okR {
		return jstr(missing)
	}
	return jnum(left - right)
}

func fixedDelta(header Header, pbCmp string, heightPB json.RawMessage) json.RawMessage {
	if header.IndexComparison == "not read" || pbCmp == "not read" {
		return jstr("not read")
	}
	if header.IndexComparison == "mismatch" || pbCmp == "mismatch" {
		return jstr("not defined")
	}
	heightI, _ := endpoint(header)
	return subRaw(heightPB, heightI, "not defined")
}

// countDefined is len(stages[g].checkpoints), counting an element of the
// wrong width: N, fixed_start.N and legs_outside count checkpoints (section
// 14.4, Rule A4.7: "An integer when bookends exist"). Only a missing stage
// has no count. A stage whose legs are "not defined" still has a count.
func countDefined(counts []int, g int) (int, bool) {
	if g < 0 || g >= len(counts) || counts[g] < 0 {
		return 0, false
	}
	return counts[g], true
}

func countRange(counts []int, from, until int) json.RawMessage {
	if from >= until {
		return jnum(0)
	}
	var total int64
	for g := from; g < until; g++ {
		count, ok := countDefined(counts, g)
		if !ok {
			return jstr("not defined")
		}
		total += int64(count)
	}
	return jnum(total)
}

func legsOutside(counts []int, a, b int) json.RawMessage {
	var total int64
	for g := 0; g < Stages; g++ {
		if g >= a && g < b {
			continue
		}
		count, ok := countDefined(counts, g)
		if !ok {
			return jstr("not defined")
		}
		total += int64(count)
	}
	return jnum(total)
}

func tokenBook(token string) book {
	nd := jstr(token)
	return book{
		a: nd, b: nd, n: nd, s: nd, delta: nd, pb: nd, heightPB: nd, cmpPB: nd, tpb: nd, heightI: nd, tI: nd,
		fixedN: nd, fixedS: nd, fixedD: nd, outside: nd,
	}
}

func emptyBookends(profile Profile, shape string, items []string, headers []Header, cps []CheckpointRow) Record {
	nd := jstr("not defined")
	ways := make([]WaypointRow, Stages-1)
	for i := range ways {
		ways[i] = WaypointRow{G: i + 1, Result: "mismatch"}
	}
	paths := make([]json.RawMessage, Stages-1)
	for g := 1; g < Stages; g++ {
		if g == 17 {
			paths[g-1] = jraw(path17{G: 17, W17: "not defined", Claimed: nd})
		} else {
			paths[g-1] = jraw(pathSkip{G: g, PathState: "skip", Plant: "n/a", PG: nd, HeightPG: nd, DG: nd, Claimed: nd})
		}
	}
	per := make([]StageSpan, Stages)
	for g := 0; g < Stages; g++ {
		per[g] = StageSpan{G: g, CheckpointCount: jnum(0), Span: jstr("not read")}
	}
	rec := Record{
		Profile: profile.Name, Shape: shape, ShapeItem: items,
		Headers: headers, Checkpoints: cps, Waypoints: ways, Paths: paths,
		AnchoredTotal: 0, SkipList: []int{},
		A: nd, B: nd, N: nd, S: nd, DeltaH: nd, PB: nd, HeightPB: nd,
		IndexComparisonPB: nd, TPB: nd, HeightIAMinus1: nd, TIAMinus1: nd,
		FixedStart: FixedStart{N: nd, S: nd, DeltaH: nd},
		Analysis: Analysis{
			WholeHikeLegs: jnum(0), WholeHikeSpan: jstr("not read"), PerStage: per, LegsOutside: nd,
			HikeLegs: jstr("not defined"), HikeSpan: jstr("not read"),
		},
	}
	if profile.Name == ConformanceSmallV2.Name {
		s, sl, t, st := profile.SampleBytes, profile.SlotBytes, profile.TerrainBytes, profile.StepsPerLeg
		rec.SampleBytes, rec.SlotBytes, rec.TerrainBytes, rec.StepsPerLeg = &s, &sl, &t, &st
	}
	if profile.Kind == Kind && len(rec.Anchors) != 16 {
		rec.Anchors = make([]string, 16)
		for i := range rec.Anchors {
			rec.Anchors[i] = "not anchored"
		}
	}
	return rec
}

type pathSkip struct {
	G         int             `json:"g"`
	PathState string          `json:"path_state"`
	Plant     string          `json:"plant"`
	PG        json.RawMessage `json:"P_g"`
	HeightPG  json.RawMessage `json:"height_P_g"`
	DG        json.RawMessage `json:"d_g"`
	Claimed   json.RawMessage `json:"claimed_block"`
}

type path17 struct {
	G       int             `json:"g"`
	W17     string          `json:"w17_path"`
	Claimed json.RawMessage `json:"claimed_block"`
}

func pathRow(g int, path []byte) (any, string, string) {
	nd := jstr("not defined")
	if g == 17 {
		if len(path) == 0 {
			return path17{G: 17, W17: "not fetched by design", Claimed: nd}, "w17", "n/a"
		}
		return path17{G: 17, W17: "not defined", Claimed: nd}, "w17", "n/a"
	}
	if len(path) == 0 {
		return pathSkip{G: g, PathState: "skip", Plant: "n/a", PG: nd, HeightPG: nd, DG: nd, Claimed: nd}, "skip", "n/a"
	}
	// A present path that does not decode as a canonical path body, including
	// every path shorter than a 32-byte leaf, is plant mismatch. claimed_block
	// is not defined. The pack does not publish a path layout.
	return pathSkip{G: g, PathState: "present", Plant: "mismatch", PG: nd, HeightPG: nd, DG: nd, Claimed: nd}, "present", "mismatch"
}

type blockRef struct {
	Hash   string `json:"block_hash"`
	Height uint64 `json:"block_height"`
}

type pathMeasure struct {
	row       any
	state     string
	plant     string
	anchor    string
	plantRead bool
	hash      []byte
	height    uint64
	hasBlock  bool
}

// absentStagePath is a stage the object does not store. That is not `skip`
// and, for g = 17, not `not fetched by design`.
func absentStagePath(g int) pathMeasure {
	nd := jstr("not defined")
	out := pathMeasure{anchor: "not anchored", state: "absent", plant: "not defined"}
	if g == 17 {
		out.row = path17{G: 17, W17: "not defined", Claimed: nd}
		return out
	}
	out.row = pathSkip{G: g, PathState: "not defined", Plant: "not defined", PG: nd, HeightPG: nd, DG: nd, Claimed: nd}
	return out
}

// section14Path is the section 14.3 row for one stage of a v2.0 object.
// The chain bind runs only after the inclusion height is in the anchor
// window and the block is a stored interrupt header. A nil chain is a
// successful bind when the index returned a document for that block. A
// refused connection, with no document, is plant `not read`. A body that
// does not validate is plant mismatch and does not call the chain. H is
// the stage's stored beacon height.
func section14Path(g int, path, waypoint []byte, beaconHeight uint64, header Header, item, stages cborVal, chain func(PathLocators) error, index Index) pathMeasure {
	nd := jstr("not defined")
	out := pathMeasure{anchor: "not anchored"}
	if g == 17 {
		claimed := nd
		if loc, err := PathLocatorsFor(path, waypoint); err == nil {
			claimed = blockJSON(loc)
		}
		token := "not defined"
		if len(path) == 0 {
			token = "not fetched by design"
			claimed = nd
		}
		out.row = path17{G: 17, W17: token, Claimed: claimed}
		out.state, out.plant = "w17", "n/a"
		return out
	}
	if len(path) == 0 {
		out.row = pathSkip{G: g, PathState: "skip", Plant: "n/a", PG: nd, HeightPG: nd, DG: nd, Claimed: nd}
		out.state, out.plant = "skip", "n/a"
		return out
	}
	loc, err := PathLocatorsFor(path, waypoint)
	if err != nil {
		out.row = pathSkip{G: g, PathState: "present", Plant: "mismatch", PG: nd, HeightPG: nd, DG: nd, Claimed: nd}
		out.state, out.plant = "present", "mismatch"
		return out
	}
	claimed := blockJSON(loc)
	out.hash = append([]byte(nil), loc.BlockHash[:]...)
	out.height = uint64(loc.BlockHeight)
	out.hasBlock = true
	if !heightInAnchorWindow(beaconHeight, out.height) || !storedInterrupt(item, stages, loc.BlockHash[:], out.height) {
		out.row = pathSkip{G: g, PathState: "present", Plant: "not read", PG: nd, HeightPG: nd, DG: nd, Claimed: claimed}
		out.state, out.plant = "present", "not read"
		return out
	}
	out.plantRead = true
	out.plant = "mismatch"
	switch {
	case chain != nil:
		if chain(loc) == nil {
			out.plant = "match"
			out.anchor = "anchored"
		}
	default:
		// No chain callback. A document that was read counts as bound.
		// No document (connection refused, or the height is absent) is not read.
		if _, ok := index.hasBody(out.height); ok {
			out.plant = "match"
			out.anchor = "anchored"
		} else {
			out.plant = "not read"
		}
	}
	pg, heightPG, dg := nd, nd, nd
	if out.plant == "match" {
		pg = claimed
		if out.height <= JSONIntMax {
			heightPG = jnum(int64(out.height))
		}
		dg = plantGap(header, out.height)
	}
	out.row = pathSkip{G: g, PathState: "present", Plant: out.plant, PG: pg, HeightPG: heightPG, DG: dg, Claimed: claimed}
	out.state = "present"
	return out
}

func blockRaw(hash []byte, height uint64) json.RawMessage {
	if len(hash) != 32 || height > JSONIntMax {
		return jstr("not defined")
	}
	return jraw(blockRef{Hash: displayHex(hash), Height: height})
}

func blockJSON(loc PathLocators) json.RawMessage {
	return jraw(blockRef{Hash: displayHex(loc.BlockHash[:]), Height: uint64(loc.BlockHeight)})
}

func plantGap(header Header, plantHeight uint64) json.RawMessage {
	if header.IndexComparison != "match" || plantHeight > JSONIntMax {
		return jstr("not defined")
	}
	stored, ok := rawInt(header.StoredHeight)
	if !ok {
		return jstr("not defined")
	}
	return jnum(int64(plantHeight) - stored)
}

// storedInterrupt is SPEC §13.1. The stored interrupt headers are
// stages[1].beacon through stages[17].beacon, and last_interrupt when the
// object stores both the hash and the height.
func storedInterrupt(item, stages cborVal, hash []byte, height uint64) bool {
	for g := 1; g < Stages; g++ {
		st, ok := stageAt(stages, g)
		if !ok {
			continue
		}
		beacon, okB := bytesField(st, "beacon")
		got, okH, over := uintField(st, "beacon_height")
		if okB && okH && !over && got == height && bytes.Equal(beacon, hash) {
			return true
		}
	}
	last, okL := bytesOf(item, "last_interrupt")
	got, okH, over := uintField(item, "last_interrupt_height")
	return okL && okH && !over && got == height && bytes.Equal(last, hash)
}

func spanOf(later, earlier json.RawMessage) json.RawMessage {
	li, okL := rawInt(later)
	ei, okE := rawInt(earlier)
	if !okL || !okE {
		return jstr("not read")
	}
	return jnum(li - ei)
}

func rawInt(r json.RawMessage) (int64, bool) {
	if len(r) == 0 || r[0] == '"' {
		return 0, false
	}
	n, err := strconv.ParseInt(string(r), 10, 64)
	if err != nil {
		return 0, false
	}
	return n, true
}

func sixLeg(terrain *Terrain, stages cborVal, counts []int, computed []byte, stream []byte) (json.RawMessage, json.RawMessage) {
	var total uint64
	for _, c := range counts {
		if c < 0 {
			continue
		}
		total += uint64(c)
	}
	indices, token := DrawSix(stream, total)
	if token == "not read" {
		s := jstr("not read")
		return s, s
	}
	if token == "not defined" {
		return jstr("not defined"), nil
	}
	results := make([]string, len(indices))
	for i, q := range indices {
		g, j := locate(counts, int(q))
		st, ok := stageAt(stages, g)
		results[i] = "mismatch"
		if !ok || terrain == nil {
			continue
		}
		legs := byteList(st, "leg_results")
		checks := byteList(st, "checkpoints")
		if j >= len(legs) || len(legs[j]) != 8 {
			continue
		}
		var previous []byte
		if j == 0 {
			// Section 14.6: the leg-0 start is the stored waypoint, which is
			// 32 bytes (section 12.2). A shorter or longer value is not that
			// start, so the leg is mismatch. Stage 0 uses the computed seed.
			if g == 0 {
				previous = computed
			} else if wp, ok := bytesField(st, "waypoint"); ok && len(wp) == 32 {
				previous = wp
			}
		} else if checks != nil && j-1 < len(checks) {
			previous = checks[j-1]
		}
		if len(previous) < 8 {
			continue
		}
		got := terrain.Leg(previous)
		if got == U64LE(legs[j]) {
			results[i] = "match"
		}
	}
	draw := SixDraw{Indices: indices, Results: results}
	return jraw(draw), jraw([]SixDraw{draw})
}

func locate(counts []int, index int) (int, int) {
	start := 0
	for g, count := range counts {
		if index < start+count {
			return g, index - start
		}
		start += count
	}
	return 0, 0
}

func headerFromStage(st cborVal, index Index) Header {
	digest, okD := bytesField(st, "beacon")
	// A stored hash that is not 32 bytes has no usable value: `not defined`.
	okD = okD && len(digest) == 32
	height, okH, over := uintField(st, "beacon_height")
	if !okD && !okH {
		return unreadHeader(false, false, 0)
	}
	var d []byte
	if okD {
		d = digest
	}
	return headerRow(d, okD, height, okH && !over, over, index)
}

func headerFromLast(item cborVal, index Index) Header {
	digest, okD := bytesOf(item, "last_interrupt")
	okD = okD && len(digest) == 32
	height, okH, over := uintField(item, "last_interrupt_height")
	if !okD && !okH {
		return unreadHeader(false, false, 0)
	}
	return headerRow(digest, okD, height, okH && !over, over, index)
}

func unreadHeader(hasHash bool, hasHeight bool, height uint64) Header {
	h := Header{IndexComparison: "not read", T: jstr("not read")}
	if hasHash {
		h.StoredHash = jstr(hexEncode(nil))
	}
	_ = hasHeight
	_ = height
	h.StoredHash = jstr("not defined")
	h.StoredHeight = jstr("not defined")
	return h
}

// HeaderMeasurement is section 14.2 for one stored header.
// present is false when the index has no document at height.
func HeaderMeasurement(natural []byte, height uint64, document string, present bool) (comparison string, t json.RawMessage) {
	idx := Index{}
	if present {
		idx.Documents = map[uint64]string{height: document}
	}
	row := headerRow(natural, len(natural) == 32, height, true, false, idx)
	return row.IndexComparison, row.T
}

func headerRow(digest []byte, hasHash bool, height uint64, hasHeight, over bool, index Index) Header {
	row := Header{IndexComparison: "not read", T: jstr("not read")}
	if !hasHash {
		row.StoredHash = jstr("not defined")
	} else {
		row.StoredHash = jstr(hexEncode(digest))
	}
	if !hasHeight || over {
		row.StoredHeight = jstr("not defined")
		return row
	}
	row.StoredHeight = jnum(int64(height))
	if !hasHash || len(digest) != 32 {
		return row
	}
	read := index.Read(height)
	if read == nil {
		return row
	}
	if bytes.Equal(read.Natural, digest) && read.Height == height {
		row.IndexComparison = "match"
		if read.Time != nil {
			row.T = jnum(int64(*read.Time))
		}
		return row
	}
	row.IndexComparison = "mismatch"
	row.T = jstr("not read")
	return row
}

func shapeItems(item cborVal, profile Profile) []string {
	stages, hasStages := item.field("stages")
	fieldSet := !exactKeys(item.keys, topKeys)
	if b, ok := bytesOf(item, "public_key"); !ok || len(b) != 32 {
		fieldSet = true
	}
	if b, ok := bytesOf(item, "seed_beacon"); !ok || len(b) != 32 {
		fieldSet = true
	}
	if s, ok := textOf(item, "kind"); !ok || s == "" && !ok {
		fieldSet = true
	}
	if _, ok := textOf(item, "kind"); !ok {
		fieldSet = true
	}
	if _, ok := textOf(item, "fill"); !ok {
		fieldSet = true
	}
	for _, name := range []string{"slot_bytes", "n_slots", "sample_bytes", "steps_per_leg"} {
		if _, ok, _ := uintField(item, name); !ok {
			fieldSet = true
		}
	}
	if !hasStages || stages.kind != 4 {
		fieldSet = true
	}
	// last_interrupt present and not a byte string, or last_interrupt_height
	// present and not an unsigned integer, is a wrong value type (section
	// 14.1 row 2). A byte string of the wrong width is last_interrupt only
	// (row 11), the same as a wrong-width checkpoint.
	if f, ok := item.field("last_interrupt"); ok && f.kind != 2 {
		fieldSet = true
	}
	if f, ok := item.field("last_interrupt_height"); ok && f.kind != 0 {
		fieldSet = true
	}
	stage0Path := false
	w17Bad := true
	if stages.kind == 4 {
		for g, st := range stages.arr {
			if st.kind != 5 {
				fieldSet = true
				continue
			}
			expect := stageKeys
			if g == 0 {
				expect = stage0Keys
			}
			if !exactKeys(st.keys, expect) {
				fieldSet = true
			}
			if g == 0 {
				if _, ok := st.field("path"); ok {
					stage0Path = true
				}
			}
			for _, key := range []string{"beacon", "waypoint"} {
				b, ok := bytesField(st, key)
				if !ok || len(b) != 32 {
					fieldSet = true
				}
			}
			if p, ok := st.field("path"); ok && p.kind != 2 {
				fieldSet = true
			}
			// A checkpoint or leg-result value that is not an array is a wrong
			// value type (section 14.1 row 2). So is an element of that array
			// that is not a byte string: the width conditions still apply,
			// and field set is not dropped because another condition is too.
			for _, key := range []string{"checkpoints", "leg_results"} {
				f, ok := st.field(key)
				if !ok {
					continue
				}
				if f.kind != 4 {
					fieldSet = true
					continue
				}
				for _, item := range f.arr {
					if item.kind != 2 {
						fieldSet = true
					}
				}
			}
			if _, ok, _ := uintField(st, "beacon_height"); !ok {
				fieldSet = true
			}
			if _, ok, _ := uintField(st, "leg_count"); !ok {
				fieldSet = true
			}
		}
		if len(stages.arr) > 17 && stages.arr[17].kind == 5 {
			if p, ok := bytesField(stages.arr[17], "path"); ok {
				w17Bad = len(p) != 0
			} else {
				w17Bad = true
			}
		}
	}
	kind, _ := textOf(item, "kind")
	fill, _ := textOf(item, "fill")
	slot, slotOK, _ := uintField(item, "slot_bytes")
	nslots, nOK, _ := uintField(item, "n_slots")
	sample, sampleOK, _ := uintField(item, "sample_bytes")
	steps, stepsOK, _ := uintField(item, "steps_per_leg")
	constant := kind != profile.Kind || fill != Fill ||
		!slotOK || int(slot) != profile.SlotBytes ||
		!nOK || int(nslots) != profile.NSlots ||
		!sampleOK || int(sample) != profile.SampleBytes ||
		!stepsOK || int(steps) != profile.StepsPerLeg
	stagesLength := stages.kind != 4 || len(stages.arr) != Stages
	legCountBad := false
	checkpointWidth := false
	legWidth := false
	// Absent stages have no leg_count. Section 14.1: a condition that tests a
	// field's value is violated when that field is absent. A longer array is
	// only "stages length": stage 17 may still be present.
	if stages.kind != 4 || len(stages.arr) < Stages {
		legCountBad = true
	}
	if stages.kind != 4 || len(stages.arr) == 0 {
		checkpointWidth = true
		legWidth = true
	} else {
		for _, st := range stages.arr {
			if st.kind != 5 {
				legCountBad = true
				checkpointWidth = true
				legWidth = true
				continue
			}
			checks := byteList(st, "checkpoints")
			results := byteList(st, "leg_results")
			count, countOK, _ := uintField(st, "leg_count")
			if _, ok := st.field("checkpoints"); !ok || checks == nil {
				checkpointWidth = true
				legCountBad = true
			}
			if _, ok := st.field("leg_results"); !ok || results == nil {
				legWidth = true
				legCountBad = true
			}
			if checks != nil {
				for _, item := range checks {
					if len(item) != 32 {
						checkpointWidth = true
					}
				}
			}
			if results != nil {
				for _, item := range results {
					if len(item) != 8 {
						legWidth = true
					}
				}
			}
			if !countOK || count < 1 || checks == nil || results == nil || int(count) != len(checks) || len(results) != len(checks) {
				legCountBad = true
			}
		}
	}
	seedBad := true
	if st, ok := stageAt(stages, 0); ok {
		beacon, okB := bytesField(st, "beacon")
		seed, okS := bytesOf(item, "seed_beacon")
		if okB && okS && bytes.Equal(beacon, seed) {
			seedBad = false
		}
	}
	last, lastOK := bytesOf(item, "last_interrupt")
	lastH, lastHOK, _ := uintField(item, "last_interrupt_height")
	lastBad := true
	if lastOK && len(last) == 32 && lastHOK {
		if st, ok := stageAt(stages, 17); ok {
			beacon, okB := bytesField(st, "beacon")
			// Exact unsigned integers, including a value above 2^53−1.
			// Header height order uses the same integers.
			bh, okBH, _ := uintField(st, "beacon_height")
			if okB && len(beacon) == 32 && okBH && !bytes.Equal(last, beacon) && lastH > bh {
				lastBad = false
			}
		}
	}
	type pair struct {
		d        []byte
		h        uint64
		okD, okH bool
	}
	headers := make([]pair, Stages+1)
	for g := 0; g < Stages; g++ {
		if st, ok := stageAt(stages, g); ok {
			d, okD := bytesField(st, "beacon")
			h, okH, _ := uintField(st, "beacon_height")
			// A height above 2^53−1 still takes part, by its exact value.
			// The record writes it `not defined` (`height_range`).
			headers[g] = pair{d: d, h: h, okD: okD && len(d) == 32, okH: okH}
		}
	}
	{
		d, okD := bytesOf(item, "last_interrupt")
		h, okH, _ := uintField(item, "last_interrupt_height")
		headers[Stages] = pair{d: d, h: h, okD: okD && len(d) == 32, okH: okH}
	}
	heightBad := false
	hashBad := false
	heightRange := false
	for i, h := range headers {
		if !h.okD {
			hashBad = true
		}
		if !h.okH {
			heightBad = true
		}
		if overHeight(item, stages, i) {
			heightRange = true
		}
		if i == 0 {
			continue
		}
		prev := headers[i-1]
		if h.okD && prev.okD && bytes.Equal(h.d, prev.d) {
			hashBad = true
		}
		if h.okH && prev.okH && h.h <= prev.h {
			heightBad = true
		}
	}
	if _, ok := item.field("last_interrupt"); !ok {
		hashBad = true
	}
	if _, ok := item.field("last_interrupt_height"); !ok {
		heightBad = true
	}
	ordered := []struct {
		name string
		bad  bool
	}{
		{"field set", fieldSet},
		{"stage-0 path key", stage0Path},
		{"W_17 path", w17Bad},
		{"constant", constant},
		{"stages length", stagesLength},
		{"leg_count", legCountBad},
		{"checkpoint width", checkpointWidth},
		{"leg result width", legWidth},
		{"seed beacon", seedBad},
		{"last_interrupt", lastBad},
		{"header height order", heightBad},
		{"header hash differ", hashBad},
		{"height_range", heightRange},
	}
	var out []string
	for _, row := range ordered {
		if row.bad {
			out = append(out, row.name)
		}
	}
	if out == nil {
		out = []string{}
	}
	return out
}

func overHeight(item cborVal, stages cborVal, index int) bool {
	if index < Stages {
		st, ok := stageAt(stages, index)
		if !ok {
			return false
		}
		_, _, over := uintField(st, "beacon_height")
		return over
	}
	_, _, over := uintField(item, "last_interrupt_height")
	return over
}

var topKeys = []string{
	"fill", "kind", "stages", "n_slots", "public_key", "slot_bytes",
	"seed_beacon", "sample_bytes", "steps_per_leg", "last_interrupt", "last_interrupt_height",
}

var stage0Keys = []string{"beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results"}

var stageKeys = []string{"beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results", "path"}

func exactKeys(got, expect []string) bool {
	if len(got) != len(expect) {
		return false
	}
	seen := map[string]int{}
	for _, k := range got {
		seen[k]++
	}
	for _, k := range expect {
		if seen[k] != 1 {
			return false
		}
	}
	return true
}

func stageAt(stages cborVal, g int) (cborVal, bool) {
	if stages.kind != 4 || g < 0 || g >= len(stages.arr) {
		return cborVal{}, false
	}
	st := stages.arr[g]
	if st.kind != 5 {
		// A non-map element is a stage with every field absent.
		return cborVal{kind: 5}, true
	}
	return st, true
}

func bytesOf(v cborVal, name string) ([]byte, bool) {
	f, ok := v.field(name)
	if !ok || f.kind != 2 {
		return nil, false
	}
	return f.b, true
}

func bytesField(v cborVal, name string) ([]byte, bool) {
	return bytesOf(v, name)
}

func textOf(v cborVal, name string) (string, bool) {
	f, ok := v.field(name)
	if !ok || f.kind != 3 {
		return "", false
	}
	return f.s, true
}

func uintField(v cborVal, name string) (uint64, bool, bool) {
	f, ok := v.field(name)
	if !ok || f.kind != 0 {
		return 0, false, false
	}
	return f.u, true, f.u > JSONIntMax
}

func byteList(v cborVal, name string) [][]byte {
	f, ok := v.field(name)
	if !ok || f.kind != 4 {
		return nil
	}
	out := make([][]byte, len(f.arr))
	for i, item := range f.arr {
		if item.kind != 2 {
			out[i] = nil
			continue
		}
		out[i] = item.b
	}
	return out
}

func allLen(items [][]byte, n int) bool {
	for _, item := range items {
		if len(item) != n {
			return false
		}
	}
	return true
}

func hexEncode(b []byte) string {
	const digits = "0123456789abcdef"
	out := make([]byte, len(b)*2)
	for i, c := range b {
		out[i*2] = digits[c>>4]
		out[i*2+1] = digits[c&0x0f]
	}
	return string(out)
}

// displayHex is SPEC §9. Bitcoin display order is the natural 32 bytes
// reversed: display[i] = natural[31 − i]. P_g, P_b, and claimed_block use it.
// stored_hash stays hexEncode, byte 0 first.
func displayHex(natural []byte) string {
	rev := make([]byte, len(natural))
	for i := range natural {
		rev[i] = natural[len(natural)-1-i]
	}
	return hexEncode(rev)
}

// DG is the index-confirmed plant gap. It subtracts header.height only when
// the header comparison is match. Otherwise the token is not defined.
func DG(plantMatch, headerMatch bool, heightPG, headerHeight int64) (int64, string) {
	if plantMatch && headerMatch {
		return heightPG - headerHeight, ""
	}
	return 0, "not defined"
}

// hikeSpanOf is Rule A4.10. hike_span is MTP(last beacon) minus MTP(first
// beacon). A missing height or a missing window time is not read.
func hikeSpanOf(index Index, first, last Header) json.RawMessage {
	fh, okF := rawInt(first.StoredHeight)
	lh, okL := rawInt(last.StoredHeight)
	if !okF || !okL || fh < 0 || lh < 0 {
		return jstr("not read")
	}
	mtpF, okF := MedianTimePast(index, uint64(fh), first)
	mtpL, okL := MedianTimePast(index, uint64(lh), last)
	if !okF || !okL {
		return jstr("not read")
	}
	return jnum(mtpL - mtpF)
}

// MedianTimePast is the BIP113 median at height h. n = min(11, h+1).
// The beacon's own time is taken only when that header's index comparison
// is match and the time follows section 9. Each other height in the window
// supplies header.time when the read returned a document, header.height
// equals the requested height, and the time follows section 9.
func MedianTimePast(index Index, height uint64, beacon Header) (int64, bool) {
	n := uint64(11)
	if height+1 < n {
		n = height + 1
	}
	if n == 0 {
		return 0, false
	}
	start := height - n + 1
	times := make([]int64, 0, n)
	for h := start; h <= height; h++ {
		var t int64
		var ok bool
		if h == height {
			t, ok = beaconTime(beacon, height)
		} else {
			t, ok = windowTime(index, h)
		}
		if !ok {
			return 0, false
		}
		times = append(times, t)
	}
	sort.Slice(times, func(i, j int) bool { return times[i] < times[j] })
	return times[n/2], true
}

func beaconTime(beacon Header, height uint64) (int64, bool) {
	if beacon.IndexComparison != "match" {
		return 0, false
	}
	stored, ok := rawInt(beacon.StoredHeight)
	if !ok || stored != int64(height) {
		return 0, false
	}
	return rawInt(beacon.T)
}

func windowTime(index Index, height uint64) (int64, bool) {
	doc := index.Read(height)
	if doc == nil || doc.Height != height || doc.Time == nil {
		return 0, false
	}
	return int64(*doc.Time), true
}

// RateOutcome is Rule B2 on a recorded hike_span. A span that is not read
// is held. A span of zero or below is refuse. A span that meets the floor
// is accept. A span that falls short is refuse.
func RateOutcome(hikeLegs, hikeSpan int64, spanRead bool, p, q int64) string {
	if !spanRead {
		return "held"
	}
	if hikeSpan <= 0 || !LegRateMeets(hikeLegs, hikeSpan, p, q) {
		return "refuse"
	}
	return "accept"
}

// SubmissionDeadline is the gate receipt rule. tip below the last beacon,
// and a tip that was not read, are held. A gap greater than K refuses with
// reason submission_deadline. That refuse is not a fake leg.
func SubmissionDeadline(tip *int64, lastBeacon, k int64) (outcome, reason string) {
	if tip == nil || *tip < lastBeacon {
		return "held", ""
	}
	if *tip-lastBeacon > k {
		return "refuse", "submission_deadline"
	}
	return "accept", ""
}

// DrawThorough is the §15.5 index draw. Below 6 legs the draw is not
// defined. Otherwise one section 14.6 stream fills 18 indices, split in
// order into rounds of 6. An index already in the current group of 6 is
// discarded. An index already in the phase is discarded while an unused
// index remains. A short byte source is not read.
func DrawThorough(stream []byte, legTotal uint64, rounds, perRound int) (groups [][]uint64, token string) {
	if legTotal < 6 {
		return nil, "not defined"
	}
	if rounds < 1 || perRound < 1 {
		return nil, "not defined"
	}
	need := rounds * perRound
	phase := make([]uint64, 0, need)
	pos := 0
	for len(phase) < need {
		if pos+8 > len(stream) {
			return nil, "not read"
		}
		draw := binaryLE(stream[pos : pos+8])
		pos += 8
		if !belowLimit(draw, legTotal) {
			continue
		}
		q := draw % legTotal
		roundStart := (len(phase) / perRound) * perRound
		if containsU64(phase[roundStart:], q) {
			continue
		}
		if containsU64(phase, q) && unusedIndex(phase, legTotal) {
			continue
		}
		phase = append(phase, q)
	}
	groups = make([][]uint64, rounds)
	for r := 0; r < rounds; r++ {
		groups[r] = append([]uint64(nil), phase[r*perRound:(r+1)*perRound]...)
	}
	return groups, ""
}

func binaryLE(b []byte) uint64 {
	var v uint64
	for i := 7; i >= 0; i-- {
		v = v<<8 | uint64(b[i])
	}
	return v
}

func unusedIndex(phase []uint64, legTotal uint64) bool {
	seen := make([]bool, legTotal)
	for _, q := range phase {
		if q < legTotal {
			seen[q] = true
		}
	}
	for _, used := range seen {
		if !used {
			return true
		}
	}
	return false
}

// LegRateMeets is Rule B2. The comparison is integer-only:
// hike_span > 0 and q·hike_legs ≥ p·hike_span. It does not round.
// A span that is not a positive integer does not meet the floor.
// The bookend pair (N, S) is not an input.
func LegRateMeets(hikeLegs, hikeSpan, p, q int64) bool {
	if hikeSpan <= 0 || q <= 0 || p < 0 || hikeLegs < 0 {
		return false
	}
	hiL, loL := bits.Mul64(uint64(q), uint64(hikeLegs))
	hiR, loR := bits.Mul64(uint64(p), uint64(hikeSpan))
	if hiL != hiR {
		return hiL > hiR
	}
	return loL >= loR
}

// AnchorsMeet is Rule B4. anchored_total ≥ 6, anywhere among W_1 … W_16.
// There is no first-planted rule and no last-planted rule.
func AnchorsMeet(anchoredTotal int) bool {
	return anchoredTotal >= MinAnchoredPaths
}

// AnchoredIndicesMeet counts distinct indices in 1 … 16 and applies Rule B4.
// An index outside that range is not one of the sixteen waypoints.
func AnchoredIndicesMeet(indices []int) bool {
	seen := make([]bool, 17)
	n := 0
	for _, g := range indices {
		if g < 1 || g > 16 || seen[g] {
			continue
		}
		seen[g] = true
		n++
	}
	return AnchorsMeet(n)
}

// Status204Appended is Rule A1.4. Status 204 means the waypoint was appended.
func Status204Appended(status int) bool {
	return status == 204
}
