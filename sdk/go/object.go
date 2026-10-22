package pcoc

// Hike object encode and decode. SPEC §7.3 and §8 Rule A3.
//
// The only kind is v2.2-pcoc-object. Encode emits one CBOR map in the core
// deterministic encoding. Decode accepts any definite-length encoding of the
// same values. A kind text other than KIND is returned as UnknownKind
// (Rule A5) and sections 14.1–14.6 are not applied.

import (
	"bytes"
	"encoding/json"
)

// Stage is one stage map (SPEC §8 Rule A3.2).
// Path is nil on stage 0, which omits the key. On stages 1–17 it is the
// path bytes, which may be empty.
type Stage struct {
	Beacon       []byte
	BeaconHeight uint64
	LegCount     uint64
	Waypoint     []byte
	Path         []byte
	HasPath      bool
	Checkpoints  [][]byte
	LegResults   [][]byte
}

// Object is the hike object (SPEC §8).
type Object struct {
	Kind                   string
	Fill                   string
	SlotBytes              uint64
	NSlots                 uint64
	SampleBytes            uint64
	StepsPerLeg            uint64
	PublicKey              []byte
	SeedBeacon             []byte
	Stages                 []Stage
	LastInterrupt          []byte
	LastInterruptHeight    uint64
	HasLastInterrupt       bool
	HasLastInterruptHeight bool
}

// UnknownKind is the Rule A5 record. It is not a decode failure and it is
// not a shape mismatch (SPEC §14.7).
type UnknownKind struct {
	Kind     string `json:"kind"`
	KindText string `json:"kind_text"`
}

// UnknownKindRecord is the Rule A5 JSON object, compact, keys kind then
// kind_text. SPEC §14.7.
func UnknownKindRecord(kindText string) []byte {
	// Encoding/json escapes <, >, &. Kind text in this construction is ASCII
	// without those bytes. Build the object directly so the bytes are the
	// record the SPEC writes.
	var buf bytes.Buffer
	buf.WriteString(`{"kind":"unknown kind","kind_text":`)
	enc, _ := json.Marshal(kindText)
	buf.Write(enc)
	buf.WriteByte('}')
	return buf.Bytes()
}

// topLevelKeys is the section 8 field set.
var topLevelKeys = []string{
	"fill", "kind", "stages", "n_slots", "public_key", "slot_bytes",
	"seed_beacon", "sample_bytes", "steps_per_leg", "last_interrupt",
	"last_interrupt_height",
}

func stage0Keys() []string {
	return []string{"beacon", "waypoint", "leg_count", "checkpoints", "leg_results", "beacon_height"}
}

func stageNKeys() []string {
	return []string{"path", "beacon", "waypoint", "leg_count", "checkpoints", "leg_results", "beacon_height"}
}

// EncodeObject is SPEC §7.3 produce encoding of a section 8 object.
// Stage 0 omits path. Stages 1–17 include path.
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
	top := cborValue{typ: cborMap, pairs: []cborPair{
		{key: "kind", val: cborTextV(o.Kind)},
		{key: "fill", val: cborTextV(o.Fill)},
		{key: "slot_bytes", val: cborUintV(o.SlotBytes)},
		{key: "n_slots", val: cborUintV(o.NSlots)},
		{key: "sample_bytes", val: cborUintV(o.SampleBytes)},
		{key: "steps_per_leg", val: cborUintV(o.StepsPerLeg)},
		{key: "public_key", val: cborBytesV(o.PublicKey)},
		{key: "seed_beacon", val: cborBytesV(o.SeedBeacon)},
		{key: "stages", val: cborValue{typ: cborArray, array: stages}},
		{key: "last_interrupt", val: cborBytesV(o.LastInterrupt)},
		{key: "last_interrupt_height", val: cborUintV(o.LastInterruptHeight)},
	}}
	return encodeValue(top)
}

