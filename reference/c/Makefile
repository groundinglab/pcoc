CC = gcc
CFLAGS = -std=c11 -O2 -Wall -Wextra -D_POSIX_C_SOURCE=200809L
SRC = src/hike.c src/sha256.c
SPEC ?= ../../spec/conformance/v2.1

.PHONY: all test clean

all: hike hike-test

hike: $(SRC) src/main.c src/hike.h
	$(CC) $(CFLAGS) -o $@ $(SRC) src/main.c

hike-test: $(SRC) src/json.c src/test_vectors.c src/hike.h src/json.h
	$(CC) $(CFLAGS) -o $@ $(SRC) src/json.c src/test_vectors.c

test: hike-test
	nice -n 10 ./hike-test $(SPEC)

clean:
	rm -f hike hike-test
