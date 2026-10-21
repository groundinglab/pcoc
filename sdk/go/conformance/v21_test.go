package conformance

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"github.com/groundinglab/pcoc/sdk/go/v2/hike"
)

func TestV21Pack(t *testing.T) {
	root := filepath.Join(repoRoot(t), "spec", "conformance", "v2.1")
	var manifest struct {
		Pack  string   `json:"pack"`
		IDs   []string `json:"ids"`
		Files []string `json:"files"`
	}
	mustJSON(t, filepath.Join(root, "manifest.json"), &manifest)
	if manifest.Pack != "satoshi-hike-v2.1" {
		t.Fatalf("pack %s", manifest.Pack)
	}

	t.Run("seed", func(t *testing.T) {
		var doc struct {
			Public   string   `json:"public_key_hex"`
			Beacon   string   `json:"seed_beacon_hex"`
			Preimage string   `json:"seed_preimage_hex"`
			Seed     string   `json:"seed_hex"`
			Lanes    []string `json:"lanes_u64_hex"`
			Equals   int      `json:"waypoint_equals_seed"`
		}
		mustJSON(t, filepath.Join(root, "seed.json"), &doc)
		pub, beacon := mustHex(t, doc.Public), mustHex(t, doc.Beacon)
		pre := append(append([]byte{}, pub...), beacon...)
		if hex.EncodeToString(pre) != doc.Preimage {
			t.Fatal("preimage")
		}
		digest, lanes := hike.SeedLanes(pub, beacon)
		if hex.EncodeToString(digest) != doc.Seed || doc.Equals != 1 {
			t.Fatalf("seed %x", digest)
		}
		if hex.EncodeToString(hike.SeedValue(pub, beacon)) != doc.Seed {
			t.Fatal("waypoint")
		}
		if len(doc.Lanes) != 4 {
			t.Fatal(doc.Lanes)
		}
		for i, want := range doc.Lanes {
			if hexU64(lanes[i]) != want {
				t.Fatalf("lane %d %s", i, hexU64(lanes[i]))
			}
		}
	})

	t.Run("nl4-words", func(t *testing.T) {
		var doc struct {
			Public  string `json:"public_key_hex"`
			Beacon  string `json:"seed_beacon_hex"`
			Samples []struct {
				I   int    `json:"i"`
				J   int    `json:"j"`
				Hex string `json:"u64_hex"`
			} `json:"samples"`
			Words  int    `json:"slot0_words"`
			Hash   string `json:"slot0_sha256"`
			Parent bool   `json:"parents"`
		}
		mustJSON(t, filepath.Join(root, "nl4-words.json"), &doc)
		if doc.Parent || doc.Words != 2048 {
			t.Fatalf("parents %v words %d", doc.Parent, doc.Words)
		}
		pub, beacon := mustHex(t, doc.Public), mustHex(t, doc.Beacon)
		for _, sample := range doc.Samples {
			got := hike.NL4BaseWord(pub, beacon, sample.I, sample.J)
			if hexU64(got) != sample.Hex {
				t.Fatalf("word %d,%d %s", sample.I, sample.J, hexU64(got))
			}
		}
		slot := hike.NL4BaseSlot(pub, beacon, 0, doc.Words)
		sum := sha256.Sum256(slot)
		if hex.EncodeToString(sum[:]) != doc.Hash {
			t.Fatalf("slot0 %x", sum)
		}
	})

	t.Run("step-c1", func(t *testing.T) {
		var doc struct {
			SealDigest string `json:"seal0_digest_hex"`
			SealPrime  string `json:"seal0_s_prime_u64_hex"`
			Public     string `json:"public_key_hex"`
			Beacon     string `json:"seed_beacon_hex"`
			Slot       int    `json:"slot_index"`
			Words      int    `json:"words"`
			Parents    bool   `json:"parents"`
			Steps      []struct {
				S      string `json:"S_u64_hex"`
				Folded string `json:"S_folded_u64_hex"`
				Digest string `json:"digest_hex"`
				Prime  string `json:"S_prime_u64_hex"`
			} `json:"steps"`
		}
		mustJSON(t, filepath.Join(root, "step-c1.json"), &doc)
		digest, next := hike.SealState(0)
		if hex.EncodeToString(digest) != doc.SealDigest || hexU64(next) != doc.SealPrime {
			t.Fatalf("seal %x %s", digest, hexU64(next))
		}
		if doc.Parents || doc.Slot != 0 || doc.Words != 2048 {
			t.Fatalf("slot setup %+v", doc.Slot)
		}
		slot := hike.NL4BaseSlot(mustHex(t, doc.Public), mustHex(t, doc.Beacon), 0, doc.Words)
		for _, step := range doc.Steps {
			state := mustU64(t, step.S)
			folded := hike.FoldSlot(slot, state)
			sum, prime := hike.SealState(folded)
			if hexU64(folded) != step.Folded || hex.EncodeToString(sum) != step.Digest || hexU64(prime) != step.Prime {
				t.Fatalf("step %s folded %s prime %s", step.S, hexU64(folded), hexU64(prime))
			}
		}
	})

	t.Run("terrain-small", func(t *testing.T) {
		var doc struct {
			Public string `json:"public_key_hex"`
			Beacon string `json:"seed_beacon_hex"`
			Cases  []struct {
				Slot    int    `json:"slot_bytes"`
				Terrain int    `json:"terrain_bytes"`
				Hash    string `json:"sha256"`
			} `json:"cases"`
		}
		mustJSON(t, filepath.Join(root, "terrain-small.json"), &doc)
		pub, beacon := mustHex(t, doc.Public), mustHex(t, doc.Beacon)
		for _, item := range doc.Cases {
			profile, err := hike.NewProfile("conformance-small", hike.Kind, 32, item.Slot, item.Terrain, 1)
			if err != nil {
				t.Fatal(err)
			}
			terrain := hike.FillTerrain(pub, beacon, profile)
			sum := sha256.Sum256(terrain.Blob)
			if hex.EncodeToString(sum[:]) != item.Hash {
				t.Fatalf("terrain %d %x", item.Terrain, sum)
			}
		}
	})

	t.Run("rate", func(t *testing.T) {
		var doc struct {
			Floor struct {
				P    int64  `json:"p"`
				Q    int64  `json:"q"`
				Mark string `json:"mark"`
			} `json:"floor"`
			Comparison string `json:"comparison"`
			Cases      []struct {
				Name  string `json:"name"`
				Legs  int64  `json:"hike_legs"`
				Span  int64  `json:"hike_span"`
				Meets bool   `json:"meets"`
			} `json:"cases"`
		}
		mustJSON(t, filepath.Join(root, "rate.json"), &doc)
		if doc.Floor.P != hike.PlaceholderLegRateP || doc.Floor.Q != hike.PlaceholderLegRateQ || doc.Floor.Mark != "placeholder, not ratified" {
			t.Fatalf("floor %+v", doc.Floor)
		}
		if doc.Comparison != "meets when hike_span > 0 and q * hike_legs >= p * hike_span; hike_span <= 0 means the rate is undefined and refuses" {
			t.Fatal(doc.Comparison)
		}
		for _, item := range doc.Cases {
			got := hike.LegRateMeets(item.Legs, item.Span, doc.Floor.P, doc.Floor.Q)
			if got != item.Meets {
				t.Fatalf("%s meets %v", item.Name, got)
			}
			if item.Span <= 0 && hike.RateOutcome(item.Legs, item.Span, true, doc.Floor.P, doc.Floor.Q) != "refuse" {
				t.Fatalf("%s span", item.Name)
			}
		}
	})

	t.Run("anchors", func(t *testing.T) {
		var doc struct {
			Minimum int `json:"minimum"`
			Cases   []struct {
				Name    string `json:"name"`
				Indices []int  `json:"indices"`
				Meets   bool   `json:"meets"`
			} `json:"cases"`
		}
		mustJSON(t, filepath.Join(root, "anchors.json"), &doc)
		if doc.Minimum != hike.MinAnchoredPaths {
			t.Fatal(doc.Minimum)
		}
		for _, item := range doc.Cases {
			if hike.AnchoredIndicesMeet(item.Indices) != item.Meets {
				t.Fatalf("%s %v", item.Name, item.Indices)
			}
		}
		if !hike.AnchoredIndicesMeet([]int{16, 15, 14, 13, 12, 11}) {
			t.Fatal("last six")
		}
		if hike.AnchoredIndicesMeet([]int{1, 2, 3, 4, 5}) {
			t.Fatal("first five")
		}
	})

	t.Run("negatives", func(t *testing.T) {
		checkNegatives(t, filepath.Join(root, "negatives.json"))
	})
	t.Run("kind", func(t *testing.T) {
		var doc struct {
			Kind    string   `json:"kind"`
			Bytes   int      `json:"kind_bytes"`
			Earlier []string `json:"earlier"`
			Reason  string   `json:"earlier_reason"`
		}
		mustJSON(t, filepath.Join(root, "kind.json"), &doc)
		if doc.Kind != hike.Kind || len(doc.Kind) != doc.Bytes || doc.Reason != "earlier_version" {
			t.Fatalf("kind %s", doc.Kind)
		}
		if len(doc.Earlier) != 2 || doc.Earlier[0] != hike.KindV20 || doc.Earlier[1] != hike.KindV135 {
			t.Fatalf("earlier %v", doc.Earlier)
		}
		for _, kind := range doc.Earlier {
			raw := hike.EarlierKindObject(kind)
			rec, ok := hike.Verify(raw, hike.Production, hike.Index{Fetch: func(uint64) (string, bool) {
				t.Fatal("terrain or index")
				return "", false
			}}, nil, false)
			if !ok || rec.EarlierKind != kind {
				t.Fatalf("%s recorded %s", kind, rec.EarlierKind)
			}
		}
	})
	t.Run("deadline", func(t *testing.T) {
		var doc struct {
			K      int64  `json:"K"`
			Mark   string `json:"K_mark"`
			Reason string `json:"reason"`
			Cases  []struct {
				Name    string `json:"name"`
				Last    int64  `json:"last_beacon_height"`
				Tip     *int64 `json:"tip_height"`
				Outcome string `json:"outcome"`
				Ignore  bool   `json:"fake_leg_ignore"`
				Reason  string `json:"reason"`
			} `json:"cases"`
		}
		mustJSON(t, filepath.Join(root, "deadline.json"), &doc)
		if doc.K != hike.PlaceholderDeadlineK || doc.Mark != "placeholder, not ratified" || doc.Reason != "submission_deadline" {
			t.Fatalf("deadline %+v", doc.K)
		}
		for _, item := range doc.Cases {
			outcome, reason := hike.SubmissionDeadline(item.Tip, item.Last, doc.K)
			if outcome != item.Outcome || reason != item.Reason || item.Ignore {
				t.Fatalf("%s %s %s", item.Name, outcome, reason)
			}
		}
	})
	t.Run("mtp", func(t *testing.T) {
		checkMTP(t, filepath.Join(root, "mtp.json"))
	})
	t.Run("rounds", func(t *testing.T) {
		checkRounds(t, filepath.Join(root, "rounds.json"))
	})
	t.Run("prod-leg0", func(t *testing.T) {
		checkProdLeg(t, filepath.Join(root, "prod-leg0.json"))
	})

	t.Run("kat-small", func(t *testing.T) {
		replayStages(t, filepath.Join(root, "kat-small.json"), 18)
	})
	t.Run("test-3leg", func(t *testing.T) {
		replayStages(t, filepath.Join(root, "test-3leg.json"), 3)
	})
}

