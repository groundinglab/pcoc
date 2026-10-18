# Java SDK

The Java SDK for Satoshi Hike v2.0, the PCOC specification. Packages `pcoc.hike` (verify, the section 14 record, produce against a tip sequence, CBOR, path bodies, profiles), `pcoc.json` (strict JSON), and `pcoc.cli` (the programs `Verify` and `Produce`). The record it writes is byte-identical to the Rust and Go SDKs. The JDK alone is enough: no third-party dependencies.

## Install / pin

JDK 21 or newer (`javac -version` must print 21 or higher; 17 does not work). There is no Maven or Gradle build and nothing is published to a repository. Pin the repository tag and compile the sources into your build:

```sh
git clone --depth 1 --branch sdk-v2.0.0-rc.1 https://github.com/groundinglab/pcoc
cd pcoc/sdk/java
find src -name '*.java' -not -path '*/conformance/*' | sort > sources.txt
javac --release 21 -d build/classes @sources.txt
jar --create --file pcoc-sdk-2.0.0-rc.1.jar -C build/classes .
```

Put the jar (or `src/` as a source root) on your classpath. `pcoc.conformance` is the test harness and is not needed at run time.

> `sdk-v2.0.0-rc.1` has been prepared but not yet published. Until it is, use a checkout of this repository at that commit.

## Quickstart (run)

[`sdk/examples/java/Quickstart.java`](../examples/java/Quickstart.java) verifies the `kat-small` conformance object against its recorded index replies. `sdk/test.sh` runs it on every change. From the repository root:

```sh
mkdir -p /tmp/pcoc-java
javac --release 21 -d /tmp/pcoc-java $(find sdk/java/src -name '*.java') sdk/examples/java/Quickstart.java
java -cp /tmp/pcoc-java Quickstart spec/conformance/v2.0
```

```text
record sha256: 79561726b24aad6a9fcdfb6ae09f0eb0b54934a57bb08dab4bd11e4113d64241
```

The digest is the SHA-256 of the record bytes. The Rust and Go quickstarts print the same digest. The core of it:

```java
Measure.Reader index = docs::get;          // height -> section 9 document body, null = no body
byte[] stream = /* 4096 draw bytes: SecureRandom in production, pinned here */;
String record = Measure.record(object, Profile.CONFORMANCE_SMALL_V2, index, stream, false);
// record == null: the object is CBOR but not a map. Otherwise these are the exact bytes the pack stores.
```

For a real object, use `Profile.PRODUCTION`, read the index over HTTP, and draw from `SecureRandom`:

```java
HttpClient client = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(5)).build();
Measure.Reader index = height -> {
    try {
        HttpResponse<String> r = client.send(
                HttpRequest.newBuilder(URI.create("https://index.groundinglab.io/index/" + height)).GET().build(),
                HttpResponse.BodyHandlers.ofString());
        return r.statusCode() == 200 ? r.body() : null;
    } catch (Exception e) {
        return null;                       // a failed read is recorded, not thrown
    }
};
byte[] stream = new byte[4096];
new SecureRandom().nextBytes(stream);
String record = Measure.record(object, Profile.PRODUCTION, index, stream, false);
```

## API overview

**Verify (section 14).**
- `Measure.record(object, profile, reader, stream, omitSix) → String` returns the record bytes: compact JSON with keys in spec order and one trailing newline. It does not apply policy, and it counts a stored plant path as bound.
- The overload `Measure.record(object, profile, reader, chain, stream, omitSix)` takes a `Summary.Chain` (`boolean ok(PathBody.Locators)`) and checks each path in the anchor window against the chain.
- `Summary.packRecord(...)` is the same record with a Rule A4.3 thread count, which does not change the bytes.
- `Measure.Reader` is `String body(long height)`. Return `null` when there is no body.
- `RecordHash.verifyRecordSha256(record)` is SHA-256 over the RFC 8785 form, excluding the newline.

**Inputs.**
- `Profile.PRODUCTION` (the section 6 constants) and `Profile.CONFORMANCE_SMALL_V2` (the section 16.1 sizes).
- `Profile.forVerify(name)` and `Profile.forProduce(name)` select a profile by program name, and return `null` for an unknown name.
- `Index.readDocument(body)` applies the section 9 document rules, and returns `null` when it rejects the body.

**Objects and paths.**
- `Cbor.decode`, `Cbor.mapKeys` and `Cbor.stageKeys` throw `Cbor.Problem`. `Cbor.issue(bytes)` returns `""` for one acceptable item. The encoders are `encodeUint`, `encodeBytes`, `encodeText`, `encodeArray` and `encodeMap`.
- `PathBody.locatorsFor(path, waypoint)` returns `null` when the body does not validate for that waypoint. `PathBody.encodePathInBlock`, `encodeForBlock` and `inclusionHeight` also work on path bodies.
- `Hike.isEarlierKind` checks for an earlier object kind.

