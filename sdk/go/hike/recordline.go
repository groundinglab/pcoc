package hike

import (
	"bytes"
	"encoding/json"
	"sync"
)

// V2Stage is one stage of a reference-shaped v2.0 fixture.
// Quiet stages store leg_count 0 and empty checkpoint arrays.
// PathKey forces a path key on stage 0. Later stages always carry the key.
type V2Stage struct {
	Beacon       [32]byte
	BeaconHeight uint64
	LegCount     uint64
	Waypoint     [32]byte
	Path         []byte
	PathKey      bool
	Checkpoints  [][32]byte
	LegResults   [][8]byte
}

// V2Object is a hike the v2.0 summary can measure.
// Encode follows encode_hike: stage 0 has no path key unless PathKey is set.
// The pack compares verify records, not these fixture bytes.
type V2Object struct {
	PublicKey  [32]byte
	SeedBeacon [32]byte
	Stages     []V2Stage
	Last       []byte
	LastHeight *uint64
	// Kind overrides the wire kind. Empty uses the profile kind.
	Kind string
}

// QuietV2 is the reference structural object after its paths, checkpoints,
// and leg results are cleared. Beacon byte 0 is g+1. Stage 0's waypoint is
// the seed value. A later waypoint is 32 bytes of g.
func QuietV2() V2Object {
	stages := make([]V2Stage, Stages)
	for g := 0; g < Stages; g++ {
		var beacon [32]byte
		beacon[0] = byte(g + 1)
		var waypoint [32]byte
		if g != 0 {
			for i := range waypoint {
				waypoint[i] = byte(g)
			}
		}
		stages[g] = V2Stage{
			Beacon:       beacon,
			BeaconHeight: uint64(g + 1),
			Waypoint:     waypoint,
		}
	}
	last := filled32(0)
	last[0] = 99
	height := uint64(Stages + 2)
	object := V2Object{
		PublicKey:  filled32(0x11),
		SeedBeacon: stages[0].Beacon,
		Stages:     stages,
		Last:       append([]byte(nil), last[:]...),
		LastHeight: &height,
	}
	copy(object.Stages[0].Waypoint[:], SeedValue(object.PublicKey[:], object.SeedBeacon[:]))
	return object
}

func filled32(b byte) [32]byte {
	var out [32]byte
	for i := range out {
		out[i] = b
	}
	return out
}

// Encode writes the reference top-level and stage key sets at production sizes.
func (o V2Object) Encode() []byte {
	return o.encode(Production)
}

func (o V2Object) encode(profile Profile) []byte {
	encoded := make([][]byte, len(o.Stages))
	for i, stage := range o.Stages {
		checks := make([][]byte, len(stage.Checkpoints))
		for j, item := range stage.Checkpoints {
			checks[j] = encodeBytes(item[:])
		}
		legs := make([][]byte, len(stage.LegResults))
		for j, item := range stage.LegResults {
			legs[j] = encodeBytes(item[:])
		}
		pairs := [][2][]byte{
			textPair("beacon", encodeBytes(stage.Beacon[:])),
			textPair("waypoint", encodeBytes(stage.Waypoint[:])),
			textPair("leg_count", encodeUint(stage.LegCount)),
			textPair("checkpoints", encodeArray(checks)),
			textPair("leg_results", encodeArray(legs)),
			textPair("beacon_height", encodeUint(stage.BeaconHeight)),
		}
		// Stage 0 omits path. A present key, including empty bytes, is the
		// shape item "stage-0 path key".
		if i != 0 || stage.PathKey {
			path := stage.Path
			if path == nil {
				path = []byte{}
			}
			pairs = append(pairs, textPair("path", encodeBytes(path)))
		}
		encoded[i] = encodeMap(pairs)
	}
	last := o.Last
	if last == nil {
		last = make([]byte, 32)
	}
	var lastHeight uint64
	if o.LastHeight != nil {
		lastHeight = *o.LastHeight
	}
	kind := o.Kind
	if kind == "" {
		kind = Kind
	}
	pairs := [][2][]byte{
		textPair("fill", encodeText(Fill)),
		textPair("kind", encodeText(kind)),
		textPair("stages", encodeArray(encoded)),
		textPair("n_slots", encodeUint(uint64(profile.NSlots))),
		textPair("public_key", encodeBytes(o.PublicKey[:])),
		textPair("slot_bytes", encodeUint(uint64(profile.SlotBytes))),
		textPair("seed_beacon", encodeBytes(o.SeedBeacon[:])),
		textPair("sample_bytes", encodeUint(uint64(profile.SampleBytes))),
		textPair("steps_per_leg", encodeUint(uint64(profile.StepsPerLeg))),
		textPair("last_interrupt", encodeBytes(last)),
		textPair("last_interrupt_height", encodeUint(lastHeight)),
	}
	return encodeMap(pairs)
}

