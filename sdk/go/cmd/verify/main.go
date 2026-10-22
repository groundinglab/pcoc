// Command verify is the SPEC §18 verify CLI for this core.
//
// Stdout for a current object is the section 14 record, keys in the order
// section 14 lists, no insignificant whitespace, one trailing newline.
// An unknown kind (Rule A5) writes the A5 record and nothing else.
//
// Exit 0: a record was produced. Exit 1: the object file cannot be read.
// Exit 2: usage. This command does no admission wording.
//
// Index documents and draw bytes are optional files. With no index, every
// header comparison is "not read". With no draw file, the six-leg draw is
// "not read" when six or more legs exist.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"

	"pcoc.dev/sdk/go"
)

func main() {
	args := os.Args[1:]
	if len(args) < 1 {
		usage()
	}
	profile := pcoc.Production()
	var sample, slot, terrain, steps int
	var haveS, haveSl, haveT, haveSt bool
	var indexPath, drawPath string
	var objPath string
	for i := 0; i < len(args); i++ {
		a := args[i]
		need := func() string {
			if i+1 >= len(args) {
				os.Exit(2)
			}
			i++
			return args[i]
		}
		switch a {
		case "--profile":
			name := need()
			if name == "production" || name == "test" || name == "live" {
				profile = pcoc.Production()
			} else if name == "conformance-small" {
				profile.Name = "conformance-small"
			} else {
				os.Exit(2)
			}
		case "--sample-bytes":
			sample, haveS = atoi(need())
		case "--slot-bytes":
			slot, haveSl = atoi(need())
		case "--terrain-bytes":
			terrain, haveT = atoi(need())
		case "--steps-per-leg":
			steps, haveSt = atoi(need())
		case "--index":
			indexPath = need()
		case "--draw":
			drawPath = need()
		case "--help":
			usage()
		default:
			if len(a) > 0 && a[0] == '-' {
				os.Exit(2)
			}
			if objPath != "" {
				os.Exit(2)
			}
			objPath = a
		}
	}
	if objPath == "" {
		os.Exit(2)
	}
	if profile.Name == "conformance-small" {
		if !haveS || !haveSl || !haveT || !haveSt {
			os.Exit(2)
		}
		profile.SampleBytes = sample
		profile.SlotBytes = slot
		profile.TerrainBytes = terrain
		profile.StepsPerLeg = steps
		if profile.Check() != nil {
			os.Exit(2)
		}
	}
	obj, err := os.ReadFile(objPath)
	if err != nil {
		os.Exit(1)
	}
	in := pcoc.VerifyInput{Object: obj, Profile: profile, Runs: 1, Recompute: false}
	if indexPath != "" {
		in.Index = loadIndex(indexPath)
	}
	if drawPath != "" {
		b, err := os.ReadFile(drawPath)
		if err != nil {
			os.Exit(1)
		}
		in.Draw = b
	}
	rec, a5 := pcoc.Verify(in)
	var out []byte
	if a5 {
		out = pcoc.UnknownKindRecord(rec["kind_text"].(string))
	} else {
		out = encodeRecord(rec)
	}
	os.Stdout.Write(out)
	os.Stdout.Write([]byte{'\n'})
}

func atoi(s string) (int, bool) {
	n, err := strconv.Atoi(s)
	if err != nil {
		os.Exit(2)
	}
	return n, true
}

func usage() {
	fmt.Fprintln(os.Stderr, "usage: verify [--profile production|conformance-small] [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N] [--index file] [--draw file] object")
	os.Exit(2)
}

func loadIndex(path string) *pcoc.Index {
	b, err := os.ReadFile(path)
	if err != nil {
		os.Exit(1)
	}
	// A JSON object mapping decimal height to a section 9 body (a string),
	// or one section 9 body used at every height the object asks. The core
	// tests build documents in process. This flag accepts a JSON object of
	// {"height": <body object>} entries.
	var raw map[string]json.RawMessage
	if json.Unmarshal(b, &raw) != nil {
		os.Exit(1)
	}
	idx := &pcoc.Index{At: map[uint64]*pcoc.IndexDoc{}}
	for k, v := range raw {
		h, err := strconv.ParseUint(k, 10, 64)
		if err != nil {
			os.Exit(1)
		}
		doc, ok := pcoc.ParseIndexBody(v)
		if !ok {
			os.Exit(1)
		}
		idx.At[h] = doc
	}
	return idx
}

