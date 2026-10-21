# Java SDK

The Java SDK for Satoshi Hike v2.1, the PCOC specification. The wire kind is `satoshi-hike-v2.1-object`. Kind `satoshi-hike-v2.0-object` and kind `satoshi-hike-v1.35-object` are earlier objects: section 14 records `version` `earlier version` and section 15 refuses `earlier_version` before any terrain fill. That refuse is not a fake leg. The seed is `SHA-256(public_key ‖ seed_beacon)`, 32 bytes, no label. Terrain words are the NL4 chain over all four lanes. A step folds every word of the selected slot as `acc = (acc + mix64(word XOR ((S + w) mod 2^64))) mod 2^64`. `mix64` includes `MIX_ADD`. The seal digest is `E`. The stored terrain is the raw word. `mix64(word XOR w XOR S)` with a wrapping-add accumulator is the negative `xor_w_fold`. An XOR accumulator is not this fold. `hike_span` is the median-time-past of the last beacon minus the median-time-past of the first beacon. There is no fill pin, no `SEED_DOMAIN`, no per-slot summary, and no leg-result cache. It has three packages:

| Package | What it holds |
|---|---|
| `pcoc.hike` | Verify and the section 14 record; produce against a tip sequence; CBOR; path bodies; profiles |
| `pcoc.json` | Strict JSON |
| `pcoc.cli` | The programs `Verify` and `Produce` |

The section 14 record keeps the same key order. Checkpoint and waypoint bytes follow the v2.1 terrain and fold. A byte match to `spec/conformance/v2.0/` is not v2.1 conformance. The JDK alone is enough: it has no third-party dependencies. SHA-256 is `java.security.MessageDigest`. On this JDK, HotSpot uses the SHA-256 intrinsic when the host has SHA-NI (`UseSHA` and `UseSHA256Intrinsics`).

## Install / pin

You need JDK 21 or newer: `javac -version` must print 21 or higher, and 17 does not work. There is no Maven or Gradle build, and nothing is published to a repository. Pin the repository tag and compile the sources into a jar of your own, outside the checkout:

```sh
git clone --depth 1 --branch sdk-v2.0.0-rc.3 https://github.com/groundinglab/pcoc /path/to/pcoc
SDK=/path/to/pcoc/sdk/java
OUT=$PWD/build                      # in your project, not in the checkout
mkdir -p "$OUT/sdk-classes" lib
find "$SDK/src" -name '*.java' -not -path '*/conformance/*' | sort > "$OUT/pcoc-sources.txt"
javac --release 21 -d "$OUT/sdk-classes" @"$OUT/pcoc-sources.txt"
jar --create --file lib/pcoc-sdk-2.0.0-rc.3.jar -C "$OUT/sdk-classes" .
```

Then compile and run against the jar: `javac --release 21 -cp lib/pcoc-sdk-2.0.0-rc.3.jar …` and `java -cp lib/pcoc-sdk-2.0.0-rc.3.jar:… YourMain`. `pcoc.conformance` is the test harness, and is not needed at run time.

> `sdk-v2.0.0-rc.3` is the repository tag for this version. Build from a checkout until that tag exists.

## Quickstart (historical v2.0 pack)

The program below reads `spec/conformance/v2.0/`. That pack was generated with the withdrawn fill pin and the withdrawn v2.0 fold. It is not the v2.1 target. `bash sdk/java/v2.sh` is the v2.1 check.

[`sdk/examples/java/Quickstart.java`](../examples/java/Quickstart.java) verifies the `kat-small` conformance object against the index replies recorded in the vector, and reproduces the vector's stored record byte for byte:

