package pcoc

// Section 14 measurement (SPEC §14, Rules A4.1 through A4.11 and A4.4).
//
// The measurement records. It does not judge. Later measurements are still
// recorded. Six recomputed legs run one at a time (Rule A4.4.4 permits that).

import (
	"crypto/sha256"
	"strconv"
)

type hdr struct {
	storedHash   string
	storedHeight string
	indexCmp     string
	t            string
	tInt         int64
	hasT         bool
	heightMatch  bool
	indexHeight  uint64
}

type pathRec struct {
	g          int
	state      string // present, skip, or w17 tokens
	plant      string
	pgHash     string
	pgHeight   string
	pgObj      bool
	heightPg   string
	dg         string
	claimed    bool
	claimHash  string
	claimH     uint64
	claimOK    bool
	anchor     string
	valid      bool
	issued     bool
	issuedMiss bool
	body       PathBody
	hasBody    bool
}

type recordBody struct {
	shape     string
	shapeItem []string
	malformed bool

	headers []hdr

	seedValue string

	// checkpoints[g] is the leg results, or nil when the stage's legs are
	// not defined (the JSON value is then the string not defined).
	cpLegs    [Stages][]string
	cpDefined [Stages]bool

	wp    [17]string // g = 1..17, index g-1
	paths [17]pathRec

	anchors       [16]string
	anchoredTotal int
	skipList      []int

	a, b, n, s, deltaH        string
	pbObj                     bool
	pbHash                    string
	pbHeight                  uint64
	heightPb, indexCmpPb, tPb string
	heightIa, tIa             string
	fsN, fsS, fsD             string
	legsOutside               string

	sixLeg    string // "object", "not defined", "not read"
	sixIdx    []int
	sixRes    []string
	sixRuns   string // "array" or "not read"
	sixRunObj []sixObj

	wholeLegs int
	wholeSpan string
	perCount  [Stages]int
	perSpan   [Stages]string
	hikeLegs int
	hikeSpan string
}

type sixObj struct {
	idx []int
	res []string
}

