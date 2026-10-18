package pcoc.conformance;

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import pcoc.hike.Anchor;
import pcoc.hike.Bytes;
import pcoc.hike.Cbor;
import pcoc.hike.Draft;
import pcoc.hike.Drive;
import pcoc.hike.Hike;
import pcoc.hike.Measure;
import pcoc.hike.PathBody;
import pcoc.hike.Profile;
import pcoc.hike.RecordHash;
import pcoc.hike.Summary;
import pcoc.hike.Terrain;
import pcoc.json.Json;

/** spec/conformance/v2.0 runner. Pack comparisons call {@link Summary#packRecord}. Issue 228 points that method at {@link Measure#record}. */
public final class V2Pack {
    private V2Pack() {}

    public static void main(String[] args) throws Exception {
        Path root = repoRoot();
        selfCheck(root);
        Path pack = root.resolve("spec/conformance/v2.0");
        if (!Files.isDirectory(pack)) {
            throw new IllegalStateException("spec/conformance/v2.0 is not in this checkout");
        }
        runPack(root, pack);
        ports219(root);
        absentStagesIncludeLegCount();
        blockHashDisplayOrder(root);
        stage0StartsOnTheLatestKnownHeader(root);
        System.out.println("v2 pack ok");
    }

    /**
     * SPEC §9. {@code P_g}, {@code P_b}, and {@code claimed_block}
     * {@code block_hash} are Bitcoin display order. The pinned header is
     * Bitcoin mainnet genesis. Inclusion height 5 puts the path in the quiet
     * object's anchor window. That height is not the Bitcoin height.
     * {@code stored_hash} stays byte 0 first.
     */
    private static void blockHashDisplayOrder(Path root) throws Exception {
        Json.Obj pin = Json.asObj(Json.parse(Files.readString(root.resolve("spec/conformance/blockhash-display.json"))));
        String naturalHex = Json.asString(pin.fields().get("natural_block_hash"));
        String display = Json.asString(pin.fields().get("display_block_hash"));
        byte[] header = Bytes.unhex(Json.asString(pin.fields().get("header_hex")));
        if (header == null || header.length != 80 || naturalHex.length() != 64 || display.length() != 64) {
            throw new AssertionError("pin");
        }
        byte[] natural = Bytes.sha256(Bytes.sha256(header));
        byte[] reversed = new byte[32];
        for (int i = 0; i < 32; i++) {
            reversed[i] = natural[31 - i];
        }
        if (!naturalHex.equals(Bytes.hex(natural)) || !display.equals(Bytes.hex(reversed)) || naturalHex.equals(display)) {
            throw new AssertionError("genesis pin");
        }
        Draft object = Draft.quiet();
        System.arraycopy(natural, 0, object.stages.get(4).beacon, 0, 32);
        object.stages.get(1).path = PathBody.encodeForBlock(object.stages.get(1).waypoint, natural, 5);
        object.stages.get(2).path = PathBody.encodeForBlock(object.stages.get(2).waypoint, natural, 5);
        String got = Summary.packRecord(object.encode(), Profile.PRODUCTION, bookendReader(object), loc -> true, 1, null);
        Json.Obj rec = Json.asObj(Json.parse(got));
        if (!naturalHex.equals(Json.asString(Json.asObj(Json.asArr(rec.fields().get("headers")).items().get(4)).fields().get("stored_hash")))) {
            throw new AssertionError("stored_hash");
        }
        if (!"match".equals(Json.asString(rec.fields().get("index_comparison_P_b")))) {
            throw new AssertionError("index_comparison_P_b");
        }
        Json.Arr anchors = Json.asArr(rec.fields().get("anchors"));
        if (!"anchored".equals(Json.asString(anchors.items().get(0))) || !"anchored".equals(Json.asString(anchors.items().get(1)))) {
            throw new AssertionError("anchors");
        }
        Json.Arr paths = Json.asArr(rec.fields().get("paths"));
        for (int index = 0; index < 2; index++) {
            Json.Obj row = Json.asObj(paths.items().get(index));
            if (!"match".equals(Json.asString(row.fields().get("plant")))) {
                throw new AssertionError("plant");
            }
            requireDisplay(row.fields().get("P_g"), display, naturalHex);
            requireDisplay(row.fields().get("claimed_block"), display, naturalHex);
        }
        requireDisplay(rec.fields().get("P_b"), display, naturalHex);
    }

    private static void requireDisplay(Json value, String display, String natural) {
        Json.Obj block = Json.asObj(value);
        String hash = Json.asString(block.fields().get("block_hash"));
        if (!display.equals(hash) || natural.equals(hash) || !"5".equals(Json.asNumber(block.fields().get("block_height")))) {
            throw new AssertionError("block_hash " + hash);
        }
    }

    /**
     * Section 14.1. A short stages array has no {@code leg_count} on the missing
     * stages. That condition is violated, in the section 14 table order.
     */
    private static void absentStagesIncludeLegCount() {
        Draft draft = Draft.quiet();
        draft.stages.subList(2, draft.stages.size()).clear();
        for (Draft.Stage stage : draft.stages) {
            stage.legCount = 1;
            stage.checkpoints.add(new byte[32]);
            stage.legResults.add(new byte[8]);
        }
        String got = Summary.packRecord(draft.encode(), Profile.PRODUCTION, height -> null, null, 1, null);
        String want = "\"shape_item\":[\"W_17 path\",\"stages length\",\"leg_count\",\"last_interrupt\",\"header height order\",\"header hash differ\"]";
        if (got == null || !got.contains(want)) {
            throw new IllegalStateException("absent leg_count");
        }
    }

    /**
     * The 19 PORTS-219 fixtures. Hard failures and the trailing object match
     * the reference record. Items 17, 21, and 22 stay on the SPEC bytes:
     * a trailing byte is ["malformed cbor"], and a stage-0 path key keeps
     * ["field set","stage-0 path key","leg_count"].
     */
    private static void ports219(Path root) throws Exception {
        byte[] hard = Files.readAllBytes(root.resolve("sdk/go/hike/testdata/ports219-hard.record"));
        byte[] trailing = Files.readAllBytes(root.resolve("sdk/go/hike/testdata/ports219-trailing.record"));
        byte[] stage0 = Files.readAllBytes(root.resolve("sdk/go/hike/testdata/ports219-stage0.record"));
        if (hard.length != 7613 || trailing.length != 7722 || stage0.length != 7748) {
            throw new IllegalStateException("golden length");
        }
        byte[] quiet = Draft.quiet().encode();
        byte[] trailingObject = Arrays.copyOf(quiet, quiet.length + 1);
        Draft stage = Draft.quiet();
        stage.stages.get(0).pathKey = true;
        String hardText = new String(hard, java.nio.charset.StandardCharsets.UTF_8);
        String trailingText = new String(trailing, java.nio.charset.StandardCharsets.UTF_8);
        String stageText = new String(stage0, java.nio.charset.StandardCharsets.UTF_8);
        record Case(String name, byte[] data, String want, String note, boolean v135Null) {}
        Case[] cases = {
            new Case("malformed-empty", new byte[0], hardText, "", true),
            new Case("malformed-not-cbor", "not-cbor".getBytes(java.nio.charset.StandardCharsets.UTF_8), hardText, "", true),
            new Case("malformed-simple", new byte[] {(byte) 0xff}, hardText, "", true),
            new Case("malformed-tag", new byte[] {(byte) 0xc0, 0x00}, hardText, "", true),
            new Case("malformed-negative", new byte[] {0x20}, hardText, "", true),
            new Case("malformed-indefinite", new byte[] {(byte) 0x9f}, hardText, "", true),
            new Case("malformed-truncated", new byte[] {0x41}, hardText, "", true),
            new Case("malformed-duplicate-key", new byte[] {(byte) 0xa2, 0x61, 0x61, 0x01, 0x61, 0x61, 0x02}, hardText, "", true),
            new Case("malformed-trailing-empty-map", new byte[] {(byte) 0xa0, 0x00}, hardText,
                    "SPEC section 14: a trailing byte is [\"malformed cbor\"]. Item 17. The reference writes 7608 bytes and [\"field set\"]. #219 owns that shape item.",
                    true),
            new Case("malformed-trailing-object", trailingObject, trailingText, "", true),
            new Case("stage0-path-key", stage.encode(), stageText,
                    "SPEC section 14.1 lists [\"field set\",\"stage-0 path key\",\"leg_count\"]. Items 21 and 22. The reference omits field set and is 7736 bytes. #219 owns that omission.",
                    false)
        };
        for (Case item : cases) {
            String got = Summary.packRecord(item.data(), Profile.PRODUCTION, height -> null, null, 1, null);
            if (got == null || !got.equals(item.want())) {
                throw new IllegalStateException(item.name() + " " + (got == null ? 0 : got.length()) + " " + item.note());
            }
        }
        if (!hardText.contains("\"seed_value\":\"not defined\"") || !hardText.contains("\"six_leg\":\"not read\"")
                || !hardText.contains("\"six_leg_runs\":\"not read\"") || !trailingText.contains("\"seed_value\":\"match\"")
                || !stageText.contains("\"shape_item\":[\"field set\",\"stage-0 path key\",\"leg_count\"]")) {
            throw new IllegalStateException("spec tokens");
        }
    }

