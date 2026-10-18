package hike

// PathInclusionHeight reads block_height from a canonical path body.
// A body that does not validate returns ok=false. The caller still has to
// check the anchor window and the stored interrupt header before a chain bind.
func PathInclusionHeight(path []byte) (height uint64, ok bool) {
	body, err := decodePath(path)
	if err != nil {
		return 0, false
	}
	return uint64(body.blockHeight), true
}

// InclusionInWindow reports whether inclusion is one of the offsets after H
// and is also one of interruptHeights. H is AnchorWindowOrigin of the
// stage's stored beacon height. offsets must name AnchorWindow entries.
// An empty or short list is not a window, so nothing is inside it.
func InclusionInWindow(inclusion, stageBeaconHeight uint64, offsets []int, interruptHeights []uint64) bool {
	if len(offsets) != AnchorWindow {
		return false
	}
	origin := AnchorWindowOrigin(stageBeaconHeight)
	matched := false
	for _, off := range offsets {
		if off < 0 {
			return false
		}
		sum := origin + uint64(off)
		if sum < origin {
			continue
		}
		if sum == inclusion {
			matched = true
		}
	}
	if !matched {
		return false
	}
	for _, h := range interruptHeights {
		if h == inclusion {
			return true
		}
	}
	return false
}

// BindInclusion reads the index at inclusion once, and only when inclusion
// is inside the window and is one of the interrupt heights. A miss reads
// nothing. The caller must not scan other heights.
func BindInclusion(index Index, inclusion, stageBeaconHeight uint64, offsets []int, interruptHeights []uint64) (read *HeaderRead, inWindow bool) {
	if !InclusionInWindow(inclusion, stageBeaconHeight, offsets, interruptHeights) {
		return nil, false
	}
	return index.Read(inclusion), true
}