<!-- from: sdk/examples/java/Quickstart.java -->
```java
// Verify the kat-small conformance object and reproduce the vector's stored
// section 14 record byte for byte.
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;
import pcoc.hike.Bytes;
import pcoc.hike.Measure;
import pcoc.hike.Profile;
import pcoc.hike.RecordHash;
import pcoc.json.Json;

public class Quickstart {
    public static void main(String[] args) throws Exception {
        Path pack = Path.of(args[0]); // spec/conformance/v2.0
        Json.Obj kat = (Json.Obj) Json.parse(Files.readString(pack.resolve("kat-small.json")));
        byte[] object = Bytes.unhex(((Json.Str) kat.fields().get("object_hex")).value);

        // The index: height -> section 9 document body. A real verifier GETs
        // <index>/index/<height>; here the vector carries the replies.
        Map<Long, String> docs = new HashMap<>();
        for (Json d : ((Json.Arr) kat.fields().get("documents")).items()) {
            Json.Obj doc = (Json.Obj) d;
            docs.put(Long.parseLong(((Json.Num) doc.fields().get("height")).digits),
                    ((Json.Str) doc.fields().get("body")).value);
        }
        Measure.Reader index = docs::get; // null = no body for that height

        // The six-leg draw bytes. kat-small's draw_bytes_hex is the pinned stream
        // SHA-256(seed || le64(0)) || SHA-256(seed || le64(1)) || ... for this seed.
        // Production passes 4096 bytes from SecureRandom.
        byte[] stream = Bytes.unhex(((Json.Str) kat.fields().get("draw_bytes_hex")).value);
        byte[] seed = new byte[32];
        Arrays.fill(seed, (byte) 0x5A);
        byte[] pinned = new byte[stream.length];
        for (int i = 0; i * 32 < pinned.length; i++) {
            byte[] block = Bytes.sha256(seed, Bytes.le64(i));
            System.arraycopy(block, 0, pinned, i * 32, Math.min(32, pinned.length - i * 32));
        }
        System.out.println("draw = draw_bytes_hex: " + Arrays.equals(pinned, stream));

        // Profile.PRODUCTION for real objects.
        String record = Measure.record(object, Profile.CONFORMANCE_SMALL_V2, index, stream, false);
        Json.Obj parsed = (Json.Obj) Json.parse(record);
        System.out.println("shape: " + ((Json.Str) parsed.fields().get("shape")).value);
        System.out.println("matches kat-small.json record: "
                + record.equals(((Json.Str) kat.fields().get("record")).value));
        System.out.println("record bytes sha256: " + Bytes.hex(Bytes.sha256(record.getBytes(StandardCharsets.UTF_8))));
        System.out.println("verify_record_sha256: " + Bytes.hex(RecordHash.verifyRecordSha256(record)));
    }
}
```

From the repository root, building into a temporary directory:

```sh
OUT=$(mktemp -d)
javac --release 21 -d "$OUT" $(find sdk/java/src -name '*.java') sdk/examples/java/Quickstart.java
java -cp "$OUT" Quickstart spec/conformance/v2.0
```

<!-- output: quickstart -->
```text
draw = draw_bytes_hex: true
shape: match
matches kat-small.json record: true
record bytes sha256: f539d89340a4e7885638412a0e1fbdf85212498c87ac5bb237a645ddc987ffac
verify_record_sha256: 084e60600907af444632fc2bb09bf8846d78384f4f23eb2a06f6f50491161c29
```

What the output lines mean:
- **The draw.** Passing the vector's `draw_bytes_hex` as `stream` is what reproduces the stored `record`. Those bytes are the pinned stream SHA-256(seed ‖ le64(i)) for `seed` = 32 bytes of `0x5a`.
- **`record bytes sha256`** is the SHA-256 of the record bytes, including the trailing newline.
- **`verify_record_sha256`** (`RecordHash.verifyRecordSha256`) is a different hash: SHA-256 over the RFC 8785 (JCS) form, excluding the newline. Use it as the record identifier. Neither is the vector's `sha256` field, which is the hash of the object bytes.

The Rust and Go quickstarts print the same lines.

## Verify a real object

[`sdk/examples/java/Live.java`](../examples/java/Live.java) uses `Profile.PRODUCTION`, reads the public index over HTTP, and draws from `SecureRandom`. `sdk/test.sh` compiles it but doesn't run it, because it needs the network.

<!-- from: sdk/examples/java/Live.java -->
```java
HttpClient client = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(5)).build();
Measure.Reader index = height -> {
    try {
        HttpResponse<String> r = client.send(
                HttpRequest.newBuilder(URI.create("https://index.groundinglab.io/index/" + height)).GET().build(),
                HttpResponse.BodyHandlers.ofString());
        return r.statusCode() == 200 ? r.body() : null;
    } catch (Exception e) {
        return null; // a failed read is recorded, not thrown
    }
};
byte[] stream = new byte[4096];
new SecureRandom().nextBytes(stream);
String record = Measure.record(object, Profile.PRODUCTION, index, stream, false);
System.out.print(record); // never null: undecodable or non-map input gets the ["malformed cbor"] record
```

