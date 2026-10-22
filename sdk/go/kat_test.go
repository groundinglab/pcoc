package pcoc

import (
	"crypto/sha256"
	"encoding/hex"
	"testing"

	"github.com/groundinglab/pcoc/sdk/go/v2/internal/fold"
)

func TestSection74(t *testing.T) {
	le1 := Le64(1)
	if hex.EncodeToString(le1) != "0100000000000000" {
		t.Fatalf("le64(1) %x", le1)
	}
	le := Le64(0x0102030405060708)
	if hex.EncodeToString(le) != "0807060504030201" {
		t.Fatalf("le64 %x", le)
	}
	if SealState(0) != 0x76042423afbd1648 {
		t.Fatalf("seal 0 %x", SealState(0))
	}
	pk := bytesRepeat(0x11, 32)
	bc := bytesRepeat(0x22, 32)
	d := Seed(pk, bc)
	if hex.EncodeToString(d) != "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c" {
		t.Fatalf("seed %x", d)
	}
	lanes := Lanes(d)
	want := [4]uint64{0x545dfe297dc78951, 0x27858669c45e046a, 0x9cdac73ac1a5fe85, 0x7c81df6efbf55f11}
	if lanes != want {
		t.Fatalf("lanes %x", lanes)
	}
	if nl4Base(lanes, 0, 0) != 0xc7582082a632c2c8 {
		t.Fatalf("nl4 %x", nl4Base(lanes, 0, 0))
	}
}

func bytesRepeat(b byte, n int) []byte {
	out := make([]byte, n)
	for i := range out {
		out[i] = b
	}
	return out
}

func TestFoldStep0(t *testing.T) {
	pk, _ := hex.DecodeString("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")
	// seed_beacon = SHA-256("pcoc-pack-2.2/beacon/seed") is computed inside.
	// Use the published D's opening state directly by filling from the beacon.
	bc := sha256ASCII("pcoc-pack-2.2/beacon/seed")
	p := Profile{Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 100}
	tr, err := FillTerrain(p, pk, bc)
	if err != nil {
		t.Fatal(err)
	}
	s := uint64(0xa34908fca5e2f21c)
	i := int(s % uint64(tr.p.NSlots()))
	if i != 540 {
		t.Fatalf("slot %d", i)
	}
	acc := foldAcc(tr, s)
	if acc != 0xcc9e04ee133f53ac {
		t.Fatalf("acc %x", acc)
	}
	next := SealState(acc)
	if next != 0xbe5f7be316f66502 {
		t.Fatalf("next %x", next)
	}
	got := Step(tr, s)
	if got != next {
		t.Fatalf("step %x", got)
	}
}

func foldAcc(t *Terrain, s uint64) uint64 {
	i := int(s % uint64(t.p.NSlots()))
	return fold.Scalar(t.Slot(i), s)
}

func sha256ASCII(s string) []byte {
	sum := sha256.Sum256([]byte(s))
	return sum[:]
}
