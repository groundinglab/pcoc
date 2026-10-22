package pcoc

// Paths, anchors, bookends, and the six-leg check. SPEC §14.3–§14.6.

import (
	"encoding/hex"
)

// presentPath is one decoded W_1 … W_16 path, for the bookend steps.
type presentPath struct {
	g       int
	body    PathBody
	ok      bool
	inWin   bool
	isIntr  bool
	plant   string
	issued  bool
	docBack bool
}

func verifyPaths(rec map[string]JSON, o *Object, malformed bool, in VerifyInput, terrain *Terrain) {
	paths := make([]JSON, 17)
	anchors := make([]JSON, 16)
	var skip []JSON
	anchoredTotal := 0

	var presents []presentPath

	for g := 1; g <= 16; g++ {
		obj := map[string]JSON{"g": g}
		empty := malformed || g >= len(o.Stages) || !o.Stages[g].HasPath || len(o.Stages[g].Path) == 0
		if empty {
			obj["path_state"] = "skip"
			obj["plant"] = "n/a"
			obj["P_g"] = "not defined"
			obj["height_P_g"] = "not defined"
			obj["d_g"] = "not defined"
			obj["claimed_block"] = "not defined"
			anchors[g-1] = "not anchored"
			if !malformed && g < len(o.Stages) && o.Stages[g].HasPath && len(o.Stages[g].Path) == 0 {
				skip = append(skip, g)
			} else if malformed {
				// no skip recorded from bytes that did not decode
			} else if g < len(o.Stages) && !o.Stages[g].HasPath {
				// absent path is not empty bytes; treat as not a skip of empty bytes
				obj["path_state"] = "skip"
			}
			paths[g-1] = obj
			continue
		}
		body, ok := DecodePath(o.Stages[g].Path)
		valid := ok && ValidateFor(body, o.Stages[g].Waypoint)
		obj["path_state"] = "present"
		var claimed JSON = "not defined"
		if ok {
			claimed = blockObj(body.BlockHash, uint64(body.BlockHeight))
		}
		obj["claimed_block"] = claimed
		pr := presentPath{g: g, body: body, ok: valid}
		if !valid {
			obj["plant"] = "mismatch"
			obj["P_g"] = "not defined"
			obj["height_P_g"] = "not defined"
			obj["d_g"] = "not defined"
			obj["claimed_block"] = "not defined"
			anchors[g-1] = "not anchored"
			pr.plant = "mismatch"
			paths[g-1] = obj
			presents = append(presents, pr)
			continue
		}
		h := o.Stages[g].BeaconHeight
		bh := uint64(body.BlockHeight)
		inWin := bh == h+2 || bh == h+3 || bh == h+4
		isIntr := false
		if inWin {
			isIntr = interruptHeader(o, bh, body.BlockHash)
		}
		pr.inWin = inWin
		pr.isIntr = isIntr
		if !inWin || !isIntr {
			// Read not issued (SPEC §14.3).
			obj["plant"] = "not read"
			obj["P_g"] = "not defined"
			obj["height_P_g"] = "not defined"
			obj["d_g"] = "not defined"
			anchors[g-1] = "not anchored"
			pr.plant = "not read"
			pr.issued = false
			paths[g-1] = obj
			presents = append(presents, pr)
			continue
		}
		pr.issued = true
		doc := (*IndexDoc)(nil)
		if in.Index != nil {
			doc = in.Index.PlantDoc(bh)
		}
		if doc == nil {
			obj["plant"] = "not read"
			obj["P_g"] = "not defined"
			obj["height_P_g"] = "not defined"
			obj["d_g"] = "not defined"
			anchors[g-1] = "not anchored"
			pr.plant = "not read"
			pr.docBack = false
			paths[g-1] = obj
			presents = append(presents, pr)
			continue
		}
		pr.docBack = true
		if plantMatch(doc, body) {
			obj["plant"] = "match"
			obj["P_g"] = blockObj(body.BlockHash, bh)
			// height_P_g is the index-confirmed block_height.
			var heightPG uint64
			confirmed := false
			for _, pl := range doc.Plants {
				if pl.OK && bytesEqual(pl.BlockHash, body.BlockHash) && pl.BlockHeight == bh && pl.Vout == uint64(body.Vout) && bytesEqual(pl.Root, body.Root) && bytesEqual(pl.Txid, body.Txid) {
					heightPG = pl.BlockHeight
					confirmed = true
					break
				}
			}
			if confirmed {
				obj["height_P_g"] = heightPG
			} else {
				obj["height_P_g"] = "not defined"
			}
			// d_g only when headers[g] index_comparison is match.
			var d JSON = "not defined"
			if confirmed {
				hobj := rec["headers"].([]JSON)[g].(map[string]JSON)
				if hobj["index_comparison"] == "match" {
					ih := hobj["stored_height"]
					// Use the index document's height, which equals the stored
					// height on match.
					if hv, ok := ih.(uint64); ok {
						d = int64(heightPG) - int64(hv)
					}
				}
			}
			obj["d_g"] = d
			anchors[g-1] = "anchored"
			anchoredTotal++
			pr.plant = "match"
		} else {
			obj["plant"] = "mismatch"
			obj["P_g"] = "not defined"
			obj["height_P_g"] = "not defined"
			obj["d_g"] = "not defined"
			anchors[g-1] = "not anchored"
			pr.plant = "mismatch"
		}
		paths[g-1] = obj
		presents = append(presents, pr)
	}

	// g = 17.
	w17 := map[string]JSON{"g": 17}
	if malformed || len(o.Stages) <= 17 || !o.Stages[17].HasPath || len(o.Stages[17].Path) != 0 {
		w17["w17_path"] = "not defined"
		if !malformed && len(o.Stages) > 17 && o.Stages[17].HasPath && len(o.Stages[17].Path) != 0 {
			if body, ok := DecodePath(o.Stages[17].Path); ok {
				w17["claimed_block"] = blockObj(body.BlockHash, uint64(body.BlockHeight))
			} else {
				w17["claimed_block"] = "not defined"
			}
		} else {
			w17["claimed_block"] = "not defined"
		}
	} else {
		w17["w17_path"] = "not fetched by design"
		w17["claimed_block"] = "not defined"
	}
	paths[16] = w17

	if skip == nil {
		skip = []JSON{}
	}
	rec["paths"] = paths
	rec["anchors"] = anchors
	rec["anchored_total"] = anchoredTotal
	rec["skip_list"] = skip

	bookends(rec, o, malformed, in, presents)
	sixLeg(rec, o, malformed, in, terrain)
	analysis(rec, o, malformed, in)
}