    static Path repoRoot() {
        Path root = Path.of("").toAbsolutePath();
        while (root != null && !Files.isDirectory(root.resolve("spec/conformance/v2.0"))) {
            root = root.getParent();
        }
        if (root == null) {
            throw new IllegalStateException("repo root");
        }
        return root;
    }

    private static void selfCheck(Path root) throws Exception {
        if (Profile.PRODUCTION.sampleBytes != 1024 || Profile.PRODUCTION.stepsPerLeg != 2_000_000) {
            throw new AssertionError("production sizes");
        }
        if (Profile.PRODUCTION.slotBytes != 16384 || Profile.PRODUCTION.terrainBytes != 268435456) {
            throw new AssertionError("slot or terrain changed");
        }
        if (Profile.PRODUCTION.offsetModulus != 268434433L) {
            throw new AssertionError("modulus " + Profile.PRODUCTION.offsetModulus);
        }
        if (!Hike.KIND.equals(Profile.PRODUCTION.kind) || Hike.KIND.equals(Hike.KIND_V135)) {
            throw new AssertionError("kind");
        }
        if (!Hike.KIND.equals(Profile.CONFORMANCE_SMALL_V2.kind)) {
            throw new AssertionError("small kind");
        }
        if (Profile.forVerify("conformance-small") != Profile.CONFORMANCE_SMALL_V2
                || Profile.forProduce("conformance-small") != null) {
            throw new AssertionError("conformance-small selector");
        }
        if (Profile.CONFORMANCE_SMALL_V2.sampleBytes != 32 || Profile.CONFORMANCE_SMALL_V2.stepsPerLeg != 32) {
            throw new AssertionError("small sizes");
        }
        byte[] recordHash = RecordHash.verifyRecordSha256("{\"b\":1,\"a\":2}\n");
        if (!"d3626ac30a87e6f7a6428233b3c68299976865fa5508e4267c5415c76af7a772".equals(Bytes.hex(recordHash))) {
            throw new AssertionError("rfc 8785 " + Bytes.hex(recordHash));
        }
        if (java.util.Arrays.equals(recordHash, Bytes.sha256("{\"b\":1,\"a\":2}\n".getBytes(java.nio.charset.StandardCharsets.UTF_8)))) {
            throw new AssertionError("newline was hashed");
        }
        String header = Json.obj()
                .put("stored_hash", "ab")
                .put("stored_height", 1)
                .put("index_comparison", "match")
                .put("T", 2)
                .compact();
        if (!header.equals("{\"stored_hash\":\"ab\",\"stored_height\":1,\"index_comparison\":\"match\",\"T\":2}")) {
            throw new AssertionError("header keys " + header);
        }
        if (Hike.LEG_RATE_FLOOR_P != 1 || Hike.LEG_RATE_FLOOR_Q != 4
                || Hike.LEG_RATE_FLOOR_STRICT_P != 7 || Hike.LEG_RATE_FLOOR_STRICT_Q != 20
                || Hike.LEG_RATE_FLOOR_DEFAULT_P != 1 || Hike.LEG_RATE_FLOOR_DEFAULT_Q != 4
                || Hike.LEG_RATE_FLOOR_LENIENT_P != 1 || Hike.LEG_RATE_FLOOR_LENIENT_Q != 8) {
            throw new AssertionError("rate floor");
        }
        if (Hike.LEG_RATE_FLOOR_STRICT_P * Hike.LEG_RATE_FLOOR_DEFAULT_Q
                        <= Hike.LEG_RATE_FLOOR_DEFAULT_P * Hike.LEG_RATE_FLOOR_STRICT_Q
                || Hike.LEG_RATE_FLOOR_DEFAULT_P * Hike.LEG_RATE_FLOOR_LENIENT_Q
                        <= Hike.LEG_RATE_FLOOR_LENIENT_P * Hike.LEG_RATE_FLOOR_DEFAULT_Q) {
            throw new AssertionError("floors do not scale with the leg");
        }
        if (Hike.anchorWindowOrigin(17) != 17) {
            throw new AssertionError("anchor origin");
        }
        int[] published = Hike.anchorOffsetsAfterPost();
        if (Hike.ANCHOR_WINDOW != 3 || published.length != 3 || published[0] != 2 || published[1] != 3 || published[2] != 4) {
            throw new AssertionError("window");
        }
        if (!Anchor.inclusionInWindow(102, 100, published, new long[] {102, 103, 104})) {
            throw new AssertionError("published window");
        }
        if (Hike.DEFAULT_BIND_THREADS != 6 || Hike.bindThreadsFrom("") != 6 || Hike.bindThreadsFrom("0") != 6 || Hike.bindThreadsFrom("1") != 1) {
            throw new AssertionError("bind threads");
        }
        long[] interrupts = {102, 110};
        if (!Anchor.inclusionInWindow(102, 100, published, interrupts)) {
            throw new AssertionError("H+2");
        }
        if (Anchor.inclusionInWindow(101, 100, published, interrupts)) {
            throw new AssertionError("H+1 is not in this offset list");
        }
        if (Anchor.inclusionInWindow(102, 100, published, new long[] {110})) {
            throw new AssertionError("not an interrupt");
        }
        if (Anchor.pathInclusionHeight(new byte[] {1, 2, 3, 4}) != null) {
            throw new AssertionError("short body");
        }
        byte[] leaf = new byte[32];
        Arrays.fill(leaf, (byte) 0x11);
        byte[] kat = PathBody.encodePathInBlock(leaf, 123456);
        String hex = "011111111111111111111111111111111111111111111111111111111111111111000000000000000100b7959d170da4982cf2361b9b75c52b70c1b42596e6b1f40665068cfc82ede39544444444444444444444444444444444444444444444444444444444444444440000000255555555555555555555555555555555555555555555555555555555555555550001e240";
        if (!hex.equals(toHex(kat)) || Anchor.pathInclusionHeight(kat) == null || Anchor.pathInclusionHeight(kat) != 123456L) {
            throw new AssertionError("path kat");
        }

        Drive.Result current = driveSmall(Profile.CONFORMANCE_SMALL_V2);
        Cbor.Val item = Cbor.decode(current.object);
        if (!Hike.KIND.equals(item.map.get("kind").text) || item.map.get("sample_bytes").uint != 32 || item.map.get("steps_per_leg").uint != 32) {
            throw new AssertionError("drive kind");
        }
        StringBuilder stderr = new StringBuilder();
        for (int g = 1; g <= 16; g++) {
            stderr.append("path_skip ").append(g).append('\n');
        }
        stderr.append("not fetched by design 17\n");
        if (!stderr.toString().equals(current.stderr) || current.stderr.contains("path_skip 0")
                || current.stderr.contains("not fetched by design 15")
                || current.stderr.contains("not fetched by design 16")) {
            throw new AssertionError(current.stderr);
        }
        checkStage0Path(current.object);
        checkProduceCutOff();
        checkBookend(root);
        checkSeed(root);
        checkLastInterrupt();
        checkSixLeg(current.object);
        checkPackRecord(current.object);

        String want = "{\"version\":\"earlier version\",\"kind\":\"satoshi-hike-v1.35-object\"}\n";
        byte[] raw = Cbor.encodeMap(List.of(java.util.Map.entry("kind", Cbor.encodeText(Hike.KIND_V135))));
        String earlier = Measure.record(raw, Profile.PRODUCTION, height -> null, null, true);
        if (!want.equals(earlier)) {
            throw new AssertionError("earlier " + earlier);
        }
        String smallEarlier = Measure.record(raw, Profile.CONFORMANCE_SMALL_V2, height -> null, null, true);
        if (!want.equals(smallEarlier)) {
            throw new AssertionError("small earlier " + smallEarlier);
        }
    }

