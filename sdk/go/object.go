package pcoc

// Hike object encode and decode. SPEC §7.3 and §8 Rule A3.
//
// The only kind is v2.2-pcoc-object. Encode emits one CBOR map in the core
// deterministic encoding. Decode classifies the input in the order SPEC §7.3
// states:
//
//  1. Input that is not one definite-length CBOR data item is a malformed
//     CBOR value. Rule A5 is not applied.
//  2. One well-formed data item that is not a map, or a map with no kind key
//     whose value is a text string, is a decode failure. There is no
//     section 14 record.
//  3. A map whose kind text is not exactly KIND is Rule A5.
//  4. Every other input is measured by section 14.

import (
	"bytes"
	"encoding/json"
)

// Stage is one stage map (SPEC §8 Rule A3.2).
// HasPath is false on stage 0, which omits the key. On stages 1–17 Path is
// the path bytes, which may be empty.
type Stage struct {
	Beacon       []byte
	BeaconHeight uint64
	HasBeacon    bool
	HasHeight    bool
	HeightAbove  bool
	LegCount     uint64
	HasLegCount  bool
	Waypoint     []byte
	HasWaypoint  bool
	Path         []byte
	HasPath      bool
	Checkpoints  [][]byte
	LegResults   [][]byte
	HasCP        bool
	HasLR        bool
	// Present is false when this stage map is absent from the stages array.
	Present bool
	// Extra is true when the stage map has a key section 8 does not list
	// for that stage, other than a stage-0 path key (its own shape string).
	Extra bool
}

// Object is the hike object (SPEC §8).
type Object struct {
	Kind                   string
	HasKind                bool
	Fill                   string
	HasFill                bool
	SlotBytes              uint64
	HasSlotBytes           bool
	NSlots                 uint64
	HasNSlots              bool
	SampleBytes            uint64
	HasSampleBytes         bool
	StepsPerLeg            uint64
	HasSteps               bool
	PublicKey              []byte
	HasPublicKey           bool
	SeedBeacon             []byte
	HasSeedBeacon          bool
	Stages                 []Stage
	HasStages              bool
	StagesLenOK            bool
	LastInterrupt          []byte
	HasLastInterrupt       bool
	LastInterruptHeight    uint64
	HasLastInterruptHeight bool
	LastHeightAbove        bool
	// ExtraKeys is true when a present map has a key outside section 8.
	ExtraKeys bool
	// BadTypes is true when a present field has a value type section 8
	// does not list.
	BadTypes bool
}

// UnknownKind is the Rule A5 record. It is not a decode failure and it is
// not a shape mismatch (SPEC §14.7).
type UnknownKind struct {
	Kind     string `json:"kind"`
	KindText string `json:"kind_text"`
}

// UnknownKindRecord is the Rule A5 JSON object, compact, keys kind then
// kind_text (SPEC §14.7). The CLI adds one trailing newline; these bytes
// do not include it.
func UnknownKindRecord(kindText string) []byte {
	var buf bytes.Buffer
	buf.WriteString(`{"kind":"unknown kind","kind_text":`)
	enc, _ := json.Marshal(kindText)
	buf.Write(enc)
	buf.WriteByte('}')
	return buf.Bytes()
}

// DecodeClass is the SPEC §7.3 classification of one input.
type DecodeClass int

const (
	// ClassMalformed is not one definite-length CBOR data item.
	// shape is mismatch and shape_item is exactly ["malformed cbor"].
	ClassMalformed DecodeClass = iota
	// ClassDecodeFailure is one well-formed data item that is not a map,
	// or a map with no kind text. There is no section 14 record.
	ClassDecodeFailure
	// ClassUnknownKind is Rule A5.
	ClassUnknownKind
	// ClassObject is measured by section 14.
	ClassObject
)

// DecodeResult is one decode of object bytes (SPEC §7.3, §14.7).
type DecodeResult struct {
	Class   DecodeClass
	Unknown *UnknownKind
	Object  *Object
}