**Produce (section 13.2).** `Drive.run(publicKey, profile, tips) → Drive.Result` runs against an already-read `List<Drive.Tip>`. Network produce is `pcoc.cli.Produce`.

**Building blocks.**
- `Terrain.fill`, `Terrain.fillPin`, `Measure.drawSix`.
- `Bytes`: `sha256`, `hex`, `unhex`, `le64`, `u64le`, `displayHex`.
- The constants in `Hike`, including the informative §15.5 `LEG_RATE_FLOOR_*` examples.

**JSON.** `Json.parse` and `Json.parseStrict` (which rejects duplicate keys), `Json.Obj`, `Arr`, `Str`, `Num`, `Bool` and `Null`, and `Json.deepEqual`.

**Programs.** `pcoc.cli.Verify` and `pcoc.cli.Produce`.

The Rust SDK has two things these packages do not: direct node reads for the plant check, and the section 15 `admit` helper.

## Errors

| Error | Thrown or returned by | Cause |
|---|---|---|
| `Cbor.Problem("trailing")` | `Cbor.decode`, `mapKeys`, `stageKeys`, `decodeHead`; `Cbor.issue` returns the same text | Bytes after the first item |
| `Cbor.Problem("indefinite")` | same | Indefinite-length item |
| `Cbor.Problem("negative")` | same | A negative integer |
| `Cbor.Problem("truncated")` | same | The item runs past the end, a length is too large, a reserved length code (28–30), or a text string that is not UTF-8 |
| `Cbor.Problem("non-text")` | same | A map key that is not a text string |
| `Cbor.Problem("duplicate")` | same | A map key repeated |
| `Cbor.Problem("tag")` | same | A CBOR tag |
| `Cbor.Problem("float-or-simple")` | same | Major type 7 |
| `Cbor.Problem("not a map")` | `Cbor.mapKeys` | The top item is not a map |
| `Cbor.Problem("stage")` | `Cbor.stageKeys` | `stages` is missing, is not an array, or has no such index, or that stage is not a map. Call `mapKeys` first: `stageKeys` assumes the top item is a map |
| `Drive.Clock` | `Drive.run` | The public key is not 32 bytes, no tips were given, the tips ran out, or a tip's `natural` is not 32 bytes |
| `IllegalArgumentException("production_seed")` | `Terrain.fillPin` | The seed is not 64 bytes |
| `IllegalArgumentException` (`"trailing json"`, `"truncated json"`, `"duplicate json key"`, `"escape"`, `"control"`, `"number"`, `"expected …"`, `"object"`, `"array"`, `"string"`) | `Json.parse`, `parseStrict`, `asObj` … | Text that is not one strict JSON value of the expected type |
| `IllegalArgumentException` from `Json.parse` | `RecordHash.verifyRecordSha256` | The record is not JSON |
| `IllegalStateException` | `Bytes.digest`, `Terrain` | The JRE has no SHA-256 (cannot happen on a conforming JDK) |
| `null` | `Measure.record` | The object is CBOR but not a map. Bytes that are not CBOR give the "undecodable" record instead |
| `null` | `PathBody.locatorsFor`, `inclusionHeight`, `Index.readDocument`, `Bytes.unhex`, `Profile.forVerify` / `forProduce` | Invalid path body or wrong waypoint; rejected section 9 document; odd-length or non-hex text; unknown profile name |

The `Profile` constructor checks (`SLOT_BYTES`, `TERRAIN_BYTES`, `profile`) guard the two built-in profiles and cannot be reached from outside the package. `Measure.record` throws nothing for malformed objects. It records `mismatch`, `not defined` or `not read`. The programs exit 0 when the record is written, 1 when the object does not decode or cannot be read, and 2 on a usage error.

## Conformance vectors

`spec/conformance/v2.0/` is the source of truth ([what each file is](../../spec/conformance/v2.0/README.md)). This SDK does not generate the vectors. `pcoc.conformance.V2Pack` reads them in place and compares the record byte for byte, and `pcoc.hike.Spec6Pins` checks the section 6 constants. [../SPEC_PIN](../SPEC_PIN) holds the spec tag and the vector hash.

## Test

```sh
bash sdk/test.sh       # every SDK, plus the cross-check (from the repo root)
bash sdk/java/v2.sh    # this SDK only: javac --release 21 -Xlint:all -Werror, then V2Pack and Spec6Pins
```

## Programs

After compiling (as above), run `java -cp build/classes pcoc.cli.Verify --input OBJECT --profile live --index-url https://index.groundinglab.io`, or `pcoc.cli.Produce --help`. The flags are the same as the Rust and Go programs. The default `--profile` is `test`. `--profile conformance-small` uses the section 16.1 sizes, and `--audit-seed HEX` pins the draw. Verify writes the section 14 record to stdout and applies no admission policy. `--chain-rpc URL` is accepted with `--profile live` for flag compatibility, but this program does not read the node: it calls verify without a chain callback, so a stored path in the anchor window counts as bound. `--demo-mnemonic` is not supported in this SDK.
