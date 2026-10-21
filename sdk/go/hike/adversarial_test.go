package hike

import (
	"bytes"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"reflect"
	"strings"
	"testing"
)

// Withdrawn constructions live in this file so verify keeps rejecting them.
// They are not the producer.

func TestTerrainKeepsNoSlotPrecompute(t *testing.T) {
	typ := reflect.TypeOf(Terrain{})
	for i := 0; i < typ.NumField(); i++ {
		name := strings.ToLower(typ.Field(i).Name)
		if strings.Contains(name, "xor") || strings.Contains(name, "cache") || strings.Contains(name, "digest") {
			t.Fatalf("terrain field %s", typ.Field(i).Name)
		}
	}
	if typ.NumField() != 2 {
		t.Fatalf("fields %d", typ.NumField())
	}
}

func TestVerifyRejectsWithdrawnFolds(t *testing.T) {
	pub := bytes.Repeat([]byte{0x11}, 32)
	beacon := bytes.Repeat([]byte{0x22}, 32)
	slot := NL4BaseSlot(pub, beacon, 0, 2048)
	state := uint64(0)
	got := FoldSlot(slot, state)
	if got != 0x76e07e211682940a {
		t.Fatalf("conforming fold %x", got)
	}
	if foldWithoutMixAdd(slot, state) == got || foldWithoutMixAdd(slot, state) != 0x6b98972f2e622525 {
		t.Fatalf("mix64 without MIX_ADD %x", foldWithoutMixAdd(slot, state))
	}
	if fatSlotFold(slot, state) == got || fatSlotFold(slot, state) != 0x4d57a0275efaf0c0 {
		t.Fatalf("v2.0 fold %x", fatSlotFold(slot, state))
	}
	if xorAccFold(slot, state) == got || xorAccFold(slot, state) != 0xa284caf306ff68a0 {
		t.Fatalf("xor accumulator %x", xorAccFold(slot, state))
	}
	// At S = 0, (S + w) equals w, so xor_w_fold matches. S = 1 separates them.
	const s1 = uint64(1)
	if FoldSlot(slot, s1) == xorWFold(slot, s1) || hexU64Fold(xorWFold(slot, s1)) != "8d9839f10c72f3fb" || hexU64Fold(FoldSlot(slot, s1)) != "bd0b175b96598842" {
		t.Fatalf("xor_w S1 conforming %x withdrawn %x", FoldSlot(slot, s1), xorWFold(slot, s1))
	}
	partial := foldFirstWords(slot, state, 1)
	if partial == got || partial != 0x0836dd6697660d54 {
		t.Fatalf("partial %x", partial)
	}
	word := NL4BaseWord(pub, beacon, 0, 0)
	if word == 0x6bd6efcd910ce11d || word == 0x880a81da1f152536 {
		t.Fatalf("emitted a withdrawn word %x", word)
	}
	if word != 0xc7582082a632c2c8 {
		t.Fatalf("nl4 %x", word)
	}
}

func TestVerifyRejectsFillPinTerrain(t *testing.T) {
	pub := bytes.Repeat([]byte{0x11}, 32)
	beacon := bytes.Repeat([]byte{0x22}, 32)
	profile := mustProfile("conformance-small", Kind, 32, 64, 256, 32)
	good := FillTerrain(pub, beacon, profile)
	bad := fillPinTerrain(append(pub, beacon...), profile)
	sumGood := sha256.Sum256(good.Blob)
	sumBad := sha256.Sum256(bad)
	if hex.EncodeToString(sumGood[:]) != "88c645f617547382474640c765a93d46b4369c064dc610511f76fccca8e2c81c" {
		t.Fatalf("conforming %x", sumGood)
	}
	if hex.EncodeToString(sumBad[:]) != "85fbfe9b82178c9cc9306a32ed6db6bc60d1a5ce245963279671147b6d7cf81b" {
		t.Fatalf("fill pin %x", sumBad)
	}
	if bytes.Equal(good.Blob, bad) {
		t.Fatal("terrains matched")
	}
	tips := smallTips()
	object := driveOn(pub, profile, tips, bad, func(previous []byte) uint64 {
		return fatLeg(bad, profile, previous)
	})
	rec, ok := Verify(object, profile, Index{}, drawStream(), false)
	if !ok {
		t.Fatal("decode")
	}
	var parsed struct {
		Checkpoints []struct {
			Legs []struct {
				Result string `json:"result"`
			} `json:"legs"`
		} `json:"checkpoints"`
	}
	if err := json.Unmarshal(bytes.TrimRight(rec.Marshal(), "\n"), &parsed); err != nil {
		t.Fatal(err)
	}
	if parsed.Checkpoints[0].Legs[0].Result != "mismatch" {
		t.Fatalf("fill pin checkpoint %s", parsed.Checkpoints[0].Legs[0].Result)
	}
}

