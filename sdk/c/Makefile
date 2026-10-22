# Satoshi Hike 2.2, C. Baseline ISA. SHA-NI is selected at runtime.
CC      ?= gcc
CFLAGS  ?= -std=c11 -O2 -Wall -Wextra -Wno-unused-function -march=x86-64 -fPIC
CPPFLAGS += -Iinclude -Igen
LDFLAGS ?=

LIB_SRC = src/sha256.c src/terrain.c src/cbor.c src/hashcash.c src/verify.c src/produce.c src/info.c src/br1.c
LIB_OBJ = $(LIB_SRC:src/%.c=build/%.o)

.PHONY: all check candidates clean

all: build/libpcoc.a build/libpcoc.so build/pcoc build/pcoc-gen

build:
	mkdir -p build

build/%.o: src/%.c include/pcoc.h | build
	$(CC) $(CFLAGS) $(CPPFLAGS) -c $< -o $@

build/libpcoc.a: $(LIB_OBJ)
	ar rcs $@ $(LIB_OBJ)

build/libpcoc.so: $(LIB_OBJ)
	$(CC) -shared -o $@ $(LIB_OBJ) $(LDFLAGS)

build/pcoc: cli/main.c build/libpcoc.a
	$(CC) $(CFLAGS) $(CPPFLAGS) cli/main.c build/libpcoc.a -o $@ $(LDFLAGS)

build/pcoc-gen: gen/gen.c gen/negatives.c build/libpcoc.a
	$(CC) $(CFLAGS) $(CPPFLAGS) gen/gen.c gen/negatives.c build/libpcoc.a -o $@ $(LDFLAGS)

build/check_ac: tests/check_ac.c gen/negatives.c build/libpcoc.a include/pcoc.h
	$(CC) $(CFLAGS) $(CPPFLAGS) tests/check_ac.c gen/negatives.c build/libpcoc.a -o $@ $(LDFLAGS)

candidates: build/pcoc-gen
	mkdir -p candidates
	nice -n 10 ./build/pcoc-gen candidates

check: build/check_ac build/pcoc build/pcoc-gen build/libpcoc.so
	nice -n 10 ./build/check_ac $(AC)

clean:
	rm -rf build
