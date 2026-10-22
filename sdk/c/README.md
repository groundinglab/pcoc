# PCOC SDK 2.2, C

Library and verify entry point for Satoshi Hike 2.2, written from `spec/satoshi-hike-v2.2/SPEC.md`. Version `2.2.0`.

```sh
make -C sdk/c
make -C sdk/c test
make -C sdk/c candidates
```

`build/libpcoc.a` is the library. `build/pcoc-verify` writes the section 14 record, or the Rule A5 record for any other kind. `build/gen-candidates` writes `candidates/*.txt`.

There is no producer networking in this core: no header source, no Bari client, no posts.