func replayStages(t *testing.T, path string, nStages int) {
	t.Helper()
	var doc struct {
		Profile struct {
			Slot    int `json:"slot_bytes"`
			Terrain int `json:"terrain_bytes"`
			Sample  int `json:"sample_bytes"`
			Steps   int `json:"steps_per_leg"`
		} `json:"profile"`
		Public  string `json:"public_key_hex"`
		Beacon  string `json:"seed_beacon_hex"`
		Seed    string `json:"seed_hex"`
		Opening string `json:"opening_waypoint_hex"`
		Equals  int    `json:"waypoint_equals_seed"`
		Terrain string `json:"terrain_sha256"`
		Stages  []struct {
			LegCount int      `json:"leg_count"`
			NextBeac *string  `json:"next_beacon_hex"`
			Legs     []string `json:"legs_le_hex"`
			Checks   []string `json:"checkpoints_hex"`
			NextWay  *string  `json:"next_waypoint_hex"`
		} `json:"stages"`
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatal(err)
	}
	if len(doc.Stages) != nStages || doc.Equals != 1 || doc.Opening != doc.Seed {
		t.Fatalf("stages %d", len(doc.Stages))
	}
	profile, err := hike.NewProfile("conformance-small", hike.Kind, doc.Profile.Sample, doc.Profile.Slot, doc.Profile.Terrain, doc.Profile.Steps)
	if err != nil {
		t.Fatal(err)
	}
	pub, beacon := mustHex(t, doc.Public), mustHex(t, doc.Beacon)
	terrain := hike.FillTerrain(pub, beacon, profile)
	sum := sha256.Sum256(terrain.Blob)
	if hex.EncodeToString(sum[:]) != doc.Terrain {
		t.Fatalf("terrain %x", sum)
	}
	waypoint := hike.SeedValue(pub, beacon)
	if hex.EncodeToString(waypoint) != doc.Seed {
		t.Fatal("seed")
	}
	for g, stage := range doc.Stages {
		limb := append([]byte(nil), waypoint...)
		previous := limb
		checks := make([][]byte, 0, stage.LegCount)
		if stage.LegCount != len(stage.Legs) || stage.LegCount != len(stage.Checks) {
			t.Fatalf("stage %d lengths", g)
		}
		for j := 0; j < stage.LegCount; j++ {
			leg := terrain.Leg(previous)
			if hex.EncodeToString(hike.Le64(leg)) != stage.Legs[j] {
				t.Fatalf("stage %d leg %d %x", g, j, hike.Le64(leg))
			}
			digest := hike.CheckpointDigest(terrain.Blob, previous, limb, leg, profile)
			if hex.EncodeToString(digest) != stage.Checks[j] {
				t.Fatalf("stage %d checkpoint %d %x", g, j, digest)
			}
			checks = append(checks, digest)
			previous = digest
		}
		if stage.NextBeac == nil || stage.NextWay == nil {
			if stage.NextBeac != nil || stage.NextWay != nil {
				t.Fatalf("stage %d half waypoint", g)
			}
			continue
		}
		interrupt := mustHex(t, *stage.NextBeac)
		next := hike.WaypointDigest(terrain.Blob, interrupt, limb, checks, profile)
		if hex.EncodeToString(next) != *stage.NextWay {
			t.Fatalf("stage %d waypoint %x", g, next)
		}
		waypoint = next
	}
}