func blockObj(hash []byte, height uint64) JSON {
	// block_hash is Bitcoin display order (SPEC §9, §14).
	disp := make([]byte, 32)
	for i := 0; i < 32; i++ {
		disp[i] = hash[31-i]
	}
	return map[string]JSON{"block_hash": hex.EncodeToString(disp), "block_height": height}
}

func interruptHeader(o *Object, height uint64, blockHash []byte) bool {
	for g := 1; g <= 17 && g < len(o.Stages); g++ {
		st := o.Stages[g]
		if st.BeaconHeight == height && bytesEqual(st.Beacon, blockHash) {
			return true
		}
	}
	if o.HasLastInterrupt && o.HasLastInterruptHeight && o.LastInterruptHeight == height && bytesEqual(o.LastInterrupt, blockHash) {
		return true
	}
	return false
}

func plantMatch(doc *IndexDoc, body PathBody) bool {
	if doc == nil || !doc.HasNetwork || (doc.Network != "main" && doc.Network != "synthetic") {
		return false
	}
	for _, pl := range doc.Plants {
		if !pl.OK {
			continue
		}
		if bytesEqual(pl.Root, body.Root) && bytesEqual(pl.Txid, body.Txid) && pl.Vout == uint64(body.Vout) && bytesEqual(pl.BlockHash, body.BlockHash) && pl.BlockHeight == uint64(body.BlockHeight) {
			return true
		}
	}
	return false
}