    private static void checkStage0Path(byte[] produced) throws Exception {
        if (Cbor.stageKeys(produced, 0).contains("path")) {
            throw new AssertionError("produce stage 0 path");
        }
        if (!Cbor.stageKeys(produced, 1).contains("path")) {
            throw new AssertionError("produce stage 1 path");
        }
        Draft harness = Draft.quiet();
        if (Cbor.stageKeys(harness.encode(), 0).contains("path")) {
            throw new AssertionError("harness stage 0 path");
        }
        for (int g = 1; g < Hike.STAGES; g++) {
            if (!Cbor.stageKeys(harness.encode(), g).contains("path")) {
                throw new AssertionError("harness stage " + g + " path");
            }
        }
        harness.stages.get(0).pathKey = true;
        byte[] forced = harness.encode();
        if (!Cbor.stageKeys(forced, 0).contains("path")) {
            throw new AssertionError("forced stage 0 path");
        }
        Json.Obj forcedLine = Json.asObj(Json.parse(Summary.line(forced, Profile.PRODUCTION, height -> null, null, 1, null)));
        if (Json.asArr(forcedLine.fields().get("anchors")).items().size() != 16
                || !"mismatch".equals(Json.asString(forcedLine.fields().get("shape")))) {
            throw new AssertionError("forced path " + forcedLine.compact());
        }
        String forcedRecord = Measure.record(forced, Profile.PRODUCTION, height -> null, null, false);
        if (forcedRecord == null || !forcedRecord.contains("\"stage-0 path key\"")) {
            throw new AssertionError("forced record " + forcedRecord);
        }

        Json.Obj line = Json.asObj(Json.parse(Summary.line(produced, Profile.CONFORMANCE_SMALL_V2, height -> null, null, 1, null)));
        if (Json.asArr(line.fields().get("anchors")).items().size() != 16
                || "not read".equals(Json.asString(line.fields().get("checkpoints")))
                || "not read".equals(Json.asString(line.fields().get("waypoints")))) {
            throw new AssertionError("parse failure " + line.compact());
        }
        String record = Measure.record(produced, Profile.CONFORMANCE_SMALL_V2, height -> null, null, false);
        Json.Obj full = Json.asObj(Json.parse(record));
        if (!"match".equals(Json.asString(full.fields().get("shape")))
                || !Json.asArr(full.fields().get("shape_item")).items().isEmpty()) {
            throw new AssertionError("full shape " + record);
        }
        if (!Json.asString(line.fields().get("profile")).equals(Json.asString(full.fields().get("profile")))
                || !Json.asString(line.fields().get("shape")).equals(Json.asString(full.fields().get("shape")))) {
            throw new AssertionError("agree " + line.compact() + " " + record);
        }
        if (!"match".equals(Json.asString(line.fields().get("waypoints")))) {
            throw new AssertionError("waypoints " + line.compact());
        }
        for (Json row : Json.asArr(full.fields().get("waypoints")).items()) {
            if (!"match".equals(Json.asString(Json.asObj(row).fields().get("result")))) {
                throw new AssertionError("waypoint " + row.compact());
            }
        }
        agreeSix(Json.asString(line.fields().get("six_leg")), full.fields().get("six_leg"));

        byte[] mutated = withStage0Path(produced, new byte[0]);
        Json.Obj mutLine = Json.asObj(Json.parse(Summary.line(mutated, Profile.CONFORMANCE_SMALL_V2, height -> null, null, 1, null)));
        if (Json.asArr(mutLine.fields().get("anchors")).items().size() != 16
                || !"mismatch".equals(Json.asString(mutLine.fields().get("shape")))) {
            throw new AssertionError("empty stage-0 path " + mutLine.compact());
        }
        String mutRecord = Measure.record(mutated, Profile.CONFORMANCE_SMALL_V2, height -> null, null, false);
        if (mutRecord == null || !mutRecord.contains("\"stage-0 path key\"")) {
            throw new AssertionError("mutated record " + mutRecord);
        }
    }

    private static void checkBookend(Path root) throws Exception {
        Draft object = bookendDraft();
        String got = Summary.packRecord(object.encode(), Profile.PRODUCTION, bookendReader(object), loc -> true, 1, null);
        String want = Files.readString(root.resolve("sdk/go/hike/testdata/bookend.record"));
        if (!want.equals(got)) {
            throw new AssertionError("bookend record " + got.length() + " reference " + want.length());
        }
        if (!got.contains("\"a\":1") || !got.contains("\"b\":2") || !got.contains("\"N\":3")
                || !got.contains("\"S\":3000") || !got.contains("\"legs_outside\":0")) {
            throw new AssertionError("bookend integers");
        }
    }

    private static void checkSeed(Path root) throws Exception {
        Draft object = structuralDraft();
        Arrays.fill(object.stages.get(0).waypoint, (byte) 0x22);
        byte[] stream = auditStream(new byte[32], 4096);
        String got = Summary.packRecord(object.encode(), Profile.PRODUCTION, height -> null, loc -> true, 1, stream);
        String want = Files.readString(root.resolve("sdk/go/hike/testdata/seed.record"));
        if (!want.equals(got)) {
            throw new AssertionError("seed record " + got.length() + " reference " + want.length());
        }
    }

    private static Draft bookendDraft() {
        Draft object = Draft.quiet();
        Arrays.fill(object.stages.get(4).beacon, (byte) 0x55);
        Draft.Stage stage = object.stages.get(1);
        stage.legCount = 3;
        byte[] checkpoint = filled(0xAB, 32);
        byte[] leg = new byte[8];
        leg[0] = 1;
        for (int i = 0; i < 3; i++) {
            stage.checkpoints.add(checkpoint.clone());
            stage.legResults.add(leg.clone());
        }
        object.stages.get(1).path = PathBody.encodePathInBlock(object.stages.get(1).waypoint, 5);
        object.stages.get(2).path = PathBody.encodePathInBlock(object.stages.get(2).waypoint, 5);
        return object;
    }

