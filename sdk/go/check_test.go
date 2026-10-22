package pcoc

import (
	"crypto/sha256"
	"encoding/hex"
	"testing"
)

// SPEC §7.4 checks. These pin mix64, le64, the seal, and the NL4 base word
// against the sentences in that section.

func TestLe64(t *testing.T) {
	if got := hex.EncodeToString(Le64(1)); got != "0100000000000000" {
		t.Fatalf("le64(1) = %s", got)
	}
	if got := hex.EncodeToString(Le64(0x0102030405060708)); got != "0807060504030201" {
		t.Fatalf("le64(0x0102030405060708) = %s", got)
	}
}

func TestSHA256ABC(t *testing.T) {
	sum := sha256.Sum256([]byte("abc"))
	got := hex.EncodeToString(sum[:])
	want := "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
	if got != want {
		t.Fatalf("sha256(abc) = %s", got)
	}
}

func TestSealZero(t *testing.T) {
	var buf [20]byte
	copy(buf[0:12], SealDomain)
	sum := sha256.Sum256(buf[:])
	got := hex.EncodeToString(sum[:])
	want := "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533"
	if got != want {
		t.Fatalf("seal(0) = %s", got)
	}
	s := u64le(sum[0:8])
	if s != 0x76042423afbd1648 {
		t.Fatalf("S' = %x", s)
	}
}

func TestSeedLanesAndNL4(t *testing.T) {
	pre := make([]byte, 64)
	for i := 0; i < 32; i++ {
		pre[i] = 0x11
		pre[32+i] = 0x22
	}
	sum := sha256.Sum256(pre)
	got := hex.EncodeToString(sum[:])
	want := "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c"
	if got != want {
		t.Fatalf("seed = %s", got)
	}
	p := Lanes(sum[:])
	wantLanes := [4]uint64{0x545dfe297dc78951, 0x27858669c45e046a, 0x9cdac73ac1a5fe85, 0x7c81df6efbf55f11}
	for k := 0; k < 4; k++ {
		if p[k] != wantLanes[k] {
			t.Fatalf("P[%d] = %x", k, p[k])
		}
	}
	// NL4 base word at i = 0, j = 0.
	c := (uint64(0) << 32) ^ uint64(0) ^ WordXor
	x := Mix64(p[0] ^ c)
	x = Mix64(x ^ Rotl(p[1], 17))
	x = Mix64(x ^ Rotl(p[2], 31))
	x = Mix64(x ^ Rotl(p[3], 47))
	if x != 0xc7582082a632c2c8 {
		t.Fatalf("NL4(0,0) = %x", x)
	}
}

func TestVersion(t *testing.T) {
	if Version != "2.2.0" {
		t.Fatalf("version %s", Version)
	}
}
