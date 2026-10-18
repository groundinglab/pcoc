package hike

import (
	"bytes"
	"encoding/hex"
	"testing"
)

func TestInformativePins(t *testing.T) {
	if U64LE(SeedDomain) != 0x0034364445455350 {
		t.Fatalf("seed domain")
	}
	sealed := SHA256(append(append([]byte{}, SealDomain...), Le64(0)...))
	if hex.EncodeToString(sealed) != "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533" {
		t.Fatalf("seal %x", sealed)
	}
	if U64LE(sealed) != 0x76042423AFBD1648 {
		t.Fatalf("step")
	}
	pin := FillPin(append(bytes.Repeat([]byte{0x11}, 32), bytes.Repeat([]byte{0x22}, 32)...))
	if pin != 0x7DF0AFF879FB1171 {
		t.Fatalf("pin %x", pin)
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