func measure(in VerifyInput, o *Object) []byte {
	p := in.Profile
	items := map[string]bool{}

	// --- Rule A4.1 shape ---
	if !o.HasStages || !o.StagesLenOK {
		items["stages length"] = true
	}
	if o.ExtraKeys || o.BadTypes {
		items["field set"] = true
	}
	for g := 0; g < Stages; g++ {
		st := stageAt(o, g)
		if !st.Present {
			// An absent stage map is reported through the per-field
			// conditions (SPEC §14.1). Width is reported only for a
			// checkpoint or a leg result that is present.
			items["field set"] = true
			items["leg_count"] = true
			if g == 0 {
				items["seed beacon"] = true
			}
			if g == 17 {
				items["W_17 path"] = true
			}
			continue
		}
		if st.Extra {
			items["field set"] = true
		}
		if g == 0 && st.HasPath {
			items["stage-0 path key"] = true
		}
		if g > 0 && !st.HasPath {
			items["field set"] = true
		}
		if g == 17 && (!st.HasPath || len(st.Path) != 0) {
			items["W_17 path"] = true
		}
		// value types: a present key of the wrong type leaves Has* false.
		if !st.HasBeacon || !st.HasHeight || !st.HasLegCount || !st.HasWaypoint || !st.HasCP || !st.HasLR {
			items["field set"] = true
		}
		if g > 0 && st.HasPath && st.Path == nil && !pathIsBytes(st) {
			// path present but not a byte string: field set, and W_17 when g=17
			items["field set"] = true
		}
		checkLegShape(st, items)
		if g == 0 {
			if !o.HasSeedBeacon || !st.HasBeacon || !bytesEq(st.Beacon, o.SeedBeacon) {
				items["seed beacon"] = true
			}
		}
		if st.HeightAbove {
			items["height_range"] = true
		}
	}
	if !topFieldsPresent(o) {
		items["field set"] = true
	}
	if o.LastHeightAbove {
		items["height_range"] = true
	}
	if !constantsMatch(o, p) {
		items["constant"] = true
	}

	// header order and hash differ, treating an over-range height as absent.
	hashes, heights, hashOK, heightOK := headerSeries(o)
	for i := 1; i < len(hashes); i++ {
		if !hashOK[i] || !hashOK[i-1] || bytesEq(hashes[i], hashes[i-1]) {
			items["header hash differ"] = true
		}
		if !heightOK[i] || !heightOK[i-1] || heights[i] <= heights[i-1] {
			items["header height order"] = true
		}
	}
	// last_interrupt: 32 bytes, differs from stages[17].beacon, height strictly greater.
	liOK := o.HasLastInterrupt && len(o.LastInterrupt) == 32
	st17 := stageAt(o, 17)
	if !liOK || !o.HasLastInterruptHeight || o.LastHeightAbove || !st17.HasBeacon || len(st17.Beacon) != 32 || bytesEq(o.LastInterrupt, st17.Beacon) || !st17.HasHeight || st17.HeightAbove || o.LastInterruptHeight <= st17.BeaconHeight {
		items["last_interrupt"] = true
	}

	rb := recordBody{}
	if len(items) == 0 {
		rb.shape = "match"
	} else {
		rb.shape = "mismatch"
		for _, s := range shapeOrder {
			if items[s] {
				rb.shapeItem = append(rb.shapeItem, s)
			}
		}
	}

	// --- Rule A4.2 headers ---
	rb.headers = make([]hdr, 19)
	cache := map[uint64]*IndexDoc{}
	get := func(h uint64) *IndexDoc {
		if d, ok := cache[h]; ok {
			return d
		}
		var doc *IndexDoc
		if in.Index != nil && !heightAboveU(h) {
			if body, ok := in.Index.Get(h); ok {
				if parsed, good := ParseIndexBody(body); good {
					doc = &parsed
				}
			}
		}
		cache[h] = doc
		return doc
	}
	for i := 0; i < 19; i++ {
		rb.headers[i] = oneHeader(hashes[i], hashOK[i], heights[i], heightOK[i], get)
	}

	// terrain, once, when the seed inputs are usable.
	var terrain *Terrain
	var seed []byte
	if o.HasPublicKey && len(o.PublicKey) == 32 && o.HasSeedBeacon && len(o.SeedBeacon) == 32 && p.Check() == nil {
		terrain, _ = FillTerrain(p, o.PublicKey, o.SeedBeacon)
		if terrain != nil {
			seed = terrain.Seed()
		}
	}

	// seed_value
	st0 := stageAt(o, 0)
	if seed != nil && st0.HasWaypoint && len(st0.Waypoint) == 32 && bytesEq(st0.Waypoint, seed) {
		rb.seedValue = "match"
	} else {
		rb.seedValue = "mismatch"
	}

	// --- Rule A4.3 chain ---
	for g := 0; g < Stages; g++ {
		st := stageAt(o, g)
		usable := stageUsable(st)
		if !usable {
			rb.cpDefined[g] = false
			continue
		}
		rb.cpDefined[g] = true
		n := len(st.Checkpoints)
		rb.cpLegs[g] = make([]string, n)
		var limb []byte
		if g == 0 {
			limb = seed
		} else if st.HasWaypoint && len(st.Waypoint) == 32 {
			limb = st.Waypoint
		}
		for j := 0; j < n; j++ {
			hasLR := j < len(st.LegResults) && len(st.LegResults[j]) == 8
			if !hasLR || terrain == nil || limb == nil {
				rb.cpLegs[g][j] = "mismatch"
				continue
			}
			var prev []byte
			if j == 0 {
				prev = limb
			} else if len(st.Checkpoints[j-1]) == 32 {
				prev = st.Checkpoints[j-1]
			}
			if prev == nil || len(st.Checkpoints[j]) != 32 {
				rb.cpLegs[g][j] = "mismatch"
				continue
			}
			lr := U64le(st.LegResults[j])
			want := Checkpoint(terrain, prev, limb, lr)
			if bytesEq(want, st.Checkpoints[j]) {
				rb.cpLegs[g][j] = "match"
			} else {
				rb.cpLegs[g][j] = "mismatch"
			}
		}
	}
	for g := 0; g < 17; g++ {
		stg := stageAt(o, g)
		stn := stageAt(o, g+1)
		if !stagePairUsable(stg, stn) || terrain == nil {
			rb.wp[g] = "not defined"
			continue
		}
		b := stn.Beacon
		w := stg.Waypoint
		fold := CheckpointFold(stg.Checkpoints)
		sample := SlotSample(terrain, b)
		h := sha256.New()
		h.Write(b)
		h.Write(sample)
		h.Write(w)
		h.Write(fold)
		got := h.Sum(nil)
		if bytesEq(got, stn.Waypoint) {
			rb.wp[g] = "match"
		} else {
			rb.wp[g] = "mismatch"
		}
	}

	// --- Rule A4.5 paths ---
	interrupts := storedInterrupts(o)
	for g := 1; g <= 16; g++ {
		pr := pathRec{g: g}
		st := stageAt(o, g)
		if !st.HasPath || len(st.Path) == 0 {
			pr.state = "skip"
			pr.plant = "n/a"
			pr.anchor = "not anchored"
			rb.paths[g-1] = pr
			rb.anchors[g-1] = "not anchored"
			rb.skipList = append(rb.skipList, g)
			continue
		}
		pr.state = "present"
		body, ok := DecodePathBody(st.Path)
		wp := st.Waypoint
		if ok && st.HasWaypoint && body.ValidateFor(wp) {
			pr.valid = true
			pr.hasBody = true
			pr.body = body
			pr.claimed = true
			pr.claimOK = true
			pr.claimHash = DisplayHex(body.BlockHash)
			pr.claimH = uint64(body.BlockHeight)
		}
		inWindow := false
		stored := false
		if pr.valid {
			H := st.BeaconHeight
			if st.HasHeight && !st.HeightAbove {
				inWindow = anchorHit(H, uint64(body.BlockHeight))
				stored = interruptMatch(interrupts, uint64(body.BlockHeight), body.BlockHash)
			}
		}
		if !pr.valid {
			pr.plant = "mismatch"
			pr.anchor = "not anchored"
		} else if !inWindow || !stored {
			pr.plant = "not read"
			pr.anchor = "not anchored"
			pr.issued = false
		} else {
			pr.issued = true
			doc := get(uint64(body.BlockHeight))
			if doc == nil {
				pr.plant = "not read"
				pr.issuedMiss = true
				pr.anchor = "not anchored"
			} else if plantMatch(doc, body) {
				pr.plant = "match"
				pr.anchor = "anchored"
				pr.pgObj = true
				pr.pgHash = DisplayHex(body.BlockHash)
				pr.pgHeight = strconv.FormatUint(uint64(body.BlockHeight), 10)
				pr.heightPg = pr.pgHeight
				// d_g uses header.height of I_{g-1} when that comparison matches.
				ih := rb.headers[g]
				if ih.indexCmp == "match" {
					d := int64(body.BlockHeight) - int64(ih.indexHeight)
					pr.dg = strconv.FormatInt(d, 10)
				}
			} else {
				pr.plant = "mismatch"
				pr.anchor = "not anchored"
			}
		}
		if pr.anchor == "anchored" {
			rb.anchoredTotal++
		}
		rb.anchors[g-1] = pr.anchor
		rb.paths[g-1] = pr
	}
	// g = 17
	{
		pr := pathRec{g: 17}
		st := stageAt(o, 17)
		if st.HasPath && len(st.Path) == 0 {
			pr.state = "not fetched by design"
		} else {
			pr.state = "not defined"
			if body, ok := DecodePathBody(st.Path); ok {
				pr.claimed = true
				pr.claimOK = true
				pr.claimHash = DisplayHex(body.BlockHash)
				pr.claimH = uint64(body.BlockHeight)
			}
		}
		rb.paths[16] = pr
	}

	// --- Rule A4.11 bookends, then A4.6–A4.8 ---
	bookends(&rb, o, get)

	// --- analysis (Rule A4.10), independent of bookends except legs_outside ---
	analysis(&rb, o)

	// --- Rule A4.4 six-leg ---
	sixLeg(&rb, in, o, terrain, seed)

	return encodeRecord(p, rb)
}

