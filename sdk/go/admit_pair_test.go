package pcoc

import "testing"

// TestAdmitPairStartsAtFirstInterrupt locks Rule A4.10. The admit pair
// starts at I_0 (stages[1].beacon). Stage 0 checkpoints are not in
// hike_legs. hike_span is raw header nTime, the same integer as
// whole_hike_span, including a last-block spike.
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
	rb := &recordBody{headers: headers}
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
	// nTime(I_0) = 1_100_000. nTime(last) = 1_617_000. The spike is in the span.
	// A seed start is 617000 and is not the admit span.
	if rb.hikeSpan != "517000" {
		t.Fatalf("hike_span %s, want 517000", rb.hikeSpan)
	}
	if rb.wholeSpan != "517000" {
		t.Fatalf("whole_hike_span %s, want 517000", rb.wholeSpan)
	}
	if rb.hikeSpan != rb.wholeSpan {
		t.Fatal("hike_span and whole_hike_span disagreed")
	}
	if rb.hikeSpan == "617000" {
		t.Fatal("admit span still starts at the seed")
	}
}
