package pcoc

// Canonical path body. SPEC §14.3.

import "crypto/sha256"

// PathBody is a decoded canonical path body (SPEC §14.3).
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

// DecodePath decodes a canonical path body. No other length decodes.
// sibling_count is at most 32. tree_size is at least 1. leaf_index is in
// 0 … tree_size − 1. The sibling count equals canonical_sibling_count.
func DecodePath(b []byte) (PathBody, bool) {
	if len(b) < 146 || b[0] != 0x01 {
		return PathBody{}, false
	}
	siblingCount := int(b[1+32+4+4])
	if siblingCount > 32 {
		return PathBody{}, false
	}
	if len(b) != 146+32*siblingCount {
		return PathBody{}, false
	}
	o := 1
	leaf := b[o : o+32]
	o += 32
	leafIndex := be32(b[o : o+4])
	o += 4
	treeSize := be32(b[o : o+4])
	o += 4
	o++ // sibling_count already read
	if treeSize < 1 || leafIndex >= treeSize {
		return PathBody{}, false
	}
	if siblingCount != canonicalSiblingCount(treeSize) {
		return PathBody{}, false
	}
	sibs := make([][]byte, siblingCount)
	for i := 0; i < siblingCount; i++ {
		sibs[i] = append([]byte(nil), b[o:o+32]...)
		o += 32
	}
	root := append([]byte(nil), b[o:o+32]...)
	o += 32
	txid := append([]byte(nil), b[o:o+32]...)
	o += 32
	vout := be32(b[o : o+4])
	o += 4
	block := append([]byte(nil), b[o:o+32]...)
	o += 32
	height := be32(b[o : o+4])
	return PathBody{
		Leaf:        append([]byte(nil), leaf...),
		LeafIndex:   leafIndex,
		TreeSize:    treeSize,
		Siblings:    sibs,
		Root:        root,
		Txid:        txid,
		Vout:        vout,
		BlockHash:   block,
		BlockHeight: height,
	}, true
}

func be32(b []byte) uint32 {
	return uint32(b[0])<<24 | uint32(b[1])<<16 | uint32(b[2])<<8 | uint32(b[3])
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
func ValidateFor(body PathBody, waypoint []byte) bool {
	if len(waypoint) != 32 || !bytesEqual(body.Leaf, waypoint) {
		return false
	}
	// Leaf node is SHA-256(0x1A ‖ leaf).
	node := hashTagged(0x1a, body.Leaf, nil)
	index := body.LeafIndex
	width := body.TreeSize
	for _, sib := range body.Siblings {
		if width%2 == 1 && index == width-1 {
			if !bytesEqual(sib, node) {
				return false
			}
			node = hashTagged(0x1b, node, node)
		} else if index%2 == 0 {
			node = hashTagged(0x1b, node, sib)
		} else {
			node = hashTagged(0x1b, sib, node)
		}
		index = index / 2
		width = width/2 + width%2
	}
	return bytesEqual(node, body.Root)
}

func hashTagged(tag byte, a, b []byte) []byte {
	h := sha256.New()
	h.Write([]byte{tag})
	h.Write(a)
	if b != nil {
		h.Write(b)
	}
	return h.Sum(nil)
}

func bytesEqual(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