func checkNegatives(t *testing.T, path string) {
	t.Helper()
	var doc struct {
		Public string `json:"public_key_hex"`
		Beacon string `json:"seed_beacon_hex"`
		Cases  []struct {
			Name       string `json:"name"`
			I          int    `json:"i"`
			J          int    `json:"j"`
			Forbidden  string `json:"forbidden_u64_hex"`
			Conforming string `json:"conforming_u64_hex"`
			Words      int    `json:"words"`
			WordsRead  int    `json:"words_read"`
			S          string `json:"S_u64_hex"`
			BadFold    string `json:"forbidden_S_folded_u64_hex"`
			GoodFold   string `json:"conforming_S_folded_u64_hex"`
			Slot       int    `json:"slot_bytes"`
			Terrain    int    `json:"terrain_bytes"`
			BadSHA     string `json:"forbidden_sha256"`
			GoodSHA    string `json:"conforming_sha256"`
			Leg        string `json:"leg_u64_hex"`
			Offset     uint64 `json:"sample_offset"`
			GoodWindow string `json:"conforming_hex"`
			BadWindow  string `json:"forbidden_hex"`
			Sample     int    `json:"sample_bytes"`
		} `json:"cases"`
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatal(err)
	}
	want := []string{
		"fill_pin_terrain", "ref_word", "all4_word", "mix64_without_mix_add",
		"v2_0_fold", "xor_accumulator_fold", "partial_slot_read", "partial_slot_read_2047",
		"one_lane_flip", "all4_kernel", "forged_window", "f3_shared_terrain",
	}
	if len(doc.Cases) != len(want) || doc.Public == "" {
		t.Fatalf("cases %d", len(doc.Cases))
	}
	pub, beacon := mustHex(t, doc.Public), mustHex(t, doc.Beacon)
	slot := hike.NL4BaseSlot(pub, beacon, 0, 2048)
	for i, name := range want {
		item := doc.Cases[i]
		if item.Name != name {
			t.Fatalf("case %s", item.Name)
		}
		switch name {
		case "ref_word", "all4_word":
			got := hexU64(hike.NL4BaseWord(pub, beacon, item.I, item.J))
			if got != item.Conforming || got == item.Forbidden {
				t.Fatalf("%s %s", name, got)
			}
		case "mix64_without_mix_add", "v2_0_fold", "xor_accumulator_fold", "partial_slot_read", "partial_slot_read_2047":
			got := hexU64(hike.FoldSlot(slot[:item.Words*8], mustU64(t, item.S)))
			if got != item.GoodFold || got == item.BadFold {
				t.Fatalf("%s %s", name, got)
			}
		case "fill_pin_terrain", "one_lane_flip", "all4_kernel":
			profile, err := hike.NewProfile("conformance-small", hike.Kind, 32, item.Slot, item.Terrain, 1)
			if err != nil {
				t.Fatal(err)
			}
			sum := sha256.Sum256(hike.FillTerrain(pub, beacon, profile).Blob)
			if hex.EncodeToString(sum[:]) != item.GoodSHA || hex.EncodeToString(sum[:]) == item.BadSHA {
				t.Fatalf("%s %x", name, sum)
			}
		case "forged_window":
			profile, err := hike.NewProfile("conformance-small", hike.Kind, item.Sample, item.Slot, item.Terrain, 1)
			if err != nil {
				t.Fatal(err)
			}
			terrain := hike.FillTerrain(pub, beacon, profile)
			seed := hike.SeedValue(pub, beacon)
			_, window := hike.SampleAt(terrain.Blob, seed, profile)
			pre := append(append(append(append([]byte{}, seed...), seed...), window...), hike.Le64(mustU64(t, item.Leg))...)
			sum := sha256.Sum256(pre)
			if hex.EncodeToString(sum[:]) != item.GoodWindow || hex.EncodeToString(sum[:]) == item.BadWindow {
				t.Fatalf("window %x", sum)
			}
			if int(item.Offset) != int(hike.U64LE(seed)%profile.OffsetModulus) {
				t.Fatalf("offset %d", item.Offset)
			}
		}
	}
}

