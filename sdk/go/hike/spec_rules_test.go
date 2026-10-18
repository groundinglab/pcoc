package hike

import (
	"crypto/sha256"
	"strings"
	"testing"
)

// Section 14.1. A short stages array has no leg_count on the missing stages.
// That condition is violated, and the string sits in the section 14 table order.
func TestAbsentStagesIncludeLegCount(t *testing.T) {
	object := QuietV2()
	object.Stages = object.Stages[:2]
	for i := range object.Stages {
		object.Stages[i].LegCount = 1
		object.Stages[i].Checkpoints = [][32]byte{{}}
		object.Stages[i].LegResults = [][8]byte{{}}
	}
	got := PackRecord(object.Encode(), Production, Index{}, nil, 1, nil)
	const want = `"shape_item":["W_17 path","stages length","leg_count","last_interrupt","header height order","header hash differ"]`
	if !strings.Contains(got, want) {
		t.Fatalf("shape_item %s", got)
	}
}

func TestFewerThanSixLegsWritesEmptyRuns(t *testing.T) {
	six, runs := sixLeg(nil, cborVal{}, []int{1, 1, 1}, nil, make([]byte, 64))
	if string(six) != `"not defined"` || runs != nil {
		t.Fatalf("six %s runs %s", six, runs)
	}
	object := QuietV2()
	object.Stages = object.Stages[:3]
	height := uint64(5)
	object.LastHeight = &height
	got := PackRecord(object.encode(ConformanceSmallV2), ConformanceSmallV2, Index{}, nil, 1, make([]byte, 64))
	if !strings.Contains(got, `"six_leg":"not defined"`) || !strings.Contains(got, `"six_leg_runs":[]`) {
		t.Fatal("six_leg_runs")
	}
	if strings.Contains(got, `"six_leg_runs":"not defined"`) || strings.Contains(got, `"six_leg_runs":"not read"`) {
		t.Fatal("string runs")
	}
	if !strings.Contains(got, `"g":1,"result":"match"`) && !strings.Contains(got, `"g":1,"result":"mismatch"`) {
		t.Fatal("present waypoint")
	}
	if !strings.Contains(got, `"g":3,"result":"not defined"`) || strings.Contains(got, `"g":3,"result":"mismatch"`) {
		t.Fatal("absent successor")
	}
}

func TestUnreadHeaderIsNotABookendMiss(t *testing.T) {
	headers := make([]Header, 4)
	for i := range headers {
		headers[i] = Header{
			StoredHash:      jstr("aa"),
			StoredHeight:    jnum(int64(i + 1)),
			IndexComparison: "not read",
			T:               jstr("not read"),
		}
	}
	facts := []pathMeasure{
		{plant: "not read", plantRead: false, anchor: "not anchored"},
		{plant: "not read", plantRead: false, anchor: "not anchored"},
	}
	book := v2Book(2, facts, []string{"not anchored", "not anchored"}, headers, nil, nil, Index{})
	if string(book.a) != `"not defined"` {
		t.Fatalf("bookends %s", book.a)
	}
	facts[0].plantRead = true
	book = v2Book(2, facts, []string{"not anchored", "not anchored"}, headers, nil, nil, Index{})
	if string(book.a) != `"not read"` {
		t.Fatalf("issued plant %s", book.a)
	}
}

func TestVerifyRecordSHA256OmitsTheTrailingNewline(t *testing.T) {
	file := "{\"b\":1,\"a\":2}\n"
	got, err := VerifyRecordSHA256(file)
	if err != nil {
		t.Fatal(err)
	}
	if got == sha256.Sum256([]byte(file)) {
		t.Fatal("file bytes")
	}
	const want = "d3626ac30a87e6f7a6428233b3c68299976865fa5508e4267c5415c76af7a772"
	if hexEncode(got[:]) != want {
		t.Fatalf("%x", got)
	}
	header := jraw(Header{
		StoredHash:      jstr("ab"),
		StoredHeight:    jnum(1),
		IndexComparison: "match",
		T:               jnum(2),
	})
	if string(header) != `{"stored_hash":"ab","stored_height":1,"index_comparison":"match","T":2}` {
		t.Fatal(string(header))
	}
}

func TestNotReadDrawStaysAString(t *testing.T) {
	six, runs := sixLeg(nil, cborVal{}, []int{1, 1, 1, 1, 1, 1}, nil, []byte{1})
	if string(six) != `"not read"` || string(runs) != `"not read"` {
		t.Fatalf("six %s runs %s", six, runs)
	}
}
