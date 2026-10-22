package main

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strconv"

	"github.com/groundinglab/pcoc/sdk/go/v2"
)

// liveAfterHeaderHex is the 80-byte fee header for after_live. It is the first
// header, at the first nTime at or after 1233062596, whose SHA-256d meets the
// mainnet pow limit. It is a constant so candidate regeneration does not search.
const liveAfterHeaderHex = "0100000007fd9d1abc8d5b4c25c9f451e215c0c90b68b1a46917374e5274cbc6086327b051b29fc516c3c3aecac4a77c019cb535bde7a446cd0ae8e32afc61d094b7fd37c50a7f49ffff001d27167cb1"

func br1File() map[string]string {
	origin := "https://example.com"
	waypoint := sha256.Sum256([]byte("pcoc-pack-2.2/br1/waypoint"))
	tipHeader := pcoc.AssembleHeader(fill32(0x11), 1_700_000_000, pcoc.SynthNBits, 4)
	tipHash := as32(pcoc.HeaderHash(tipHeader[:]))
	beforeHeader := pcoc.AssembleHeader(fill32(0x33), 1_700_000_001, pcoc.SynthNBits, 5)
	beforeHash := as32(pcoc.HeaderHash(beforeHeader[:]))
	outsideHeader := pcoc.AssembleHeader(fill32(0x55), 1_700_000_002, pcoc.SynthNBits, 9)
	testTip := pcoc.Br1Tip{
		Hash:            tipHash,
		Previous:        fill32(0x11),
		Height:          100,
		Time:            1_700_000_000,
		Bits:            pcoc.SynthNBits,
		PeriodFirstTime: 0,
	}
	afterTest := headerWithPow(tipHash, 1_700_000_010, pcoc.SynthNBits)
	reversedTip := reversed32(tipHash)
	displayAfter := headerWithPow(reversedTip, 1_700_000_011, pcoc.SynthNBits)
	wrongBitsTest := pcoc.AssembleHeader(tipHash, 1_700_000_012, 0x1d00ffff, 1)

	liveHash := sha256.Sum256([]byte("pcoc-pack-2.2/br1/live-tip"))
	livePrev := sha256.Sum256([]byte("pcoc-pack-2.2/br1/live-prev"))
	liveTip := pcoc.Br1Tip{
		Hash:            liveHash,
		Previous:        livePrev,
		Height:          2015,
		Time:            1_233_061_996,
		Bits:            0x1d00ffff,
		PeriodFirstTime: 1_231_006_505,
	}
	liveRaw, err := hex.DecodeString(liveAfterHeaderHex)
	dieIf(err)
	if len(liveRaw) != 80 {
		die(fmt.Errorf("live header hex"))
	}
	var liveHeader [80]byte
	copy(liveHeader[:], liveRaw)
	if !bytes.Equal(liveHeader[4:36], liveHash[:]) {
		die(fmt.Errorf("live header prev is the tip hash"))
	}
	if pcoc.RequiredBits(liveTip, "live") != 0x1d00ffff {
		die(fmt.Errorf("historical retarget clamps to the pow limit"))
	}
	wrongBitsLive := headerWithPow(liveHash, 1_233_062_000, pcoc.SynthNBits)

	m := map[string]string{}
	br1Emit(m, "current", origin, origin, waypoint, tipHeader, testTip, "test")
	beforeTip := testTip
	beforeTip.Previous = beforeHash
	br1Emit(m, "before", origin, origin, waypoint, beforeHeader, beforeTip, "test")
	br1Emit(m, "after_test", origin, origin, waypoint, afterTest, testTip, "test")
	br1Emit(m, "after_live_copy", origin, origin, waypoint, afterTest, testTip, "live")
	br1Emit(m, "after_live", origin, origin, waypoint, liveHeader, liveTip, "live")
	br1Emit(m, "outside", origin, origin, waypoint, outsideHeader, testTip, "test")
	br1Emit(m, "wrong_origin", origin, "https://other.example", waypoint, tipHeader, testTip, "test")
	displayCurrent := testTip
	displayCurrent.Hash = reversedTip
	br1Emit(m, "display_current", origin, origin, waypoint, tipHeader, displayCurrent, "test")
	displayBefore := testTip
	displayBefore.Previous = reversed32(beforeHash)
	br1Emit(m, "display_before", origin, origin, waypoint, beforeHeader, displayBefore, "test")
	br1Emit(m, "display_after", origin, origin, waypoint, displayAfter, testTip, "test")
	br1Emit(m, "wrong_nbits_test", origin, origin, waypoint, wrongBitsTest, testTip, "test")
	br1Emit(m, "wrong_nbits_live", origin, origin, waypoint, wrongBitsLive, liveTip, "live")
	return m
}