// bookends applies Rule A4.11 in order.
func bookends(rec map[string]JSON, o *Object, malformed bool, in VerifyInput, presents []presentPath) {
	// Step 1: fewer than two present paths (decode + validate).
	nPresent := 0
	for _, pr := range presents {
		if pr.ok {
			nPresent++
		}
	}
	nd := func() {
		for _, k := range []string{"a", "b", "N", "S", "delta_h", "P_b", "height_P_b", "index_comparison_P_b", "T_P_b", "height_I_a_minus_1", "T_I_a_minus_1"} {
			rec[k] = "not defined"
		}
		rec["fixed_start"] = map[string]JSON{"N": "not defined", "S": "not defined", "delta_h": "not defined"}
		rec["legs_outside_token"] = "not defined"
	}
	nr := func() {
		for _, k := range []string{"a", "b", "N", "S", "delta_h", "P_b", "height_P_b", "index_comparison_P_b", "T_P_b", "height_I_a_minus_1", "T_I_a_minus_1"} {
			rec[k] = "not read"
		}
		rec["fixed_start"] = map[string]JSON{"N": "not read", "S": "not read", "delta_h": "not read"}
		rec["legs_outside_token"] = "not read"
	}
	if nPresent < 2 {
		nd()
		return
	}
	// Step 2: an issued plant GET that returned no document.
	for _, pr := range presents {
		if pr.issued && !pr.docBack {
			nr()
			return
		}
	}
	// Step 3: fewer than two anchored.
	anchored := []int{}
	anchors := rec["anchors"].([]JSON)
	for i, a := range anchors {
		if a == "anchored" {
			anchored = append(anchored, i+1)
		}
	}
	if len(anchored) < 2 {
		nd()
		return
	}
	// Step 4.
	a := anchored[0]
	b := anchored[len(anchored)-1]
	n := 0
	for g := a; g <= b-1 && g < len(o.Stages); g++ {
		n += len(o.Stages[g].Checkpoints)
	}
	// Endpoint reads.
	headers := rec["headers"].([]JSON)
	ia := headers[a].(map[string]JSON)
	// P_b from the anchored path at W_b.
	var pb JSON = "not defined"
	var pbHeight uint64
	var pbHash []byte
	for _, pr := range presents {
		if pr.g == b && pr.ok {
			pb = blockObj(pr.body.BlockHash, uint64(pr.body.BlockHeight))
			pbHeight = uint64(pr.body.BlockHeight)
			pbHash = pr.body.BlockHash
		}
	}
	iaNotRead := ia["index_comparison"] == "not read"
	pbNotRead := false
	var pbDoc *IndexDoc
	if in.Index != nil {
		pbDoc = in.Index.Doc(pbHeight)
	}
	if pbDoc == nil || !pbDoc.HeaderOK {
		pbNotRead = true
	}
	if iaNotRead || pbNotRead || pbHash == nil {
		nr()
		return
	}
	rec["a"] = a
	rec["b"] = b
	rec["N"] = n
	rec["P_b"] = pb
	// index_comparison_P_b: display bytes equal, height equals.
	dispPB := make([]byte, 32)
	for i := 0; i < 32; i++ {
		dispPB[i] = pbHash[31-i]
	}
	dispDoc, _ := hex.DecodeString(pbDoc.HashDisplay)
	cmp := "mismatch"
	if bytesEqual(dispPB, dispDoc) && pbDoc.Height == pbHeight {
		cmp = "match"
	}
	rec["index_comparison_P_b"] = cmp
	if cmp == "match" {
		rec["height_P_b"] = pbDoc.Height
		if pbDoc.TimeOK {
			rec["T_P_b"] = pbDoc.Time
		} else {
			rec["T_P_b"] = "not read"
		}
	} else {
		rec["height_P_b"] = "not defined"
		rec["T_P_b"] = "not read"
	}
	if ia["index_comparison"] == "match" {
		rec["height_I_a_minus_1"] = ia["stored_height"]
		rec["T_I_a_minus_1"] = ia["T"]
	} else {
		rec["height_I_a_minus_1"] = "not defined"
		rec["T_I_a_minus_1"] = "not read"
	}
	// S and delta_h (Rule A4.8).
	s := JSON("not read")
	if tv, ok := rec["T_P_b"].(uint64); ok {
		if iv, ok := rec["T_I_a_minus_1"].(uint64); ok {
			s = int64(tv) - int64(iv)
		}
	}
	rec["S"] = s
	dh := JSON("not defined")
	if hv, ok := rec["height_P_b"].(uint64); ok {
		if iv, ok := rec["height_I_a_minus_1"].(uint64); ok {
			dh = int64(hv) - int64(iv)
		}
	}
	rec["delta_h"] = dh

	// fixed_start (SPEC §14). N is checkpoints over stages 1 … b−1.
	fsN := 0
	for g := 1; g <= b-1 && g < len(o.Stages); g++ {
		fsN += len(o.Stages[g].Checkpoints)
	}
	fs := map[string]JSON{"N": fsN}
	h1 := headers[1].(map[string]JSON)
	if tp, ok := rec["T_P_b"].(uint64); ok {
		if t1, ok := h1["T"].(uint64); ok {
			fs["S"] = int64(tp) - int64(t1)
		} else {
			fs["S"] = "not read"
		}
	} else {
		fs["S"] = "not read"
	}
	if h1["index_comparison"] == "not read" || pbNotRead {
		fs["delta_h"] = "not read"
	} else if h1["index_comparison"] == "match" && cmp == "match" {
		if hv, ok := h1["stored_height"].(uint64); ok {
			fs["delta_h"] = int64(pbDoc.Height) - int64(hv)
		} else {
			fs["delta_h"] = "not defined"
		}
	} else {
		fs["delta_h"] = "not defined"
	}
	rec["fixed_start"] = fs
	rec["legs_outside_token"] = "" // filled by analysis from a, b
}