// DecodeObject classifies b under SPEC §7.3 and, when kind is KIND, reads
// the section 8 fields that are present. A field of the wrong type is kept
// as absent so section 14 records the shape condition. Extra keys are noted.
func DecodeObject(b []byte) DecodeResult {
	v, rest, err := decodeOne(b)
	if err != nil || len(rest) != 0 {
		return DecodeResult{Class: ClassMalformed}
	}
	if v.typ != cborMap {
		return DecodeResult{Class: ClassDecodeFailure}
	}
	kindV, ok := v.lookup("kind")
	if !ok || kindV.typ != cborText {
		return DecodeResult{Class: ClassDecodeFailure}
	}
	kind := string(kindV.bytes)
	if kind != Kind {
		return DecodeResult{Class: ClassUnknownKind, Unknown: &UnknownKind{Kind: "unknown kind", KindText: kind}}
	}
	return DecodeResult{Class: ClassObject, Object: readObject(v)}
}

func readObject(v cborValue) *Object {
	o := &Object{Kind: Kind, HasKind: true}
	allowed := map[string]struct{}{
		"fill": {}, "kind": {}, "stages": {}, "n_slots": {}, "public_key": {},
		"slot_bytes": {}, "seed_beacon": {}, "sample_bytes": {}, "steps_per_leg": {},
		"last_interrupt": {}, "last_interrupt_height": {},
	}
	for _, p := range v.pairs {
		if _, ok := allowed[p.key]; !ok {
			o.ExtraKeys = true
		}
	}
	if x, ok := v.lookup("fill"); ok {
		if x.typ == cborText {
			o.Fill = string(x.bytes)
			o.HasFill = true
		} else {
			o.BadTypes = true
		}
	}
	readUint(v, "slot_bytes", &o.SlotBytes, &o.HasSlotBytes, &o.BadTypes)
	readUint(v, "n_slots", &o.NSlots, &o.HasNSlots, &o.BadTypes)
	readUint(v, "sample_bytes", &o.SampleBytes, &o.HasSampleBytes, &o.BadTypes)
	readUint(v, "steps_per_leg", &o.StepsPerLeg, &o.HasSteps, &o.BadTypes)
	if x, ok := v.lookup("public_key"); ok {
		if x.typ == cborBytes {
			o.PublicKey = x.bytes
			o.HasPublicKey = true
		} else {
			o.BadTypes = true
		}
	}
	if x, ok := v.lookup("seed_beacon"); ok {
		if x.typ == cborBytes {
			o.SeedBeacon = x.bytes
			o.HasSeedBeacon = true
		} else {
			o.BadTypes = true
		}
	}
	if x, ok := v.lookup("last_interrupt"); ok {
		if x.typ == cborBytes {
			o.LastInterrupt = x.bytes
			o.HasLastInterrupt = true
		} else {
			o.BadTypes = true
		}
	}
	if x, ok := v.lookup("last_interrupt_height"); ok {
		if x.typ == cborUint {
			o.HasLastInterruptHeight = true
			if x.uint > MaxJSONInt {
				o.LastHeightAbove = true
			} else {
				o.LastInterruptHeight = x.uint
			}
		} else {
			o.BadTypes = true
		}
	}
	if x, ok := v.lookup("stages"); ok {
		if x.typ == cborArray {
			o.HasStages = true
			o.StagesLenOK = len(x.array) == Stages
			n := len(x.array)
			if n > Stages {
				n = Stages
			}
			o.Stages = make([]Stage, Stages)
			for g := 0; g < n; g++ {
				if x.array[g].typ != cborMap {
					o.BadTypes = true
					continue
				}
				o.Stages[g] = readStage(x.array[g], g == 0)
			}
		} else {
			o.BadTypes = true
		}
	}
	return o
}

func readStage(v cborValue, stage0 bool) Stage {
	st := Stage{Present: true, Extra: stageExtra(v, stage0)}
	if x, ok := v.lookup("path"); ok {
		st.HasPath = true
		if x.typ == cborBytes {
			st.Path = x.bytes
		}
	}

	if x, ok := v.lookup("beacon"); ok {
		if x.typ == cborBytes {
			st.Beacon = x.bytes
			st.HasBeacon = true
		}
	}
	if x, ok := v.lookup("beacon_height"); ok {
		if x.typ == cborUint {
			st.HasHeight = true
			if x.uint > MaxJSONInt {
				st.HeightAbove = true
			} else {
				st.BeaconHeight = x.uint
			}
		}
	}
	if x, ok := v.lookup("leg_count"); ok {
		if x.typ == cborUint {
			st.LegCount = x.uint
			st.HasLegCount = true
		}
	}
	if x, ok := v.lookup("waypoint"); ok {
		if x.typ == cborBytes {
			st.Waypoint = x.bytes
			st.HasWaypoint = true
		}
	}
	if x, ok := v.lookup("checkpoints"); ok && x.typ == cborArray {
		st.HasCP = true
		st.Checkpoints = byteArray(x)
	}
	if x, ok := v.lookup("leg_results"); ok && x.typ == cborArray {
		st.HasLR = true
		st.LegResults = byteArray(x)
	}
	return st
}