func stageAt(o *Object, g int) Stage {
	if o == nil || g < 0 || g >= len(o.Stages) {
		return Stage{}
	}
	return o.Stages[g]
}

func pathIsBytes(st Stage) bool { return true }

func checkLegShape(st Stage, items map[string]bool) {
	if !st.HasCP || !st.HasLR || !st.HasLegCount {
		items["leg_count"] = true
		return
	}
	if st.LegCount < 1 || int(st.LegCount) != len(st.Checkpoints) || len(st.Checkpoints) != len(st.LegResults) {
		items["leg_count"] = true
	}
	for _, c := range st.Checkpoints {
		if len(c) != 32 {
			items["checkpoint width"] = true
			break
		}
	}
	for _, c := range st.LegResults {
		if len(c) != 8 {
			items["leg result width"] = true
			break
		}
	}
}

func topFieldsPresent(o *Object) bool {
	if !o.HasKind || !o.HasFill || !o.HasSlotBytes || !o.HasNSlots || !o.HasSampleBytes || !o.HasSteps {
		return false
	}
	if !o.HasPublicKey || len(o.PublicKey) != 32 {
		return false
	}
	if !o.HasSeedBeacon || len(o.SeedBeacon) != 32 {
		return false
	}
	if !o.HasStages {
		return false
	}
	return true
}