func checkMTP(t *testing.T, path string) {
	t.Helper()
	var doc struct {
		Cases []struct {
			Name    string          `json:"name"`
			First   mtpEnd          `json:"first"`
			Last    *mtpEnd         `json:"last"`
			Span    json.RawMessage `json:"hike_span"`
			Outcome string          `json:"outcome"`
		} `json:"cases"`
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatal(err)
	}
	if len(doc.Cases) == 0 {
		t.Fatal("mtp cases")
	}
	for _, item := range doc.Cases {
		index := indexFromEnd(item.First)
		if item.Last != nil {
			addEnd(index, *item.Last)
		}
		first := beaconFromEnd(item.First)
		mtp, ok := hike.MedianTimePast(index, uint64(item.First.Height), first)
		if item.First.MTP == nil {
			if ok {
				t.Fatal("incomplete window read")
			}
			if item.Outcome != "held" || hike.RateOutcome(1, 0, false, 1, 5) != "held" {
				t.Fatalf("%s held", item.Name)
			}
			continue
		}
		if !ok || mtp != *item.First.MTP {
			t.Fatalf("%s first %d ok %v", item.Name, mtp, ok)
		}
		if item.Last == nil {
			continue
		}
		last := beaconFromEnd(*item.Last)
		mtpL, okL := hike.MedianTimePast(index, uint64(item.Last.Height), last)
		span, spanOK := spanNumber(item.Span)
		if !okL || item.Last.MTP == nil || mtpL != *item.Last.MTP || !spanOK || mtpL-mtp != span {
			t.Fatalf("%s span %d %d", item.Name, mtpL, mtp)
		}
		outcome := hike.RateOutcome(1, span, true, 1, 5)
		if item.Outcome == "refuse" && outcome != "refuse" {
			t.Fatalf("%s outcome %s", item.Name, outcome)
		}
		if item.Outcome == "read" && span <= 0 {
			t.Fatalf("%s read a non-positive span", item.Name)
		}
	}
}