func TestVerifyRejectsCachedLeg(t *testing.T) {
	profile := ConformanceSmallV2
	pub := bytes.Repeat([]byte{0x31}, 32)
	tips := smallTips()
	good := driveOn(pub, profile, tips, nil, nil)
	rec, ok := Verify(good, profile, Index{}, drawStream(), false)
	if !ok {
		t.Fatal("decode")
	}
	results := sixResults(t, rec)
	for i, token := range results {
		if token != "match" {
			t.Fatalf("conforming leg %d %s", i, token)
		}
	}
	terrain := FillTerrain(pub, tips[0].Natural, profile)
	cached := terrain.Leg(SeedValue(pub, tips[0].Natural))
	bad := driveOn(pub, profile, tips, terrain.Blob, func(previous []byte) uint64 {
		return cached
	})
	badRec, ok := Verify(bad, profile, Index{}, drawStream(), false)
	if !ok {
		t.Fatal("decode bad")
	}
	var draw struct {
		Indices []uint64 `json:"indices"`
		Results []string `json:"results"`
	}
	if err := json.Unmarshal(badRec.SixLeg, &draw); err != nil {
		t.Fatal(err)
	}
	mismatches := 0
	for i, idx := range draw.Indices {
		want := "mismatch"
		if idx == 0 {
			want = "match"
		} else {
			mismatches++
		}
		if draw.Results[i] != want {
			t.Fatalf("cached index %d %s", idx, draw.Results[i])
		}
	}
	if mismatches == 0 {
		t.Fatal("draw never left the cached leg")
	}
	var legs []struct {
		Result string `json:"result"`
	}
	if err := json.Unmarshal(badRec.Checkpoints[0].Legs, &legs); err != nil {
		t.Fatal(err)
	}
	if legs[0].Result != "match" {
		t.Fatalf("checkpoint bound the cached limb as %s", legs[0].Result)
	}
}

func TestSharedFillPinTerrainCollides(t *testing.T) {
	beacon := bytes.Repeat([]byte{0x22}, 32)
	pk1 := bytes.Repeat([]byte{0x11}, 32)
	pk2, err := hex.DecodeString("4b11111111111111112d1111111111111111111111111111fee8e38dee97a619")
	if err != nil {
		t.Fatal(err)
	}
	profile := mustProfile("conformance-small", Kind, 32, 64, 1024, 1)
	a := FillTerrain(pk1, beacon, profile)
	b := FillTerrain(pk2, beacon, profile)
	if bytes.Equal(a.Blob, b.Blob) {
		t.Fatal("NL4 terrains collided")
	}
	pinA := fillPinTerrain(append(pk1, beacon...), profile)
	pinB := fillPinTerrain(append(pk2, beacon...), profile)
	if !bytes.Equal(pinA, pinB) {
		t.Fatal("withdrawn fill pin did not collide")
	}
	sumA := sha256.Sum256(a.Blob)
	sumB := sha256.Sum256(b.Blob)
	sumP := sha256.Sum256(pinA)
	if hex.EncodeToString(sumP[:]) != "910e1451f65cac9c1f03e9ce565a3f194e8bb22391745118bc7ec63f27107fcd" {
		t.Fatalf("shared pin %x", sumP)
	}
	if hex.EncodeToString(sumA[:]) != "b30fa73f11634d09030c196997412b385743ce869eb5160821e15d47a42aabf5" {
		t.Fatalf("pk1 %x", sumA)
	}
	if hex.EncodeToString(sumB[:]) != "0698c63b0bb6b1ef7b9073407e2f7f2bdf1792868011c560c078b4ee52749067" {
		t.Fatalf("pk2 %x", sumB)
	}
}