// DecodeResult is one decode of object bytes (SPEC §7.3, §14.7).
//
// Malformed reports that the input is not one definite-length CBOR data item.
// Unknown is set when the input is one CBOR map that yields a kind text other
// than KIND. Rule A5 then applies and the object is not decoded further.
// Object is set when kind is KIND. Fields may still fail shape; that is a
// section 14 measurement, not a decode failure.
type DecodeResult struct {
	Malformed bool
	Unknown   *UnknownKind
	Object    *Object
}

// DecodeObject decodes one hike object (SPEC §7.3, §8, Rule A5).
func DecodeObject(b []byte) DecodeResult {
	v, rest, err := decodeOne(b)
	if err != nil || len(rest) != 0 || v.typ != cborMap {
		return DecodeResult{Malformed: true}
	}
	kindV, ok := v.lookup("kind")
	if !ok || kindV.typ != cborText {
		return DecodeResult{Malformed: true}
	}
	kind := string(kindV.bytes)
	if kind != Kind {
		return DecodeResult{Unknown: &UnknownKind{Kind: "unknown kind", KindText: kind}}
	}
	o := &Object{Kind: kind}
	if fv, ok := v.lookup("fill"); ok && fv.typ == cborText {
		o.Fill = string(fv.bytes)
	}
	if u, ok := uintField(v, "slot_bytes"); ok {
		o.SlotBytes = u
	}
	if u, ok := uintField(v, "n_slots"); ok {
		o.NSlots = u
	}
	if u, ok := uintField(v, "sample_bytes"); ok {
		o.SampleBytes = u
	}
	if u, ok := uintField(v, "steps_per_leg"); ok {
		o.StepsPerLeg = u
	}
	if bv, ok := bytesField(v, "public_key"); ok {
		o.PublicKey = bv
	}
	if bv, ok := bytesField(v, "seed_beacon"); ok {
		o.SeedBeacon = bv
	}
	if bv, ok := bytesField(v, "last_interrupt"); ok {
		o.LastInterrupt = bv
		o.HasLastInterrupt = true
	}
	if u, ok := uintField(v, "last_interrupt_height"); ok {
		o.LastInterruptHeight = u
		o.HasLastInterruptHeight = true
	}
	if sv, ok := v.lookup("stages"); ok && sv.typ == cborArray {
		o.Stages = make([]Stage, len(sv.array))
		for i, st := range sv.array {
			if st.typ != cborMap {
				continue
			}
			o.Stages[i] = decodeStage(st)
		}
	}
	return DecodeResult{Object: o}
}

func decodeStage(v cborValue) Stage {
	var s Stage
	if bv, ok := bytesField(v, "beacon"); ok {
		s.Beacon = bv
	}
	if u, ok := uintField(v, "beacon_height"); ok {
		s.BeaconHeight = u
	}
	if u, ok := uintField(v, "leg_count"); ok {
		s.LegCount = u
	}
	if bv, ok := bytesField(v, "waypoint"); ok {
		s.Waypoint = bv
	}
	if pv, ok := v.lookup("path"); ok && pv.typ == cborBytes {
		s.Path = pv.bytes
		s.HasPath = true
	}
	if cv, ok := v.lookup("checkpoints"); ok && cv.typ == cborArray {
		for _, e := range cv.array {
			if e.typ == cborBytes {
				s.Checkpoints = append(s.Checkpoints, e.bytes)
			} else {
				s.Checkpoints = append(s.Checkpoints, nil)
			}
		}
	}
	if lv, ok := v.lookup("leg_results"); ok && lv.typ == cborArray {
		for _, e := range lv.array {
			if e.typ == cborBytes {
				s.LegResults = append(s.LegResults, e.bytes)
			} else {
				s.LegResults = append(s.LegResults, nil)
			}
		}
	}
	return s
}

func uintField(v cborValue, key string) (uint64, bool) {
	fv, ok := v.lookup(key)
	if !ok || fv.typ != cborUint {
		return 0, false
	}
	return fv.uint, true
}

func bytesField(v cborValue, key string) ([]byte, bool) {
	fv, ok := v.lookup(key)
	if !ok || fv.typ != cborBytes {
		return nil, false
	}
	return fv.bytes, true
}