type mtpEnd struct {
	Height int      `json:"height"`
	N      int      `json:"n"`
	Reads  []int    `json:"reads"`
	Times  []*int64 `json:"times"`
	MTP    *int64   `json:"mtp"`
	Raw    *int64   `json:"raw_ntime"`
}

func spanNumber(raw json.RawMessage) (int64, bool) {
	if len(raw) == 0 || raw[0] == '"' {
		return 0, false
	}
	n, err := strconv.ParseInt(string(raw), 10, 64)
	if err != nil {
		return 0, false
	}
	return n, true
}

func indexFromEnd(end mtpEnd) hike.Index {
	index := hike.Index{Documents: map[uint64]string{}}
	addEnd(index, end)
	return index
}

func addEnd(index hike.Index, end mtpEnd) {
	for i, height := range end.Reads {
		if end.Times[i] == nil {
			continue
		}
		index.Documents[uint64(height)] = fmt.Sprintf(
			`{"header":{"hash":"%s","height":%d,"time":%d}}`,
			strings.Repeat("ab", 32), height, *end.Times[i])
	}
}

func beaconFromEnd(end mtpEnd) hike.Header {
	t := json.RawMessage(`"not read"`)
	if end.Raw != nil {
		t = json.RawMessage(strconv.FormatInt(*end.Raw, 10))
	}
	return hike.Header{
		StoredHash:      json.RawMessage(`"` + strings.Repeat("11", 32) + `"`),
		StoredHeight:    json.RawMessage(strconv.Itoa(end.Height)),
		IndexComparison: "match",
		T:               t,
	}
}