func mix64WithoutAdd(x uint64) uint64 {
	y := x
	y = (y ^ (y >> 30)) * MixMul1
	y = (y ^ (y >> 27)) * MixMul2
	return y ^ (y >> 31)
}

func xorWFold(slot []byte, state uint64) uint64 {
	var acc uint64
	n := len(slot) / 8
	for w := 0; w < n; w++ {
		word := binary.LittleEndian.Uint64(slot[w*8 : w*8+8])
		acc += mix64(word ^ uint64(w) ^ state)
	}
	return acc
}

func hexU64Fold(v uint64) string {
	return hex.EncodeToString([]byte{
		byte(v >> 56), byte(v >> 48), byte(v >> 40), byte(v >> 32),
		byte(v >> 24), byte(v >> 16), byte(v >> 8), byte(v),
	})
}

func xorAccFold(slot []byte, state uint64) uint64 {
	var acc uint64
	n := len(slot) / 8
	for w := 0; w < n; w++ {
		word := binary.LittleEndian.Uint64(slot[w*8 : w*8+8])
		acc ^= mix64(word ^ uint64(w) ^ state)
	}
	return acc
}

func foldWithoutMixAdd(slot []byte, state uint64) uint64 {
	var acc uint64
	n := len(slot) / 8
	for w := 0; w < n; w++ {
		word := binary.LittleEndian.Uint64(slot[w*8 : w*8+8])
		acc += mix64WithoutAdd(word ^ uint64(w) ^ state)
	}
	return acc
}

func foldFirstWords(slot []byte, state uint64, n int) uint64 {
	var acc uint64
	for w := 0; w < n; w++ {
		word := binary.LittleEndian.Uint64(slot[w*8 : w*8+8])
		acc ^= mix64(word ^ uint64(w) ^ state)
	}
	return acc
}

func fatSlotFold(slot []byte, state uint64) uint64 {
	var folded uint64
	n := len(slot) / 8
	for w := 0; w < n; w++ {
		word := binary.LittleEndian.Uint64(slot[w*8 : w*8+8])
		folded ^= word ^ uint64(w)
	}
	return state ^ folded
}

func fillPin(preimage []byte) uint64 {
	state := U64LE([]byte{'P', 'S', 'E', 'E', 'D', '6', '4', 0})
	for i := 0; i < 8; i++ {
		lane := U64LE(preimage[8*i : 8*i+8])
		state = mix64(state ^ lane ^ (uint64(i) * MixAdd))
	}
	return state
}

func fillPinTerrain(preimage []byte, profile Profile) []byte {
	pin := fillPin(preimage)
	blob := make([]byte, profile.TerrainBytes)
	words := make([]uint64, profile.WordsPerSlot)
	for i := 0; i < profile.NSlots; i++ {
		base := pin ^ (uint64(i) << 32) ^ WordXOR
		for j := 0; j < profile.WordsPerSlot; j++ {
			words[j] = mix64(base ^ uint64(j))
		}
		if i > 0 {
			parents := [3]int{i - 1, bitrevParent(i), mulParent(i)}
			for k := 1; k <= 3; k++ {
				parent := parents[k-1]
				xk := uint64(k) * ParentMul
				for j := 0; j < profile.WordsPerSlot; j++ {
					pw := loadWord(blob, profile, parent, j)
					rot := uint((j + k) % 64)
					words[j] = mix64((words[j] ^ rotl(pw, rot)) ^ xk)
				}
			}
			slotMix := uint64(i) * SlotMul
			for j := 0; j < profile.WordsPerSlot; j++ {
				words[j] = mix64(words[j] ^ slotMix)
			}
		}
		writeSlot(blob, profile, i, words)
	}
	return blob
}