    private static Draft structuralDraft() {
        Draft object = Draft.quiet();
        byte[] checkpoint = filled(0xAB, 32);
        byte[] leg = new byte[8];
        leg[0] = 1;
        for (int g = 0; g < object.stages.size(); g++) {
            Draft.Stage stage = object.stages.get(g);
            stage.legCount = 1;
            stage.checkpoints.add(checkpoint.clone());
            stage.legResults.add(leg.clone());
            if (g != 0 && g != Hike.STAGES - 1) {
                stage.path = PathBody.encodePathInBlock(stage.waypoint, 123456);
            }
        }
        return object;
    }

    private static Measure.Reader bookendReader(Draft object) {
        java.util.Map<Long, String> docs = new java.util.HashMap<>();
        for (Draft.Stage stage : object.stages) {
            docs.put(stage.beaconHeight, indexDocument(stage.beacon, stage.beaconHeight, stage.beaconHeight * 1000));
        }
        if (object.lastInterruptHeight != null) {
            docs.put(object.lastInterruptHeight, indexDocument(object.lastInterrupt, object.lastInterruptHeight, object.lastInterruptHeight * 1000));
        }
        return docs::get;
    }

    private static String indexDocument(byte[] natural, long height, long time) {
        byte[] display = new byte[natural.length];
        for (int i = 0; i < natural.length; i++) {
            display[i] = natural[natural.length - 1 - i];
        }
        return "{\"header\":{\"hash\":\"" + Bytes.hex(display) + "\",\"height\":" + height + ",\"time\":" + time + "}}";
    }

    private static void checkProduceCutOff() throws Exception {
        long[] asked = Drive.pathAskHeights(100, 102);
        long[] window = Drive.pathAskHeights(100, Long.MAX_VALUE);
        if (asked.length != 1 || asked[0] != 102 || window.length != 3 || window[2] != 104 || asked.length == window.length) {
            throw new AssertionError("cut-off asks");
        }
        if (Drive.pathAskHeights(-2L, -1L).length != 0) {
            throw new AssertionError("offset that does not fit");
        }
        byte[] waypoint = filled(0x11, 32);
        byte[] body = PathBody.encodePathInBlock(waypoint, 104);
        if (Drive.acceptPath(body, waypoint, 100, 102) != null) {
            throw new AssertionError("stored a body above last_interrupt");
        }
        if (Drive.acceptPath(body, waypoint, 100, 104) == null) {
            throw new AssertionError("dropped a body at the cut-off");
        }
        if (Drive.acceptPath(new byte[] {1, 2, 3}, waypoint, 100, 200) != null) {
            throw new AssertionError("stored a miss");
        }
        Drive.Result kept = driveSmall(Profile.CONFORMANCE_SMALL_V2);
        if (kept.object.length == 0 || !kept.stderr.contains("path_skip 1\n") || !kept.stderr.endsWith("not fetched by design 17\n")) {
            throw new AssertionError("a path miss withheld the object");
        }
        String present = Drive.report("out.cbor", true);
        String absent = Drive.report("", false);
        if (!present.equals("{\"schema\":\"satoshi-hike-cli-report-v0.2\",\"command\":\"produce\",\"result\":\"present\",\"rung\":null,\"output\":\"out.cbor\"}\n")
                || !absent.equals("{\"schema\":\"satoshi-hike-cli-report-v0.2\",\"command\":\"produce\",\"result\":\"not read\",\"rung\":null}\n")
                || present.contains("\"result\":\"pass\"") || present.contains("\"result\":\"fail\"")
                || absent.contains("\"result\":\"pass\"") || absent.contains("\"result\":\"fail\"")) {
            throw new AssertionError("report " + present + " " + absent);
        }
    }

    private static void checkPackRecord(byte[] produced) {
        String pack = Summary.packRecord(produced, Profile.CONFORMANCE_SMALL_V2, height -> null, null, 1, null);
        String line = Summary.line(produced, Profile.CONFORMANCE_SMALL_V2, height -> null, null, 1, null);
        String record = Measure.record(produced, Profile.CONFORMANCE_SMALL_V2, height -> null, null, false);
        if (record == null || !record.contains("\"shape_item\"") || !pack.equals(record) || pack.equals(line)) {
            throw new AssertionError("pack record " + pack);
        }
    }

    private static void checkSixLeg(byte[] produced) throws Exception {
        if (Profile.PRODUCTION.stepsPerLeg != 2_000_000
                || Profile.CONFORMANCE_SMALL_V2.stepsPerLeg == Profile.PRODUCTION.stepsPerLeg) {
            throw new AssertionError("steps " + Profile.PRODUCTION.stepsPerLeg);
        }
        byte[] stream = auditStream(filled(0x11, 32), 4096);
        Json.Obj line = Json.asObj(Json.parse(
                Summary.line(produced, Profile.CONFORMANCE_SMALL_V2, height -> null, null, 1, stream)));
        Json.Obj full = Json.asObj(Json.parse(
                Measure.record(produced, Profile.CONFORMANCE_SMALL_V2, height -> null, stream, false)));
        if (!"match".equals(Json.asString(line.fields().get("six_leg")))
                || !"match".equals(Json.asString(line.fields().get("shape")))
                || !"match".equals(Json.asString(full.fields().get("shape")))) {
            throw new AssertionError("six " + line.compact() + " " + full.compact());
        }
        agreeSix(Json.asString(line.fields().get("six_leg")), full.fields().get("six_leg"));
        byte[] shortStream = new byte[] {1};
        Json.Obj shortLine = Json.asObj(Json.parse(
                Summary.line(produced, Profile.CONFORMANCE_SMALL_V2, height -> null, null, 1, shortStream)));
        Json.Obj shortFull = Json.asObj(Json.parse(
                Measure.record(produced, Profile.CONFORMANCE_SMALL_V2, height -> null, shortStream, false)));
        if (!"not read".equals(Json.asString(shortLine.fields().get("six_leg")))) {
            throw new AssertionError("short " + shortLine.compact());
        }
        agreeSix(Json.asString(shortLine.fields().get("six_leg")), shortFull.fields().get("six_leg"));
    }

    private static byte[] auditStream(byte[] seed, int bytes) throws Exception {
        byte[] out = new byte[bytes];
        int filledAt = 0;
        long i = 0;
        MessageDigest md = MessageDigest.getInstance("SHA-256");
        while (filledAt < bytes) {
            md.update(seed);
            md.update(Bytes.le64(i));
            byte[] block = md.digest();
            int n = Math.min(block.length, bytes - filledAt);
            System.arraycopy(block, 0, out, filledAt, n);
            filledAt += n;
            i++;
        }
        return out;
    }

