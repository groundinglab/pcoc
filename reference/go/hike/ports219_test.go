package hike

import "testing"

// PORTS-219 fixtures. Hard failures and the trailing quiet object match the
// reference record. The trailing empty map and the stage-0 path key are the
// SPEC section 14 records: items 17, 21, and 22 are owned by #219, and these
// tests do not copy the reference bytes for those three.
func TestPorts219Records(t *testing.T) {
	hard := readProbe(t, "ports219-hard.record")
	trailing := readProbe(t, "ports219-trailing.record")
	stage0 := readProbe(t, "ports219-stage0.record")
	if len(hard) != 7613 || hard[len(hard)-1] != '\n' {
		t.Fatalf("hard golden %d", len(hard))
	}
	if len(trailing) != 7722 || trailing[len(trailing)-1] != '\n' {
		t.Fatalf("trailing golden %d", len(trailing))
	}
	if len(stage0) != 7748 || stage0[len(stage0)-1] != '\n' {
		t.Fatalf("stage0 golden %d", len(stage0))
	}
	quiet := append(QuietV2().Encode(), 0x00)
	object := QuietV2()
	object.Stages[0].PathKey = true
	cases := []struct {
		name string
		data []byte
		want []byte
		spec string
	}{
		{"malformed-empty", []byte{}, hard, ""},
		{"malformed-not-cbor", []byte("not-cbor"), hard, ""},
		{"malformed-simple", []byte{0xff}, hard, ""},
		{"malformed-tag", []byte{0xc0, 0x00}, hard, ""},
		{"malformed-negative", []byte{0x20}, hard, ""},
		{"malformed-indefinite", []byte{0x9f}, hard, ""},
		{"malformed-truncated", []byte{0x41}, hard, ""},
		{"malformed-duplicate-key", []byte{0xa2, 0x61, 0x61, 0x01, 0x61, 0x61, 0x02}, hard, ""},
		{
			"malformed-trailing-empty-map",
			[]byte{0xa0, 0x00},
			hard,
			`SPEC section 14: a trailing byte is ["malformed cbor"]. Item 17. The reference writes 7608 bytes and ["field set"]. #219 owns that shape item.`,
		},
		{"malformed-trailing-object", quiet, trailing, ""},
		{
			"stage0-path-key",
			object.Encode(),
			stage0,
			`SPEC section 14.1 lists ["field set","stage-0 path key","leg_count"]. Items 21 and 22. The reference omits "field set" and is 7736 bytes. #219 owns that omission.`,
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := PackRecord(tc.data, Production, Index{}, nil, 1, nil)
			if got != string(tc.want) {
				t.Fatalf("%s %d bytes, spec %d. %s", tc.name, len(got), len(tc.want), tc.spec)
			}
		})
	}
	if !bytesHas(string(stage0), `"shape_item":["field set","stage-0 path key","leg_count"]`) {
		t.Fatal("stage0 spec shape item")
	}
	if !bytesHas(string(hard), `"shape_item":["malformed cbor"]`) || !bytesHas(string(hard), `"seed_value":"not defined"`) || !bytesHas(string(hard), `"six_leg":"not read"`) || !bytesHas(string(hard), `"six_leg_runs":"not read"`) {
		t.Fatal("hard spec tokens")
	}
	if !bytesHas(string(trailing), `"shape_item":["malformed cbor"]`) || !bytesHas(string(trailing), `"seed_value":"match"`) || !bytesHas(string(trailing), `"w17_path":"not fetched by design"`) {
		t.Fatal("trailing spec tokens")
	}
}

func bytesHas(body, part string) bool {
	return stringIndex(body, part) >= 0
}
