package pcoc

// Terrain is the §10.4 byte string for one seed. It is filled once per
// terrain, before the first leg, and the step does not write it (SPEC §10,
// §11.1). One terrain is not shared with another PCOC.
type Terrain struct {
	p     Profile
	bytes []byte
	lanes [4]uint64
	seed  []byte
}

// FillTerrain is SPEC §10.4 dag2. Every terrain word depends on all 256 bits
// of the seed through the NL4 base word. Parent mixing and slot_mix do not
// remove that dependence.
//
// publicKey and seedBeacon are each 32 bytes. The returned terrain's seed
// field is D.
func FillTerrain(profile Profile, publicKey, seedBeacon []byte) (*Terrain, error) {
	if err := profile.Check(); err != nil {
		return nil, err
	}
	d := Seed(publicKey, seedBeacon)
	lanes := Lanes(d)
	nSlots := profile.NSlots()
	words := profile.WordsPerSlot()
	slotBytes := profile.SlotBytes

	bytes := make([]byte, profile.TerrainBytes)
	w := make([]uint64, words)

	for i := 0; i < nSlots; i++ {
		// NL4 base words, SPEC §10.4.
		for j := 0; j < words; j++ {
			c := (uint64(i) << 32) ^ uint64(j) ^ WordXor
			t := Mix64(lanes[j%4] ^ c)
			t = Mix64(t ^ Rotl(lanes[(j+1)%4], 17))
			t = Mix64(t ^ Rotl(lanes[(j+2)%4], 31))
			t = Mix64(t ^ Rotl(lanes[(j+3)%4], 47))
			w[j] = t
		}
		// Slot 0 keeps those words. It has no parent lanes and no slot mix.
		if i > 0 {
			parents := [3]int{i - 1, BitrevParent(i), MulParent(i)}
			for k := 1; k <= 3; k++ {
				pidx := parents[k-1]
				base := pidx * slotBytes
				xorK := uint64(k) * ParentMul
				for j := 0; j < words; j++ {
					pw := u64le(bytes[base+8*j : base+8*j+8])
					rot := uint((j + k) % 64)
					w[j] = Mix64((w[j] ^ Rotl(pw, rot)) ^ xorK)
				}
			}
			slotMix := uint64(i) * SlotMul
			for j := 0; j < words; j++ {
				w[j] = Mix64(w[j] ^ slotMix)
			}
		}
		base := i * slotBytes
		for j := 0; j < words; j++ {
			putU64le(bytes[base+8*j:base+8*j+8], w[j])
		}
	}

	seedCopy := make([]byte, 32)
	copy(seedCopy, d)
	return &Terrain{p: profile, bytes: bytes, lanes: lanes, seed: seedCopy}, nil
}

// Bytes returns the filled terrain. The caller must not write it.
func (t *Terrain) Bytes() []byte { return t.bytes }

// Seed returns D, 32 bytes.
func (t *Terrain) Seed() []byte { return t.seed }

// Lanes returns P[0] … P[3].
func (t *Terrain) Lanes() [4]uint64 { return t.lanes }

// Word returns the little-endian word j of slot i, as stored.
func (t *Terrain) Word(i, j int) uint64 {
	base := i*t.p.SlotBytes + 8*j
	return u64le(t.bytes[base : base+8])
}

// Profile returns the profile this terrain was filled under.
func (t *Terrain) Profile() Profile { return t.p }

// Slot returns the SLOT_BYTES of slot i. The returned slice aliases the
// terrain; the step reads it and does not write it.
func (t *Terrain) Slot(i int) []byte {
	base := i * t.p.SlotBytes
	return t.bytes[base : base+t.p.SlotBytes]
}