    private static void checkLastInterrupt() throws Exception {
        Draft draft = referenceShaped();
        byte[] raw = draft.encode(Profile.CONFORMANCE_SMALL_V2);
        if (Cbor.stageKeys(raw, 0).contains("path")) {
            throw new AssertionError("reference stage 0 path");
        }
        List<String> top = Cbor.mapKeys(raw);
        if (!top.contains("last_interrupt") || !top.contains("last_interrupt_height")) {
            throw new AssertionError("reference keys " + top);
        }
        Json.Obj line = Json.asObj(Json.parse(Summary.line(raw, Profile.CONFORMANCE_SMALL_V2, height -> null, null, 1, null)));
        String record = Measure.record(raw, Profile.CONFORMANCE_SMALL_V2, height -> null, null, false);
        Json.Obj full = Json.asObj(Json.parse(record));
        if (!"match".equals(Json.asString(line.fields().get("shape")))
                || !"match".equals(Json.asString(full.fields().get("shape")))
                || !Json.asArr(full.fields().get("shape_item")).items().isEmpty()) {
            throw new AssertionError("reference " + line.compact() + " " + record);
        }
        byte[] stripped = dropKeys(raw, "last_interrupt", "last_interrupt_height");
        Json.Obj strippedLine = Json.asObj(Json.parse(Summary.line(stripped, Profile.CONFORMANCE_SMALL_V2, height -> null, null, 1, null)));
        String strippedRecord = Measure.record(stripped, Profile.CONFORMANCE_SMALL_V2, height -> null, null, false);
        if (Json.asArr(strippedLine.fields().get("anchors")).items().size() != 16
                || !"mismatch".equals(Json.asString(strippedLine.fields().get("shape")))
                || !strippedRecord.contains("\"last_interrupt\"")) {
            throw new AssertionError("stripped " + strippedLine.compact() + " " + strippedRecord);
        }
    }

    private static Draft referenceShaped() {
        Draft draft = Draft.quiet();
        draft.stages.get(0).waypoint = Bytes.sha256(draft.publicKey, draft.seedBeacon);
        byte[] checkpoint = filled(0xAB, 32);
        byte[] leg = new byte[8];
        leg[0] = 1;
        for (Draft.Stage stage : draft.stages) {
            stage.legCount = 1;
            stage.checkpoints.add(checkpoint);
            stage.legResults.add(leg);
        }
        return draft;
    }

    private static void agreeSix(String compact, Json full) {
        if (full instanceof Json.Str) {
            if (!compact.equals(Json.asString(full))) {
                throw new AssertionError("six " + compact + " " + full.compact());
            }
            return;
        }
        Json.Arr results = Json.asArr(Json.asObj(full).fields().get("results"));
        if (results.items().isEmpty()) {
            throw new AssertionError("empty draw");
        }
        String want = "match";
        for (Json item : results.items()) {
            if (!"match".equals(Json.asString(item))) {
                want = "mismatch";
            }
        }
        if (!compact.equals(want)) {
            throw new AssertionError("six " + compact + " " + full.compact());
        }
    }

    private static byte[] dropKeys(byte[] raw, String... names) throws Exception {
        Cbor.Val top = Cbor.decode(raw);
        java.util.Set<String> skip = new java.util.HashSet<>();
        java.util.Collections.addAll(skip, names);
        List<java.util.Map.Entry<String, byte[]>> fields = new ArrayList<>();
        for (java.util.Map.Entry<String, Cbor.Val> entry : top.map.entrySet()) {
            if (!skip.contains(entry.getKey())) {
                fields.add(java.util.Map.entry(entry.getKey(), encodeVal(entry.getValue())));
            }
        }
        return Cbor.encodeMap(fields);
    }

    private static byte[] withStage0Path(byte[] raw, byte[] path) throws Exception {
        Cbor.Val top = Cbor.decode(raw);
        Cbor.Val stages = top.map.get("stages");
        List<java.util.Map.Entry<String, byte[]>> pairs = new ArrayList<>();
        pairs.add(java.util.Map.entry("path", Cbor.encodeBytes(path)));
        for (java.util.Map.Entry<String, Cbor.Val> entry : stages.array.get(0).map.entrySet()) {
            if (!"path".equals(entry.getKey())) {
                pairs.add(java.util.Map.entry(entry.getKey(), encodeVal(entry.getValue())));
            }
        }
        List<byte[]> encodedStages = new ArrayList<>();
        for (int g = 0; g < stages.array.size(); g++) {
            encodedStages.add(g == 0 ? Cbor.encodeMap(pairs) : encodeVal(stages.array.get(g)));
        }
        List<java.util.Map.Entry<String, byte[]>> fields = new ArrayList<>();
        for (java.util.Map.Entry<String, Cbor.Val> entry : top.map.entrySet()) {
            if ("stages".equals(entry.getKey())) {
                fields.add(java.util.Map.entry("stages", Cbor.encodeArray(encodedStages)));
            } else {
                fields.add(java.util.Map.entry(entry.getKey(), encodeVal(entry.getValue())));
            }
        }
        return Cbor.encodeMap(fields);
    }

    private static byte[] encodeVal(Cbor.Val value) {
        return switch (value.kind) {
            case UINT -> Cbor.encodeUint(value.uint);
            case BYTES -> Cbor.encodeBytes(value.bytes);
            case TEXT -> Cbor.encodeText(value.text);
            case ARRAY -> {
                List<byte[]> items = new ArrayList<>();
                for (Cbor.Val item : value.array) {
                    items.add(encodeVal(item));
                }
                yield Cbor.encodeArray(items);
            }
            case MAP -> {
                List<java.util.Map.Entry<String, byte[]>> pairs = new ArrayList<>();
                for (java.util.Map.Entry<String, Cbor.Val> entry : value.map.entrySet()) {
                    pairs.add(java.util.Map.entry(entry.getKey(), encodeVal(entry.getValue())));
                }
                yield Cbor.encodeMap(pairs);
            }
        };
    }

    private static Drive.Result driveSmall(Profile profile) throws Drive.Clock {
        byte[] pub = new byte[32];
        Arrays.fill(pub, (byte) 0x11);
        List<Drive.Tip> tips = new ArrayList<>();
        for (int i = 0; i < Hike.STAGES + 1; i++) {
            byte[] natural = new byte[32];
            Arrays.fill(natural, (byte) (i + 1));
            tips.add(new Drive.Tip(natural, 1000L + i));
        }
        return Drive.run(pub, profile, tips);
    }

    /**
     * P9.14. The first tip is already known. The same header after the first
     * leg does not replace the seed. The next header is {@code I_0}.
     */
    private static void stage0StartsOnTheLatestKnownHeader(Path root) throws Exception {
        Json.Obj doc = Json.asObj(Json.parse(Files.readString(root.resolve("spec/conformance/v2.0/stage0-start.json"))));
        if (!"P9.14".equals(Json.asString(doc.fields().get("rule")))) {
            throw new AssertionError("rule");
        }
        if (!"conformance-small".equals(Json.asString(doc.fields().get("profile")))) {
            throw new AssertionError("profile");
        }
        byte[] pub = Bytes.unhex(Json.asString(doc.fields().get("public_key_hex")));
        long seedHeight = Long.parseLong(Json.asNumber(doc.fields().get("seed_height")));
        long stage0Legs = Long.parseLong(Json.asNumber(doc.fields().get("stage0_legs")));
        List<Drive.Tip> tips = new ArrayList<>();
        for (Json item : Json.asArr(doc.fields().get("tips")).items()) {
            Json.Obj tip = Json.asObj(item);
            tips.add(new Drive.Tip(
                    Bytes.unhex(Json.asString(tip.fields().get("natural_hex"))),
                    Long.parseLong(Json.asNumber(tip.fields().get("height")))));
        }
        if (tips.size() < 3
                || !Arrays.equals(tips.get(0).natural, tips.get(1).natural)
                || tips.get(0).height != seedHeight) {
            throw new AssertionError("vector tips");
        }
        Drive.Result result = Drive.run(pub, Profile.CONFORMANCE_SMALL_V2, tips);
        Cbor.Val top = Cbor.decode(result.object);
        byte[] seed = top.map.get("seed_beacon").bytesOrNull(32);
        Cbor.Val stage0 = top.map.get("stages").array.get(0);
        Cbor.Val stage1 = top.map.get("stages").array.get(1);
        if (seed == null || !Arrays.equals(seed, tips.get(0).natural)) {
            throw new AssertionError("seed beacon");
        }
        byte[] beacon = stage0.map.get("beacon").bytesOrNull(32);
        if (beacon == null
                || !Arrays.equals(beacon, tips.get(0).natural)
                || stage0.map.get("beacon_height").uint != seedHeight) {
            throw new AssertionError("stage 0 beacon");
        }
        if (stage0.map.get("leg_count").uint != stage0Legs) {
            throw new AssertionError("stage 0 legs");
        }
        byte[] interrupt = stage1.map.get("beacon").bytesOrNull(32);
        if (interrupt == null
                || !Arrays.equals(interrupt, tips.get(2).natural)
                || stage1.map.get("beacon_height").uint != tips.get(2).height) {
            throw new AssertionError("I_0");
        }
    }

