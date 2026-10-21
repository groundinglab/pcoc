package conformance

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"

	"github.com/groundinglab/pcoc/sdk/go/v2/hike"
)

func mustJSON(t *testing.T, path string, dest any) {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(raw, dest); err != nil {
		t.Fatalf("%s: %v", path, err)
	}
}

func repoRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("caller")
	}
	return filepath.Clean(filepath.Join(filepath.Dir(file), "../../.."))
}

// TestV20Pack measures the published spec/conformance/v2.0 files through
// PackRecord. Issue #228 points that function at the section 14 record.
func TestV20Pack(t *testing.T) {
	root := filepath.Join(repoRoot(t), "spec", "conformance", "v2.0")
	info, err := os.Stat(root)
	if err != nil || !info.IsDir() {
		t.Fatal("spec/conformance/v2.0 is not in this checkout")
	}
	var manifest struct {
		Pack  string   `json:"pack"`
		Files []string `json:"files"`
		IDs   []string `json:"ids"`
	}
	mustJSON(t, filepath.Join(root, "manifest.json"), &manifest)
	if manifest.Pack != "satoshi-hike-v2.0" {
		t.Fatalf("pack %s", manifest.Pack)
	}
	wantFiles := []string{"constants.json", "checkpoint-preimage.json", "anchor-window.json", "anchor-w16-i17.json", "earlier-run-b.json", "bind-threads.json", "test-3leg.json", "kat-small.json"}
	if strings.Join(manifest.Files, ",") != strings.Join(wantFiles, ",") {
		t.Fatalf("files %v", manifest.Files)
	}

	t.Run("constants", func(t *testing.T) {
		var doc struct {
			Sample      int      `json:"SAMPLE_BYTES"`
			Modulus     uint64   `json:"OFFSET_MODULUS"`
			Steps       int      `json:"STEPS_PER_LEG"`
			Preimage    int      `json:"CHECKPOINT_PREIMAGE_LEN"`
			Kind        string   `json:"KIND"`
			Earlier     []string `json:"EARLIER_KINDS"`
			Offsets     []int    `json:"ANCHOR_WINDOW_OFFSETS"`
			BindThreads int      `json:"DEFAULT_BIND_THREADS"`
			BindNote    string   `json:"DEFAULT_BIND_THREADS_NOTE"`
			Floor       struct {
				Strict  struct{ P, Q uint64 } `json:"strict"`
				Default struct{ P, Q uint64 } `json:"default"`
				Lenient struct{ P, Q uint64 } `json:"lenient"`
			} `json:"LEG_RATE_FLOOR"`
			FloorNote string `json:"LEG_RATE_FLOOR_NOTE"`
		}
		mustJSON(t, filepath.Join(root, "constants.json"), &doc)
		if doc.Sample != hike.Production.SampleBytes || doc.Steps != hike.Production.StepsPerLeg {
			t.Fatalf("sizes %d %d", doc.Sample, doc.Steps)
		}
		if doc.Modulus != hike.Production.OffsetModulus || doc.Preimage != 1096 {
			t.Fatalf("modulus %d preimage %d", doc.Modulus, doc.Preimage)
		}
		if doc.Kind != hike.KindV20 || doc.Kind == hike.Kind || len(doc.Earlier) != 1 || doc.Earlier[0] != hike.KindV135 {
			t.Fatalf("historical kind %s %v", doc.Kind, doc.Earlier)
		}
		offs := hike.AnchorOffsetsAfterPost()
		if len(doc.Offsets) != len(offs) || doc.BindThreads != hike.DefaultBindThreads {
			t.Fatalf("offsets %v threads %d", doc.Offsets, doc.BindThreads)
		}
		for i := range offs {
			if doc.Offsets[i] != offs[i] {
				t.Fatalf("offset %v", doc.Offsets)
			}
		}
		if doc.Steps == doc.Sample {
			t.Fatal("steps and sample collapsed")
		}
		// The section 15.5 examples are pack fixtures; the SDK carries no floor (rc.2).
		if doc.Floor.Strict.P != 7 || doc.Floor.Strict.Q != 20 ||
			doc.Floor.Default.P != 1 || doc.Floor.Default.Q != 4 ||
			doc.Floor.Lenient.P != 1 || doc.Floor.Lenient.Q != 8 {
			t.Fatalf("floors %+v", doc.Floor)
		}
		if doc.BindNote != "non-normative reference default" || doc.FloorNote != "Informative example values from §15.5 (Part B). Not a conformance target; policy sets the value." {
			t.Fatalf("notes %q %q", doc.BindNote, doc.FloorNote)
		}
	})

	// #302: every SPEC section 6 name has a value in constants.json that matches this port.
	t.Run("constants-section6", func(t *testing.T) {
		var raw map[string]any
		mustJSON(t, filepath.Join(root, "constants.json"), &raw)
		p := hike.Production
		hx := func(v uint64) string { return "0x" + strings.ToUpper(strconv.FormatUint(v, 16)) }
		hx16 := func(v uint64) string { return fmt.Sprintf("0x%016X", v) }
		want := map[string]any{
			"SLOT_BYTES": float64(p.SlotBytes), "WORDS_PER_SLOT": float64(p.WordsPerSlot),
			"TERRAIN_BYTES": float64(p.TerrainBytes), "N_SLOTS": float64(p.NSlots),
			"SAMPLE_BYTES": float64(p.SampleBytes), "OFFSET_MODULUS": float64(p.OffsetModulus),
			"STEPS_PER_LEG": float64(p.StepsPerLeg), "STAGES": float64(hike.Stages),
			"HASH_BYTES": float64(hike.HashBytes), "LEG_RESULT_BYTES": float64(hike.LegResultBytes),
			"PUBLIC_KEY_BYTES": float64(hike.PublicKeyBytes), "BEACON_BYTES": float64(hike.BeaconBytes),
			"SEAL_DOMAIN": hex.EncodeToString(hike.SealDomain),
			"KIND":        hike.KindV20, "FILL": hike.Fill,
			"MIX_ADD": hx16(hike.MixAdd), "MIX_MUL1": hx16(hike.MixMul1), "MIX_MUL2": hx16(hike.MixMul2),
			"WORD_XOR": hx(hike.WordXOR), "PARENT_MUL": hx16(hike.ParentMul), "SLOT_MUL": hx16(hike.SlotMul),
		}
		if len(want) != 21 {
			t.Fatalf("historical section 6 table has %d", len(want))
		}
		for k, v := range want {
			if raw[k] != v {
				t.Errorf("%s = %v, want %v", k, raw[k], v)
			}
		}
		// P6.14 is retired. The historical v2.0 file still spells the deleted domain.
		if raw["SEED_DOMAIN"] != "5053454544363400" {
			t.Fatalf("historical SEED_DOMAIN %v", raw["SEED_DOMAIN"])
		}
	})

	t.Run("checkpoint-preimage", func(t *testing.T) {
		var doc struct {
			Len  int    `json:"preimage_len"`
			Hash string `json:"sha256"`
			Hex  string `json:"preimage_hex"`
		}
		mustJSON(t, filepath.Join(root, "checkpoint-preimage.json"), &doc)
		previous := bytesRepeat(0x11, 32)
		limb := bytesRepeat(0x22, 32)
		sample := make([]byte, hike.Production.SampleBytes)
		sample[0] = 0x33
		sample[len(sample)-1] = 0x44
		pre := append(append(append(append([]byte{}, previous...), limb...), sample...), hike.Le64(7)...)
		if len(pre) != doc.Len || doc.Len != 1096 {
			t.Fatalf("len %d %d", len(pre), doc.Len)
		}
		sum := sha256.Sum256(pre)
		if hex.EncodeToString(sum[:]) != doc.Hash {
			t.Fatalf("sha256 %x", sum)
		}
		if hex.EncodeToString(pre) != doc.Hex {
			t.Fatal("preimage_hex")
		}
	})

	t.Run("anchor-window", func(t *testing.T) {
		var doc struct {
			Cases struct {
				Inside  string `json:"inside_the_window"`
				Outside string `json:"outside_the_window"`
				Missing string `json:"header_not_in_the_object"`
				None    string `json:"no_path"`
			} `json:"cases"`
			Reads  []uint32 `json:"index_reads"`
			Object string   `json:"object_hex"`
			Plants []struct {
				Height uint32 `json:"height"`
				Body   string `json:"body"`
			} `json:"plant_reads"`
			Record string `json:"record"`
		}
		mustJSON(t, filepath.Join(root, "anchor-window.json"), &doc)
		object := hike.QuietV2()
		object.Kind = hike.KindV20
		for i := range object.Stages[3].Beacon {
			object.Stages[3].Beacon[i] = 0x55
		}
		object.Stages[1].Path = hike.EncodePathInBlock(object.Stages[1].Waypoint[:], 4)
		object.Stages[2].Path = hike.EncodePathInBlock(object.Stages[2].Waypoint[:], 100)
		object.Stages[4].Path = hike.EncodePathInBlock(object.Stages[4].Waypoint[:], 7)
		raw := object.Encode()
		if len(doc.Plants) != 1 || doc.Plants[0].Height != 4 {
			t.Fatalf("plants %+v", doc.Plants)
		}
		requirePlantBody(t, doc.Plants[0].Body, pathRoot(object.Stages[1].Waypoint[:]), 4, false)
		var reads []uint32
		line := hike.PackRecord(raw, hike.Production, hike.Index{}, func(loc hike.PathLocators) error {
			reads = append(reads, loc.BlockHeight)
			return nil
		}, 1, nil)
		if len(reads) != 0 || !strings.Contains(line, `"version":"earlier version"`) {
			t.Fatalf("earlier reads %v %s", reads, line)
		}
	})

	t.Run("anchor-w16-i17", func(t *testing.T) {
		var doc struct {
			Case   string   `json:"case"`
			Reads  []uint32 `json:"index_reads"`
			Object string   `json:"object_hex"`
			Plants []struct {
				Height uint32 `json:"height"`
				Body   string `json:"body"`
			} `json:"plant_reads"`
			Record string `json:"record"`
		}
		mustJSON(t, filepath.Join(root, "anchor-w16-i17.json"), &doc)
		if doc.Case != "W_16 included in I_17" {
			t.Fatalf("case %s", doc.Case)
		}
		object := hike.QuietV2()
		object.Kind = hike.KindV20
		object.Last = bytesRepeat(0x55, 32)
		height := uint64(19)
		object.LastHeight = &height
		object.Stages[16].Path = hike.EncodePathInBlock(object.Stages[16].Waypoint[:], 19)
		raw := object.Encode()
		if len(doc.Plants) != 1 || doc.Plants[0].Height != 19 {
			t.Fatalf("plants %+v", doc.Plants)
		}
		requirePlantBody(t, doc.Plants[0].Body, pathRoot(object.Stages[16].Waypoint[:]), 19, false)
		var reads []uint32
		line := hike.PackRecord(raw, hike.Production, hike.Index{}, func(loc hike.PathLocators) error {
			reads = append(reads, loc.BlockHeight)
			return nil
		}, 1, nil)
		if len(reads) != 0 || !strings.Contains(line, `"version":"earlier version"`) {
			t.Fatalf("earlier reads %v %s", reads, line)
		}
		var parsed struct {
			Anchors []string `json:"anchors"`
		}
		if err := json.Unmarshal([]byte(doc.Record), &parsed); err != nil {
			t.Fatal(err)
		}
		if len(parsed.Anchors) != 16 || parsed.Anchors[15] != "anchored" {
			t.Fatalf("anchors %v", parsed.Anchors)
		}
		for i := 0; i < 15; i++ {
			if parsed.Anchors[i] != "not anchored" {
				t.Fatalf("anchors %v", parsed.Anchors)
			}
		}
	})

	t.Run("earlier-run-b", func(t *testing.T) {
		var doc struct {
			Object    string `json:"object"`
			ObjectHex string `json:"object_hex"`
			SHA       string `json:"sha256"`
			Record    string `json:"record"`
		}
		mustJSON(t, filepath.Join(root, "earlier-run-b.json"), &doc)
		body, err := os.ReadFile(filepath.Join(repoRoot(t), doc.Object))
		if err != nil {
			t.Fatal(err)
		}
		sum := sha256.Sum256(body)
		if hex.EncodeToString(sum[:]) != doc.SHA || hex.EncodeToString(body) != doc.ObjectHex {
			t.Fatalf("sha256 %x", sum)
		}
		called := false
		line := hike.PackRecord(body, hike.Production, hike.Index{Fetch: func(uint64) (string, bool) {
			called = true
			return "", false
		}}, func(hike.PathLocators) error {
			called = true
			return nil
		}, hike.DefaultBindThreads, nil)
		if called || line != doc.Record {
			t.Fatalf("record %q called %v", line, called)
		}
	})

	t.Run("bind-threads", func(t *testing.T) {
		var doc struct {
			Threads []int  `json:"threads"`
			Object  string `json:"object_hex"`
			Record  string `json:"record"`
		}
		mustJSON(t, filepath.Join(root, "bind-threads.json"), &doc)
		if len(doc.Threads) != 2 || doc.Threads[0] != 1 || doc.Threads[1] != hike.DefaultBindThreads {
			t.Fatalf("threads %v", doc.Threads)
		}
		object := hike.QuietV2()
		object.Kind = hike.KindV20
		object.Stages[3].LegCount = 4
		object.Stages[3].Checkpoints = make([][32]byte, 4)
		object.Stages[3].LegResults = make([][8]byte, 4)
		for j := 0; j < 4; j++ {
			for i := range object.Stages[3].Checkpoints[j] {
				object.Stages[3].Checkpoints[j][i] = byte(j)
			}
			object.Stages[3].LegResults[j] = [8]byte{byte(j)}
		}
		raw := object.Encode()
		one := hike.PackRecord(raw, hike.Production, hike.Index{}, nil, doc.Threads[0], nil)
		many := hike.PackRecord(raw, hike.Production, hike.Index{}, nil, doc.Threads[1], nil)
		if one != many {
			t.Fatal("record depended on the pool size")
		}
	})

	t.Run("test-3leg", func(t *testing.T) {
		var doc struct {
			Label    string `json:"label"`
			Legs     []int  `json:"legs_per_stage"`
			DrawHex  string `json:"draw_bytes_hex"`
			Consumed bool   `json:"draw_consumed"`
			Object   string `json:"object_hex"`
			SHA      string `json:"sha256"`
			Record   string `json:"record"`
		}
		mustJSON(t, filepath.Join(root, "test-3leg.json"), &doc)
		if doc.Consumed || len(doc.Legs) != 3 || doc.Legs[0] != 1 || doc.Legs[1] != 1 || doc.Legs[2] != 1 {
			t.Fatalf("fixture legs %v consumed %v", doc.Legs, doc.Consumed)
		}
		object, err := hex.DecodeString(doc.Object)
		if err != nil {
			t.Fatal(err)
		}
		sum := sha256.Sum256(object)
		if hex.EncodeToString(sum[:]) != doc.SHA {
			t.Fatalf("sha256 %x", sum)
		}
		draw, err := hex.DecodeString(doc.DrawHex)
		if err != nil {
			t.Fatal(err)
		}
		if len(draw) != 48 {
			t.Fatalf("draw %d", len(draw))
		}
		// Kind satoshi-hike-v2.0-object is earlier. Rule A5 records it before
		// any terrain fill. It is not the v2.1 section 14 record.
		line := hike.PackRecord(object, hike.ConformanceSmallV2, hike.Index{}, nil, 1, draw)
		if !strings.Contains(line, `"version":"earlier version"`) || !strings.Contains(line, `"kind":"satoshi-hike-v2.0-object"`) {
			t.Fatalf("earlier %s", line)
		}
		if line == doc.Record {
			t.Fatal("matched the withdrawn v2.0 record")
		}
	})

	t.Run("kat-small", func(t *testing.T) {
		var doc struct {
			Profile  string `json:"profile"`
			Sample   int    `json:"sample_bytes"`
			Slot     int    `json:"slot_bytes"`
			TerrainN int    `json:"terrain_bytes"`
			Steps    int    `json:"steps_per_leg"`
			Pub      string `json:"public_key_hex"`
			Beacon   string `json:"seed_beacon_hex"`
			Terrain  string `json:"terrain_hex"`
			Leg      struct {
				Previous   string `json:"previous_hex"`
				Limb       string `json:"limb_hex"`
				Sample     string `json:"sample_hex"`
				LegResult  string `json:"leg_result_hex"`
				Checkpoint string `json:"checkpoint_hex"`
			} `json:"leg"`
			W1 struct {
				Interrupt  string `json:"interrupt_hex"`
				Limb       string `json:"limb_hex"`
				Checkpoint string `json:"checkpoint_hex"`
				Digest     string `json:"digest_hex"`
			} `json:"waypoint_w1"`
			Object string `json:"object_hex"`
			SHA    string `json:"sha256"`
			Draw   string `json:"draw_bytes_hex"`
			Docs   []struct {
				Height uint64 `json:"height"`
				Body   string `json:"body"`
			} `json:"documents"`
			Reads []uint32 `json:"index_reads"`
			Book  struct {
				A      int `json:"a"`
				B      int `json:"b"`
				N      int `json:"N"`
				S      int `json:"S"`
				DeltaH int `json:"delta_h"`
			} `json:"bookends"`
			Six struct {
				Indices []uint64 `json:"indices"`
				Results []string `json:"results"`
			} `json:"six_leg"`
			Record string `json:"record"`
		}
		mustJSON(t, filepath.Join(root, "kat-small.json"), &doc)
		if doc.Profile != hike.ConformanceSmallV2.Name || doc.Sample != 32 || doc.Slot != 64 || doc.TerrainN != 256 || doc.Steps != 32 {
			t.Fatalf("profile %+v", doc)
		}
		if doc.Book.A != 1 || doc.Book.B != 2 || doc.Book.N != 1 || doc.Book.S != 3 || doc.Book.DeltaH != 3 {
			t.Fatalf("bookends %+v", doc.Book)
		}
		pub := mustHex(t, doc.Pub)
		beacon := mustHex(t, doc.Beacon)
		terrain := hike.FillTerrain(pub, beacon, hike.ConformanceSmallV2)
		if hex.EncodeToString(terrain.Blob) == doc.Terrain {
			t.Fatal("NL4 terrain matched the withdrawn v2.0 terrain")
		}
		draw := mustHex(t, doc.Draw)
		if len(draw) != 256 {
			t.Fatalf("draw %d", len(draw))
		}
		indices, token := hike.DrawSix(draw, 18)
		if token != "" || len(indices) != len(doc.Six.Indices) || len(doc.Six.Results) != 6 {
			t.Fatalf("draw %s %v", token, indices)
		}
		for i := range indices {
			if indices[i] != doc.Six.Indices[i] || doc.Six.Results[i] != "match" {
				t.Fatalf("six %v", indices)
			}
		}
		object := mustHex(t, doc.Object)
		sum := sha256.Sum256(object)
		if hex.EncodeToString(sum[:]) != doc.SHA {
			t.Fatal("sha256")
		}
		docs := map[uint64]string{}
		for _, item := range doc.Docs {
			docs[item.Height] = item.Body
		}
		if len(docs) != 19 {
			t.Fatalf("documents %d", len(docs))
		}
		var reads []uint32
		line := hike.PackRecord(object, hike.ConformanceSmallV2, hike.Index{Documents: docs}, func(loc hike.PathLocators) error {
			reads = append(reads, loc.BlockHeight)
			return nil
		}, 1, draw)
		if !strings.Contains(line, `"version":"earlier version"`) || !strings.Contains(line, `"kind":"satoshi-hike-v2.0-object"`) {
			t.Fatalf("earlier %s", line)
		}
		if line == doc.Record {
			t.Fatal("matched the withdrawn v2.0 verify record")
		}
		if len(reads) != 0 {
			t.Fatalf("reads %v", reads)
		}
	})
}

