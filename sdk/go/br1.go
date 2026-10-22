package pcoc

import (
	"bytes"
	"encoding/binary"
	"math/bits"
	"strconv"
)

// SynthNBits is the compact nBits every synth header carries (SPEC §9).
const SynthNBits uint32 = 0x207fffff

// MainnetPowLimitBits is the Bitcoin mainnet proof-of-work limit, compact form.
const MainnetPowLimitBits uint32 = 0x1d00ffff

const (
	retargetInterval uint32 = 2016
	targetTimespan   int64  = 14 * 24 * 60 * 60
)

// Br1Tip is Bari's tip in the form Rule BR1 compares (SPEC §13.6).
// Hash and Previous are natural order. Hash is SHA-256(SHA-256(tip header)).
// Previous is the 32 bytes at offset 4 of the tip header. Bits and Time are
// the tip header's nBits and nTime. PeriodFirstTime is the nTime of the block
// at height Height-2015, and is read only when (Height+1) is a multiple of 2016.
type Br1Tip struct {
	Hash            [32]byte
	Previous        [32]byte
	Height          uint32
	Time            uint32
	Bits            uint32
	PeriodFirstTime uint32
}

// ParseNonce is SPEC §13.1. Shortest decimal, no sign, no leading zeros.
// Zero is "0". ok is false on any other spelling, including a value past u64.
func ParseNonce(raw string) (uint64, bool) {
	if raw == "" {
		return 0, false
	}
	for i := 0; i < len(raw); i++ {
		if raw[i] < '0' || raw[i] > '9' {
			return 0, false
		}
	}
	if len(raw) > 1 && raw[0] == '0' {
		return 0, false
	}
	n, err := strconv.ParseUint(raw, 10, 64)
	if err != nil {
		return 0, false
	}
	return n, true
}

// ParseFeeHeader is SPEC §13.1. Exactly 160 lowercase hex digits, the 80
// fee-header bytes, byte 0 first. Upper case is not this spelling.
func ParseFeeHeader(raw string) ([]byte, bool) {
	if len(raw) != 160 {
		return nil, false
	}
	out := make([]byte, 80)
	for i := 0; i < 80; i++ {
		hi, ok1 := hexNibble(raw[2*i])
		lo, ok2 := hexNibble(raw[2*i+1])
		if !ok1 || !ok2 {
			return nil, false
		}
		out[i] = hi<<4 | lo
	}
	return out, true
}

func hexNibble(c byte) (byte, bool) {
	switch {
	case c >= '0' && c <= '9':
		return c - '0', true
	case c >= 'a' && c <= 'f':
		return c - 'a' + 10, true
	default:
		return 0, false
	}
}

// Br1Window is SPEC §13.6 Rule BR1 step 2, before the Hashcash check.
// It returns "current", "before", "after", or "outside". A short or long
// header is "outside". Comparisons are natural order on both sides (SPEC §9).
// plane "test" requires nBits 0x207fffff. plane "live" requires the Bitcoin
// mainnet target for the block after the tip.
func Br1Window(header []byte, tip Br1Tip, plane string) string {
	if len(header) != 80 {
		return "outside"
	}
	hash := HeaderHash(header)
	if bytes.Equal(hash, tip.Hash[:]) {
		return "current"
	}
	if bytes.Equal(hash, tip.Previous[:]) {
		return "before"
	}
	if bytes.Equal(header[4:36], tip.Hash[:]) && bitsOf(header) == RequiredBits(tip, plane) && powMeets(header) {
		return "after"
	}
	return "outside"
}

// RequiredBits is the compact target an "after" header must carry (SPEC §9, §13.6).
// An unknown plane has no target; 0 encodes an empty target, which fails proof of work.
func RequiredBits(tip Br1Tip, plane string) uint32 {
	switch plane {
	case "test":
		return SynthNBits
	case "live":
		return NextBits(tip.Height, tip.Bits, tip.Time, tip.PeriodFirstTime)
	default:
		return 0
	}
}

// NextBits is Bitcoin mainnet GetNextWorkRequired for the block after height (SPEC §9).
// Between retargets the compact target is tipBits. On a 2016-block boundary the
// timespan is tipTime-periodFirst, clamped to a factor of four around two weeks,
// and the new target is clamped to the mainnet proof-of-work limit.
// The multiply and divide are truncating 256-bit arithmetic.
func NextBits(height, tipBits, tipTime, periodFirst uint32) uint32 {
	if (height+1)%retargetInterval != 0 {
		return tipBits
	}
	span := int64(tipTime) - int64(periodFirst)
	minSpan := targetTimespan / 4
	maxSpan := targetTimespan * 4
	if span < minSpan {
		span = minSpan
	}
	if span > maxSpan {
		span = maxSpan
	}
	target, _, _ := setCompact(tipBits)
	target = target.mulU64(uint64(span)).divU64(uint64(targetTimespan))
	limit, _, _ := setCompact(MainnetPowLimitBits)
	if target.cmp(limit) > 0 {
		target = limit
	}
	return target.getCompact()
}