    private static void runPack(Path root, Path pack) throws Exception {
        Json.Obj manifest = load(pack.resolve("manifest.json"));
        if (!"satoshi-hike-v2.0".equals(text(manifest, "pack"))) {
            throw new IllegalStateException("pack");
        }
        List<String> files = strings(manifest, "files");
        List<String> want = List.of(
                "constants.json",
                "checkpoint-preimage.json",
                "anchor-window.json",
                "anchor-w16-i17.json",
                "earlier-run-b.json",
                "bind-threads.json",
                "test-3leg.json",
                "kat-small.json");
        if (!files.equals(want)) {
            throw new IllegalStateException("files " + files);
        }
        checkConstants(load(pack.resolve("constants.json")));
        checkPreimage(load(pack.resolve("checkpoint-preimage.json")));
        checkAnchor(load(pack.resolve("anchor-window.json")));
        checkW16(load(pack.resolve("anchor-w16-i17.json")));
        checkEarlier(root, load(pack.resolve("earlier-run-b.json")));
        checkBind(load(pack.resolve("bind-threads.json")));
        checkTest3Leg(load(pack.resolve("test-3leg.json")));
        checkKat(load(pack.resolve("kat-small.json")));
    }

    /** TEST vector. Mock anchor. Not a real anchored PCOC. */
    private static void checkTest3Leg(Json.Obj doc) throws Exception {
        Json consumed = doc.fields().get("draw_consumed");
        if (!(consumed instanceof Json.Bool) || ((Json.Bool) consumed).value) {
            throw new IllegalStateException("draw consumed");
        }
        List<Json> legs = Json.asArr(doc.fields().get("legs_per_stage")).items();
        if (legs.size() != 3) {
            throw new IllegalStateException("legs");
        }
        byte[] object = Bytes.unhex(text(doc, "object_hex"));
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        String sha = Bytes.hex(digest.digest(object));
        if (!sha.equals(text(doc, "sha256"))) {
            throw new IllegalStateException("sha256 " + sha);
        }
        byte[] draw = Bytes.unhex(text(doc, "draw_bytes_hex"));
        if (draw.length != 48) {
            throw new IllegalStateException("draw");
        }
        String got = Summary.packRecord(
                object, Profile.CONFORMANCE_SMALL_V2, height -> null, loc -> true, 1, draw);
        if (!got.equals(text(doc, "record"))) {
            throw new IllegalStateException("test-3leg record " + got.length());
        }
        if (!got.endsWith("\n")) {
            throw new IllegalStateException("newline");
        }
        Json.Obj record = Json.asObj(Json.parse(got.substring(0, got.length() - 1)));
        if (!(record.fields().get("six_leg_runs") instanceof Json.Arr runs) || !runs.items().isEmpty()) {
            throw new IllegalStateException("six_leg_runs");
        }
        if (!"not defined".equals(Json.asString(record.fields().get("six_leg")))) {
            throw new IllegalStateException("six_leg");
        }
        Json.Arr waypoints = Json.asArr(record.fields().get("waypoints"));
        if (!"match".equals(Json.asString(Json.asObj(waypoints.items().get(0)).fields().get("result")))
                || !"match".equals(Json.asString(Json.asObj(waypoints.items().get(1)).fields().get("result")))
                || !"not defined".equals(Json.asString(Json.asObj(waypoints.items().get(2)).fields().get("result")))) {
            throw new IllegalStateException("waypoints");
        }
        byte[] hashed = RecordHash.verifyRecordSha256(got);
        if (java.util.Arrays.equals(hashed, Bytes.sha256(got.getBytes(java.nio.charset.StandardCharsets.UTF_8)))) {
            throw new IllegalStateException("record hash included the newline");
        }
    }

    private static void checkKat(Json.Obj doc) throws Exception {
        if (!"conformance-small".equals(text(doc, "profile"))
                || longNum(doc, "sample_bytes") != 32
                || longNum(doc, "slot_bytes") != 64
                || longNum(doc, "terrain_bytes") != 256
                || longNum(doc, "steps_per_leg") != 32) {
            throw new IllegalStateException("profile");
        }
        Json.Obj book = Json.asObj(doc.fields().get("bookends"));
        if (longNum(book, "a") != 1 || longNum(book, "b") != 2 || longNum(book, "N") != 1
                || longNum(book, "S") != 3 || longNum(book, "delta_h") != 3) {
            throw new IllegalStateException("bookends");
        }
        byte[] pub = Bytes.unhex(text(doc, "public_key_hex"));
        byte[] beacon = Bytes.unhex(text(doc, "seed_beacon_hex"));
        Terrain terrain = Terrain.fill(pub, beacon, Profile.CONFORMANCE_SMALL_V2);
        if (!Bytes.hex(terrain.blob).equals(text(doc, "terrain_hex"))) {
            throw new IllegalStateException("terrain");
        }
        byte[] seed = Bytes.sha256(pub, beacon);
        Json.Obj leg = Json.asObj(doc.fields().get("leg"));
        if (!Bytes.hex(seed).equals(text(leg, "previous_hex")) || !text(leg, "previous_hex").equals(text(leg, "limb_hex"))) {
            throw new IllegalStateException("seed");
        }
        byte[] sample = terrain.sample(seed);
        long legResult = terrain.leg(seed);
        if (!Bytes.hex(sample).equals(text(leg, "sample_hex"))
                || !Bytes.hex(Bytes.le64(legResult)).equals(text(leg, "leg_result_hex"))) {
            throw new IllegalStateException("leg");
        }
        byte[] checkpoint = Bytes.sha256(seed, seed, sample, Bytes.le64(legResult));
        if (!Bytes.hex(checkpoint).equals(text(leg, "checkpoint_hex"))) {
            throw new IllegalStateException("checkpoint");
        }
        Json.Obj w1 = Json.asObj(doc.fields().get("waypoint_w1"));
        byte[] interrupt = Bytes.unhex(text(w1, "interrupt_hex"));
        byte[] digest = Bytes.sha256(interrupt, terrain.slot(interrupt), seed, Bytes.sha256(new byte[32], checkpoint));
        if (!Bytes.hex(digest).equals(text(w1, "digest_hex"))
                || !text(w1, "limb_hex").equals(text(leg, "limb_hex"))
                || !text(w1, "checkpoint_hex").equals(text(leg, "checkpoint_hex"))) {
            throw new IllegalStateException("waypoint");
        }
        byte[] draw = Bytes.unhex(text(doc, "draw_bytes_hex"));
        if (draw.length != 256) {
            throw new IllegalStateException("draw");
        }
        Measure.Drawn drawn = Measure.drawSix(draw, 18);
        if (drawn.token != null) {
            throw new IllegalStateException(drawn.token);
        }
        Json.Obj six = Json.asObj(doc.fields().get("six_leg"));
        Json.Arr indices = Json.asArr(six.fields().get("indices"));
        Json.Arr results = Json.asArr(six.fields().get("results"));
        if (drawn.indices.length != 6 || indices.items().size() != 6 || results.items().size() != 6) {
            throw new IllegalStateException("six");
        }
        for (int i = 0; i < 6; i++) {
            if (drawn.indices[i] != Long.parseLong(Json.asNumber(indices.items().get(i)))
                    || !"match".equals(Json.asString(results.items().get(i)))) {
                throw new IllegalStateException("six");
            }
        }
        byte[] object = Bytes.unhex(text(doc, "object_hex"));
        if (!sha256(object).equals(text(doc, "sha256"))) {
            throw new IllegalStateException("sha256");
        }
        Map<Long, String> docs = new HashMap<>();
        for (Json item : Json.asArr(doc.fields().get("documents")).items()) {
            Json.Obj row = Json.asObj(item);
            docs.put(longNum(row, "height"), text(row, "body"));
        }
        if (docs.size() != 19) {
            throw new IllegalStateException("documents");
        }
        List<Long> reads = new ArrayList<>();
        String line = Summary.packRecord(object, Profile.CONFORMANCE_SMALL_V2, docs::get, loc -> {
            reads.add(loc.blockHeight);
            return true;
        }, 1, draw);
        if (!text(doc, "record").equals(line)) {
            throw new IllegalStateException("kat record " + line.length());
        }
        Json.Arr wantReads = Json.asArr(doc.fields().get("index_reads"));
        if (wantReads.items().size() != reads.size()) {
            throw new IllegalStateException("reads " + reads);
        }
        for (int i = 0; i < reads.size(); i++) {
            if (Long.parseLong(Json.asNumber(wantReads.items().get(i))) != reads.get(i)) {
                throw new IllegalStateException("reads " + reads);
            }
        }
        Json.Obj record = Json.asObj(Json.parse(line.substring(0, line.length() - 1)));
        Json.Obj analysis = Json.asObj(record.fields().get("analysis"));
        if (longNum(analysis, "whole_hike_legs") != 17 || longNum(analysis, "legs_outside") != 17) {
            throw new IllegalStateException("analysis");
        }
    }