func checkRounds(t *testing.T, path string) {
	t.Helper()
	var doc struct {
		Rounds int `json:"thorough_rounds"`
		Per    int `json:"per_round"`
		Cases  []struct {
			Name    string     `json:"name"`
			Total   uint64     `json:"leg_total"`
			Rounds  [][]uint64 `json:"rounds"`
			Draws   []string   `json:"draws_u64_hex"`
			Draw    string     `json:"draw"`
			Started int        `json:"rounds_started"`
		} `json:"cases"`
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatal(err)
	}
	if doc.Rounds != 3 || doc.Per != 6 {
		t.Fatalf("rounds %d %d", doc.Rounds, doc.Per)
	}
	for _, item := range doc.Cases {
		var stream []byte
		for _, spelling := range item.Draws {
			raw := mustHex(t, spelling)
			if len(raw) != 8 {
				t.Fatalf("draw %s", spelling)
			}
			var le [8]byte
			for i := 0; i < 8; i++ {
				le[i] = raw[7-i]
			}
			stream = append(stream, le[:]...)
		}
		groups, token := hike.DrawThorough(stream, item.Total, doc.Rounds, doc.Per)
		if item.Draw == "not defined" {
			if token != "not defined" || groups != nil {
				t.Fatalf("%s %s", item.Name, token)
			}
			continue
		}
		if token != "" || len(groups) != doc.Rounds {
			t.Fatalf("%s %s %d", item.Name, token, len(groups))
		}
		limit := item.Started
		if limit == 0 {
			limit = len(item.Rounds)
		}
		for r := 0; r < limit; r++ {
			if len(groups[r]) != len(item.Rounds[r]) {
				t.Fatalf("%s round %d", item.Name, r)
			}
			for i := range item.Rounds[r] {
				if groups[r][i] != item.Rounds[r][i] {
					t.Fatalf("%s round %d got %v", item.Name, r, groups[r])
				}
			}
		}
	}
}

