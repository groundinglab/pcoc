package conformance

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
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
		if doc.Comparison != "q * hike_legs >= p * hike_span and hike_span > 0" {
			t.Fatal(doc.Comparison)
		}
		for _, item := range doc.Cases {
			got := hike.LegRateMeets(item.Legs, item.Span, doc.Floor.P, doc.Floor.Q)
			if got != item.Meets {
				t.Fatalf("%s meets %v", item.Name, got)
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
		var doc struct {
			Public string `json:"public_key_hex"`
			Beacon string `json:"seed_beacon_hex"`
			Cases  []struct {
				Name string `json:"name"`
			} `json:"cases"`
		}
		mustJSON(t, filepath.Join(root, "negatives.json"), &doc)
		want := []string{"fill_pin_terrain", "ref_word", "all4_word", "mix64_without_mix_add", "fatslot_fold", "partial_slot_read", "f3_shared_terrain"}
		if len(doc.Cases) != len(want) {
			t.Fatalf("cases %d", len(doc.Cases))
		}
		for i, name := range want {
			if doc.Cases[i].Name != name {
				t.Fatalf("case %s", doc.Cases[i].Name)
			}
		}
		if doc.Public != "1111111111111111111111111111111111111111111111111111111111111111" {
			t.Fatal("key")
		}
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