**The public index.** `https://index.groundinglab.io` serves `GET /index/<height>` only for the mainnet heights it retains, and answers 404 for any other height. The `conformance-small` vectors use small, non-mainnet heights, so on a vector object every read is `not read`. Use a real production object.

**Gating.** This SDK has no section 15 helper; only Rust has one (`hike::admit` / `hike::failure_facts`). A Java gate reads the record's fields and applies the section 15 invariant refusals itself: shape, the checkpoint comparison, seed value, plant, successor, and six legs (spec section 15). Then it applies its own thresholds. groundinglab's argued default is [POLICY.md](../../services/admission-gate/POLICY.md), a recommendation and not part of the spec. Writing that is new code; the SDK does not provide it.

## Produce

[`sdk/examples/java/ProduceExample.java`](../examples/java/ProduceExample.java) produces an object offline. It uses the `stage0-start.json` tip script at `conformance-small` sizes, and then verifies the result:

<!-- from: sdk/examples/java/ProduceExample.java -->
```java
List<Drive.Tip> tips = new ArrayList<>();
for (Json t : ((Json.Arr) v.fields().get("tips")).items()) {
    Json.Obj tip = (Json.Obj) t;
    tips.add(new Drive.Tip(
            Bytes.unhex(((Json.Str) tip.fields().get("natural_hex")).value),
            Long.parseLong(((Json.Num) tip.fields().get("height")).digits)));
}
// ...
Drive.Result result = Drive.run(publicKey, Profile.CONFORMANCE_SMALL_V2, tips); // Profile.PRODUCTION for a real PCOC
```

```sh
java -cp "$OUT" ProduceExample spec/conformance/v2.0   # after compiling ProduceExample.java like Quickstart.java
```

<!-- output: produce -->
```text
object bytes: 3670
object sha256: da3acd20d063b7492e192b557fdf711fbe5761a49e446a6b2f664af17f72f765
shape: match
```