func constantsMatch(o *Object, p Profile) bool {
	if p.Check() != nil {
		return false
	}
	if !o.HasKind || o.Kind != Kind || !o.HasFill || o.Fill != FillName {
		return false
	}
	if !o.HasSlotBytes || int(o.SlotBytes) != p.SlotBytes {
		return false
	}
	if !o.HasSampleBytes || int(o.SampleBytes) != p.SampleBytes {
		return false
	}
	if !o.HasSteps || int(o.StepsPerLeg) != p.StepsPerLeg {
		return false
	}
	if !o.HasNSlots || int(o.NSlots) != p.NSlots() {
		return false
	}
	return true
}

// headerSeries is the 19 stored headers. An over-range height is absent.
func headerSeries(o *Object) (hashes [][]byte, heights []uint64, hashOK, heightOK []bool) {
	hashes = make([][]byte, 19)
	heights = make([]uint64, 19)
	hashOK = make([]bool, 19)
	heightOK = make([]bool, 19)
	for g := 0; g < Stages; g++ {
		st := stageAt(o, g)
		if st.HasBeacon && len(st.Beacon) == 32 {
			hashes[g] = st.Beacon
			hashOK[g] = true
		}
		if st.HasHeight && !st.HeightAbove {
			heights[g] = st.BeaconHeight
			heightOK[g] = true
		}
	}
	if o.HasLastInterrupt && len(o.LastInterrupt) == 32 {
		hashes[18] = o.LastInterrupt
		hashOK[18] = true
	}
	if o.HasLastInterruptHeight && !o.LastHeightAbove {
		heights[18] = o.LastInterruptHeight
		heightOK[18] = true
	}
	return
}

func heightAboveU(h uint64) bool { return h > MaxJSONInt }

func oneHeader(hash []byte, hashOK bool, height uint64, heightOK bool, get func(uint64) *IndexDoc) hdr {
	h := hdr{indexCmp: "not read", t: "not read"}
	if hashOK {
		h.storedHash = NaturalHex(hash)
	} else {
		h.storedHash = "not defined"
		h.indexCmp = "not read"
	}
	if heightOK {
		h.storedHeight = strconv.FormatUint(height, 10)
	} else {
		h.storedHeight = "not defined"
	}
	if !hashOK || !heightOK {
		return h
	}
	doc := get(height)
	if doc == nil || !doc.HeaderOK {
		return h
	}
	if bytesEq(doc.Header.HashNatural, hash) && doc.Header.Height == height {
		h.indexCmp = "match"
		h.heightMatch = true
		h.indexHeight = doc.Header.Height
		if doc.Header.HasTime {
			h.t = strconv.FormatUint(doc.Header.Time, 10)
			h.tInt = int64(doc.Header.Time)
			h.hasT = true
		}
	} else {
		h.indexCmp = "mismatch"
	}
	return h
}

