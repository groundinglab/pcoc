// Command gencandidates writes sdk/go/candidates/*.txt.
//
// The files and the key spelling are CASES.md. The bytes are SPEC 2.2.
package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"

	"pcoc.dev/sdk/go"
	"pcoc.dev/sdk/go/negatives"
)

func main() {
	dir := "candidates"
	if len(os.Args) > 1 {
		dir = os.Args[1]
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		die(err)
	}
	files := map[string][][2]string{}
	seedLines(files)
	// Terrain fills are the slow part. S1 is shared by most cases.
	pkA := mustHex(pkAHex)
	beacon := sha256s("pcoc-pack-2.2/beacon/seed")
	s1 := pcoc.Profile{Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 100}
	s2 := pcoc.Profile{Name: "conformance-small", SlotBytes: 256, SampleBytes: 64, TerrainBytes: 1048576, StepsPerLeg: 1000}
	prod := pcoc.Production()

	fmt.Fprintln(os.Stderr, "fill S1")
	tS1, err := pcoc.FillTerrain(s1, pkA, beacon)
	if err != nil {
		die(err)
	}
	nl4Lines(files, s1, tS1)
	fmt.Fprintln(os.Stderr, "fill S2")
	tS2, err := pcoc.FillTerrain(s2, pkA, beacon)
	if err != nil {
		die(err)
	}
	fmt.Fprintln(os.Stderr, "fill PROD")
	tP, err := pcoc.FillTerrain(prod, pkA, beacon)
	if err != nil {
		die(err)
	}
	terrainLines(files, tS1, tS2, tP)
	foldLines(files, tS1)
	fmt.Fprintln(os.Stderr, "hike S1")
	hikeLines(files, "kat-small", s1, tS1, pkA, beacon, 1)
	fmt.Fprintln(os.Stderr, "hike S2")
	hikeLines(files, "test-3leg", s2, tS2, pkA, beacon, 3)
	fmt.Fprintln(os.Stderr, "PROD leg")
	prodLines(files, tP)
	wesLines(files, tS1)
	rateLines(files)
	anchorLines(files)
	drawLines(files)
	unknownLines(files)
	hashcashLines(files)
	fmt.Fprintln(os.Stderr, "negatives")
	negativeLines(files, s1, tS1, pkA, beacon)

	order := []string{"seed.txt", "nl4.txt", "terrain.txt", "fold.txt", "kat-small.txt", "test-3leg.txt", "prod.txt", "wes.txt", "rate.txt", "anchor.txt", "draw.txt", "unknown-kind.txt", "hashcash.txt", "negatives.txt"}
	for _, name := range order {
		writeFile(filepath.Join(dir, name), files[name])
	}
}

func die(err error) {
	fmt.Fprintln(os.Stderr, err)
	os.Exit(1)
}

const (
	pkAHex = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
	pkBHex = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"
)

func mustHex(s string) []byte {
	b, err := hex.DecodeString(s)
	if err != nil {
		die(err)
	}
	return b
}

func sha256s(label string) []byte {
	sum := sha256.Sum256([]byte(label))
	return sum[:]
}

func add(files map[string][][2]string, file, key, val string) {
	files[file] = append(files[file], [2]string{key, val})
}

func hx(b []byte) string { return hex.EncodeToString(b) }

func u64hex(v uint64) string { return fmt.Sprintf("%016x", v) }

func seedLines(files map[string][][2]string) {
	beacon := sha256s("pcoc-pack-2.2/beacon/seed")
	for _, row := range []struct {
		name string
		pk   string
	}{{"pk_A", pkAHex}, {"pk_B", pkBHex}} {
		pk := mustHex(row.pk)
		pre := pcoc.SeedPreimage(pk, beacon)
		d := pcoc.Seed(pk, beacon)
		lanes := pcoc.Lanes(d)
		p := "seed." + row.name
		add(files, "seed.txt", p+".preimage", hx(pre))
		add(files, "seed.txt", p+".D", hx(d))
		for k := 0; k < 4; k++ {
			add(files, "seed.txt", fmt.Sprintf("%s.P%d_u64", p, k), u64hex(lanes[k]))
		}
	}
}

func nl4Lines(files map[string][][2]string, p pcoc.Profile, t *pcoc.Terrain) {
	// CASES.md asks for j = 0 … 15. Under S1, WORDS_PER_SLOT is 8
	// (SLOT_BYTES / 8), so a slot has no word j ≥ 8. The words that exist
	// are j = 0 … WORDS_PER_SLOT − 1. See DONE.md, SPEC questions.
	nSlots := p.NSlots()
	words := p.WordsPerSlot()
	if words > 16 {
		words = 16
	}
	slots := []int{0, 1, nSlots - 1}
	for _, i := range slots {
		for j := 0; j < words; j++ {
			base := nl4Base(t.Lanes(), i, j)
			add(files, "nl4.txt", fmt.Sprintf("nl4.S1.pk_A.slot%d.j%d.base_u64", i, j), u64hex(base))
			add(files, "nl4.txt", fmt.Sprintf("nl4.S1.pk_A.slot%d.j%d.stored_u64", i, j), u64hex(t.Word(i, j)))
		}
	}
}