func checkProdLeg(t *testing.T, path string) {
	t.Helper()
	var doc struct {
		Profile  string `json:"profile"`
		Public   string `json:"public_key_hex"`
		Beacon   string `json:"seed_beacon_hex"`
		Slot     int    `json:"slot_bytes"`
		Terrain  int    `json:"terrain_bytes"`
		Sample   int    `json:"sample_bytes"`
		Steps    int    `json:"steps"`
		Modulus  uint64 `json:"offset_modulus"`
		TerrainH string `json:"terrain_sha256"`
		Leg      string `json:"leg0_u64_hex"`
		Offset   uint64 `json:"sample_offset"`
		Check    string `json:"checkpoint0_hex"`
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatal(err)
	}
	if doc.Profile != "production" || doc.Slot != hike.Production.SlotBytes || doc.Terrain != hike.Production.TerrainBytes || doc.Steps != hike.Production.StepsPerLeg || doc.Modulus != hike.Production.OffsetModulus {
		t.Fatalf("profile %+v", doc.Slot)
	}
	pub, beacon := mustHex(t, doc.Public), mustHex(t, doc.Beacon)
	terrain := hike.FillTerrain(pub, beacon, hike.Production)
	sum := sha256.Sum256(terrain.Blob)
	if hex.EncodeToString(sum[:]) != doc.TerrainH {
		t.Fatalf("terrain %x", sum)
	}
	seed := hike.SeedValue(pub, beacon)
	leg := terrain.Leg(seed)
	if hexU64(leg) != doc.Leg {
		t.Fatalf("leg %s", hexU64(leg))
	}
	offset, sample := hike.SampleAt(terrain.Blob, seed, hike.Production)
	if offset != doc.Offset || len(sample) != doc.Sample {
		t.Fatalf("offset %d", offset)
	}
	digest := hike.CheckpointDigest(terrain.Blob, seed, seed, leg, hike.Production)
	if hex.EncodeToString(digest) != doc.Check {
		t.Fatalf("checkpoint %x", digest)
	}
}

func hexU64(v uint64) string {
	var buf [8]byte
	binary.BigEndian.PutUint64(buf[:], v)
	return hex.EncodeToString(buf[:])
}

func mustU64(t *testing.T, spelling string) uint64 {
	t.Helper()
	raw := mustHex(t, spelling)
	if len(raw) != 8 {
		t.Fatalf("u64 %s", spelling)
	}
	return binary.BigEndian.Uint64(raw)
}
