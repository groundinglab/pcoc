# SDK 2.2 core, C. gcc, C11. No system crypto library.
CC      ?= gcc
CFLAGS  ?= -std=c11 -O2 -Wall -Wextra -Wno-unused-function
CPPFLAGS += -Iinclude
LDFLAGS ?=

SRC = src/sha256.c src/terrain.c src/cbor.c src/hashcash.c src/verify.c
# negatives.c is the generator-only module. It is not in the library.
OBJ = $(SRC:src/%.c=build/%.o)

.PHONY: all test candidates clean

all: build/libpcoc.a build/gen-candidates build/pcoc-verify build/unit

build:
	mkdir -p build

build/%.o: src/%.c include/pcoc.h | build
	$(CC) $(CFLAGS) $(CPPFLAGS) -c $< -o $@

build/libpcoc.a: $(OBJ)
	ar rcs $@ $(OBJ)

build/gen-candidates: src/gen.c src/negatives.c build/libpcoc.a
	$(CC) $(CFLAGS) $(CPPFLAGS) -D_POSIX_C_SOURCE=200809L src/gen.c src/negatives.c build/libpcoc.a -o $@ $(LDFLAGS)

build/pcoc-verify: src/verify_main.c build/libpcoc.a
	$(CC) $(CFLAGS) $(CPPFLAGS) src/verify_main.c build/libpcoc.a -o $@ $(LDFLAGS)

build/unit: src/unit.c build/libpcoc.a
	$(CC) $(CFLAGS) $(CPPFLAGS) src/unit.c build/libpcoc.a -o $@ $(LDFLAGS)

candidates: build/gen-candidates
	./build/gen-candidates candidates

test: build/unit build/gen-candidates
	nice -n 10 ./build/unit

clean:
	rm -rf build