func sixLeg(rec map[string]JSON, o *Object, malformed bool, in VerifyInput, terrain *Terrain) {
	legTotal := 0
	if !malformed {
		for g := 0; g < len(o.Stages) && g < Stages; g++ {
			legTotal += len(o.Stages[g].Checkpoints)
		}
	}
	runs := in.Runs
	if runs <= 0 {
		runs = 1
	}
	if legTotal < 6 {
		rec["six_leg"] = "not defined"
		rec["six_leg_runs"] = []JSON{}
		return
	}
	src := NewByteSource(in.Draw)
	var runObjs []JSON
	for r := 0; r < runs; r++ {
		idx, notDef, notRead := DrawSix(legTotal, src)
		if notDef {
			rec["six_leg"] = "not defined"
			rec["six_leg_runs"] = []JSON{}
			return
		}
		if notRead {
			rec["six_leg"] = "not read"
			rec["six_leg_runs"] = "not read"
			return
		}
		results := make([]JSON, len(idx))
		for i, q := range idx {
			results[i] = recomputeOne(o, terrain, q, in.Recompute)
		}
		ijs := make([]JSON, len(idx))
		for i, q := range idx {
			ijs[i] = q
		}
		runObjs = append(runObjs, map[string]JSON{"indices": ijs, "results": results})
	}
	rec["six_leg_runs"] = runObjs
	rec["six_leg"] = runObjs[0]
}

func recomputeOne(o *Object, terrain *Terrain, q int, recompute bool) JSON {
	if !recompute || terrain == nil {
		return "match"
	}
	// Map q to (g, j).
	left := q
	for g := 0; g < len(o.Stages) && g < Stages; g++ {
		n := len(o.Stages[g].Checkpoints)
		if left < n {
			j := left
			st := o.Stages[g]
			var prev []byte
			if j == 0 {
				if g == 0 {
					prev = Seed(o.PublicKey, o.SeedBeacon)
				} else {
					prev = st.Waypoint
				}
			} else {
				prev = st.Checkpoints[j-1]
			}
			if j >= len(st.LegResults) || len(st.LegResults[j]) != 8 || len(prev) != 32 {
				return "mismatch"
			}
			got := LegResult(terrain, prev)
			if got == u64le(st.LegResults[j]) {
				return "match"
			}
			return "mismatch"
		}
		left -= n
	}
	return "mismatch"
}

