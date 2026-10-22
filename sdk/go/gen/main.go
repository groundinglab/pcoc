// Command gen writes the CASES candidate files (SPEC §16, CASES v2).
//
// The thirteen permanent negatives are built only in this command, through
// package negatives. No release package imports that package.
package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"

	"github.com/groundinglab/pcoc/sdk/go/v2"
	"github.com/groundinglab/pcoc/sdk/go/v2/gen/negatives"
	"github.com/groundinglab/pcoc/sdk/go/v2/produce"
)

func main() {
	dir := "candidates"
	if len(os.Args) > 1 {
		dir = os.Args[1]
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		die(err)
	}
	pkA := mustHex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9")
	pkB := mustHex("dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659")
	seedBeacon := produce.BeaconAt(-1).Hash

	write(dir, "seed.txt", seedFile(pkA, pkB, seedBeacon))
	s1 := profileS1()
	tA, err := pcoc.FillTerrain(s1, pkA, seedBeacon)
	dieIf(err)
	write(dir, "nl4.txt", nl4File(tA, pkA, seedBeacon))
	write(dir, "fold.txt", foldFile(tA))
	write(dir, "wes.txt", wesFile(tA))
	write(dir, "rate.txt", rateFile())
	write(dir, "anchor.txt", anchorFile())
	write(dir, "draw.txt", drawFile())
	write(dir, "hashcash.txt", hashcashFile())

	// S1 full hike for kat-small and the honest leg.
	s1Legs := legsOf(1)
	s1Obj := mustDrive(s1, pkA, s1Legs)
	write(dir, "kat-small.txt", hikeFile("kat-small.S1", tA, s1Obj, 1))

	s2 := profileS2()
	tS2, err := pcoc.FillTerrain(s2, pkA, seedBeacon)
	dieIf(err)
	s2Obj := mustDrive(s2, pkA, legsOf(3))
	write(dir, "test-3leg.txt", hikeFile("test-3leg.S2", tS2, s2Obj, 3))

	prod := pcoc.Production()
	tProd, err := pcoc.FillTerrain(prod, pkA, seedBeacon)
	dieIf(err)
	write(dir, "terrain.txt", terrainFile(tA, tS2, tProd))
	prodLeg := pcoc.LegResult(tProd, tProd.Seed())
	prodCP := pcoc.Checkpoint(tProd, tProd.Seed(), tProd.Seed(), prodLeg)
	write(dir, "prod.txt", map[string]string{
		"prod.PROD.terrain_sha256":            hex.EncodeToString(negatives.TerrainSHA256(tProd.Bytes())),
		"prod.PROD.stage.0.leg.0.legs_le_hex": hex.EncodeToString(pcoc.Le64(prodLeg)),
		"prod.PROD.stage.0.leg.0.checkpoint":  hex.EncodeToString(prodCP),
	})

	write(dir, "negatives.txt", negativesFile(s1, tA, pkA, pkB, seedBeacon))

	// unknown-kind needs the S1 object bytes.
	obj := mustObject(s1, pkA, s1Legs)
	write(dir, "unknown-kind.txt", unknownFile(obj))
}

func profileS1() pcoc.Profile {
	return pcoc.Profile{Name: "conformance-small", SlotBytes: 64, SampleBytes: 32, TerrainBytes: 65536, StepsPerLeg: 100}
}

func profileS2() pcoc.Profile {
	return pcoc.Profile{Name: "conformance-small", SlotBytes: 256, SampleBytes: 64, TerrainBytes: 1048576, StepsPerLeg: 1000}
}

func legsOf(n int) []int {
	out := make([]int, 18)
	for i := range out {
		out[i] = n
	}
	return out
}

func mustDrive(p pcoc.Profile, pk []byte, legs []int) *pcoc.Object {
	b := mustObject(p, pk, legs)
	d := pcoc.DecodeObject(b)
	if d.Class != pcoc.ClassObject {
		die(fmt.Errorf("decode %d", d.Class))
	}
	return d.Object
}

func mustObject(p pcoc.Profile, pk []byte, legs []int) []byte {
	b, err := produce.Drive(p, pk, produce.CASESBeacons(), legs)
	dieIf(err)
	return b
}