    private static void requirePlant(Json.Obj doc, byte[] waypoint, long height, boolean header) {
        Json.Arr reads = Json.asArr(doc.fields().get("plant_reads"));
        if (reads.items().size() != 1) {
            throw new IllegalStateException("plant_reads");
        }
        Json.Obj row = Json.asObj(reads.items().get(0));
        if (longNum(row, "height") != height) {
            throw new IllegalStateException("plant height");
        }
        Json.Obj body = Json.asObj(Json.parse(text(row, "body")));
        String network = text(body, "network");
        if (!"main".equals(network) && !"synthetic".equals(network)) {
            throw new IllegalStateException("network");
        }
        if ((body.fields().get("header") != null) != header) {
            throw new IllegalStateException("header");
        }
        String root = Bytes.hex(Bytes.sha256(new byte[] {0x1a}, waypoint));
        boolean matched = plantMatches(body.fields().get("plant"), root, height);
        Json listed = body.fields().get("plants");
        if (listed instanceof Json.Arr plants) {
            for (Json item : plants.items()) {
                matched = matched || plantMatches(item, root, height);
            }
        }
        if (!matched) {
            throw new IllegalStateException("plant");
        }
    }

    private static boolean plantMatches(Json value, String root, long height) {
        if (!(value instanceof Json.Obj plant)) {
            return false;
        }
        return root.equals(text(plant, "root")) && longNum(plant, "vout") == 2 && longNum(plant, "block_height") == height;
    }

    private static void checkConstants(Json.Obj doc) {
        checkSection6(doc);
        if (longNum(doc, "SAMPLE_BYTES") != Profile.PRODUCTION.sampleBytes
                || longNum(doc, "STEPS_PER_LEG") != Profile.PRODUCTION.stepsPerLeg
                || longNum(doc, "OFFSET_MODULUS") != Profile.PRODUCTION.offsetModulus
                || longNum(doc, "CHECKPOINT_PREIMAGE_LEN") != 1096
                || longNum(doc, "DEFAULT_BIND_THREADS") != Hike.DEFAULT_BIND_THREADS
                || !Hike.KIND.equals(text(doc, "KIND"))) {
            throw new IllegalStateException("constants");
        }
        List<String> earlier = strings(doc, "EARLIER_KINDS");
        if (earlier.size() != 1 || !Hike.KIND_V135.equals(earlier.get(0))) {
            throw new IllegalStateException("earlier kinds");
        }
        Json.Arr offsets = Json.asArr(doc.fields().get("ANCHOR_WINDOW_OFFSETS"));
        int[] published = Hike.anchorOffsetsAfterPost();
        if (offsets.items().size() != published.length) {
            throw new IllegalStateException("offsets");
        }
        for (int i = 0; i < published.length; i++) {
            if (Long.parseLong(Json.asNumber(offsets.items().get(i))) != published[i]) {
                throw new IllegalStateException("offsets");
            }
        }
        Json.Obj floors = Json.asObj(doc.fields().get("LEG_RATE_FLOOR"));
        if (!floorEq(floors, "strict", Hike.LEG_RATE_FLOOR_STRICT_P, Hike.LEG_RATE_FLOOR_STRICT_Q)
                || !floorEq(floors, "default", Hike.LEG_RATE_FLOOR_DEFAULT_P, Hike.LEG_RATE_FLOOR_DEFAULT_Q)
                || !floorEq(floors, "lenient", Hike.LEG_RATE_FLOOR_LENIENT_P, Hike.LEG_RATE_FLOOR_LENIENT_Q)) {
            throw new IllegalStateException("floors");
        }
        if (!"non-normative reference default".equals(text(doc, "DEFAULT_BIND_THREADS_NOTE"))
                || !"Informative example values from §15.5 (Part B). Not a conformance target; policy sets the value.".equals(text(doc, "LEG_RATE_FLOOR_NOTE"))) {
            throw new IllegalStateException("constant notes");
        }
    }

    private static boolean floorEq(Json.Obj floors, String name, int p, int q) {
        Json.Obj pair = Json.asObj(floors.fields().get(name));
        return longNum(pair, "p") == p && longNum(pair, "q") == q;
    }

    private static void checkPreimage(Json.Obj doc) throws Exception {
        byte[] previous = filled(0x11, 32);
        byte[] limb = filled(0x22, 32);
        byte[] sample = new byte[Profile.PRODUCTION.sampleBytes];
        sample[0] = 0x33;
        sample[sample.length - 1] = 0x44;
        byte[] pre = Bytes.concat(Bytes.concat(Bytes.concat(previous, limb), sample), Bytes.le64(7));
        if (pre.length != 1096 || longNum(doc, "preimage_len") != 1096) {
            throw new IllegalStateException("preimage len");
        }
        if (!text(doc, "sha256").equals(sha256(pre)) || !text(doc, "preimage_hex").equals(Bytes.hex(pre))) {
            throw new IllegalStateException("preimage sha256");
        }
    }

