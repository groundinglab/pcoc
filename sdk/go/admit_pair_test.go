package pcoc

import "testing"

// TestAdmitPairStartsAtFirstInterrupt locks Rule A4.10 after Christian
// relocated the admit start off the seed beacon to I_0 (stages[1].beacon).
// Stage 0 checkpoints and the pre-start wait are not in (hike_legs, hike_span).
// whole_hike_span stays raw header.time.
func TestAdmitPairStartsAtFirstInterrupt(t *testing.T) {
	const seed = uint64(1000)
	const i0 = uint64(1100)
	const last = uint64(1117)

	stages := make([]Stage, Stages)
	for g := 0; g < Stages; g++ {
		n := 50
		height := i0 + uint64(g) - 1
		if g == 0 {
			n = 90
			height = seed
		}
		if g == 17 {
			n = 140
			height = last - 1
		}
		cps := make([][]byte, n)
		for i := range cps {
			cps[i] = []byte{byte(i)}
		}
		stages[g] = Stage{
			BeaconHeight: height,
			HasHeight:    true,
			HasCP:        true,
			Checkpoints:  cps,
		}
	}
	obj := &Object{
		Stages:                 stages,
		HasLastInterruptHeight: true,
		LastInterruptHeight:    last,
	}

	ntime := func(h uint64) int64 {
		base := int64(1000) * int64(h)
		if h == last {
			return base + 500_000
		}
		return base
	}
	headers := make([]hdr, 19)
	headers[0] = hdr{indexCmp: "match", hasT: true, tInt: ntime(seed)}
	headers[1] = hdr{indexCmp: "match", hasT: true, tInt: ntime(i0)}
	headers[18] = hdr{indexCmp: "match", hasT: true, tInt: ntime(last)}
	rb := &recordBody{
		headers: headers,
		mtpGet: func(h uint64) *IndexDoc {
			return &IndexDoc{
				HeaderOK: true,
				Header: HeaderDoc{
					Height:  h,
					Time:    uint64(ntime(h)),
					HasTime: true,
				},
			}
		},
	}
	analysis(rb, obj)

	if rb.hikeLegs != 940 {
		t.Fatalf("hike_legs %d, want 940 (stages 1–17)", rb.hikeLegs)
	}
	if rb.wholeLegs != 940 {
		t.Fatalf("whole_hike_legs %d, want 940", rb.wholeLegs)
	}
	if rb.hikeLegs == 1030 {
		t.Fatal("stage 0's 90 checkpoints were counted")
	}
	// MTP(I_0) = time(1095). MTP(last) = time(1112). The last block's
	// nTime spike is not the median. A seed start would be 117000.
	if rb.hikeSpan != "17000" {
		t.Fatalf("hike_span %s, want 17000", rb.hikeSpan)
	}
	if rb.wholeSpan != "517000" {
		t.Fatalf("whole_hike_span %s, want 517000 raw T", rb.wholeSpan)
	}
	seeded := mtpAt(rb, seed, headers[0])
	ended := mtpAt(rb, last, headers[18])
	if ended-seeded != 117000 {
		t.Fatalf("seed→last MTP %d, want 117000", ended-seeded)
	}
	if rb.hikeSpan == "117000" {
		t.Fatal("admit span still starts at the seed")
	}
}