func stageUsable(st Stage) bool {
	if !st.Present || !st.HasCP || !st.HasWaypoint || len(st.Waypoint) != 32 {
		return false
	}
	for _, c := range st.Checkpoints {
		if len(c) != 32 {
			return false
		}
	}
	return true
}

func stagePairUsable(stg, stn Stage) bool {
	if !stageUsable(stg) {
		return false
	}
	if !stn.Present || !stn.HasBeacon || len(stn.Beacon) != 32 || !stn.HasWaypoint || len(stn.Waypoint) != 32 {
		return false
	}
	return true
}

type interrupt struct {
	height uint64
	hash   []byte
	ok     bool
}

func storedInterrupts(o *Object) []interrupt {
	out := make([]interrupt, 0, 18)
	for g := 1; g <= 17; g++ {
		st := stageAt(o, g)
		if st.HasBeacon && len(st.Beacon) == 32 && st.HasHeight && !st.HeightAbove {
			out = append(out, interrupt{height: st.BeaconHeight, hash: st.Beacon, ok: true})
		}
	}
	if o.HasLastInterrupt && len(o.LastInterrupt) == 32 && o.HasLastInterruptHeight && !o.LastHeightAbove {
		out = append(out, interrupt{height: o.LastInterruptHeight, hash: o.LastInterrupt, ok: true})
	}
	return out
}

func anchorHit(h, inclusion uint64) bool {
	return AnchorCompare(h, inclusion) == "anchored"
}

func interruptMatch(list []interrupt, height uint64, natural []byte) bool {
	for _, it := range list {
		if it.ok && it.height == height && bytesEq(it.hash, natural) {
			return true
		}
	}
	return false
}

func plantMatch(doc *IndexDoc, body PathBody) bool {
	if doc == nil || !doc.HasNet || (doc.Network != "main" && doc.Network != "synthetic") {
		return false
	}
	for _, p := range doc.Plants {
		if !p.OK {
			continue
		}
		if bytesEq(p.Root, body.Root) && bytesEq(p.Txid, body.Txid) && p.Vout == uint64(body.Vout) &&
			bytesEq(p.BlockHash, body.BlockHash) && p.BlockHeight == uint64(body.BlockHeight) {
			return true
		}
	}
	return false
}