// RecordLine is the compact line the SPEC names only to exclude it.
// It is not a conformance output and satoshi-hike-verify does not write it.
// PackRecord is the section 14 record spec/conformance/v2.0 compares.
//
// threads is the Rule A4.3 pool size. It is not StepsPerLeg. A non-positive
// count uses ConfiguredBindThreads. One at a time is conforming. The record
// does not depend on the count. Checkpoint binding hashes the sample.
// It does not run a leg. six_leg recomputes six stored legs when the hike
// has at least six. That recompute uses profile.StepsPerLeg.
//
// stream is the draw byte source. A nil stream with at least six legs
// records six_leg as not read. Fewer than six legs is not defined.
//
// An earlier kind returns before index or pathChain is called.
// pathChain nil is a successful chain bind.
func RecordLine(data []byte, profile Profile, index Index, pathChain func(PathLocators) error, threads int, stream []byte) string {
	if threads < 1 {
		threads = ConfiguredBindThreads()
	}
	item, err := decodeItem(data)
	if err != nil || item.kind != 5 {
		return summaryLine(profile.Name, "mismatch", "not read", "not read", "not read", nil)
	}
	if kind, ok := textOf(item, "kind"); ok && profile.Kind == Kind && IsEarlierKind(kind) {
		raw, mErr := json.Marshal(struct {
			Version string `json:"version"`
			Kind    string `json:"kind"`
		}{Version: EarlierVersionToken, Kind: kind})
		if mErr != nil {
			return "{}\n"
		}
		return string(raw) + "\n"
	}
	hike, ok := parseV2(item, profile)
	if !ok {
		return summaryLine(profile.Name, "mismatch", "not read", "not read", "not read", nil)
	}
	return measureLine(hike, profile, index, pathChain, threads, stream)
}

// PackRecord is the verify line spec/conformance/v2.0 compares.
// Issue #228 settled that a v2.0 verifier writes one output, the section 14
// record, and the pack stores those bytes. This is that switch.
// threads is the Rule A4.3 pool size. The record does not depend on it.
// One at a time is conforming. Checkpoint binding hashes the 1 KB sample.
// It does not run a leg.
func PackRecord(data []byte, profile Profile, index Index, pathChain func(PathLocators) error, threads int, stream []byte) string {
	rec, _ := verifyWithChain(data, profile, index, pathChain, stream, false)
	return string(rec.Marshal())
}

func summaryLine(profileName, shape, checkpoints, waypoints, six string, anchors []string) string {
	if anchors == nil {
		anchors = []string{}
	}
	raw, err := json.Marshal(struct {
		Profile     string   `json:"profile"`
		Shape       string   `json:"shape"`
		Checkpoints string   `json:"checkpoints"`
		Waypoints   string   `json:"waypoints"`
		LegRate     string   `json:"leg_rate"`
		SixLeg      string   `json:"six_leg"`
		Anchors     []string `json:"anchors"`
	}{profileName, shape, checkpoints, waypoints, "not read", six, anchors})
	if err != nil {
		return "{}\n"
	}
	return string(raw) + "\n"
}

type parsedV2 struct {
	publicKey  [32]byte
	seedBeacon [32]byte
	stages     []v2Stage
	last       []byte
	lastHeight *uint64
	shape      bool
}

type v2Stage struct {
	beacon       [32]byte
	beaconHeight uint64
	legCount     uint64
	waypoint     [32]byte
	path         []byte
	checkpoints  [][]byte
	legResults   [][]byte
	pathKey      bool
}