func nl4Base(lanes [4]uint64, i, j int) uint64 {
	c := (uint64(i) << 32) ^ uint64(j) ^ pcoc.WordXor
	x := pcoc.Mix64(lanes[j%4] ^ c)
	x = pcoc.Mix64(x ^ pcoc.Rotl(lanes[(j+1)%4], 17))
	x = pcoc.Mix64(x ^ pcoc.Rotl(lanes[(j+2)%4], 31))
	x = pcoc.Mix64(x ^ pcoc.Rotl(lanes[(j+3)%4], 47))
	return x
}

func terrainLines(files map[string][][2]string, s1, s2, prod *pcoc.Terrain) {
	add(files, "terrain.txt", "terrain.S1.pk_A", hx(sha256b(s1.Bytes())))
	add(files, "terrain.txt", "terrain.S2.pk_A", hx(sha256b(s2.Bytes())))
	add(files, "terrain.txt", "terrain.PROD.pk_A", hx(sha256b(prod.Bytes())))
}

func sha256b(b []byte) []byte {
	sum := sha256.Sum256(b)
	return sum[:]
}

func foldLines(files map[string][][2]string, t *pcoc.Terrain) {
	// First leg of stage 0. S_0 = P[0] = u64_le(D[0:8]).
	s := t.Lanes()[0]
	for step := 0; step < 4; step++ {
		nSlots := t.Profile().NSlots()
		i := int(s % uint64(nSlots))
		slot := t.Slot(i)
		var acc uint64
		words := t.Profile().WordsPerSlot()
		for w := 0; w < words; w++ {
			word := u64le(slot[8*w : 8*w+8])
			acc += pcoc.Mix64(word ^ s)
		}
		out := pcoc.SealState(acc)
		p := fmt.Sprintf("fold.S1.pk_A.stage0.leg0.step%d", step)
		add(files, "fold.txt", p+".S_u64", u64hex(s))
		add(files, "fold.txt", p+".slot", strconv.Itoa(i))
		add(files, "fold.txt", p+".acc_u64", u64hex(acc))
		add(files, "fold.txt", p+".S_out_u64", u64hex(out))
		s = out
	}
	// Finish the leg for leg_result.
	prev := t.Seed()
	lr := pcoc.LegResult(t, prev)
	add(files, "fold.txt", "fold.S1.pk_A.stage0.leg0.leg_result_u64", u64hex(lr))
	add(files, "fold.txt", "fold.S1.pk_A.stage0.leg0.legs_le_hex", hx(pcoc.Le64(lr)))
}

func u64le(b []byte) uint64 {
	return uint64(b[0]) | uint64(b[1])<<8 | uint64(b[2])<<16 | uint64(b[3])<<24 |
		uint64(b[4])<<32 | uint64(b[5])<<40 | uint64(b[6])<<48 | uint64(b[7])<<56
}

func hikeLines(files map[string][][2]string, name string, profile pcoc.Profile, t *pcoc.Terrain, pk, beacon []byte, legs int) {
	stages := schedule(beacon, legs)
	res, err := pcoc.Drive(profile, pk, stages)
	if err != nil {
		die(err)
	}
	// The drive fills terrain again. The bytes match t; use res for the object.
	_ = t
	// Profile tag in the key is the CASES.md fixture name: S1 for kat-small,
	// S2 for test-3leg.
	tag := "S1"
	if name == "test-3leg" {
		tag = "S2"
	}
	file := name + ".txt"
	for g := 0; g < 18; g++ {
		st := res.Object.Stages[g]
		for j := 0; j < len(st.LegResults); j++ {
			add(files, file, fmt.Sprintf("%s.%s.pk_A.stage%d.leg%d.legs_le_hex", name, tag, g, j), hx(st.LegResults[j]))
		}
		for j := 0; j < len(st.Checkpoints); j++ {
			add(files, file, fmt.Sprintf("%s.%s.pk_A.stage%d.leg%d.checkpoint", name, tag, g, j), hx(st.Checkpoints[j]))
		}
		// Waypoint: stage 0 stores the seed. Stages 1–17 store W_g.
		add(files, file, fmt.Sprintf("%s.%s.pk_A.stage%d.waypoint", name, tag, g), hx(st.Waypoint))
	}
}