func seedFile(pkA, pkB, beacon []byte) map[string]string {
	m := map[string]string{}
	for _, pair := range []struct {
		name string
		pk   []byte
	}{{"pk_A", pkA}, {"pk_B", pkB}} {
		pre := pcoc.SeedPreimage(pair.pk, beacon)
		d := pcoc.Seed(pair.pk, beacon)
		lanes := pcoc.Lanes(d)
		m["seed."+pair.name+".preimage"] = hex.EncodeToString(pre)
		m["seed."+pair.name+".D"] = hex.EncodeToString(d)
		for k := 0; k < 4; k++ {
			m[fmt.Sprintf("seed.%s.P%d_u64", pair.name, k)] = fmt.Sprintf("%016x", lanes[k])
		}
	}
	return m
}

func nl4File(t *pcoc.Terrain, pk, beacon []byte) map[string]string {
	lanes := pcoc.Lanes(pcoc.Seed(pk, beacon))
	m := map[string]string{}
	for _, i := range []int{0, 1, 1023} {
		for j := 0; j < 8; j++ {
			base := nl4(lanes, i, j)
			m[fmt.Sprintf("nl4.S1.slot.%d.word.%d.base_u64", i, j)] = fmt.Sprintf("%016x", base)
			m[fmt.Sprintf("nl4.S1.slot.%d.word.%d.stored_u64", i, j)] = fmt.Sprintf("%016x", t.Word(i, j))
		}
	}
	return m
}

func nl4(lanes [4]uint64, i, j int) uint64 {
	c := (uint64(i) << 32) ^ uint64(j) ^ pcoc.WordXor
	x := pcoc.Mix64(lanes[j%4] ^ c)
	x = pcoc.Mix64(x ^ pcoc.Rotl(lanes[(j+1)%4], 17))
	x = pcoc.Mix64(x ^ pcoc.Rotl(lanes[(j+2)%4], 31))
	x = pcoc.Mix64(x ^ pcoc.Rotl(lanes[(j+3)%4], 47))
	return x
}

func foldFile(t *pcoc.Terrain) map[string]string {
	s := pcoc.U64le(t.Seed()[:8])
	m := map[string]string{}
	for n := 0; n < 4; n++ {
		i := int(s % uint64(t.Profile().NSlots()))
		acc := accOf(t, s)
		next := pcoc.SealState(acc)
		m[fmt.Sprintf("fold.S1.step.%d.S_u64", n)] = fmt.Sprintf("%016x", s)
		m[fmt.Sprintf("fold.S1.step.%d.slot_index", n)] = strconv.Itoa(i)
		m[fmt.Sprintf("fold.S1.step.%d.acc_u64", n)] = fmt.Sprintf("%016x", acc)
		m[fmt.Sprintf("fold.S1.step.%d.S_next_u64", n)] = fmt.Sprintf("%016x", next)
		s = next
	}
	leg := pcoc.LegResult(t, t.Seed())
	m["fold.S1.leg_result_u64"] = fmt.Sprintf("%016x", leg)
	return m
}

func accOf(t *pcoc.Terrain, s uint64) uint64 {
	// The accumulator is the scalar fold. Reaching it through Step's seal
	// would hide the value, so the generator calls the kernel the step calls.
	i := int(s % uint64(t.Profile().NSlots()))
	slot := t.Slot(i)
	var acc uint64
	words := len(slot) / 8
	for w := 0; w < words; w++ {
		word := u64le(slot[8*w : 8*w+8])
		acc += pcoc.Mix64(word ^ s)
	}
	return acc
}

func u64le(b []byte) uint64 { return pcoc.U64le(b) }

func wesFile(t *pcoc.Terrain) map[string]string {
	d := t.Seed()
	return map[string]string{
		"wes.S1.D":                hex.EncodeToString(d),
		"wes.S1.stage.0.waypoint": hex.EncodeToString(d),
		"wes.S1.equal":            "true",
	}
}

func rateFile() map[string]string {
	m := map[string]string{}
	legs := []int64{0, 1, 5, 6, 1000}
	spans := []int64{1, 300, 3600}
	floors := [][2]int64{{1, 5}, {1, 4}, {7, 20}}
	for _, L := range legs {
		for _, s := range spans {
			for _, f := range floors {
				key := fmt.Sprintf("rate.legs.%d.span.%d.floor.%d_%d", L, s, f[0], f[1])
				m[key] = pcoc.RateCompare(L, s, f[0], f[1])
			}
		}
	}
	return m
}

