package hike

import (
	"bytes"
	"encoding/hex"
	"testing"
)

func TestInformativePins(t *testing.T) {
	sealed := SHA256(append(append([]byte{}, SealDomain...), Le64(0)...))
	if hex.EncodeToString(sealed) != "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533" {
		t.Fatalf("seal %x", sealed)
	}
	if U64LE(sealed) != 0x76042423AFBD1648 {
		t.Fatalf("step")
	}
	// §7.4. 32 bytes of 0x11, then 32 bytes of 0x22. No domain label.
	preimage := append(bytes.Repeat([]byte{0x11}, 32), bytes.Repeat([]byte{0x22}, 32)...)
	digest, lanes := SeedLanes(preimage[:32], preimage[32:])
	if hex.EncodeToString(digest) != "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c" {
		t.Fatalf("seed %x", digest)
	}
	wantLanes := []uint64{0x545dfe297dc78951, 0x27858669c45e046a, 0x9cdac73ac1a5fe85, 0x7c81df6efbf55f11}
	for i, want := range wantLanes {
		if lanes[i] != want {
			t.Fatalf("lane %d %x", i, lanes[i])
		}
	}
	word := NL4BaseWord(preimage[:32], preimage[32:], 0, 0)
	if word != 0xc7582082a632c2c8 {
		t.Fatalf("nl4 %x", word)
	}
}

func TestDrawLimit(t *testing.T) {
	stream, _ := hex.DecodeString("faffffffffffffff000000000000000001000000000000000200000000000000030000000000000004000000000000000500000000000000")
	idx, token := DrawSix(stream, 10)
	if token != "" {
		t.Fatal(token)
	}
	want := []uint64{0, 1, 2, 3, 4, 5}
	if len(idx) != len(want) {
		t.Fatal(idx)
	}
	for i := range want {
		if idx[i] != want[i] {
			t.Fatalf("got %v", idx)
		}
	}
}

func TestIndexRead(t *testing.T) {
	body := `{"network":"main","header":{"hash":"2222222222222222222222222222222222222222222222222222222222222222","height":1000,"time":1600000000}}`
	got := ReadIndexDocument(body)
	if got == nil || got.Height != 1000 || got.Time == nil || *got.Time != 1600000000 {
		t.Fatalf("%+v", got)
	}
	if hex.EncodeToString(got.Natural) != "2222222222222222222222222222222222222222222222222222222222222222" {
		t.Fatalf("natural %x", got.Natural)
	}
	if ReadIndexDocument("\uFEFF"+body) != nil {
		t.Fatal("bom")
	}
	dup := `{"header":{"hash":"aa","height":1},"header":{"hash":"bb","height":2}}`
	if ReadIndexDocument(dup) != nil {
		t.Fatal("duplicate")
	}
	if ReadIndexDocument(`{"header":{"hash":"abababababababababababababababababababababababababababababababab","height":1.5,"time":1}}`) != nil {
		t.Fatal("fraction")
	}
}