func fatLeg(blob []byte, profile Profile, previous []byte) uint64 {
	state := U64LE(previous)
	nslots := uint64(profile.NSlots)
	for step := 0; step < profile.StepsPerLeg; step++ {
		slot := int(state % nslots)
		start := slot * profile.SlotBytes
		folded := fatSlotFold(blob[start:start+profile.SlotBytes], state)
		sum := sealSum(folded)
		state = binary.LittleEndian.Uint64(sum[:8])
	}
	return state
}

func smallTips() []Tip {
	tips := make([]Tip, Stages+1)
	for i := range tips {
		tips[i] = Tip{Natural: bytes.Repeat([]byte{byte(i + 1)}, 32), Height: uint64(1000 + i)}
	}
	return tips
}

func drawStream() []byte {
	out := make([]byte, 48)
	for i := uint64(0); i < 6; i++ {
		binary.LittleEndian.PutUint64(out[i*8:], i)
	}
	return out
}

// driveOn follows section 13.2. A nil blob uses the NL4 terrain. A nil leg
// function uses the §11.1 leg. Tests pass a withdrawn blob or leg function.
func driveOn(publicKey []byte, profile Profile, tips []Tip, blob []byte, legFn func(previous []byte) uint64) []byte {
	seedBeacon := append([]byte(nil), tips[0].Natural...)
	terrain := FillTerrain(publicKey, seedBeacon, profile)
	if blob == nil {
		blob = terrain.Blob
	}
	if legFn == nil {
		legFn = func(previous []byte) uint64 { return terrain.Leg(previous) }
	}
	waypoint := SeedValue(publicKey, seedBeacon)
	beacon := seedBeacon
	beaconHeight := tips[0].Height
	cursor := 1
	stages := make([]builtStage, Stages)
	var last []byte
	var lastHeight uint64
	for g := 0; g < Stages; g++ {
		limb := append([]byte(nil), waypoint...)
		previous := limb
		var checkpoints [][]byte
		var results [][]byte
		var interrupt Tip
		for {
			leg := legFn(previous)
			digest := CheckpointDigest(blob, previous, limb, leg, profile)
			checkpoints = append(checkpoints, digest)
			results = append(results, Le64(leg))
			previous = digest
			tip := tips[cursor]
			cursor++
			if !bytesEqual(tip.Natural, beacon) && tip.Height > beaconHeight {
				interrupt = tip
				break
			}
		}
		stages[g] = builtStage{
			Beacon: append([]byte(nil), beacon...), BeaconHeight: beaconHeight,
			LegCount: uint64(len(checkpoints)), Waypoint: limb,
			Checkpoints: checkpoints, LegResults: results, HasPath: g > 0,
		}
		if g < Stages-1 {
			waypoint = WaypointDigest(blob, interrupt.Natural, stages[g].Waypoint, stages[g].Checkpoints, profile)
			beacon = append([]byte(nil), interrupt.Natural...)
			beaconHeight = interrupt.Height
		} else {
			last = append([]byte(nil), interrupt.Natural...)
			lastHeight = interrupt.Height
		}
	}
	return encodeBuilt(profile, publicKey, seedBeacon, stages, last, lastHeight)
}

func sixResults(t *testing.T, rec Record) []string {
	t.Helper()
	var draw struct {
		Results []string `json:"results"`
	}
	if err := json.Unmarshal(rec.SixLeg, &draw); err != nil {
		t.Fatalf("six_leg %s", rec.SixLeg)
	}
	if len(draw.Results) != 6 {
		t.Fatalf("results %v", draw.Results)
	}
	return draw.Results
}
