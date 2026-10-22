package pcoc

// Draws (SPEC §14.6 and §15.5).
//
// DrawSix and DrawThorough read a supplied byte source. They are the rig
// functions. Production verify reads the operating-system CSPRNG through
// DrawSixOS and does not take a byte source.
//
// One unbiased draw (SPEC §14.6):
//
//	M = 2^64
//	limit = M − (M mod leg_total)
//	read 8 bytes, u = u64_le
//	if u < limit, return u mod leg_total
//
// An 8-byte draw with u ≥ limit is consumed. When leg_total is a power of
// two, limit = M and the first draw is used.

import (
	"crypto/rand"
	"crypto/sha256"
	"io"
)

// ByteSource supplies the section 14.6 bytes in order.
// Read returns the next len(b) bytes or an error when it cannot.
type ByteSource interface {
	Read(b []byte) (int, error)
}

// ErrDrawShort is the byte source running out before the draw finishes.
var ErrDrawShort = io.ErrUnexpectedEOF

// DrawSix is one SPEC §14.6 six-leg draw from src.
// legTotal < 6 is not defined: ok is false and short is false, and src is
// not read. short is true when src cannot supply the next 8 bytes before
// six distinct indices are obtained. Those indices are not a result.
func DrawSix(src ByteSource, legTotal int) (indices []int, short bool, ok bool) {
	if legTotal < SixLegDrawCount {
		return nil, false, false
	}
	idx, exhausted := drawDistinct(src, legTotal, SixLegDrawCount)
	if exhausted {
		return nil, true, false
	}
	return idx, false, true
}

// DrawSixOS is the release-path SPEC §14.6 draw. It reads the operating
// system CSPRNG. It has no byte-source input.
func DrawSixOS(legTotal int) (indices []int, short bool, ok bool) {
	return DrawSix(csprng{}, legTotal)
}

// DrawThorough is one SPEC §15.5 thorough draw from src, read from the
// first byte of that source. D = 6 · thoroughRounds.
// legTotal < 6 is not defined. Exhaustion before D indices is not read:
// the returned indices are the complete groups of 6 already in phase, and
// short is true. A trailing group of fewer than 6 is not included.
func DrawThorough(src ByteSource, legTotal, thoroughRounds int) (indices []int, short bool, defined bool) {
	if legTotal < SixLegDrawCount {
		return nil, false, false
	}
	d := SixLegDrawCount * thoroughRounds
	phase := make([]int, 0, d)
	used := map[int]struct{}{}
	for len(phase) < d {
		q, err := unbiased(src, legTotal)
		if err != nil {
			complete := len(phase) - len(phase)%SixLegDrawCount
			return phase[:complete], true, true
		}
		groupStart := (len(phase) / SixLegDrawCount) * SixLegDrawCount
		if inSlice(q, phase[groupStart:]) {
			continue
		}
		if _, seen := used[q]; seen && len(used) < legTotal {
			continue
		}
		phase = append(phase, q)
		used[q] = struct{}{}
	}
	return phase, false, true
}

// ExpandDrawLabel is the SPEC §16.1 draw-byte chain.
// b_0 = SHA-256(label) and b_(n+1) = SHA-256(b_n), concatenated.
// The returned reader yields that chain and does not end.
func ExpandDrawLabel(label string) ByteSource {
	return &labelChain{label: []byte(label)}
}

type labelChain struct {
	label []byte
	block [32]byte
	off   int
	init  bool
}

func (c *labelChain) Read(b []byte) (int, error) {
	n := 0
	for n < len(b) {
		if !c.init || c.off == 32 {
			if !c.init {
				sum := sha256.Sum256(c.label)
				c.block = sum
				c.init = true
			} else {
				c.block = sha256.Sum256(c.block[:])
			}
			c.off = 0
		}
		k := copy(b[n:], c.block[c.off:])
		c.off += k
		n += k
	}
	return n, nil
}

type csprng struct{}

func (csprng) Read(b []byte) (int, error) { return rand.Read(b) }

// drawDistinct collects n distinct indices. It discards an index already
// chosen (SPEC §14.6).
func drawDistinct(src ByteSource, legTotal, n int) ([]int, bool) {
	out := make([]int, 0, n)
	for len(out) < n {
		q, err := unbiased(src, legTotal)
		if err != nil {
			return nil, true
		}
		if inSlice(q, out) {
			continue
		}
		out = append(out, q)
	}
	return out, false
}

// unbiased is one SPEC §14.6 draw from 0 … legTotal−1.
//
// M = 2^64 does not fit in a uint64. For a power-of-two leg total, M mod
// leg_total is 0, so limit = M and every u is accepted. Otherwise
// floor((2^64−1)/leg_total) equals floor(2^64/leg_total), the remainder
// rem = 2^64 − leg_total·q is the uint64 value −(leg_total·q), and
// limit = M − rem is the uint64 value −rem. u < limit is then the test.
func unbiased(src ByteSource, legTotal int) (int, error) {
	if legTotal <= 0 {
		return 0, ErrDrawShort
	}
	lt := uint64(legTotal)
	var buf [8]byte
	if lt&(lt-1) == 0 {
		if _, err := io.ReadFull(src, buf[:]); err != nil {
			return 0, ErrDrawShort
		}
		return int(U64le(buf[:]) % lt), nil
	}
	q := ^uint64(0) / lt
	rem := -(lt * q) // 2^64 mod lt
	limit := -rem    // M − rem
	for {
		if _, err := io.ReadFull(src, buf[:]); err != nil {
			return 0, ErrDrawShort
		}
		u := U64le(buf[:])
		if u < limit {
			return int(u % lt), nil
		}
	}
}

func inSlice(q int, s []int) bool {
	for _, v := range s {
		if v == q {
			return true
		}
	}
	return false
}
