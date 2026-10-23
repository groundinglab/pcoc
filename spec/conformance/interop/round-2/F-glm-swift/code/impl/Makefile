# Satoshi Hike 2.2 Swift implementation — convenience targets.
# The generator command (CASES.md output format) is `make candidates`.

SWIFT ?= swift
BUILD ?= release

.PHONY: all build candidates objects verify-o1 verify-o2 verify-o3 test bench policy-check clean

all: build candidates objects test

build:
	$(SWIFT) build -c $(BUILD)

candidates:
	$(SWIFT) run -c $(BUILD) GenCandidatesCLI

objects:
	$(SWIFT) run -c $(BUILD) SHikeCLI produce --out objects

verify-o1:
	$(SWIFT) run -c $(BUILD) SHikeCLI verify objects/O1-S1.cbor \
		--profile conformance-small \
		--sample-bytes 32 --slot-bytes 64 --terrain-bytes 65536 --steps-per-leg 100 \
		--draw-source chain:pcoc-pack-2.2/draw

verify-o2:
	$(SWIFT) run -c $(BUILD) SHikeCLI verify objects/O2-S2.cbor \
		--profile conformance-small \
		--sample-bytes 64 --slot-bytes 256 --terrain-bytes 1048576 --steps-per-leg 1000 \
		--draw-source chain:pcoc-pack-2.2/draw

verify-o3:
	$(SWIFT) run -c $(BUILD) SHikeCLI verify objects/O3-PROD.cbor \
		--profile production --draw-source chain:pcoc-pack-2.2/draw

test:
	$(SWIFT) test -c $(BUILD)

bench:
	$(SWIFT) run -c $(BUILD) SHikeCLI bench

policy-check:
	$(SWIFT) run -c $(BUILD) SHikeCLI policy-check ../refs/gl-admit-v9.toml

clean:
	rm -rf .build
