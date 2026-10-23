PYTHON ?= python3
CC ?= cc
CFLAGS ?= -O3 -DNDEBUG -fPIC -Wall -Wextra -Wpedantic -march=native

.PHONY: all native candidates objects test clean benchmark

all: native

native: impl/native/libhike.so

impl/native/libhike.so: impl/native/hike.c
	$(CC) $(CFLAGS) -shared -o $@ $< -Wl,-l:libcrypto.so.3

candidates: native
	$(PYTHON) -m impl.satoshi_hike generate-candidates

objects: native
	$(PYTHON) -m impl.satoshi_hike produce-objects

test: native
	$(PYTHON) -m unittest discover -s tests -v

benchmark: native
	$(PYTHON) -m impl.satoshi_hike benchmark

clean:
	rm -f impl/native/libhike.so
	rm -rf impl/satoshi_hike/__pycache__ tests/__pycache__
