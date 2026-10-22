package pcoc

import (
	"encoding/hex"
	"strconv"
	"testing"
)

func tipFrom(header [80]byte, height, periodFirst uint32) Br1Tip {
	var hash, previous [32]byte
	copy(hash[:], HeaderHash(header[:]))
	copy(previous[:], header[4:36])
	return Br1Tip{
		Hash:            hash,
		Previous:        previous,
		Height:          height,
		Time:            headerTime(header),
		Bits:            bitsOf(header[:]),
		PeriodFirstTime: periodFirst,
	}
}

func headerTime(header [80]byte) uint32 {
	return uint32(header[68]) | uint32(header[69])<<8 | uint32(header[70])<<16 | uint32(header[71])<<24
}

func TestBR1Retarget(t *testing.T) {
	target, neg, over := setCompact(MainnetPowLimitBits)
	if neg || over || target.getCompact() != MainnetPowLimitBits {
		t.Fatalf("pow limit compact %08x neg %v over %v", target.getCompact(), neg, over)
	}
	synth, _, _ := setCompact(SynthNBits)
	if synth.getCompact() != SynthNBits {
		t.Fatalf("synth compact %08x", synth.getCompact())
	}
	// Block 2015 time 1233061996, block 0 time 1231006505, both nBits 0x1d00ffff.
	// The unclamped target is easier than the pow limit, so the next compact stays
	// 0x1d00ffff. A timespan of one quarter makes the target four times harder.
	if got := NextBits(2015, 0x1d00ffff, 1233061996, 1231006505); got != 0x1d00ffff {
		t.Fatalf("historical retarget %08x", got)
	}
	if got := NextBits(2015, 0x1d00ffff, 1231006505+302400, 1231006505); got != 0x1c3fffc0 {
		t.Fatalf("quarter retarget %08x", got)
	}
	if got := NextBits(100, 0x207fffff, 1, 0); got != 0x207fffff {
		t.Fatalf("between retargets %08x", got)
	}
}

func TestBR1NegativeOverflowCompact(t *testing.T) {
	neg := AssembleHeader([32]byte{}, 0, 0x1d80ffff, 0)
	if powMeets(neg[:]) {
		t.Fatal("negative compact met proof of work")
	}
	over := AssembleHeader([32]byte{}, 0, 0x2300ffff, 0)
	if powMeets(over[:]) {
		t.Fatal("overflow compact met proof of work")
	}
	zero := AssembleHeader([32]byte{}, 0, 0x01000000, 0)
	if powMeets(zero[:]) {
		t.Fatal("zero target met proof of work")
	}
}

func TestBR1SynthAfter(t *testing.T) {
	var prev [32]byte
	for i := range prev {
		prev[i] = 0x11
	}
	tipHeader := AssembleHeader(prev, 1_700_000_000, SynthNBits, 4)
	tip := tipFrom(tipHeader, 100, 0)
	if Br1Window(tipHeader[:], tip, "test") != "current" {
		t.Fatal("tip header is not current")
	}
	var after []byte
	for nonce := uint32(0); nonce < 64; nonce++ {
		header := AssembleHeader(tip.Hash, 1_700_000_010, SynthNBits, nonce)
		if Br1Window(header[:], tip, "test") == "after" {
			after = append([]byte(nil), header[:]...)
			break
		}
	}
	if after == nil {
		t.Fatal("synth target")
	}
	if Br1Window(after, tip, "live") != "after" {
		t.Fatal("a non-retarget live tip with synth nBits keeps that compact")
	}
	wrong := AssembleHeader(tip.Hash, 1_700_000_010, 0x1d00ffff, 1)
	if Br1Window(wrong[:], tip, "test") != "outside" {
		t.Fatal("wrong test-plane nBits")
	}
}

