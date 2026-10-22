package pcoc

// Canonical path body (SPEC §14.3).
//
// Version is the first byte and is 0x01. Then, in order: leaf 32, leaf_index
// u32be, tree_size u32be, sibling_count one byte, that many siblings of 32
// bytes, root 32, txid 32, vout u32be, block_hash 32, block_height u32be.
// The length is 146 + 32 × sibling_count and no other length decodes.

import (
	"crypto/sha256"
	"encoding/binary"
)

// PathBody is one decoded canonical path body (SPEC §14.3).
type PathBody struct {
	Leaf        []byte
	LeafIndex   uint32
	TreeSize    uint32
	Siblings    [][]byte
	Root        []byte
	Txid        []byte
	Vout        uint32
	BlockHash   []byte
	BlockHeight uint32
}

// DecodePathBody decodes a canonical path body. ok is false when the bytes
// are not that layout, including a sibling count that is not the canonical
// count for the tree size.
func DecodePathBody(b []byte) (PathBody, bool) {
	if len(b) < 146 || b[0] != 0x01 {
		return PathBody{}, false
	}
	siblingCount := int(b[145])
	if siblingCount > 32 {
		return PathBody{}, false
	}
	if len(b) != 146+32*siblingCount {
		return PathBody{}, false
	}
	leaf := append([]byte(nil), b[1:33]...)
	leafIndex := binary.BigEndian.Uint32(b[33:37])
	treeSize := binary.BigEndian.Uint32(b[37:41])
	if treeSize < 1 || leafIndex >= treeSize {
		return PathBody{}, false
	}
	if siblingCount != canonicalSiblingCount(treeSize) {
		return PathBody{}, false
	}
	sibs := make([][]byte, siblingCount)
	off := 146
	for i := 0; i < siblingCount; i++ {
		sibs[i] = append([]byte(nil), b[off:off+32]...)
		off += 32
	}
	root := append([]byte(nil), b[off:off+32]...)
	off += 32
	txid := append([]byte(nil), b[off:off+32]...)
	off += 32
	vout := binary.BigEndian.Uint32(b[off : off+4])
	off += 4
	blockHash := append([]byte(nil), b[off:off+32]...)
	off += 32
	blockHeight := binary.BigEndian.Uint32(b[off : off+4])
	return PathBody{
		Leaf: leaf, LeafIndex: leafIndex, TreeSize: treeSize, Siblings: sibs,
		Root: root, Txid: txid, Vout: vout, BlockHash: blockHash, BlockHeight: blockHeight,
	}, true
}

// canonicalSiblingCount is SPEC §14.3 canonical_sibling_count.
func canonicalSiblingCount(treeSize uint32) int {
	width := treeSize
	n := 0
	for width > 1 {
		n++
		width = width/2 + width%2
	}
	return n
}

// ValidateFor succeeds only when leaf equals waypoint and the Merkle path
// resolves to root (SPEC §14.3).
func (p PathBody) ValidateFor(waypoint []byte) bool {
	if len(waypoint) != 32 || !bytesEq(p.Leaf, waypoint) {
		return false
	}
	node := taggedHash(0x1a, p.Leaf)
	index := p.LeafIndex
	width := p.TreeSize
	for _, sib := range p.Siblings {
		if width%2 == 1 && index == width-1 {
			if !bytesEq(sib, node) {
				return false
			}
			node = parentHash(node, node)
		} else if index%2 == 0 {
			node = parentHash(node, sib)
		} else {
			node = parentHash(sib, node)
		}
		index = index / 2
		width = width/2 + width%2
	}
	return bytesEq(node, p.Root)
}

func taggedHash(tag byte, payload []byte) []byte {
	h := sha256.New()
	h.Write([]byte{tag})
	h.Write(payload)
	return h.Sum(nil)
}

func parentHash(left, right []byte) []byte {
	h := sha256.New()
	h.Write([]byte{0x1b})
	h.Write(left)
	h.Write(right)
	return h.Sum(nil)
}

func bytesEq(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	var d byte
	for i := range a {
		d |= a[i] ^ b[i]
	}
	return d == 0
}

// DisplayHex is Bitcoin display order of natural bytes: display[i] = natural[31−i]
// (SPEC §9). The spelling is lowercase hex.
func DisplayHex(natural []byte) string {
	out := make([]byte, len(natural)*2)
	const hexdigits = "0123456789abcdef"
	for i := 0; i < len(natural); i++ {
		b := natural[len(natural)-1-i]
		out[2*i] = hexdigits[b>>4]
		out[2*i+1] = hexdigits[b&0x0f]
	}
	return string(out)
}

// NaturalHex is byte-0-first lowercase hex (SPEC §7.2).
func NaturalHex(b []byte) string {
	const hexdigits = "0123456789abcdef"
	out := make([]byte, len(b)*2)
	for i, v := range b {
		out[2*i] = hexdigits[v>>4]
		out[2*i+1] = hexdigits[v&0x0f]
	}
	return string(out)
}

// ParseHex32 parses 64 hexadecimal digits into 32 bytes (SPEC §7.2).
// Letter case does not change the value. ok is false on any other spelling.
func ParseHex32(s string) ([]byte, bool) {
	if len(s) != 64 {
		return nil, false
	}
	out := make([]byte, 32)
	for i := 0; i < 32; i++ {
		hi, ok1 := hexVal(s[2*i])
		lo, ok2 := hexVal(s[2*i+1])
		if !ok1 || !ok2 {
			return nil, false
		}
		out[i] = hi<<4 | lo
	}
	return out, true
}

func hexVal(c byte) (byte, bool) {
	switch {
	case c >= '0' && c <= '9':
		return c - '0', true
	case c >= 'a' && c <= 'f':
		return c - 'a' + 10, true
	case c >= 'A' && c <= 'F':
		return c - 'A' + 10, true
	default:
		return 0, false
	}
}

// Reverse32 is the single reversal rule (SPEC §9): natural[i] = display[31−i].
func Reverse32(b []byte) []byte {
	out := make([]byte, 32)
	for i := 0; i < 32; i++ {
		out[i] = b[31-i]
	}
	return out
}