func anchorFile() map[string]string {
	m := map[string]string{}
	for h := uint64(100); h <= 106; h++ {
		m[fmt.Sprintf("anchor.H.100.height.%d", h)] = pcoc.AnchorCompare(100, h)
	}
	return m
}

func drawFile() map[string]string {
	m := map[string]string{
		"draw.b0_label":        "pcoc-pack-2.2/draw",
		"draw.thorough_rounds": "3",
	}
	for _, L := range []int{6, 18, 54} {
		src := pcoc.ExpandDrawLabel("pcoc-pack-2.2/draw")
		idx, _, ok := pcoc.DrawSix(src, L)
		if !ok {
			die(fmt.Errorf("six draw %d", L))
		}
		m[fmt.Sprintf("draw.legs.%d.six_leg.indices", L)] = joinInts(idx)
		src2 := pcoc.ExpandDrawLabel("pcoc-pack-2.2/draw")
		th, _, def := pcoc.DrawThorough(src2, L, 3)
		if !def || len(th) != 18 {
			die(fmt.Errorf("thorough %d len %d", L, len(th)))
		}
		m[fmt.Sprintf("draw.legs.%d.thorough.indices", L)] = joinInts(th)
	}
	return m
}

func joinInts(ns []int) string {
	s := ""
	for i, n := range ns {
		if i > 0 {
			s += ","
		}
		s += strconv.Itoa(n)
	}
	return s
}

func hashcashFile() map[string]string {
	origin := "https://example.com"
	h0 := sha256.Sum256([]byte("pcoc-pack-2.2/hashcash/fee_header/0"))
	h1 := sha256.Sum256([]byte("pcoc-pack-2.2/hashcash/fee_header/1"))
	h2 := sha256.Sum256([]byte("pcoc-pack-2.2/hashcash/fee_header/2"))
	fee := append(append(append([]byte{}, h0[:]...), h1[:]...), h2[:16]...)
	hh := pcoc.HeaderHash(fee)
	wp := sha256.Sum256([]byte("pcoc-pack-2.2/hashcash/waypoint"))
	ch := pcoc.HashcashChallenge(origin, hh)
	nonce, dig := pcoc.HashcashSolve(ch, wp[:], pcoc.HashcashDifficultyBits)
	return map[string]string{
		"hashcash.bari_origin":       origin,
		"hashcash.fee_header":        hex.EncodeToString(fee),
		"hashcash.header_hash":       hex.EncodeToString(hh),
		"hashcash.waypoint":          hex.EncodeToString(wp[:]),
		"hashcash.difficulty_bits":   "21",
		"hashcash.challenge":         hex.EncodeToString(ch),
		"hashcash.nonce":             strconv.FormatUint(nonce, 10),
		"hashcash.digest":            hex.EncodeToString(dig),
		"hashcash.leading_zero_bits": strconv.Itoa(pcoc.LeadingZeroBits(dig)),
	}
}

func terrainFile(t1, t2, tp *pcoc.Terrain) map[string]string {
	return map[string]string{
		"terrain.S1.terrain_sha256":   hex.EncodeToString(negatives.TerrainSHA256(t1.Bytes())),
		"terrain.S2.terrain_sha256":   hex.EncodeToString(negatives.TerrainSHA256(t2.Bytes())),
		"terrain.PROD.terrain_sha256": hex.EncodeToString(negatives.TerrainSHA256(tp.Bytes())),
	}
}

func hikeFile(prefix string, t *pcoc.Terrain, o *pcoc.Object, legs int) map[string]string {
	m := map[string]string{}
	m[prefix+".terrain_sha256"] = hex.EncodeToString(negatives.TerrainSHA256(t.Bytes()))
	for g := 0; g < 18; g++ {
		st := o.Stages[g]
		m[fmt.Sprintf("%s.stage.%d.waypoint", prefix, g)] = hex.EncodeToString(st.Waypoint)
		for j := 0; j < legs; j++ {
			m[fmt.Sprintf("%s.stage.%d.leg.%d.legs_le_hex", prefix, g, j)] = hex.EncodeToString(st.LegResults[j])
			m[fmt.Sprintf("%s.stage.%d.leg.%d.checkpoint", prefix, g, j)] = hex.EncodeToString(st.Checkpoints[j])
		}
	}
	return m
}