func TestBR1DisplayReversal(t *testing.T) {
	var prev [32]byte
	for i := range prev {
		prev[i] = 0x10
	}
	tipHeader := AssembleHeader(prev, 1_700_000_000, SynthNBits, 4)
	tip := tipFrom(tipHeader, 50, 0)
	origin := "https://example.com"
	waypoint := string(bytesRepeat(0x22, 32))
	nonce, _ := HashcashSolve(HashcashChallenge(origin, HeaderHash(tipHeader[:])), []byte(waypoint), HashcashDifficultyBits)
	headerHex := hex.EncodeToString(tipHeader[:])
	nonceS := strconv.FormatUint(nonce, 10)
	if Br1Status(waypoint, nonceS, headerHex, origin, tip, "test") != 204 {
		t.Fatal("natural-order current post")
	}
	reversed := tip
	for i := 0; i < 16; i++ {
		reversed.Hash[i], reversed.Hash[31-i] = reversed.Hash[31-i], reversed.Hash[i]
	}
	if Br1Window(tipHeader[:], reversed, "test") != "outside" {
		t.Fatal("display-order tip hash")
	}
	if Br1Status(waypoint, nonceS, headerHex, origin, reversed, "test") != 402 {
		t.Fatal("display-order reversal")
	}

	var beforePrev [32]byte
	for i := range beforePrev {
		beforePrev[i] = 0x12
	}
	before := AssembleHeader(beforePrev, 1_700_000_000, SynthNBits, 5)
	tipBefore := tip
	copy(tipBefore.Previous[:], HeaderHash(before[:]))
	if Br1Window(before[:], tipBefore, "test") != "before" {
		t.Fatal("before")
	}
	for i := 0; i < 16; i++ {
		tipBefore.Previous[i], tipBefore.Previous[31-i] = tipBefore.Previous[31-i], tipBefore.Previous[i]
	}
	if Br1Window(before[:], tipBefore, "test") != "outside" {
		t.Fatal("display-order previous")
	}
	var flippedAfter []byte
	for nonce := uint32(0); nonce < 64; nonce++ {
		header := AssembleHeader(tip.Hash, 1_700_000_010, SynthNBits, nonce)
		if Br1Window(header[:], tip, "test") == "after" {
			flippedAfter = append([]byte(nil), header[:]...)
			break
		}
	}
	if flippedAfter == nil {
		t.Fatal("synth after")
	}
	for i := 0; i < 16; i++ {
		flippedAfter[4+i], flippedAfter[35-i] = flippedAfter[35-i], flippedAfter[4+i]
	}
	if Br1Window(flippedAfter, tip, "test") != "outside" {
		t.Fatal("display-order previous field")
	}
}

func TestBR1PaidCurrent(t *testing.T) {
	var prev [32]byte
	for i := range prev {
		prev[i] = 0x11
	}
	tipHeader := AssembleHeader(prev, 1_700_000_000, SynthNBits, 4)
	tip := tipFrom(tipHeader, 100, 0)
	if headerTime(tipHeader) != tip.Time {
		t.Fatalf("tip time %d", tip.Time)
	}
	origin := "https://example.com"
	waypoint := string(bytesRepeat(0x22, 32))
	hash := HeaderHash(tipHeader[:])
	nonce, _ := HashcashSolve(HashcashChallenge(origin, hash), []byte(waypoint), HashcashDifficultyBits)
	headerHex := hex.EncodeToString(tipHeader[:])
	nonceS := strconv.FormatUint(nonce, 10)
	if Br1Status(waypoint, nonceS, headerHex, origin, tip, "test") != 204 {
		t.Fatal("paid current")
	}
	if Br1Status(waypoint, nonceS, headerHex, "https://other.example", tip, "test") != 402 {
		t.Fatal("wrong origin")
	}
	if Br1Status(waypoint, "01", headerHex, origin, tip, "test") != 402 {
		t.Fatal("leading zero")
	}
	if Br1Status(waypoint, nonceS, hexUpper(headerHex), origin, tip, "test") != 402 {
		t.Fatal("upper hex")
	}
	if Br1Status(string(make([]byte, 31)), nonceS, headerHex, origin, tip, "test") != 402 {
		t.Fatal("short body")
	}
	if n, ok := ParseNonce("0"); !ok || n != 0 {
		t.Fatal("zero nonce")
	}
	if _, ok := ParseNonce("18446744073709551616"); ok {
		t.Fatal("overflow nonce")
	}
	if _, ok := ParseFeeHeader(headerHex[:159]); ok {
		t.Fatal("short header")
	}
}

func hexUpper(s string) string {
	b := []byte(s)
	for i, c := range b {
		if c >= 'a' && c <= 'f' {
			b[i] = c - 'a' + 'A'
		}
	}
	return string(b)
}