// Br1Status is SPEC §13.6 Rule BR1. 204 when the post is a paid waypoint, 402 otherwise.
// body is the POST body. nonceDecimal is the X-Bari-Hashcash-Nonce spelling.
// feeHeaderHex is the X-Bari-Hashcash-Header spelling. bariOrigin is Bari's own
// public origin, already serialized as SPEC §13.1 states. plane is "test" or "live".
func Br1Status(body, nonceDecimal, feeHeaderHex, bariOrigin string, tip Br1Tip, plane string) uint16 {
	header, ok := ParseFeeHeader(feeHeaderHex)
	if !ok {
		return 402
	}
	nonce, ok := ParseNonce(nonceDecimal)
	if !ok {
		return 402
	}
	if len(body) != 32 {
		return 402
	}
	if Br1Window(header, tip, plane) == "outside" {
		return 402
	}
	hash := HeaderHash(header)
	challenge := HashcashChallenge(bariOrigin, hash)
	digest := HashcashDigest(challenge, []byte(body), nonce)
	if LeadingZeroBits(digest) < HashcashDifficultyBits {
		return 402
	}
	return 204
}

// AssembleHeader builds an 80-byte header. Version is 1. previous is natural
// order at offset 4. nTime is at 68, nBits at 72, nonce at 76.
// Multi-byte fields are little-endian. The merkle root is zero.
func AssembleHeader(previous [32]byte, nTime, bits, nonce uint32) [80]byte {
	var header [80]byte
	header[0] = 1
	copy(header[4:36], previous[:])
	binary.LittleEndian.PutUint32(header[68:72], nTime)
	binary.LittleEndian.PutUint32(header[72:76], bits)
	binary.LittleEndian.PutUint32(header[76:80], nonce)
	return header
}

func bitsOf(header []byte) uint32 {
	return binary.LittleEndian.Uint32(header[72:76])
}

// powMeets is SPEC §9. SHA-256d as a little-endian integer is at most the
// target nBits encodes. A negative compact or an overflow compact does not
// encode a usable target.
func powMeets(header []byte) bool {
	nBits := bitsOf(header)
	target, negative, overflow := setCompact(nBits)
	if negative || overflow || target.isZero() {
		return false
	}
	hash := HeaderHash(header)
	var raw [32]byte
	copy(raw[:], hash)
	got := u256FromBytes(raw)
	return got.cmp(target) <= 0
}

// setCompact is Bitcoin arith_uint256::SetCompact. The booleans are (negative, overflow).
func setCompact(nBits uint32) (u256, bool, bool) {
	size := int(nBits >> 24)
	word := nBits & 0x007fffff
	var value u256
	switch {
	case word == 0:
	case size <= 3:
		value[0] = uint64(word) >> (8 * uint(3-size))
	default:
		value[0] = uint64(word)
		value = value.shlBytes(uint32(size - 3))
	}
	negative := word != 0 && nBits&0x00800000 != 0
	overflow := word != 0 && (size > 34 || (word > 0xff && size > 33) || (word > 0xffff && size > 32))
	return value, negative, overflow
}

// u256 is a 256-bit little-endian integer. Limb 0 is the least significant.
type u256 [4]uint64

func u256FromBytes(b [32]byte) u256 {
	var u u256
	for i := 0; i < 4; i++ {
		u[i] = binary.LittleEndian.Uint64(b[i*8:])
	}
	return u
}

func (u u256) isZero() bool {
	return u == (u256{})
}

func (u u256) cmp(other u256) int {
	for i := 3; i >= 0; i-- {
		if u[i] < other[i] {
			return -1
		}
		if u[i] > other[i] {
			return 1
		}
	}
	return 0
}

func (u u256) bitLen() uint32 {
	for i := 3; i >= 0; i-- {
		if u[i] != 0 {
			return uint32(i)*64 + uint32(bits.Len64(u[i]))
		}
	}
	return 0
}

// shlBytes shifts left by whole bytes. Bits that leave the 256-bit width are dropped.
func (u u256) shlBytes(n uint32) u256 {
	if n >= 32 {
		return u256{}
	}
	b := u.leBytes()
	var out [32]byte
	copy(out[n:], b[:32-n])
	return u256FromBytes(out)
}

func (u u256) shrBytes(n uint32) u256 {
	if n >= 32 {
		return u256{}
	}
	b := u.leBytes()
	var out [32]byte
	copy(out[:32-n], b[n:])
	return u256FromBytes(out)
}

func (u u256) leBytes() [32]byte {
	var b [32]byte
	for i := 0; i < 4; i++ {
		binary.LittleEndian.PutUint64(b[i*8:], u[i])
	}
	return b
}

// mulU64 is a truncating 256-bit product with a u64, matching arith_uint256.
func (u u256) mulU64(factor uint64) u256 {
	var out u256
	var carry uint64
	for i := 0; i < 4; i++ {
		hi, lo := bits.Mul64(u[i], factor)
		lo, c := bits.Add64(lo, carry, 0)
		out[i] = lo
		carry = hi + c
	}
	return out
}

func (u u256) divU64(divisor uint64) u256 {
	var out u256
	var rem uint64
	for i := 3; i >= 0; i-- {
		q, r := bits.Div64(rem, u[i], divisor)
		out[i] = q
		rem = r
	}
	return out
}

// getCompact is Bitcoin arith_uint256::GetCompact for a non-negative value.
func (u u256) getCompact() uint32 {
	if u.isZero() {
		return 0
	}
	size := (u.bitLen() + 7) / 8
	var compact uint64
	if size <= 3 {
		compact = u[0] << (8 * (3 - size))
	} else {
		compact = u.shrBytes(size - 3)[0]
	}
	if compact&0x00800000 != 0 {
		compact >>= 8
		size++
	}
	return uint32(compact)&0x007fffff | size<<24
}