func negativesFile(p pcoc.Profile, tA *pcoc.Terrain, pkA, pkB, beacon []byte) map[string]string {
	honest := pcoc.LegResult(tA, tA.Seed())
	honestHex := hex.EncodeToString(pcoc.Le64(honest))
	terrainHex := hex.EncodeToString(negatives.TerrainSHA256(tA.Bytes()))
	m := map[string]string{
		"negatives.honest.legs_le_hex":    honestHex,
		"negatives.honest.terrain_sha256": terrainHex,
	}
	steps := p.StepsPerLeg
	seed := tA.Seed()
	legs := map[string]uint64{}
	one := func(name string, step func(uint64) uint64) {
		legs[name] = negatives.LegResult(tA, seed, steps, step)
	}
	one("slot_summary_fold", func(s uint64) uint64 { return negatives.SlotSummaryFold(tA, s) })
	one("xor_accumulator_fold", func(s uint64) uint64 { return negatives.XorAccumulatorFold(tA, s) })
	one("xor_w_fold", func(s uint64) uint64 { return negatives.XorWFold(tA, s) })
	one("s_plus_w_fold", func(s uint64) uint64 { return negatives.SPlusWFold(tA, s) })
	pre := negatives.PrefoldHoist(tA)
	one("prefold_hoist", pre)
	one("final_stage_hoist", func(s uint64) uint64 { return negatives.FinalStageHoist(tA, s) })
	one("partial_slot_read", func(s uint64) uint64 { return negatives.PartialSlotRead(tA, s) })
	one("partial_slot_read_2047", func(s uint64) uint64 { return negatives.PartialSlotRead2047(tA, s) })

	tB, err := pcoc.FillTerrain(p, pkB, beacon)
	dieIf(err)
	// pk_A's own start, on pk_B's terrain.
	f3 := pcoc.LegResult(tB, pcoc.Seed(pkA, beacon))
	legs["f3_shared_terrain"] = f3

	cached, revisited := negatives.CachedLeg(tA, seed, steps)
	if !revisited {
		die(fmt.Errorf("cached_leg: the leg did not revisit a slot"))
	}
	legs["cached_leg"] = cached

	for _, name := range negatives.LegNames {
		v := legs[name]
		hx := hex.EncodeToString(pcoc.Le64(v))
		match := "false"
		if hx == honestHex {
			die(fmt.Errorf("negative %s equals honest", name))
		}
		m["negatives."+name+".legs_le_hex"] = hx
		m["negatives."+name+".matches_honest"] = match
	}

	lanes := tA.Lanes()
	terrains := map[string][]byte{
		"one_lane_flip":         negatives.FillOneLane(p, lanes),
		"all4_kernel":           negatives.FillAll4(p, lanes),
		"squeezed_seed_terrain": negatives.FillSqueezed(p, lanes),
	}
	for _, name := range negatives.TerrainNames {
		hx := hex.EncodeToString(negatives.TerrainSHA256(terrains[name]))
		if hx == terrainHex {
			die(fmt.Errorf("negative %s equals honest", name))
		}
		m["negatives."+name+".terrain_sha256"] = hx
		m["negatives."+name+".matches_honest"] = "false"
	}
	return m
}

func unknownFile(obj []byte) map[string]string {
	labels := []string{
		"v2.1-pcoc-object",
		"satoshi-hike-v2.0-object",
		"",
		"v2.2-pcoc-objecT",
	}
	m := map[string]string{}
	for n, label := range labels {
		m[fmt.Sprintf("unknown-kind.%d.kind_text_hex", n)] = hex.EncodeToString([]byte(label))
		m[fmt.Sprintf("unknown-kind.%d.record_hex", n)] = hex.EncodeToString(pcoc.UnknownKindRecord(label))
	}
	_ = obj
	return m
}

func write(dir, name string, m map[string]string) {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	var buf []byte
	for _, k := range keys {
		buf = append(buf, k...)
		buf = append(buf, '\t')
		buf = append(buf, m[k]...)
		buf = append(buf, '\n')
	}
	if err := os.WriteFile(filepath.Join(dir, name), buf, 0o644); err != nil {
		die(err)
	}
}

func mustHex(s string) []byte {
	b, err := hex.DecodeString(s)
	dieIf(err)
	return b
}

func dieIf(err error) {
	if err != nil {
		die(err)
	}
}

func die(err error) {
	fmt.Fprintln(os.Stderr, err)
	os.Exit(1)
}
