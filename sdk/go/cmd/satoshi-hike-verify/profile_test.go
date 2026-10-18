package main

import (
	"bytes"
	"io"
	"os"
	"testing"

	"github.com/groundinglab/pcoc/sdk/go/v2/hike"
)

func TestVerifyConformanceSmallIsTheV2Profile(t *testing.T) {
	sample, slot, terrain, steps := 32, 64, 256, 32
	profile, ok := measureProfile(parsedArgs{
		profile: "conformance-small", sampleBytes: &sample, slotBytes: &slot, terrainBytes: &terrain, stepsPerLeg: &steps,
	})
	if !ok {
		t.Fatal("verify rejected conformance-small")
	}
	if profile.Kind != hike.Kind || profile.SampleBytes != 32 || profile.SlotBytes != 64 || profile.TerrainBytes != 256 || profile.StepsPerLeg != 32 {
		t.Fatalf("verify profile %+v", profile)
	}
	if profile.Kind == hike.KindV135 {
		t.Fatal("verify selected the v1.35 kind")
	}
}

func TestConformanceSmallRequiresTheFourIntegers(t *testing.T) {
	dir := t.TempDir()
	input := dir + "/hike.cbor"
	if err := os.WriteFile(input, []byte{0xa0}, 0o644); err != nil {
		t.Fatal(err)
	}
	base := []string{"--input", input, "--index-url", "http://127.0.0.1:9", "--profile", "conformance-small"}
	if code := run(base); code != 2 {
		t.Fatalf("missing all: %d", code)
	}
	one := append(append([]string{}, base...), "--sample-bytes", "32", "--slot-bytes", "64", "--terrain-bytes", "256")
	if code := run(one); code != 2 {
		t.Fatalf("missing one: %d", code)
	}
	bad := append(append([]string{}, base...), "--sample-bytes", "nope", "--slot-bytes", "64", "--terrain-bytes", "256", "--steps-per-leg", "32")
	if code := run(bad); code != 2 {
		t.Fatalf("not an integer: %d", code)
	}
	other := []string{"--input", input, "--index-url", "http://127.0.0.1:9", "--profile", "production", "--sample-bytes", "32"}
	if code := run(other); code != 2 {
		t.Fatalf("other profile: %d", code)
	}
	given := append(append([]string{}, base...), "--sample-bytes", "16", "--slot-bytes", "64", "--terrain-bytes", "128", "--steps-per-leg", "8")
	oldOut, oldErr := os.Stdout, os.Stderr
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w
	os.Stderr, _ = os.Open(os.DevNull)
	code := run(given)
	w.Close()
	os.Stdout, os.Stderr = oldOut, oldErr
	body, _ := io.ReadAll(r)
	if code != 0 || !bytes.Contains(body, []byte(`"sample_bytes":16`)) || !bytes.Contains(body, []byte(`"terrain_bytes":128`)) || !bytes.Contains(body, []byte(`"steps_per_leg":8`)) {
		t.Fatalf("code %d record %s", code, body)
	}
}
