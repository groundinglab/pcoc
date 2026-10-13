package main

import (
	"testing"

	"github.com/groundinglab/pcoc/reference/go/hike"
)

func TestVerifyConformanceSmallIsTheV2Profile(t *testing.T) {
	profile, ok := measureProfile("conformance-small")
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