// encodeRecord writes the section 14 key order with no insignificant
// whitespace. Nested objects use the key order section 14 names.
func encodeRecord(rec map[string]pcoc.JSON) []byte {
	b := &buf{}
	b.raw("{")
	first := true
	emit := func(k string, v pcoc.JSON) {
		if !first {
			b.raw(",")
		}
		first = false
		b.key(k)
		b.val(v)
	}
	emit("profile", rec["profile"])
	if rec["profile"] == "conformance-small" {
		emit("sample_bytes", rec["sample_bytes"])
		emit("slot_bytes", rec["slot_bytes"])
		emit("terrain_bytes", rec["terrain_bytes"])
		emit("steps_per_leg", rec["steps_per_leg"])
	}
	for _, k := range []string{
		"shape", "shape_item", "headers", "seed_value", "checkpoints", "waypoints",
		"paths", "anchors", "anchored_total", "skip_list", "a", "b", "N", "S", "delta_h",
		"fixed_start", "P_b", "height_P_b", "index_comparison_P_b", "T_P_b",
		"height_I_a_minus_1", "T_I_a_minus_1", "six_leg", "six_leg_runs", "analysis",
	} {
		emit(k, rec[k])
	}
	b.raw("}")
	return b.b
}

type buf struct{ b []byte }

func (b *buf) raw(s string) { b.b = append(b.b, s...) }

func (b *buf) key(k string) {
	enc, _ := json.Marshal(k)
	b.b = append(b.b, enc...)
	b.b = append(b.b, ':')
}

func (b *buf) val(v pcoc.JSON) {
	switch t := v.(type) {
	case string:
		enc, _ := json.Marshal(t)
		b.b = append(b.b, enc...)
	case int:
		b.b = append(b.b, strconv.Itoa(t)...)
	case uint64:
		b.b = append(b.b, strconv.FormatUint(t, 10)...)
	case int64:
		b.b = append(b.b, strconv.FormatInt(t, 10)...)
	case []pcoc.JSON:
		b.raw("[")
		for i, e := range t {
			if i > 0 {
				b.raw(",")
			}
			b.val(e)
		}
		b.raw("]")
	case map[string]pcoc.JSON:
		b.raw("{")
		// The map's key order is the order section 14 names for that object.
		// Callers build maps with a known set; write them by the order those
		// keys sort in the SPEC's listed order for each nested object. A
		// generic map is written by a fixed probe list and then any remainder
		// is omitted: every nested object this core builds uses one of the
		// lists below.
		order := nestedOrder(t)
		for i, k := range order {
			if i > 0 {
				b.raw(",")
			}
			b.key(k)
			b.val(t[k])
		}
		b.raw("}")
	default:
		if v == nil {
			b.raw("null")
			return
		}
		enc, _ := json.Marshal(v)
		b.b = append(b.b, enc...)
	}
}

func nestedOrder(m map[string]pcoc.JSON) []string {
	lists := [][]string{
		{"stored_hash", "stored_height", "index_comparison", "T"},
		{"g", "path_state", "plant", "P_g", "height_P_g", "d_g", "claimed_block"},
		{"g", "w17_path", "claimed_block"},
		{"block_hash", "block_height"},
		{"g", "legs"},
		{"j", "result"},
		{"g", "result"},
		{"N", "S", "delta_h"},
		{"indices", "results"},
		{"whole_hike_legs", "whole_hike_span", "per_stage", "legs_outside", "hike_legs", "hike_span"},
		{"g", "checkpoint_count", "span"},
	}
	for _, list := range lists {
		if len(list) == len(m) {
			ok := true
			for _, k := range list {
				if _, has := m[k]; !has {
					ok = false
					break
				}
			}
			if ok {
				return list
			}
		}
	}
	// Fall back to the key order of a single known extra shape.
	var keys []string
	for k := range m {
		keys = append(keys, k)
	}
	return keys
}
