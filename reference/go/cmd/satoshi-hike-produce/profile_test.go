package main

import "testing"

func TestProduceRejectsConformanceSmallBeforeTips(t *testing.T) {
	const pub = "4242424242424242424242424242424242424242424242424242424242424242"
	if _, ok := measureProfile("conformance-small"); ok {
		t.Fatal("produce selected conformance-small")
	}
	if _, ok := parse([]string{
		"--pubkey-hex", pub,
		"--output", "out.cbor",
		"--profile", "conformance-small",
		"--tips", "tips.json",
	}); ok {
		t.Fatal("tips accepted conformance-small")
	}
	parsed, ok := parse([]string{
		"--pubkey-hex", pub,
		"--output", "out.cbor",
		"--profile", "production",
		"--tips", "tips.json",
	})
	if !ok || parsed.profile != "production" {
		t.Fatal("production tips")
	}
}