var v2StageKeys = []string{"path", "beacon", "waypoint", "leg_count", "checkpoints", "leg_results", "beacon_height"}

var v2Stage0Keys = []string{"beacon", "waypoint", "leg_count", "checkpoints", "leg_results", "beacon_height"}

var v2TopKeys = []string{
	"fill", "kind", "stages", "n_slots", "public_key", "slot_bytes",
	"seed_beacon", "sample_bytes", "steps_per_leg", "last_interrupt", "last_interrupt_height",
}

func parseV2(item cborVal, profile Profile) (parsedV2, bool) {
	var out parsedV2
	pub, ok := bytesOf(item, "public_key")
	if !ok || len(pub) != 32 {
		return out, false
	}
	seed, ok := bytesOf(item, "seed_beacon")
	if !ok || len(seed) != 32 {
		return out, false
	}
	copy(out.publicKey[:], pub)
	copy(out.seedBeacon[:], seed)
	stages, ok := item.field("stages")
	if !ok || stages.kind != 4 {
		return out, false
	}
	out.stages = make([]v2Stage, len(stages.arr))
	for i, raw := range stages.arr {
		stage, ok := parseV2Stage(raw)
		if !ok {
			return out, false
		}
		out.stages[i] = stage
	}
	if last, ok := bytesOf(item, "last_interrupt"); ok {
		out.last = append([]byte(nil), last...)
	}
	if h, ok, _ := uintField(item, "last_interrupt_height"); ok {
		out.lastHeight = &h
	}
	out.shape = v2Shape(item, out, profile)
	return out, true
}

func parseV2Stage(raw cborVal) (v2Stage, bool) {
	var stage v2Stage
	withPath := keySet(raw, v2StageKeys)
	if !withPath && !keySet(raw, v2Stage0Keys) {
		return stage, false
	}
	beacon, ok := bytesOf(raw, "beacon")
	if !ok || len(beacon) != 32 {
		return stage, false
	}
	waypoint, ok := bytesOf(raw, "waypoint")
	if !ok || len(waypoint) != 32 {
		return stage, false
	}
	var path []byte
	if withPath {
		var okPath bool
		path, okPath = bytesOf(raw, "path")
		if !okPath {
			return stage, false
		}
		stage.pathKey = true
	}
	count, ok, _ := uintField(raw, "leg_count")
	if !ok {
		return stage, false
	}
	height, ok, _ := uintField(raw, "beacon_height")
	if !ok {
		return stage, false
	}
	checks := byteList(raw, "checkpoints")
	results := byteList(raw, "leg_results")
	if checks == nil || results == nil || uint64(len(checks)) != count || len(checks) != len(results) {
		return stage, false
	}
	if !allLen(checks, 32) || !allLen(results, 8) {
		return stage, false
	}
	copy(stage.beacon[:], beacon)
	copy(stage.waypoint[:], waypoint)
	stage.path = append([]byte(nil), path...)
	stage.legCount = count
	stage.beaconHeight = height
	stage.checkpoints = checks
	stage.legResults = results
	return stage, true
}

func keySet(v cborVal, keys []string) bool {
	return v.kind == 5 && exactKeys(v.keys, keys)
}