func bookends(rb *recordBody, o *Object, get func(uint64) *IndexDoc) {
	valid := 0
	issuedMiss := false
	anchored := 0
	var aG, bG int
	for g := 1; g <= 16; g++ {
		pr := rb.paths[g-1]
		if pr.valid {
			valid++
		}
		if pr.issued && pr.issuedMiss {
			issuedMiss = true
		}
		if pr.anchor == "anchored" {
			anchored++
			if aG == 0 {
				aG = g
			}
			bG = g
		}
	}
	nd := func() {
		rb.a, rb.b, rb.n, rb.s, rb.deltaH = "not defined", "not defined", "not defined", "not defined", "not defined"
		rb.pbObj = false
		rb.heightPb, rb.indexCmpPb, rb.tPb = "not defined", "not defined", "not defined"
		rb.heightIa, rb.tIa = "not defined", "not defined"
		rb.fsN, rb.fsS, rb.fsD = "not defined", "not defined", "not defined"
		rb.legsOutside = "not defined"
	}
	nr := func() {
		rb.a, rb.b, rb.n, rb.s, rb.deltaH = "not read", "not read", "not read", "not read", "not read"
		rb.pbObj = false
		rb.heightPb, rb.indexCmpPb, rb.tPb = "not read", "not read", "not read"
		rb.heightIa, rb.tIa = "not read", "not read"
		rb.fsN, rb.fsS, rb.fsD = "not read", "not read", "not read"
		rb.legsOutside = "not read"
	}
	// step 1
	if valid < 2 {
		nd()
		return
	}
	// step 2
	if issuedMiss {
		nr()
		return
	}
	// step 3
	if anchored < 2 {
		nd()
		return
	}
	// step 4
	rb.a = strconv.Itoa(aG)
	rb.b = strconv.Itoa(bG)
	n := 0
	for g := aG; g <= bG-1 && g < Stages; g++ {
		st := stageAt(o, g)
		if st.HasCP {
			n += len(st.Checkpoints)
		}
	}
	rb.n = strconv.Itoa(n)
	pb := rb.paths[bG-1]
	ia := rb.headers[aG]
	var pbDoc *IndexDoc
	pbNotRead := true
	pbMismatch := false
	if pb.claimOK {
		pbDoc = get(pb.claimH)
		if pbDoc != nil && pbDoc.HeaderOK {
			pbNotRead = false
			disp, _ := ParseHex32(pb.claimHash)
			// claimHash is display hex. Compare display bytes with header.hash
			// display bytes, without reversing either side (SPEC §14).
			natural := Reverse32(disp)
			if bytesEq(pbDoc.Header.HashNatural, natural) && pbDoc.Header.Height == pb.claimH {
				rb.indexCmpPb = "match"
				rb.heightPb = strconv.FormatUint(pbDoc.Header.Height, 10)
				rb.pbObj = true
				rb.pbHash = pb.claimHash
				rb.pbHeight = pb.claimH
				if pbDoc.Header.HasTime {
					rb.tPb = strconv.FormatUint(pbDoc.Header.Time, 10)
				} else {
					rb.tPb = "not read"
				}
			} else {
				pbMismatch = true
				rb.indexCmpPb = "mismatch"
				rb.heightPb = "not defined"
				rb.tPb = "not read"
				rb.pbObj = true
				rb.pbHash = pb.claimHash
				rb.pbHeight = pb.claimH
			}
		}
	}
	if ia.indexCmp == "not read" || !pb.claimOK || pbNotRead {
		nr()
		return
	}
	// mismatch keeps a, b, N.
	if ia.indexCmp == "match" {
		rb.heightIa = strconv.FormatUint(ia.indexHeight, 10)
		if ia.hasT {
			rb.tIa = ia.t
		} else {
			rb.tIa = "not read"
		}
	} else {
		rb.heightIa = "not defined"
		rb.tIa = "not read"
	}
	// S
	if rb.tPb != "not read" && rb.tIa != "not read" && ia.hasT && pbDoc != nil && pbDoc.Header.HasTime && rb.indexCmpPb == "match" {
		s := int64(pbDoc.Header.Time) - ia.tInt
		rb.s = strconv.FormatInt(s, 10)
	} else {
		rb.s = "not read"
	}
	// delta_h
	if rb.indexCmpPb == "match" && ia.indexCmp == "match" {
		d := int64(pbDoc.Header.Height) - int64(ia.indexHeight)
		rb.deltaH = strconv.FormatInt(d, 10)
	} else {
		rb.deltaH = "not defined"
	}
	if pbMismatch && rb.indexCmpPb == "" {
		rb.indexCmpPb = "mismatch"
	}
	// fixed_start
	fsN := 0
	for g := 1; g <= bG-1 && g < Stages; g++ {
		st := stageAt(o, g)
		if st.HasCP {
			fsN += len(st.Checkpoints)
		}
	}
	rb.fsN = strconv.Itoa(fsN)
	h1 := rb.headers[1]
	if rb.tPb != "not read" && h1.hasT && rb.indexCmpPb == "match" {
		tpb, _ := strconv.ParseInt(rb.tPb, 10, 64)
		rb.fsS = strconv.FormatInt(tpb-h1.tInt, 10)
	} else if rb.indexCmpPb == "not read" || !h1.hasT && h1.indexCmp == "not read" {
		rb.fsS = "not read"
	} else if !h1.hasT || rb.tPb == "not read" {
		rb.fsS = "not read"
	} else {
		rb.fsS = "not read"
	}
	if h1.indexCmp == "not read" || pbNotRead {
		rb.fsD = "not read"
	} else if h1.indexCmp == "match" && rb.indexCmpPb == "match" {
		d := int64(pbDoc.Header.Height) - int64(h1.indexHeight)
		rb.fsD = strconv.FormatInt(d, 10)
	} else {
		rb.fsD = "not defined"
	}
	// legs outside a..b-1
	out := 0
	for g := 0; g < Stages; g++ {
		if g >= aG && g <= bG-1 {
			continue
		}
		st := stageAt(o, g)
		if st.HasCP {
			out += len(st.Checkpoints)
		}
	}
	rb.legsOutside = strconv.Itoa(out)
	_ = pbMismatch
}