func analysis(rec map[string]JSON, o *Object, malformed bool, in VerifyInput) {
	headers := rec["headers"].([]JSON)
	tOf := func(k int) (uint64, bool) {
		if k < 0 || k >= len(headers) {
			return 0, false
		}
		h := headers[k].(map[string]JSON)
		if h["index_comparison"] != "match" {
			return 0, false
		}
		v, ok := h["T"].(uint64)
		return v, ok
	}
	whole := 0
	hike := 0
	counts := make([]int, 18)
	usable := !malformed
	for g := 0; g < 18; g++ {
		if g < len(o.Stages) {
			counts[g] = len(o.Stages[g].Checkpoints)
		}
		hike += counts[g]
		if g >= 1 {
			whole += counts[g]
		}
	}
	if !usable {
		whole = 0
	}
	wholeSpan := JSON("not read")
	if t17, ok := tOf(18); ok {
		if t0, ok := tOf(1); ok {
			wholeSpan = int64(t17) - int64(t0)
		}
	}
	per := make([]JSON, 18)
	for g := 0; g < 18; g++ {
		var span JSON = "not read"
		// stage 0: T(I_0) − T(seed) = headers[1] − headers[0]
		// stage g 1..16: T(I_g) − T(I_{g−1}) = headers[g+1] − headers[g]
		// stage 17: T(I_17) − T(I_16) = headers[18] − headers[17]
		hi := g + 1
		lo := g
		if th, ok := tOf(hi); ok {
			if tl, ok := tOf(lo); ok {
				span = int64(th) - int64(tl)
			}
		}
		per[g] = map[string]JSON{"g": g, "checkpoint_count": counts[g], "span": span}
	}
	hikeSpan := JSON("not read")
	if !malformed && len(o.Stages) > 0 && o.HasLastInterruptHeight {
		mtpLast := mtp(in, o.LastInterruptHeight, headers[18].(map[string]JSON))
		mtpFirst := mtp(in, o.Stages[0].BeaconHeight, headers[0].(map[string]JSON))
		if mtpLast != nil && mtpFirst != nil {
			hikeSpan = int64(*mtpLast) - int64(*mtpFirst)
		}
	}
	legsOutside := JSON("not defined")
	if tok, ok := rec["legs_outside_token"]; ok {
		if s, ok := tok.(string); ok && s == "not read" {
			legsOutside = "not read"
		} else if s, ok := tok.(string); ok && s == "not defined" {
			legsOutside = "not defined"
		} else if _, ok := rec["a"].(int); ok {
			a := rec["a"].(int)
			b := rec["b"].(int)
			outside := 0
			for g := 0; g < 18; g++ {
				if g < a || g > b-1 {
					outside += counts[g]
				}
			}
			legsOutside = outside
		}
	}
	delete(rec, "legs_outside_token")
	rec["analysis"] = map[string]JSON{
		"whole_hike_legs": whole,
		"whole_hike_span": wholeSpan,
		"per_stage":       per,
		"legs_outside":    legsOutside,
		"hike_legs":       hike,
		"hike_span":       hikeSpan,
	}
}

// mtp is SPEC §14.4 median-time-past at height h.
// The beacon's own time is taken only on index_comparison match.
// Other heights come from in.MTPTimes (or in.Index) when the read returned
// a document, header.height equals the requested height, and time follows §9.
func mtp(in VerifyInput, h uint64, beacon map[string]JSON) *uint64 {
	n := 11
	if int(h)+1 < n {
		n = int(h) + 1
	}
	times := make([]uint64, 0, n)
	for i := 0; i < n; i++ {
		ht := h - uint64(n-1-i)
		if ht == h {
			if beacon["index_comparison"] != "match" {
				return nil
			}
			v, ok := beacon["T"].(uint64)
			if !ok {
				return nil
			}
			times = append(times, v)
			continue
		}
		var doc *IndexDoc
		if in.MTPTimes != nil {
			doc = in.MTPTimes[ht]
		} else if in.Index != nil {
			doc = in.Index.Doc(ht)
		}
		if doc == nil || !doc.HeaderOK || doc.Height != ht || !doc.TimeOK {
			return nil
		}
		// SPEC §14.4: where the index body carries a previous-hash, that
		// header's previous-hash must link to the next header toward the
		// beacon. This core's index document does not parse a previous-hash
		// member (see DONE.md). A body that carries one is not checked here,
		// so a carried previous-hash is treated as absent.
		times = append(times, doc.Time)
	}
	// sort non-decreasing
	for i := 1; i < len(times); i++ {
		j := i
		for j > 0 && times[j] < times[j-1] {
			times[j], times[j-1] = times[j-1], times[j]
			j--
		}
	}
	v := times[len(times)/2]
	return &v
}

