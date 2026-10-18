package hike

import (
	"encoding/hex"
	"testing"
)

func TestBariOneLeafWireKAT(t *testing.T) {
	leaf := bytesRepeat(0x11, 32)
	got := EncodePathInBlock(leaf, 123456)
	const want = "011111111111111111111111111111111111111111111111111111111111111111000000000000000100b7959d170da4982cf2361b9b75c52b70c1b42596e6b1f40665068cfc82ede39544444444444444444444444444444444444444444444444444444444444444440000000255555555555555555555555555555555555555555555555555555555555555550001e240"
	if hex.EncodeToString(got) != want {
		t.Fatalf("kat %x", got)
	}
	height, ok := PathInclusionHeight(got)
	if !ok || height != 123456 {
		t.Fatalf("height %d %v", height, ok)
	}
	if _, err := PathLocatorsFor(got, bytesRepeat(0x99, 32)); err == nil {
		t.Fatal("other waypoint")
	}
	loc, err := PathLocatorsFor(got, leaf)
	if err != nil || loc.BlockHeight != 123456 || loc.Vout != 2 {
		t.Fatalf("locators %v %v", loc, err)
	}
}

func TestPathBodyShortBodyStaysUnread(t *testing.T) {
	if _, ok := PathInclusionHeight([]byte{0x01, 0x02, 0x03, 0x04}); ok {
		t.Fatal("short body")
	}
}