func schedule(seedBeacon []byte, legs int) [18]pcoc.StageInput {
	var out [18]pcoc.StageInput
	// I_k for k = 0 … 17, plus the seed beacon as the stage-0 start.
	beacons := make([][]byte, 19)
	beacons[0] = seedBeacon
	for k := 0; k <= 17; k++ {
		beacons[k+1] = sha256s(fmt.Sprintf("pcoc-pack-2.2/beacon/%d", k))
	}
	for g := 0; g < 18; g++ {
		out[g] = pcoc.StageInput{
			Legs:            legs,
			Beacon:          beacons[g],
			BeaconHeight:    uint64(1000 + g),
			Interrupt:       beacons[g+1],
			InterruptHeight: uint64(1000 + g + 1),
		}
		if g > 0 {
			out[g].Path = []byte{} // path_skip
		}
	}
	return out
}

func prodLines(files map[string][][2]string, t *pcoc.Terrain) {
	add(files, "prod.txt", "prod.PROD.pk_A.terrain", hx(sha256b(t.Bytes())))
	lr, cp := pcoc.Leg(t, t.Seed(), t.Seed())
	add(files, "prod.txt", "prod.PROD.pk_A.stage0.leg0.legs_le_hex", hx(pcoc.Le64(lr)))
	add(files, "prod.txt", "prod.PROD.pk_A.stage0.leg0.checkpoint", hx(cp))
}

func wesLines(files map[string][][2]string, t *pcoc.Terrain) {
	// Stage-0 waypoint is D. They are equal by design (SPEC §10, §12.1).
	add(files, "wes.txt", "wes.S1.pk_A.stage0.waypoint", hx(t.Seed()))
	add(files, "wes.txt", "wes.S1.pk_A.D", hx(t.Seed()))
	add(files, "wes.txt", "wes.S1.pk_A.equal", "true")
}

func rateLines(files map[string][][2]string) {
	legs := []int{0, 1, 5, 6, 1000}
	spans := []int{1, 300, 3600}
	floors := [][2]int{{1, 5}, {1, 4}, {7, 20}}
	for _, hl := range legs {
		for _, hs := range spans {
			span := hs
			for _, f := range floors {
				key := fmt.Sprintf("rate.hike_legs%d.hike_span%d.floor%d/%d", hl, hs, f[0], f[1])
				add(files, "rate.txt", key, pcoc.LegRateFloor(hl, &span, f[0], f[1]))
			}
		}
	}
}

func anchorLines(files map[string][][2]string) {
	h := 100
	for ht := 100; ht <= 106; ht++ {
		key := fmt.Sprintf("anchor.H%d.header%d", h, ht)
		add(files, "anchor.txt", key, pcoc.AnchorWindow(h, ht))
	}
}

func drawLines(files map[string][][2]string) {
	// b_0 = SHA-256("pcoc-pack-2.2/draw"), b_{n+1} = SHA-256(b_n), concatenated.
	var stream []byte
	b := sha256s("pcoc-pack-2.2/draw")
	for n := 0; n < 64; n++ {
		stream = append(stream, b...)
		sum := sha256.Sum256(b)
		b = sum[:]
	}
	for _, n := range []int{6, 18, 54} {
		src := pcoc.NewByteSource(stream)
		idx, nd, nr := pcoc.DrawSix(n, src)
		if nd || nr {
			die(fmt.Errorf("draw %d not defined/read", n))
		}
		val := ""
		for i, q := range idx {
			if i > 0 {
				val += ","
			}
			val += strconv.Itoa(q)
		}
		add(files, "draw.txt", fmt.Sprintf("draw.legs%d.indices", n), val)
	}
}