    private static void checkAnchor(Json.Obj doc) {
        Draft object = Draft.quiet();
        Arrays.fill(object.stages.get(3).beacon, (byte) 0x55);
        object.stages.get(1).path = PathBody.encodePathInBlock(object.stages.get(1).waypoint, 4);
        object.stages.get(2).path = PathBody.encodePathInBlock(object.stages.get(2).waypoint, 100);
        object.stages.get(4).path = PathBody.encodePathInBlock(object.stages.get(4).waypoint, 7);
        byte[] raw = object.encode();
        if (!Bytes.hex(raw).equals(text(doc, "object_hex"))) {
            throw new IllegalStateException("object_hex");
        }
        requirePlant(doc, object.stages.get(1).waypoint, 4, false);
        List<Long> reads = new ArrayList<>();
        String line = Summary.packRecord(raw, Profile.PRODUCTION, height -> null, loc -> {
            reads.add(loc.blockHeight);
            return true;
        }, 1, null);
        if (!text(doc, "record").equals(line)) {
            throw new IllegalStateException("anchor record " + line);
        }
        Json.Arr wantReads = Json.asArr(doc.fields().get("index_reads"));
        if (wantReads.items().size() != reads.size()) {
            throw new IllegalStateException("reads " + reads);
        }
        for (int i = 0; i < reads.size(); i++) {
            if (Long.parseLong(Json.asNumber(wantReads.items().get(i))) != reads.get(i)) {
                throw new IllegalStateException("reads " + reads);
            }
        }
        Json.Obj cases = Json.asObj(doc.fields().get("cases"));
        Json.Arr anchors = Json.asArr(Json.asObj(Json.parse(line)).fields().get("anchors"));
        if (!text(cases, "inside_the_window").equals(Json.asString(anchors.items().get(0)))
                || !text(cases, "outside_the_window").equals(Json.asString(anchors.items().get(1)))
                || !text(cases, "header_not_in_the_object").equals(Json.asString(anchors.items().get(3)))
                || !text(cases, "no_path").equals(Json.asString(anchors.items().get(4)))) {
            throw new IllegalStateException("cases");
        }
    }

    private static void checkW16(Json.Obj doc) {
        if (!"W_16 included in I_17".equals(text(doc, "case"))) {
            throw new IllegalStateException("case");
        }
        Draft object = Draft.quiet();
        object.lastInterrupt = filled(0x55, 32);
        object.lastInterruptHeight = 19L;
        object.stages.get(16).path = PathBody.encodePathInBlock(object.stages.get(16).waypoint, 19);
        byte[] raw = object.encode();
        if (!Bytes.hex(raw).equals(text(doc, "object_hex"))) {
            throw new IllegalStateException("object_hex");
        }
        requirePlant(doc, object.stages.get(16).waypoint, 19, false);
        List<Long> reads = new ArrayList<>();
        String line = Summary.packRecord(raw, Profile.PRODUCTION, height -> null, loc -> {
            reads.add(loc.blockHeight);
            return true;
        }, 1, null);
        if (!text(doc, "record").equals(line)) {
            throw new IllegalStateException("w16 record " + line);
        }
        Json.Arr wantReads = Json.asArr(doc.fields().get("index_reads"));
        if (wantReads.items().size() != reads.size()) {
            throw new IllegalStateException("w16 reads " + reads);
        }
        for (int i = 0; i < reads.size(); i++) {
            if (Long.parseLong(Json.asNumber(wantReads.items().get(i))) != reads.get(i)) {
                throw new IllegalStateException("w16 reads " + reads);
            }
        }
        Json.Arr anchors = Json.asArr(Json.asObj(Json.parse(line)).fields().get("anchors"));
        if (anchors.items().size() != 16 || !"anchored".equals(Json.asString(anchors.items().get(15)))) {
            throw new IllegalStateException("w16 anchors");
        }
        for (int i = 0; i < 15; i++) {
            if (!"not anchored".equals(Json.asString(anchors.items().get(i)))) {
                throw new IllegalStateException("w16 anchors");
            }
        }
    }

    private static void checkEarlier(Path root, Json.Obj doc) throws Exception {
        byte[] body = Files.readAllBytes(root.resolve(text(doc, "object")));
        if (!text(doc, "sha256").equals(sha256(body)) || !text(doc, "object_hex").equals(Bytes.hex(body))) {
            throw new IllegalStateException("run-b sha256");
        }
        boolean[] called = {false};
        String line = Summary.packRecord(body, Profile.PRODUCTION, height -> {
            called[0] = true;
            return null;
        }, loc -> {
            called[0] = true;
            return true;
        }, Hike.DEFAULT_BIND_THREADS, null);
        if (called[0] || !text(doc, "record").equals(line)) {
            throw new IllegalStateException("earlier " + line);
        }
    }

    private static void checkBind(Json.Obj doc) {
        Json.Arr threads = Json.asArr(doc.fields().get("threads"));
        if (threads.items().size() != 2
                || Long.parseLong(Json.asNumber(threads.items().get(0))) != 1
                || Long.parseLong(Json.asNumber(threads.items().get(1))) != Hike.DEFAULT_BIND_THREADS) {
            throw new IllegalStateException("threads");
        }
        Draft object = Draft.quiet();
        Draft.Stage stage = object.stages.get(3);
        stage.legCount = 4;
        for (int j = 0; j < 4; j++) {
            byte[] checkpoint = new byte[32];
            Arrays.fill(checkpoint, (byte) j);
            stage.checkpoints.add(checkpoint);
            byte[] leg = new byte[8];
            leg[0] = (byte) j;
            stage.legResults.add(leg);
        }
        byte[] raw = object.encode();
        if (!Bytes.hex(raw).equals(text(doc, "object_hex"))) {
            throw new IllegalStateException("object_hex");
        }
        String one = Summary.packRecord(raw, Profile.PRODUCTION, height -> null, null, 1, null);
        String many = Summary.packRecord(raw, Profile.PRODUCTION, height -> null, null, Hike.DEFAULT_BIND_THREADS, null);
        if (!text(doc, "record").equals(one) || !one.equals(many)) {
            throw new IllegalStateException("bind " + one);
        }
    }

    private static Json.Obj load(Path path) throws Exception {
        return Json.asObj(Json.parse(Files.readString(path)));
    }

    private static String text(Json.Obj obj, String key) {
        return Json.asString(obj.fields().get(key));
    }

    /** #302: every SPEC section 6 name has a value in constants.json that matches this port. */
    private static void checkSection6(Json.Obj doc) {
        java.util.Map<String, Object> want = pcoc.hike.Spec6Pins.section6();
        if (want.size() != 22) {
            throw new IllegalStateException("section 6 table size " + want.size());
        }
        for (var e : want.entrySet()) {
            boolean ok = e.getValue() instanceof Long n
                    ? longNum(doc, e.getKey()) == n
                    : e.getValue().equals(text(doc, e.getKey()));
            if (!ok) {
                throw new IllegalStateException("constants " + e.getKey());
            }
        }
    }

    private static long longNum(Json.Obj obj, String key) {
        return Long.parseLong(Json.asNumber(obj.fields().get(key)));
    }

    private static List<String> strings(Json.Obj obj, String key) {
        List<String> out = new ArrayList<>();
        for (Json item : Json.asArr(obj.fields().get(key)).items()) {
            out.add(Json.asString(item));
        }
        return out;
    }

    private static byte[] filled(int value, int n) {
        byte[] out = new byte[n];
        Arrays.fill(out, (byte) value);
        return out;
    }

    private static String sha256(byte[] data) throws Exception {
        byte[] digest = MessageDigest.getInstance("SHA-256").digest(data);
        return toHex(digest);
    }

    private static String toHex(byte[] data) {
        StringBuilder out = new StringBuilder(data.length * 2);
        for (byte item : data) {
            out.append(Character.forDigit((item >> 4) & 0xf, 16));
            out.append(Character.forDigit(item & 0xf, 16));
        }
        return out.toString();
    }
}