func v2Shape(item cborVal, hike parsedV2, profile Profile) bool {
	if !keySet(item, v2TopKeys) {
		return false
	}
	kind, ok := textOf(item, "kind")
	if !ok || kind != profile.Kind {
		return false
	}
	fill, ok := textOf(item, "fill")
	if !ok || fill != Fill {
		return false
	}
	if !uintEqual(item, "slot_bytes", uint64(profile.SlotBytes)) ||
		!uintEqual(item, "n_slots", uint64(profile.NSlots)) ||
		!uintEqual(item, "sample_bytes", uint64(profile.SampleBytes)) ||
		!uintEqual(item, "steps_per_leg", uint64(profile.StepsPerLeg)) {
		return false
	}
	if len(hike.stages) != Stages {
		return false
	}
	for g, stage := range hike.stages {
		if stage.legCount < 1 {
			return false
		}
		if g == 0 {
			if stage.pathKey {
				return false
			}
		} else if !stage.pathKey {
			return false
		}
		if g == Stages-1 && len(stage.path) != 0 {
			return false
		}
	}
	seed := SeedValue(hike.publicKey[:], hike.seedBeacon[:])
	if hike.stages[0].beacon != hike.seedBeacon || !bytes.Equal(hike.stages[0].waypoint[:], seed) {
		return false
	}
	for g := 0; g < Stages-1; g++ {
		if hike.stages[g+1].beaconHeight <= hike.stages[g].beaconHeight {
			return false
		}
		if hike.stages[g+1].beacon == hike.stages[g].beacon {
			return false
		}
	}
	if len(hike.last) != 32 || hike.lastHeight == nil {
		return false
	}
	closing := hike.stages[Stages-1]
	if bytes.Equal(hike.last, closing.beacon[:]) || *hike.lastHeight <= closing.beaconHeight {
		return false
	}
	return true
}

func uintEqual(item cborVal, key string, want uint64) bool {
	got, ok, _ := uintField(item, key)
	return ok && got == want
}

func measureLine(hike parsedV2, profile Profile, index Index, pathChain func(PathLocators) error, threads int, stream []byte) string {
	if pathChain == nil {
		pathChain = func(PathLocators) error { return nil }
	}
	shape := "mismatch"
	if hike.shape {
		shape = "match"
	}
	beaconMiss := false
	for _, stage := range hike.stages {
		read := index.Read(stage.beaconHeight)
		if read == nil || read.Height != stage.beaconHeight || !bytes.Equal(read.Natural, stage.beacon[:]) {
			beaconMiss = true
		}
	}
	var terrain *Terrain
	digests := false
	for _, stage := range hike.stages {
		if len(stage.checkpoints) > 0 {
			digests = true
			break
		}
	}
	waypointsReady := len(hike.stages) >= Stages
	if digests || waypointsReady {
		terrain = FillTerrain(hike.publicKey[:], hike.seedBeacon[:], profile)
	}
	digestMiss := false
	if digests {
		if terrain == nil || !bindAll(terrain.Blob, hike.stages, profile, threads) {
			digestMiss = true
		}
	}
	checkpoints := "match"
	if beaconMiss || digestMiss {
		checkpoints = "mismatch"
	}
	six := sixToken(hike, profile, stream, terrain)
	if !waypointsReady {
		return summaryLine(profile.Name, shape, checkpoints, "not read", six, nil)
	}
	// Opening waypoint is the seed value, SHA-256(public_key ‖ seed_beacon).
	openingIsSeed := bytes.Equal(hike.stages[0].waypoint[:], SeedValue(hike.publicKey[:], hike.seedBeacon[:]))
	successorDiffers := false
	for g := 0; g < Stages-1; g++ {
		// SPEC §12.2 and the reference successor: I_g is stages[g+1].beacon.
		interrupt := hike.stages[g+1].beacon[:]
		digest := WaypointDigest(terrain.Blob, interrupt, hike.stages[g].waypoint[:], hike.stages[g].checkpoints, profile)
		if !bytes.Equal(digest, hike.stages[g+1].waypoint[:]) {
			successorDiffers = true
		}
	}
	anchors := make([]string, 0, 16)
	plantDiffers := false
	if len(hike.stages) >= 17 {
		for g := 1; g <= 16; g++ {
			token, differs := anchorOf(hike, g, pathChain)
			if differs {
				plantDiffers = true
			}
			anchors = append(anchors, token)
		}
	}
	waypoints := "match"
	if successorDiffers || plantDiffers || !openingIsSeed {
		waypoints = "mismatch"
	}
	return summaryLine(profile.Name, shape, checkpoints, waypoints, six, anchors)
}