// stageExtra reports a key the stage map is not allowed to carry.
// A path key on stage 0 is the stage-0 path key condition, not this one.
func stageExtra(v cborValue, stage0 bool) bool {
	allowed := map[string]struct{}{
		"beacon": {}, "waypoint": {}, "leg_count": {}, "checkpoints": {},
		"leg_results": {}, "beacon_height": {},
	}
	if !stage0 {
		allowed["path"] = struct{}{}
	}
	for _, p := range v.pairs {
		if _, ok := allowed[p.key]; ok {
			continue
		}
		if stage0 && p.key == "path" {
			continue
		}
		return true
	}
	return false
}

func byteArray(v cborValue) [][]byte {
	out := make([][]byte, len(v.array))
	for i, e := range v.array {
		if e.typ == cborBytes {
			out[i] = e.bytes
		}
	}
	return out
}

func readUint(v cborValue, key string, dst *uint64, has *bool, bad *bool) {
	x, ok := v.lookup(key)
	if !ok {
		return
	}
	if x.typ != cborUint {
		*bad = true
		return
	}
	*dst = x.uint
	*has = true
}

// MaxJSONInt is 2^53 − 1 (SPEC §14, §9). An object integer above it is not
// a usable unsigned value on the verify record.
const MaxJSONInt = uint64(1<<53 - 1)

// EncodeObject is the SPEC §7.3 produce encoding of a section 8 object.
// Stage 0 omits path. Stages 1–17 include path, which may be empty.
// last_interrupt and last_interrupt_height are omitted when not set, so a
// shape vector can carry that absence. A producer always sets both.
func EncodeObject(o *Object) []byte {
	stages := make([]cborValue, len(o.Stages))
	for g, st := range o.Stages {
		cps := make([]cborValue, len(st.Checkpoints))
		for i, c := range st.Checkpoints {
			cps[i] = cborBytesV(c)
		}
		lrs := make([]cborValue, len(st.LegResults))
		for i, c := range st.LegResults {
			lrs[i] = cborBytesV(c)
		}
		pairs := []cborPair{
			{key: "beacon", val: cborBytesV(st.Beacon)},
			{key: "beacon_height", val: cborUintV(st.BeaconHeight)},
			{key: "leg_count", val: cborUintV(st.LegCount)},
			{key: "waypoint", val: cborBytesV(st.Waypoint)},
			{key: "checkpoints", val: cborValue{typ: cborArray, array: cps}},
			{key: "leg_results", val: cborValue{typ: cborArray, array: lrs}},
		}
		if g > 0 {
			pairs = append(pairs, cborPair{key: "path", val: cborBytesV(st.Path)})
		}
		stages[g] = cborValue{typ: cborMap, pairs: pairs}
	}
	pairs := []cborPair{
		{key: "kind", val: cborTextV(firstNonEmpty(o.Kind, Kind))},
		{key: "fill", val: cborTextV(firstNonEmpty(o.Fill, FillName))},
		{key: "slot_bytes", val: cborUintV(o.SlotBytes)},
		{key: "n_slots", val: cborUintV(o.NSlots)},
		{key: "sample_bytes", val: cborUintV(o.SampleBytes)},
		{key: "steps_per_leg", val: cborUintV(o.StepsPerLeg)},
		{key: "public_key", val: cborBytesV(o.PublicKey)},
		{key: "seed_beacon", val: cborBytesV(o.SeedBeacon)},
		{key: "stages", val: cborValue{typ: cborArray, array: stages}},
	}
	if o.HasLastInterrupt || len(o.LastInterrupt) > 0 {
		pairs = append(pairs, cborPair{key: "last_interrupt", val: cborBytesV(o.LastInterrupt)})
	}
	if o.HasLastInterruptHeight {
		pairs = append(pairs, cborPair{key: "last_interrupt_height", val: cborUintV(o.LastInterruptHeight)})
	}
	return encodeValue(cborValue{typ: cborMap, pairs: pairs})
}

func firstNonEmpty(a, b string) string {
	if a != "" {
		return a
	}
	return b
}