func mustHex(t *testing.T, spelling string) []byte {
	t.Helper()
	raw, err := hex.DecodeString(spelling)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func pathRoot(waypoint []byte) string {
	sum := sha256.Sum256(append([]byte{0x1a}, waypoint...))
	return hex.EncodeToString(sum[:])
}

func requirePlantBody(t *testing.T, body, root string, height uint32, header bool) {
	t.Helper()
	var doc struct {
		Network string          `json:"network"`
		Header  json.RawMessage `json:"header"`
		Plant   *issuedPlant    `json:"plant"`
		Plants  []issuedPlant   `json:"plants"`
	}
	if err := json.Unmarshal([]byte(body), &doc); err != nil {
		t.Fatal(err)
	}
	if doc.Network != "main" && doc.Network != "synthetic" {
		t.Fatalf("network %s", doc.Network)
	}
	if header != (len(doc.Header) > 0) {
		t.Fatalf("header %t %d", header, len(doc.Header))
	}
	ok := false
	if doc.Plant != nil && doc.Plant.matches(root, height) {
		ok = true
	}
	for i := range doc.Plants {
		if doc.Plants[i].matches(root, height) {
			ok = true
		}
	}
	if !ok {
		t.Fatal("plant")
	}
}

type issuedPlant struct {
	Root        string `json:"root"`
	Vout        uint32 `json:"vout"`
	BlockHeight uint32 `json:"block_height"`
}

func (p issuedPlant) matches(root string, height uint32) bool {
	return p.Root == root && p.Vout == 2 && p.BlockHeight == height
}

func bytesRepeat(b byte, n int) []byte {
	out := make([]byte, n)
	for i := range out {
		out[i] = b
	}
	return out
}