func headerWithPow(previous [32]byte, time, nBits uint32) [80]byte {
	tip := pcoc.Br1Tip{
		Hash:   previous,
		Height: 100,
		Time:   time,
		Bits:   nBits,
	}
	plane := "live"
	if nBits == pcoc.SynthNBits {
		plane = "test"
	}
	for nonce := uint32(0); nonce < 4096; nonce++ {
		header := pcoc.AssembleHeader(previous, time, nBits, nonce)
		if pcoc.Br1Window(header[:], tip, plane) == "after" {
			return header
		}
	}
	die(fmt.Errorf("no proof-of-work header"))
	return [80]byte{}
}

func br1Emit(m map[string]string, id, checkOrigin, solveOrigin string, waypoint [32]byte, header [80]byte, tip pcoc.Br1Tip, plane string) {
	hash := pcoc.HeaderHash(header[:])
	challenge := pcoc.HashcashChallenge(solveOrigin, hash)
	nonce, _ := pcoc.HashcashSolve(challenge, waypoint[:], pcoc.HashcashDifficultyBits)
	nonceS := strconv.FormatUint(nonce, 10)
	headerHex := hex.EncodeToString(header[:])
	status := pcoc.Br1Status(string(waypoint[:]), nonceS, headerHex, checkOrigin, tip, plane)
	window := pcoc.Br1Window(header[:], tip, plane)
	wantStatus, wantWindow := br1Expect(id)
	if status != wantStatus || window != wantWindow {
		die(fmt.Errorf("br1 %s status %d window %s", id, status, window))
	}
	p := "br1." + id
	m[p+".plane"] = plane
	m[p+".origin"] = checkOrigin
	m[p+".solve_origin"] = solveOrigin
	m[p+".waypoint"] = hex.EncodeToString(waypoint[:])
	m[p+".nonce"] = nonceS
	m[p+".fee_header"] = headerHex
	m[p+".tip_hash"] = hex.EncodeToString(tip.Hash[:])
	m[p+".tip_previous"] = hex.EncodeToString(tip.Previous[:])
	m[p+".tip_height"] = strconv.FormatUint(uint64(tip.Height), 10)
	m[p+".tip_time"] = strconv.FormatUint(uint64(tip.Time), 10)
	m[p+".tip_bits"] = fmt.Sprintf("%08x", tip.Bits)
	m[p+".period_first_time"] = strconv.FormatUint(uint64(tip.PeriodFirstTime), 10)
	m[p+".window"] = window
	m[p+".status"] = strconv.FormatUint(uint64(status), 10)
}

func br1Expect(id string) (uint16, string) {
	status := uint16(402)
	switch id {
	case "current", "before", "after_test", "after_live", "after_live_copy":
		status = 204
	}
	window := "outside"
	switch id {
	case "current", "wrong_origin":
		window = "current"
	case "before":
		window = "before"
	case "after_test", "after_live", "after_live_copy":
		window = "after"
	}
	return status, window
}

func fill32(b byte) [32]byte {
	var out [32]byte
	for i := range out {
		out[i] = b
	}
	return out
}

func reversed32(b [32]byte) [32]byte {
	for i := 0; i < 16; i++ {
		b[i], b[31-i] = b[31-i], b[i]
	}
	return b
}

func as32(b []byte) [32]byte {
	var out [32]byte
	copy(out[:], b)
	return out
}