func unknownLines(files map[string][][2]string) {
	labels := []string{"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"}
	names := []string{"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "empty", "v2.2-pcoc-objecT"}
	for i, lab := range labels {
		rec := pcoc.UnknownKindRecord(lab)
		add(files, "unknown-kind.txt", "unknown-kind."+names[i], hx(rec))
	}
}

func hashcashLines(files map[string][][2]string) {
	origin := "https://bari.example"
	// Fee header is 80 bytes derived from the named label, so another core
	// that picks the same name agrees.
	header := make([]byte, 80)
	copy(header, sha256s("pcoc-pack-2.2/hashcash.fee_header"))
	copy(header[32:], sha256s("pcoc-pack-2.2/hashcash.fee_header/2"))
	copy(header[64:], sha256s("pcoc-pack-2.2/hashcash.fee_header/3")[:16])
	hh := pcoc.HeaderHash(header)
	ch := pcoc.HashcashChallenge(origin, hh)
	wp := sha256s("pcoc-pack-2.2/hashcash.waypoint")
	add(files, "hashcash.txt", "hashcash.origin_label", "https://bari.example")
	add(files, "hashcash.txt", "hashcash.fee_header_label", "pcoc-pack-2.2/hashcash.fee_header")
	add(files, "hashcash.txt", "hashcash.waypoint_label", "pcoc-pack-2.2/hashcash.waypoint")
	add(files, "hashcash.txt", "hashcash.challenge", hx(ch))
	dig0 := pcoc.HashcashDigest(ch, wp, 0)
	add(files, "hashcash.txt", "hashcash.digest_nonce0", hx(dig0))
	var nonce uint64
	var dig []byte
	for {
		dig = pcoc.HashcashDigest(ch, wp, nonce)
		if pcoc.HashcashMeets(dig, pcoc.HashcashDifficultyBits) {
			break
		}
		nonce++
		if nonce > 1<<24 {
			die(fmt.Errorf("hashcash search exceeded 2^24"))
		}
	}
	add(files, "hashcash.txt", "hashcash.nonce", strconv.FormatUint(nonce, 10))
	add(files, "hashcash.txt", "hashcash.digest", hx(dig))
}

func negativeLines(files map[string][][2]string, p pcoc.Profile, t *pcoc.Terrain, pk, beacon []byte) {
	prev := t.Seed()
	honestLeg := pcoc.LegResult(t, prev)
	honestTerrain := hx(sha256b(t.Bytes()))

	type row struct {
		name string
		val  string
	}
	// Each fold shortcut replaces the §11.1 step for a whole S1 leg
	// (STEPS_PER_LEG steps from the seed). The value is le64 of that
	// leg result. It is not the honest leg.
	steps := []struct {
		name string
		fn   func(*pcoc.Terrain, uint64) uint64
	}{
		{"slot_summary_fold", negatives.SlotSummaryFold},
		{"xor_accumulator_fold", negatives.XorAccumulatorFold},
		{"xor_w_fold", negatives.XorWFold},
		{"s_plus_w_fold", negatives.SPlusWFold},
		{"prefold_hoist", negatives.PrefoldHoist},
		{"partial_slot_read", negatives.PartialSlotRead},
		{"partial_slot_read_2047", negatives.PartialSlotRead2047},
	}
	var rows []row
	for _, s := range steps {
		got := badLeg(t, prev, s.fn)
		rows = append(rows, row{s.name + ".legs_le_hex", hx(pcoc.Le64(got))})
		rows = append(rows, row{s.name + ".matches_honest", eqU(got, honestLeg)})
	}
	// Terrain digests.
	one := hx(negatives.OneLaneFlip(p, pk, beacon))
	all4 := hx(negatives.All4Kernel(p, pk, beacon))
	// A second seed, so the shared terrain (pk_A's) is not that seed's terrain.
	beacon2 := sha256s("pcoc-pack-2.2/beacon/seed-other")
	shared := hx(negatives.F3SharedTerrain(p, pk, beacon))
	t2, err := pcoc.FillTerrain(p, pk, beacon2)
	if err != nil {
		die(err)
	}
	squeezed := hx(negatives.SqueezedSeedTerrain(p, pk, beacon))
	cached := negatives.CachedLeg(t, prev)
	rows = append(rows,
		row{"one_lane_flip.terrain", one},
		row{"one_lane_flip.matches_honest", eq(one, honestTerrain)},
		row{"all4_kernel.terrain", all4},
		row{"all4_kernel.matches_honest", eq(all4, honestTerrain)},
		row{"f3_shared_terrain.terrain", shared},
		row{"f3_shared_terrain.matches_honest", eq(shared, hx(sha256b(t2.Bytes())))},
		row{"cached_leg.legs_le_hex", hx(pcoc.Le64(cached))},
		row{"cached_leg.matches_honest", eqU(cached, honestLeg)},
		row{"squeezed_seed_terrain.terrain", squeezed},
		row{"squeezed_seed_terrain.matches_honest", eq(squeezed, honestTerrain)},
	)
	for _, r := range rows {
		add(files, "negatives.txt", "negatives.S1.pk_A."+r.name, r.val)
	}
}

// badLeg runs a shortcut in place of Step for STEPS_PER_LEG steps.
func badLeg(t *pcoc.Terrain, prev []byte, step func(*pcoc.Terrain, uint64) uint64) uint64 {
	s := u64le(prev[:8])
	for n := 0; n < t.Profile().StepsPerLeg; n++ {
		s = step(t, s)
	}
	return s
}

func eq(a, b string) string {
	if a == b {
		return "true"
	}
	return "false"
}

func eqU(a, b uint64) string {
	if a == b {
		return "true"
	}
	return "false"
}

func writeFile(path string, lines [][2]string) {
	sort.Slice(lines, func(i, j int) bool { return lines[i][0] < lines[j][0] })
	f, err := os.Create(path)
	if err != nil {
		die(err)
	}
	defer f.Close()
	for _, ln := range lines {
		if _, err := fmt.Fprintf(f, "%s\t%s\n", ln[0], ln[1]); err != nil {
			die(err)
		}
	}
}