// sixToken is the compact six_leg token from verify_record_json.
// The previous limb of a drawn leg is the stored waypoint when the leg is
// the first on its stage, and the previous checkpoint otherwise.
func sixToken(hike parsedV2, profile Profile, stream []byte, terrain *Terrain) string {
	var total uint64
	for _, stage := range hike.stages {
		next := total + stage.legCount
		if next < total {
			return "not read"
		}
		total = next
	}
	indices, token := DrawSix(stream, total)
	if token != "" {
		return token
	}
	if terrain == nil {
		terrain = FillTerrain(hike.publicKey[:], hike.seedBeacon[:], profile)
	}
	match := true
	for _, q := range indices {
		g, j := locateLegs(hike, q)
		stage := hike.stages[g]
		var previous []byte
		if j == 0 {
			previous = stage.waypoint[:]
		} else if j-1 < len(stage.checkpoints) {
			previous = stage.checkpoints[j-1]
		}
		if len(previous) < 8 || j < 0 || j >= len(stage.legResults) || len(stage.legResults[j]) != 8 || terrain.Leg(previous) != U64LE(stage.legResults[j]) {
			match = false
		}
	}
	if match {
		return "match"
	}
	return "mismatch"
}

func locateLegs(hike parsedV2, q uint64) (int, int) {
	var start uint64
	for g, stage := range hike.stages {
		if q < start+stage.legCount {
			return g, int(q - start)
		}
		start += stage.legCount
	}
	return 0, 0
}

func bindAll(blob []byte, stages []v2Stage, profile Profile, threads int) bool {
	match := true
	for _, stage := range stages {
		if len(stage.checkpoints) == 0 {
			continue
		}
		if !bindStage(blob, stage, profile, threads) {
			match = false
		}
	}
	return match
}

func bindStage(blob []byte, stage v2Stage, profile Profile, threads int) bool {
	n := len(stage.checkpoints)
	if threads < 1 {
		threads = 1
	}
	if threads > n {
		threads = n
	}
	limb := stage.waypoint[:]
	ok := make([]bool, n)
	jobs := make(chan int)
	var wg sync.WaitGroup
	for w := 0; w < threads; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := range jobs {
				previous := limb
				if j > 0 {
					previous = stage.checkpoints[j-1]
				}
				digest := CheckpointDigest(blob, previous, limb, U64LE(stage.legResults[j]), profile)
				ok[j] = bytes.Equal(digest, stage.checkpoints[j])
			}
		}()
	}
	for j := 0; j < n; j++ {
		jobs <- j
	}
	close(jobs)
	wg.Wait()
	for _, item := range ok {
		if !item {
			return false
		}
	}
	return true
}

func anchorOf(hike parsedV2, g int, pathChain func(PathLocators) error) (string, bool) {
	stage := hike.stages[g]
	if len(stage.path) == 0 {
		return "not anchored", false
	}
	loc, err := PathLocatorsFor(stage.path, stage.waypoint[:])
	if err != nil {
		return "not anchored", true
	}
	height := uint64(loc.BlockHeight)
	if !heightInAnchorWindow(stage.beaconHeight, height) || !interruptHeader(hike, loc.BlockHash[:], height) {
		return "not anchored", false
	}
	if pathChain(loc) != nil {
		return "not anchored", true
	}
	return "anchored", false
}

func heightInAnchorWindow(stageBeaconHeight, height uint64) bool {
	origin := AnchorWindowOrigin(stageBeaconHeight)
	for _, off := range anchorOffsetsAfterPost {
		if off < 0 {
			return false
		}
		if origin+uint64(off) < origin {
			return false
		}
	}
	for _, off := range anchorOffsetsAfterPost {
		if origin+uint64(off) == height {
			return true
		}
	}
	return false
}

// interruptHeader is SPEC §13.1. The stored interrupt headers are
// stages[1].beacon through stages[17].beacon, and last_interrupt (I_17)
// when the object stores both the hash and the height. A path included in
// I_17 is the inclusion header for W_16.
func interruptHeader(hike parsedV2, hash []byte, height uint64) bool {
	for i := 1; i < len(hike.stages); i++ {
		stage := hike.stages[i]
		if stage.beaconHeight == height && bytes.Equal(stage.beacon[:], hash) {
			return true
		}
	}
	if hike.last != nil && hike.lastHeight != nil && *hike.lastHeight == height && bytes.Equal(hike.last, hash) {
		return true
	}
	return false
}
