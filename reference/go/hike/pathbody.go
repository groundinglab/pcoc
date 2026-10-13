package hike

import (
	"encoding/binary"
	"errors"
)

// Canonical path body. The byte layout matches the reference path body
// copied from services/bari. This file does not define a second grammar.
const (
	pathVersion   = 0x01
	pathMaxSibs   = 32
	pathFixedLen  = 146
	pathLeafDom   = 0x1A
	pathParentDom = 0x1B
)

// PathLocators are the fields a chain bind receives after the body validates.
type PathLocators struct {
	Root        [32]byte
	Txid        [32]byte
	Vout        uint32
	BlockHash   [32]byte
	BlockHeight uint32
}

type pathBody struct {
	leaf        [32]byte
	leafIndex   uint32
	treeSize    uint32
	siblings    [][32]byte
	root        [32]byte
	txid        [32]byte
	vout        uint32
	blockHash   [32]byte
	blockHeight uint32
}

func leafNode(leaf []byte) []byte {
	pre := make([]byte, 33)
	pre[0] = pathLeafDom
	copy(pre[1:], leaf)
	return SHA256(pre)
}

func parentNode(left, right []byte) []byte {
	pre := make([]byte, 65)
	pre[0] = pathParentDom
	copy(pre[1:33], left)
	copy(pre[33:], right)
	return SHA256(pre)
}

func canonicalSiblingCount(width uint32) (int, error) {
	if width == 0 {
		return 0, errors.New("empty tree")
	}
	count := 0
	for width > 1 {
		width = width/2 + width%2
		count++
	}
	return count, nil
}

func (p pathBody) resolve() ([]byte, error) {
	if p.treeSize == 0 {
		return nil, errors.New("empty tree")
	}
	if p.leafIndex >= p.treeSize {
		return nil, errors.New("leaf index")
	}
	expected, err := canonicalSiblingCount(p.treeSize)
	if err != nil {
		return nil, err
	}
	if len(p.siblings) != expected {
		return nil, errors.New("sibling count")
	}
	node := leafNode(p.leaf[:])
	index := p.leafIndex
	width := p.treeSize
	for _, sibling := range p.siblings {
		sib := sibling[:]
		if width%2 == 1 && index == width-1 {
			if !bytesEqual(sib, node) {
				return nil, errors.New("self sibling")
			}
			node = parentNode(node, node)
		} else if index%2 == 0 {
			node = parentNode(node, sib)
		} else {
			node = parentNode(sib, node)
		}
		index /= 2
		width = width/2 + width%2
	}
	return node, nil
}

func (p pathBody) validateFor(leaf []byte) error {
	if len(leaf) != 32 || !bytesEqual(p.leaf[:], leaf) {
		return errors.New("leaf")
	}
	got, err := p.resolve()
	if err != nil {
		return err
	}
	if !bytesEqual(got, p.root[:]) {
		return errors.New("root")
	}
	return nil
}

func (p pathBody) locators() PathLocators {
	return PathLocators{
		Root:        p.root,
		Txid:        p.txid,
		Vout:        p.vout,
		BlockHash:   p.blockHash,
		BlockHeight: p.blockHeight,
	}
}

func decodePath(bytes []byte) (pathBody, error) {
	var zero pathBody
	if len(bytes) < pathFixedLen {
		return zero, errors.New("truncated")
	}
	if bytes[0] != pathVersion {
		return zero, errors.New("version")
	}
	count := int(bytes[41])
	if count > pathMaxSibs {
		return zero, errors.New("siblings")
	}
	if len(bytes) != pathFixedLen+count*32 {
		return zero, errors.New("length")
	}
	var body pathBody
	copy(body.leaf[:], bytes[1:33])
	body.leafIndex = binary.BigEndian.Uint32(bytes[33:37])
	body.treeSize = binary.BigEndian.Uint32(bytes[37:41])
	body.siblings = make([][32]byte, count)
	at := 42
	for i := 0; i < count; i++ {
		copy(body.siblings[i][:], bytes[at:at+32])
		at += 32
	}
	copy(body.root[:], bytes[at:at+32])
	at += 32
	copy(body.txid[:], bytes[at:at+32])
	at += 32
	body.vout = binary.BigEndian.Uint32(bytes[at : at+4])
	at += 4
	copy(body.blockHash[:], bytes[at:at+32])
	at += 32
	body.blockHeight = binary.BigEndian.Uint32(bytes[at : at+4])
	if err := body.validateFor(body.leaf[:]); err != nil {
		return zero, err
	}
	return body, nil
}

// PathLocatorsFor decodes a canonical body and accepts it only for waypoint.
func PathLocatorsFor(path, waypoint []byte) (PathLocators, error) {
	body, err := decodePath(path)
	if err != nil {
		return PathLocators{}, err
	}
	if err := body.validateFor(waypoint); err != nil {
		return PathLocators{}, err
	}
	return body.locators(), nil
}

// EncodePathInBlock is the one-leaf fixture body. leaf is waypoint.
// block_hash is 32 bytes of 0x55. txid is 32 bytes of 0x44. vout is 2.
func EncodePathInBlock(waypoint []byte, blockHeight uint32) []byte {
	return encodePathForBlock(waypoint, bytesRepeat(0x55, 32), blockHeight)
}

// encodePathForBlock is the one-leaf fixture. blockHash is 32 natural-order
// bytes, the order the path body stores. The verify record reverses them.
func encodePathForBlock(waypoint, blockHash []byte, blockHeight uint32) []byte {
	if len(waypoint) != 32 || len(blockHash) != 32 {
		return nil
	}
	root := leafNode(waypoint)
	out := make([]byte, 0, pathFixedLen)
	out = append(out, pathVersion)
	out = append(out, waypoint...)
	var buf [4]byte
	binary.BigEndian.PutUint32(buf[:], 0)
	out = append(out, buf[:]...)
	binary.BigEndian.PutUint32(buf[:], 1)
	out = append(out, buf[:]...)
	out = append(out, 0)
	out = append(out, root...)
	out = append(out, bytesRepeat(0x44, 32)...)
	binary.BigEndian.PutUint32(buf[:], 2)
	out = append(out, buf[:]...)
	out = append(out, blockHash...)
	binary.BigEndian.PutUint32(buf[:], blockHeight)
	out = append(out, buf[:]...)
	return out
}

func bytesRepeat(b byte, n int) []byte {
	out := make([]byte, n)
	for i := range out {
		out[i] = b
	}
	return out
}