func analysis(rb *recordBody, o *Object) {
	whole := 0
	for g := 0; g < Stages; g++ {
		st := stageAt(o, g)
		c := 0
		if st.HasCP {
			c = len(st.Checkpoints)
		}
		rb.perCount[g] = c
		// Stages 1–17 only. Stage 0 is before the first interrupt.
		if g >= 1 {
			whole += c
		}
		rb.perSpan[g] = spanOf(rb, g)
	}
	rb.wholeLegs = whole
	rb.hikeLegs = whole
	// whole span T(I_17) − T(I_0) = headers[18].T − headers[1].T
	rb.wholeSpan = subT(rb.headers[18], rb.headers[1])
	// Same integer: nTime(I_17) − nTime(I_0). Median-time-past is not this span.
	rb.hikeSpan = rb.wholeSpan
}

func spanOf(rb *recordBody, g int) string {
	switch {
	case g == 0:
		return subT(rb.headers[1], rb.headers[0])
	case g >= 1 && g <= 16:
		return subT(rb.headers[g+1], rb.headers[g])
	default:
		return subT(rb.headers[18], rb.headers[17])
	}
}

func subT(a, b hdr) string {
	if !a.hasT || !b.hasT {
		return "not read"
	}
	return strconv.FormatInt(a.tInt-b.tInt, 10)
}

func sixLeg(rb *recordBody, in VerifyInput, o *Object, terrain *Terrain, seed []byte) {
	total := 0
	for g := 0; g < Stages; g++ {
		st := stageAt(o, g)
		if st.HasCP {
			total += len(st.Checkpoints)
		}
	}
	if total < SixLegDrawCount {
		rb.sixLeg = "not defined"
		rb.sixRuns = "array"
		return
	}
	runs := in.SixRuns
	if runs < 1 {
		runs = 1
	}
	rb.sixRuns = "array"
	for r := 0; r < runs; r++ {
		var src ByteSource
		if in.Draw != nil {
			src = in.Draw
		} else {
			src = csprng{}
		}
		idx, short, ok := DrawSix(src, total)
		if short || !ok {
			rb.sixLeg = "not read"
			rb.sixRuns = "not read"
			rb.sixRunObj = nil
			return
		}
		res := make([]string, len(idx))
		for i, q := range idx {
			res[i] = recomputeLeg(o, terrain, seed, q)
		}
		rb.sixRunObj = append(rb.sixRunObj, sixObj{idx: idx, res: res})
	}
	rb.sixLeg = "object"
	rb.sixIdx = rb.sixRunObj[0].idx
	rb.sixRes = rb.sixRunObj[0].res
}

func recomputeLeg(o *Object, terrain *Terrain, seed []byte, q int) string {
	if terrain == nil || seed == nil {
		return "mismatch"
	}
	left := q
	for g := 0; g < Stages; g++ {
		st := stageAt(o, g)
		n := 0
		if st.HasCP {
			n = len(st.Checkpoints)
		}
		if left >= n {
			left -= n
			continue
		}
		j := left
		var prev []byte
		if j == 0 {
			if g == 0 {
				prev = seed
			} else if st.HasWaypoint && len(st.Waypoint) == 32 {
				prev = st.Waypoint
			}
		} else if j-1 < len(st.Checkpoints) && len(st.Checkpoints[j-1]) == 32 {
			prev = st.Checkpoints[j-1]
		}
		if prev == nil || j >= len(st.LegResults) || len(st.LegResults[j]) != 8 {
			return "mismatch"
		}
		got := LegResult(terrain, prev)
		if got == U64le(st.LegResults[j]) {
			return "match"
		}
		return "mismatch"
	}
	return "mismatch"
}