`Drive.run` writes the same object bytes as the Rust and Go produce examples. It has no plant service, so every path is empty bytes. Network produce (Bitcoin header gossip, and the Bari plant service) is `pcoc.cli.Produce`; it is not a library API in this SDK. For what a real hike costs (18 tip waits × the mean header interval, with 256 MiB held), see [What producing costs](../README.md#what-producing-costs).

## API overview

```java
// pcoc.hike.Measure
public interface Reader { String body(long height); }            // null: no body for that height
public static String record(byte[] object, Profile profile, Reader reader, byte[] stream, boolean omitSix)
public static String record(byte[] object, Profile profile, Reader reader, Summary.Chain chain, byte[] stream, boolean omitSix)
// pcoc.hike.Summary
public interface Chain { boolean ok(PathBody.Locators locators); }
// pcoc.hike.RecordHash
public static byte[] verifyRecordSha256(String record)           // JCS hash, newline excluded
// pcoc.hike.Drive
public static Drive.Result run(byte[] publicKey, Profile profile, List<Drive.Tip> tips) throws Drive.Clock
public Tip(byte[] natural, long height)                          // Drive.Tip
public final byte[] object; public final String stderr;          // Drive.Result
// pcoc.hike.PathBody
public static PathBody.Locators locatorsFor(byte[] path, byte[] waypoint)   // null when it does not match
// pcoc.hike.Index
public static Index readDocument(String body)                    // null when section 9 rejects it
// pcoc.hike.Cbor
public static Cbor.Val decode(byte[] data) throws Cbor.Problem
public static List<String> mapKeys(byte[] data) throws Cbor.Problem
public static List<String> stageKeys(byte[] data, int stage) throws Cbor.Problem
public static String issue(byte[] data)                          // "" for one well-formed item
```

**Verify.**
- **`Measure.record`** returns the record bytes: compact JSON in section 14 key order, with one trailing newline. It does not apply policy, and it counts a stored plant path as bound.
- **The overload with a `Summary.Chain`** checks each path in the anchor window against the chain.
- **`Summary.packRecord(...)`** writes the same record with a Rule A4.3 thread count, which does not change the bytes.

**Inputs.**
- `Profile.PRODUCTION` (section 6) and `Profile.CONFORMANCE_SMALL_V2` (section 16.1) are the built-in profiles.
- `Profile.forVerify(name)` and `Profile.forProduce(name)` select one by program name, and return `null` for an unknown name.

**Objects and paths.** `PathBody.encodePathInBlock`, `encodeForBlock`, `inclusionHeight`, `Hike.isEarlierKind`, and the CBOR encoders `Cbor.encodeUint`, `encodeBytes`, `encodeText`, `encodeArray` and `encodeMap`.

**JSON (`pcoc.json.Json`).**
- **Parsing.** `Json.parse(text)`, and `Json.parseStrict(text)`, which also rejects duplicate keys.
- **Values.** `Json.Obj.fields()` is a `Map<String, Json>` in document order, and `Json.Arr.items()` is a `List<Json>`. `Json.Str.value` is a `String`, `Json.Num.digits` is the number as written (a `String`), `Json.Bool.value` is a `boolean`, and `Json.Null.INSTANCE` is null.
- **Helpers.** `Json.asObj`, `asArr`, `asString`, `asNumber` and `deepEqual`. `compact()` writes compact JSON.

**Building blocks.**
- `Terrain.fill`, `Terrain.mix64`, `Terrain.baseWord`, `Terrain.foldSlot`, and `Measure.drawSix`. The fold is wrapping addition of `mix64(word XOR ((S + w) mod 2^64))`. `Admit` applies Rule B2 (`hike_legs / hike_span`, placeholder floor `1/5`, not ratified), Rule B4 (`anchored_total ≥ 6` anywhere among `W_1` … `W_16`), the submission deadline (placeholder `K = 3`, parameter `submission_deadline_blocks`, not ratified; a gap of 3 is not refused by this rule), and the section 15.5 thorough draw. Section 14 records `hike_span` as the median-time-past difference and does not carry the judgment. The verify CLI does not print it.
- `Bytes`: `sha256`, `hex`, `unhex`, `le64`, `u64le` and `displayHex`.
- The constants in `Hike`. (The §15.5 leg-rate floor examples are no longer SDK constants as of rc.2: a policy sets the value.)

**Programs.** `pcoc.cli.Verify` and `pcoc.cli.Produce`.

The Rust SDK has two things these packages do not: direct node reads for the plant check, and the section 15 helpers.

## Errors

`Cbor.Problem` is a checked exception. Its reason is the public field **`issue`** (a `String`); `getMessage()` returns the same text (rc.2; it returned `null` before). Catch it like this:

```java
try {
    List<String> keys = Cbor.mapKeys(bytes);
} catch (Cbor.Problem p) {
    System.err.println("not one well-formed CBOR map: " + p.issue);
}
```

| Error | Thrown or returned by | Cause |
|---|---|---|
| `Cbor.Problem`, `.issue` `"trailing"` | `Cbor.decode`, `mapKeys`, `stageKeys`, `decodeHead`; `Cbor.issue` returns the same text | Bytes after the first item |
| `.issue` `"indefinite"` | same | An indefinite-length item, or a lone `0xff` break byte |
| `.issue` `"negative"` | same | A negative integer |
| `.issue` `"truncated"` | same | The item runs past the end; a length is too large; a reserved length code (28–30); or a text string that is not UTF-8 |
| `.issue` `"non-text"` | same | A map key that is not a text string |
| `.issue` `"duplicate"` | same | A map key repeated |
| `.issue` `"tag"` | same | A CBOR tag |
| `.issue` `"float-or-simple"` | same | Major type 7 |
| `.issue` `"nesting depth"` | same | More than `Cbor.MAX_CBOR_NESTING` (64) nested arrays and maps (rc.3) |
| `.issue` `"not a map"` | `Cbor.mapKeys` | The top item is not a map |
| `.issue` `"stage"` | `Cbor.stageKeys` | `stages` is missing, is not an array or has no such index, or that stage is not a map. Call `mapKeys` first: `stageKeys` assumes the top item is a map, and throws `NullPointerException` when it is not |
| `Drive.Clock` | `Drive.run` | The public key is not 32 bytes; no tips were given; the tips ran out; or a tip's `natural` is not 32 bytes |
| `IllegalArgumentException("seed preimage")` | `Terrain.seed`, `Terrain.fill` | The public key or the seed beacon is not 32 bytes |
| `IllegalArgumentException` (`getMessage()`: `"trailing json"`, `"truncated json"`, `"duplicate json key"`, `"escape"`, `"control"`, `"number"`, `"expected …"`, `"object"`, `"array"`, `"string"`) | `Json.parse`, `parseStrict`, `asObj`, … | Text that is not one strict JSON value of the expected type |
| `IllegalArgumentException` from `Json.parse` | `RecordHash.verifyRecordSha256` | The record is not JSON |
| `IllegalStateException` | `Bytes.digest`, `Terrain` | The JRE has no SHA-256 (cannot happen on a conforming JDK) |
| `null` | `PathBody.locatorsFor`, `inclusionHeight` | A path body that does not decode, or does not match the waypoint |
| `null` | `Index.readDocument` | A document that section 9 rejects |
| `null` | `Bytes.unhex` | Odd-length or non-hex text |
| `null` | `Profile.forVerify`, `forProduce` | An unknown profile name |

**Nesting limit (rc.3, implementation resource limit).** The decoder accepts at most `Cbor.MAX_CBOR_NESTING` = 64 nested arrays and maps (an item counts the containers around it, itself included). A deeper item is malformed CBOR: the record is `["malformed cbor"]` and the programs exit 1. A valid section 8 hike object nests 4 deep (top-level map, `stages` array, stage map, `checkpoints`/`leg_results` array), so no valid object is refused. The value is the same in the Rust, Go and Java SDKs; see [KNOWN-DIVERGENCES.md](../KNOWN-DIVERGENCES.md#implementation-resource-limits).

The `Profile` constructor checks (`SLOT_BYTES`, `TERRAIN_BYTES`, `profile`) guard the built-in profiles. `Profile.conformanceSmall` uses the same checks for the four integers a verify CLI passes with `--profile conformance-small`, and returns null when one is rejected.

The diagnostic texts are not part of the record, and they differ between SDKs. Only the record bytes are conformance-tested. The [Rust README](../rust/README.md#error-names-across-the-sdks) has a table that maps them across Rust, Go and Java.

`Measure.record` throws nothing for malformed objects. It records `mismatch`, `not defined` or `not read`.

The programs exit 1 only when no top-level CBOR map decodes: the file cannot be read, the first item does not decode under section 7.3, or it is well-formed but not a map. In every other case, trailing bytes after a map included, they write the record and exit 0. Usage errors exit 2. `Measure.record` never returns `null` for these inputs (rc.2): bytes that do not decode and a top level that is not a map both get the `["malformed cbor"]` record. Rust and Go behave the same.

## Conformance vectors

`spec/conformance/v2.1/` is the v2.1 target ([what each file is](../../spec/conformance/v2.1/README.md)). This SDK does not generate the vectors. `pcoc.conformance.V21Pack` recomputes every file in that pack, including the production leg. `pcoc.conformance.Adversarial` rejects a fill-pin terrain, a one-lane word, a linear four-lane word, `mix64` without `MIX_ADD`, the v2.0 fold, the XOR accumulator, `xor_w_fold`, a partial slot read, a shared terrain, and a cached leg. `pcoc.hike.Spec6Pins` checks the section 6 constants. P6.14 is retired. The pack under `spec/conformance/v2.0/` is historical. [../SPEC_PIN](../SPEC_PIN) names tag `spec-v2.1.0`. CoS cuts that tag. The tag half cannot pass until the tag exists.

## Test

```sh
bash sdk/test.sh       # every SDK, the cross-check, examples and docs (from the repo root)
bash sdk/java/v2.sh    # this SDK only: javac --release 21 -Xlint:all -Werror, then F10Reserve, V21Pack, and Spec6Pins; exit 0 on success
```

## Programs

```sh
java -cp lib/pcoc-sdk-2.0.0-rc.3.jar pcoc.cli.Verify --input OBJECT --profile live --index-url https://index.groundinglab.io
java -cp lib/pcoc-sdk-2.0.0-rc.3.jar pcoc.cli.Produce --help
```

**Verify** takes the same flags as the Rust and Go programs; see [the verify programs](../README.md#the-verify-programs).
- The default `--profile` is `test`.
- `--profile conformance-small` requires `--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N`. The record states those integers. Any of the four with another profile is a usage error, exit 2.
- `--chain-rpc URL` is accepted with `--profile live` for flag compatibility, but this program does not read the node. It calls `Measure.record` without a chain. A stored path in the anchor window counts as bound when the index returned a document for that block. When the connection is refused and no document is read, `plant` is `not read`.

**Produce** has its own flags: `--tips PATH` and the profiles `test|live|production`. See `--help`. `--demo-mnemonic` is not supported in this SDK.
