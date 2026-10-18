package pcoc.hike;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ConcurrentHashMap;
import pcoc.json.Json;

/**
 * Section 14 measurement. A profile writes a record when the buffer does
 * not open, and when a trailing byte follows the map. An earlier kind
 * returns the version line and is not measured. The record does not carry
 * a section 15 verdict.
 */
public final class Measure {
    public interface Reader {
        String body(long height);
    }

    private static final ConcurrentHashMap<String, Terrain> TERRAIN = new ConcurrentHashMap<>();
    private static final String[] SHAPE_ORDER = {
        "field set",
        "stage-0 path key",
        "W_17 path",
        "constant",
        "stages length",
        "leg_count",
        "checkpoint width",
        "leg result width",
        "seed beacon",
        "last_interrupt",
        "header height order",
        "header hash differ",
        "height_range"
    };

    private Measure() {}

    /** Compact record JSON. Bytes whose first item does not decode, or is not a map, get the {@code ["malformed cbor"]} record. */
    public static String record(byte[] object, Profile profile, Reader reader, byte[] stream, boolean omitSix) {
        return record(object, profile, reader, null, stream, omitSix);
    }

    /**
     * Section 14 record. {@code chain} is the bind for a present path in the
     * anchor window that names a stored interrupt header. Null accepts that bind.
     */
    public static String record(byte[] object, Profile profile, Reader reader, Summary.Chain chain, byte[] stream, boolean omitSix) {
        boolean v2 = Hike.KIND.equals(profile.kind);
        Cbor.Val top;
        boolean trailing;
        Cbor.Opened head;
        try {
            head = Cbor.decodeHead(object);
        } catch (Cbor.Problem problem) {
            return undecoded(profile);
        }
        if (head.value.kind != Cbor.Val.Kind.MAP) {
            // Section 14.1: a top level that is not a map is exactly ["malformed cbor"].
            return undecoded(profile);
        }
        top = head.value;
        trailing = head.trailing;
        if (Hike.isEarlierKind(text(top, "kind"))) {
            return earlierLine(text(top, "kind"));
        }
        if (trailing && !usable(top)) {
            return undecoded(profile);
        }
        List<Cbor.Val> stages = stages(top);
        byte[] publicKey = bytes(top, "public_key", 32);
        byte[] seedBeacon = bytes(top, "seed_beacon", 32);
        byte[] computedSeed = publicKey != null && seedBeacon != null ? Bytes.sha256(publicKey, seedBeacon) : null;
        List<String> items = new ArrayList<>();
        if (trailing) {
            items.add("malformed cbor");
        } else {
            boolean[] bad = shape(top, stages, profile);
            for (int i = 0; i < SHAPE_ORDER.length; i++) {
                if (bad[i]) {
                    items.add(SHAPE_ORDER[i]);
                }
            }
        }
        Terrain terrain = computedSeed == null ? null : terrain(publicKey, seedBeacon, profile);
        Header[] headers = headers(top, stages, reader);
        int[] counts = new int[Hike.STAGES];
        Json.Arr checkpointRows = checkpoints(stages, terrain, computedSeed, counts);
        Json.Arr waypointRows = waypoints(stages, terrain);
        PathTally paths = paths(top, stages, profile, headers, chain);
        BookFields book = v2
                ? v2Book(paths, headers, counts, reader)
                : BookFields.token("not defined");
        Json.Obj fixed = Json.obj().put("N", book.fixedN).put("S", book.fixedS).put("delta_h", book.fixedDelta);
        Json.Arr perStage = Json.arr();
        long wholeLegs = 0;
        boolean wholeDefined = true;
        for (int g = 0; g < Hike.STAGES; g++) {
            if (g > 0) {
                if (counts[g] < 0) {
                    wholeDefined = false;
                } else {
                    wholeLegs += counts[g];
                }
            }
            Json stageSpan = g == 17
                    ? span(timeAt(headers, 18), timeAt(headers, 17))
                    : span(timeAt(headers, g == 0 ? 1 : g + 1), timeAt(headers, g == 0 ? 0 : g));
            Json count = counts[g] < 0 ? Json.str("not defined") : Json.num(counts[g]);
            perStage.add(Json.obj().put("g", g).put("checkpoint_count", count).put("span", stageSpan));
        }
        Json.Obj analysis = Json.obj()
                .put("whole_hike_legs", wholeDefined ? Json.num(wholeLegs) : Json.str("not defined"))
                .put("whole_hike_span", span(timeAt(headers, 18), timeAt(headers, 1)))
                .put("per_stage", perStage)
                .put("legs_outside", book.outside);
        Json.Obj out = Json.obj();
        out.put("profile", profile.name);
        if ("conformance-small".equals(profile.name)) {
            out.put("sample_bytes", profile.sampleBytes);
            out.put("slot_bytes", profile.slotBytes);
            out.put("terrain_bytes", profile.terrainBytes);
            out.put("steps_per_leg", profile.stepsPerLeg);
        }
        out.put("shape", items.isEmpty() ? "match" : "mismatch");
        Json.Arr shapeItems = Json.arr();
        for (String item : items) {
            shapeItems.add(item);
        }
        out.put("shape_item", shapeItems);
        Json.Arr headerArr = Json.arr();
        for (Header header : headers) {
            headerArr.add(header.json);
        }
        out.put("headers", headerArr);
        out.put("seed_value", seedToken(stages, computedSeed));
        out.put("checkpoints", checkpointRows);
        out.put("waypoints", waypointRows);
        out.put("paths", paths.rows);
        if (v2) {
            out.put("anchors", paths.anchors);
        }
        out.put("anchored_total", paths.anchored);
        out.put("skip_list", paths.skips);
        out.put("a", book.a);
        out.put("b", book.b);
        out.put("N", book.n);
        out.put("S", book.s);
        out.put("delta_h", book.delta);
        out.put("fixed_start", fixed);
        out.put("P_b", book.pb);
        out.put("height_P_b", book.heightPb);
        out.put("index_comparison_P_b", book.cmpPb);
        out.put("T_P_b", book.tPb);
        out.put("height_I_a_minus_1", book.heightI);
        out.put("T_I_a_minus_1", book.tI);
        if (!omitSix) {
            long legTotal = 0;
            for (int count : counts) {
                if (count >= 0) {
                    legTotal += count;
                }
            }
            Drawn drawn = drawSix(stream, legTotal);
            if (drawn.token != null) {
                out.put("six_leg", drawn.token);
                // `not read` is the string. Fewer than six legs is `[]`.
                // That array is `[]` only in that case.
                boolean stringRuns = "not read".equals(drawn.token);
                out.put("six_leg_runs", stringRuns ? Json.str(drawn.token) : Json.arr());
            } else {
                Json.Obj six = sixLeg(drawn.indices, stages, terrain, computedSeed, counts);
                out.put("six_leg", six);
                out.put("six_leg_runs", Json.arr().add(six));
            }
        }
        out.put("analysis", analysis);
        return out.compact() + "\n";
    }

    /** Rule A4.11. Fewer than two anchored paths leaves bookends not defined. */
    private static BookFields v2Book(PathTally paths, Header[] headers, int[] counts, Reader reader) {
        if (paths.present < 2) {
            return BookFields.token("not defined");
        }
        if (paths.plantReadNotRead) {
            return BookFields.token("not read");
        }
        if (paths.anchoredAt.size() < 2) {
            return BookFields.token("not defined");
        }
        int a = paths.anchoredAt.get(0) + 1;
        int b = paths.anchoredAt.get(paths.anchoredAt.size() - 1) + 1;
        if (a >= headers.length || "not read".equals(Json.asString(headers[a].json.fields().get("index_comparison")))) {
            return BookFields.token("not read");
        }
        if (b - 1 >= paths.blockHash.length || paths.blockHash[b - 1] == null) {
            return BookFields.token("not read");
        }
        Compared pb = compareBlock(paths.blockHash[b - 1], paths.blockHeight[b - 1], reader);
        if ("not read".equals(pb.cmp)) {
            return BookFields.token("not read");
        }
        Json[] endA = endpoint(headers[a]);
        Json heightPb;
        Json timePb;
        if ("match".equals(pb.cmp)) {
            heightPb = pb.height;
            timePb = pb.time;
        } else {
            heightPb = Json.str("not defined");
            timePb = Json.str("not read");
        }
        Json[] end1 = endpoint(headers[1]);
        BookFields ready = new BookFields();
        ready.a = Json.num(a);
        ready.b = Json.num(b);
        ready.n = countRange(counts, a, b);
        ready.s = sub(timePb, endA[1], "not read");
        ready.delta = sub(heightPb, endA[0], "not defined");
        ready.fixedN = countRange(counts, 1, b);
        ready.fixedS = sub(timePb, end1[1], "not read");
        ready.fixedDelta = fixedDelta(headers[1], pb.cmp, heightPb);
        ready.pb = blockJson(paths.blockHash[b - 1], paths.blockHeight[b - 1]);
        ready.heightPb = heightPb;
        ready.cmpPb = Json.str(pb.cmp);
        ready.tPb = timePb;
        ready.heightI = endA[0];
        ready.tI = endA[1];
        ready.outside = legsOutside(counts, a, b);
        return ready;
    }

    private static Json[] endpoint(Header header) {
        if (!"match".equals(Json.asString(header.json.fields().get("index_comparison")))) {
            return new Json[] {Json.str("not defined"), Json.str("not read")};
        }
        return new Json[] {header.json.fields().get("stored_height"), header.json.fields().get("T")};
    }

    private static Compared compareBlock(byte[] hash, long height, Reader reader) {
        Compared out = new Compared();
        if (Hike.aboveJson(height) || reader == null) {
            out.cmp = "not read";
            return out;
        }
        String body = reader.body(height);
        String[] measured = Index.headerMeasurement(hash, height, body == null ? "" : body, body != null);
        out.cmp = measured[0];
        if ("match".equals(measured[0])) {
            out.height = Json.num(height);
            if (Index.integerToken(measured[1])) {
                out.time = Json.num(Long.parseLong(measured[1]));
            } else {
                out.time = Json.str("not read");
            }
        }
        return out;
    }

    private static Json sub(Json later, Json earlier, String missing) {
        if (!(later instanceof Json.Num) || !(earlier instanceof Json.Num)) {
            return Json.str(missing);
        }
        return Json.num(Long.parseLong(Json.asNumber(later)) - Long.parseLong(Json.asNumber(earlier)));
    }

    private static Json fixedDelta(Header header, String pbCmp, Json heightPb) {
        String cmp = Json.asString(header.json.fields().get("index_comparison"));
        if ("not read".equals(cmp) || "not read".equals(pbCmp)) {
            return Json.str("not read");
        }
        if ("mismatch".equals(cmp) || "mismatch".equals(pbCmp)) {
            return Json.str("not defined");
        }
        return sub(heightPb, endpoint(header)[0], "not defined");
    }

    private static Json countRange(int[] counts, int from, int until) {
        if (from >= until) {
            return Json.num(0);
        }
        long total = 0;
        for (int g = from; g < until; g++) {
            if (g < 0 || g >= counts.length || counts[g] < 0) {
                return Json.str("not defined");
            }
            total += counts[g];
        }
        return Json.num(total);
    }

    private static Json legsOutside(int[] counts, int a, int b) {
        long total = 0;
        for (int g = 0; g < counts.length; g++) {
            if (g >= a && g < b) {
                continue;
            }
            if (counts[g] < 0) {
                return Json.str("not defined");
            }
            total += counts[g];
        }
        return Json.num(total);
    }

    private static Terrain terrain(byte[] publicKey, byte[] seedBeacon, Profile profile) {
        String key = profile.name + ":" + Bytes.hex(publicKey) + ":" + Bytes.hex(seedBeacon);
        return TERRAIN.computeIfAbsent(key, ignored -> Terrain.fill(publicKey, seedBeacon, profile));
    }

    private static boolean[] shape(Cbor.Val top, List<Cbor.Val> stages, Profile profile) {
        boolean[] bad = new boolean[SHAPE_ORDER.length];
        bad[0] = fieldSet(top, stages);
        bad[1] = stage0Path(stages);
        bad[2] = w17Path(stages);
        bad[3] = constant(top, profile);
        bad[4] = stages == null || stages.size() != Hike.STAGES;
        bad[5] = legCount(stages);
        bad[6] = checkpointWidth(stages);
        bad[7] = legWidth(stages);
        bad[8] = seedBeacon(top, stages);
        bad[9] = lastInterrupt(top, stages);
        boolean[] order = headerOrder(top, stages);
        bad[10] = order[0];
        bad[11] = order[1];
        bad[12] = heightRange(top, stages);
        return bad;
    }

    private static boolean fieldSet(Cbor.Val top, List<Cbor.Val> stages) {
        if (!keys(top, "fill", "kind", "stages", "n_slots", "public_key", "slot_bytes", "seed_beacon", "sample_bytes", "steps_per_leg", "last_interrupt", "last_interrupt_height")) {
            return true;
        }
        if (bytes(top, "public_key", 32) == null || bytes(top, "seed_beacon", 32) == null) {
            return true;
        }
        if (!(text(top, "kind") instanceof String) || !(text(top, "fill") instanceof String)) {
            return true;
        }
        for (String name : new String[] {"slot_bytes", "n_slots", "sample_bytes", "steps_per_leg"}) {
            if (uint(top, name) == null) {
                return true;
            }
        }
        if (stages == null) {
            return true;
        }
        // last_interrupt present and not a byte string, or last_interrupt_height
        // present and not an unsigned integer, is field set (section 14.1 row 2).
        // A byte string of the wrong width is last_interrupt only (row 11).
        if (top.map.containsKey("last_interrupt")) {
            Cbor.Val value = top.map.get("last_interrupt");
            if (value == null || value.kind != Cbor.Val.Kind.BYTES) {
                return true;
            }
        }
        if (top.map.containsKey("last_interrupt_height") && uint(top, "last_interrupt_height") == null) {
            return true;
        }
        for (int g = 0; g < stages.size(); g++) {
            Cbor.Val stage = stages.get(g);
            if (stage == null || stage.kind != Cbor.Val.Kind.MAP) {
                return true;
            }
            String[] expect = g == 0
                    ? new String[] {"beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results"}
                    : new String[] {"path", "beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results"};
            if (!keys(stage, expect)) {
                return true;
            }
            if (bytes(stage, "beacon", 32) == null || bytes(stage, "waypoint", 32) == null) {
                return true;
            }
            if (g > 0 && bytes(stage, "path") == null) {
                return true;
            }
            // A checkpoint or leg-result value that is not an array is a wrong
            // value type (section 14.1 row 2). So is an element that is not a
            // byte string: the width conditions still apply, and field set is
            // not dropped because another condition is too.
            for (String key : new String[] {"checkpoints", "leg_results"}) {
                if (!stage.map.containsKey(key)) {
                    continue;
                }
                List<Cbor.Val> items = array(stage, key);
                if (items == null) {
                    return true;
                }
                for (Cbor.Val item : items) {
                    if (item == null || item.kind != Cbor.Val.Kind.BYTES) {
                        return true;
                    }
                }
            }
            if (uint(stage, "beacon_height") == null || uint(stage, "leg_count") == null) {
                return true;
            }
        }
        return false;
    }

    private static boolean stage0Path(List<Cbor.Val> stages) {
        if (stages == null || stages.isEmpty() || stages.get(0) == null || stages.get(0).kind != Cbor.Val.Kind.MAP) {
            return false;
        }
        return stages.get(0).map.containsKey("path");
    }

    private static boolean w17Path(List<Cbor.Val> stages) {
        if (stages == null || stages.size() <= 17 || stages.get(17) == null || stages.get(17).kind != Cbor.Val.Kind.MAP) {
            return true;
        }
        Cbor.Val path = stages.get(17).map.get("path");
        return path == null || path.kind != Cbor.Val.Kind.BYTES || path.bytes.length != 0;
    }

    private static boolean constant(Cbor.Val top, Profile profile) {
        String kind = text(top, "kind");
        String fill = text(top, "fill");
        Long slot = uint(top, "slot_bytes");
        Long slots = uint(top, "n_slots");
        Long sample = uint(top, "sample_bytes");
        Long steps = uint(top, "steps_per_leg");
        return kind == null || fill == null || slot == null || slots == null || sample == null || steps == null
                || !kind.equals(profile.kind)
                || !fill.equals(Hike.FILL)
                || slot != profile.slotBytes
                || slots != profile.nSlots
                || sample != profile.sampleBytes
                || steps != profile.stepsPerLeg;
    }

    private static boolean legCount(List<Cbor.Val> stages) {
        // Absent stages have no leg_count. Section 14.1: a condition that
        // tests a field's value is violated when that field is absent.
        // A longer array is only "stages length": stage 17 may still be present.
        if (stages == null || stages.size() < Hike.STAGES) {
            return true;
        }
        for (Cbor.Val stage : stages) {
            if (stage == null || stage.kind != Cbor.Val.Kind.MAP) {
                return true;
            }
            List<Cbor.Val> checkpoints = array(stage, "checkpoints");
            List<Cbor.Val> results = array(stage, "leg_results");
            Long count = uint(stage, "leg_count");
            if (checkpoints == null || results == null || count == null || count < 1
                    || count != checkpoints.size() || results.size() != checkpoints.size()) {
                return true;
            }
        }
        return false;
    }

    private static boolean checkpointWidth(List<Cbor.Val> stages) {
        if (stages == null || stages.isEmpty()) {
            return true;
        }
        for (Cbor.Val stage : stages) {
            List<Cbor.Val> checkpoints = stage == null ? null : array(stage, "checkpoints");
            if (checkpoints == null) {
                return true;
            }
            for (Cbor.Val item : checkpoints) {
                if (item == null || item.kind != Cbor.Val.Kind.BYTES || item.bytes.length != 32) {
                    return true;
                }
            }
        }
        return false;
    }

    private static boolean legWidth(List<Cbor.Val> stages) {
        if (stages == null || stages.isEmpty()) {
            return true;
        }
        for (Cbor.Val stage : stages) {
            List<Cbor.Val> results = stage == null ? null : array(stage, "leg_results");
            if (results == null) {
                return true;
            }
            for (Cbor.Val item : results) {
                if (item == null || item.kind != Cbor.Val.Kind.BYTES || item.bytes.length != 8) {
                    return true;
                }
            }
        }
        return false;
    }

    private static boolean seedBeacon(Cbor.Val top, List<Cbor.Val> stages) {
        byte[] seed = bytes(top, "seed_beacon", 32);
        if (seed == null || stages == null || stages.isEmpty() || stages.get(0) == null) {
            return true;
        }
        byte[] beacon = bytes(stages.get(0), "beacon", 32);
        return beacon == null || !Bytes.eq(beacon, seed);
    }

    private static boolean lastInterrupt(Cbor.Val top, List<Cbor.Val> stages) {
        byte[] last = bytes(top, "last_interrupt", 32);
        Long height = uint(top, "last_interrupt_height");
        if (last == null || height == null || stages == null || stages.size() <= 17 || stages.get(17) == null) {
            return true;
        }
        byte[] beacon = bytes(stages.get(17), "beacon", 32);
        Long beaconHeight = uint(stages.get(17), "beacon_height");
        if (beacon == null || beaconHeight == null) {
            return true;
        }
        // Exact unsigned integers, including a value above 2^53−1.
        // Header height order uses the same integers.
        return Bytes.eq(last, beacon) || Long.compareUnsigned(height, beaconHeight) <= 0;
    }

    /** [height order bad, hash differ bad]. */
    private static boolean[] headerOrder(Cbor.Val top, List<Cbor.Val> stages) {
        byte[][] hashes = new byte[19][];
        Long[] heights = new Long[19];
        for (int g = 0; g < Hike.STAGES; g++) {
            Cbor.Val stage = stages != null && g < stages.size() ? stages.get(g) : null;
            hashes[g] = stage == null ? null : bytes(stage, "beacon", 32);
            heights[g] = stage == null ? null : uint(stage, "beacon_height");
        }
        hashes[18] = bytes(top, "last_interrupt", 32);
        heights[18] = uint(top, "last_interrupt_height");
        boolean heightBad = false;
        boolean hashBad = false;
        for (int i = 0; i < 19; i++) {
            if (hashes[i] == null) {
                hashBad = true;
            }
            // A height above 2^53−1 is written `not defined` (`height_range`)
            // and still takes part in the order, by its exact unsigned value.
            if (orderedHeight(heights[i]) == null) {
                heightBad = true;
            }
            if (i == 0) {
                continue;
            }
            if (hashes[i] != null && hashes[i - 1] != null && Bytes.eq(hashes[i], hashes[i - 1])) {
                hashBad = true;
            }
            Long height = orderedHeight(heights[i]);
            Long prev = orderedHeight(heights[i - 1]);
            if (height != null && prev != null && Long.compareUnsigned(height, prev) <= 0) {
                heightBad = true;
            }
        }
        if (!top.map.containsKey("last_interrupt")) {
            hashBad = true;
        }
        if (!top.map.containsKey("last_interrupt_height")) {
            heightBad = true;
        }
        return new boolean[] {heightBad, hashBad};
    }

    /** The exact stored height. A value above 2^53−1 still takes part in the order. */
    private static Long orderedHeight(Long height) {
        return height;
    }

    /** `height_range` is a stored height above 2^53−1: `beacon_height`, `last_interrupt_height`. */
    private static boolean heightRange(Cbor.Val top, List<Cbor.Val> stages) {
        Long last = uint(top, "last_interrupt_height");
        if (last != null && Hike.aboveJson(last)) {
            return true;
        }
        if (stages == null) {
            return false;
        }
        for (Cbor.Val stage : stages) {
            Long height = uint(stage, "beacon_height");
            if (height != null && Hike.aboveJson(height)) {
                return true;
            }
        }
        return false;
    }

    private static Header[] headers(Cbor.Val top, List<Cbor.Val> stages, Reader reader) {
        Header[] rows = new Header[19];
        for (int g = 0; g < Hike.STAGES; g++) {
            Cbor.Val stage = stages != null && g < stages.size() ? stages.get(g) : null;
            rows[g] = header(stage == null ? null : bytes(stage, "beacon", 32), stage == null ? null : uint(stage, "beacon_height"), reader);
        }
        rows[18] = header(bytes(top, "last_interrupt", 32), uint(top, "last_interrupt_height"), reader);
        return rows;
    }

    private static Header header(byte[] hash, Long height, Reader reader) {
        Json.Obj json = Json.obj();
        json.put("stored_hash", hash == null ? Json.str("not defined") : Json.str(Bytes.hex(hash)));
        boolean usable = height != null && !Hike.aboveJson(height);
        if (!usable) {
            json.put("stored_height", "not defined");
            json.put("index_comparison", "not read");
            json.put("T", "not read");
            return new Header(json, null);
        }
        json.put("stored_height", height);
        if (hash == null || reader == null) {
            json.put("index_comparison", "not read");
            json.put("T", "not read");
            return new Header(json, null);
        }
        String body = reader.body(height);
        String[] measured = Index.headerMeasurement(hash, height, body, body != null);
        json.put("index_comparison", measured[0]);
        if (Index.integerToken(measured[1])) {
            long time = Long.parseLong(measured[1]);
            json.put("T", time);
            return new Header(json, time);
        }
        json.put("T", measured[1]);
        return new Header(json, null);
    }

    private static String seedToken(List<Cbor.Val> stages, byte[] computedSeed) {
        if (computedSeed == null || stages == null || stages.isEmpty() || stages.get(0) == null) {
            return "mismatch";
        }
        byte[] stored = bytes(stages.get(0), "waypoint", 32);
        return stored != null && Bytes.eq(stored, computedSeed) ? "match" : "mismatch";
    }

    private static Json.Arr checkpoints(List<Cbor.Val> stages, Terrain terrain, byte[] computedSeed, int[] counts) {
        Json.Arr rows = Json.arr();
        for (int g = 0; g < Hike.STAGES; g++) {
            Cbor.Val stage = stages != null && g < stages.size() ? stages.get(g) : null;
            if (stage == null) {
                counts[g] = -1;
                rows.add(Json.obj().put("g", g).put("legs", "not defined"));
                continue;
            }
            List<Cbor.Val> stored = array(stage, "checkpoints");
            List<Cbor.Val> results = array(stage, "leg_results");
            counts[g] = stored == null ? 0 : stored.size();
            if (stored == null || badWidth(stored, 32)) {
                rows.add(Json.obj().put("g", g).put("legs", "not defined"));
                continue;
            }
            byte[] limb = g == 0 ? computedSeed : bytes(stage, "waypoint", 32);
            Json.Arr legs = Json.arr();
            for (int j = 0; j < stored.size(); j++) {
                String token = "mismatch";
                if (limb != null && results != null && j < results.size()) {
                    byte[] result = results.get(j).kind == Cbor.Val.Kind.BYTES ? results.get(j).bytes : null;
                    byte[] previous = j == 0 ? limb : stored.get(j - 1).bytes;
                    if (result != null && result.length == 8 && previous != null && previous.length >= 8 && terrain != null) {
                        int offset = terrain.sampleOffset(previous);
                        byte[] digest = Bytes.sha256(previous, limb, terrain.blob, offset, terrain.profile.sampleBytes, result);
                        if (Bytes.eq(digest, stored.get(j).bytes)) {
                            token = "match";
                        }
                    }
                }
                legs.add(Json.obj().put("j", j).put("result", token));
            }
            rows.add(Json.obj().put("g", g).put("legs", legs));
        }
        return rows;
    }

    private static Json.Arr waypoints(List<Cbor.Val> stages, Terrain terrain) {
        Json.Arr rows = Json.arr();
        for (int g = 0; g < Hike.STAGES - 1; g++) {
            String token = "not defined";
            Cbor.Val stage = stages != null && g < stages.size() ? stages.get(g) : null;
            Cbor.Val next = stages != null && g + 1 < stages.size() ? stages.get(g + 1) : null;
            if (stage != null && next != null && terrain != null) {
                byte[] interrupt = bytes(next, "beacon", 32);
                byte[] limb = bytes(stage, "waypoint", 32);
                byte[] stored = bytes(next, "waypoint", 32);
                List<Cbor.Val> checkpoints = array(stage, "checkpoints");
                if (interrupt != null && limb != null && stored != null && checkpoints != null && !badWidth(checkpoints, 32)) {
                    byte[] fold = new byte[32];
                    for (Cbor.Val checkpoint : checkpoints) {
                        fold = Bytes.sha256(fold, checkpoint.bytes);
                    }
                    byte[] slot = terrain.slot(interrupt);
                    byte[] digest = Bytes.sha256(interrupt, slot, limb, fold);
                    token = Bytes.eq(digest, stored) ? "match" : "mismatch";
                }
            }
            rows.add(Json.obj().put("g", g + 1).put("result", token));
        }
        return rows;
    }

    private static PathTally paths(Cbor.Val top, List<Cbor.Val> stages, Profile profile, Header[] headers, Summary.Chain chain) {
        PathTally tally = new PathTally();
        boolean v2 = Hike.KIND.equals(profile.kind);
        for (int g = 1; g < Hike.STAGES; g++) {
            Cbor.Val stage = stages != null && g < stages.size() ? stages.get(g) : null;
            if (stage == null && v2) {
                // A missing stage is not an empty stored path. It is not `skip`.
                if (g == 17) {
                    tally.rows.add(Json.obj()
                            .put("g", 17)
                            .put("w17_path", "not defined")
                            .put("claimed_block", "not defined"));
                } else {
                    tally.anchors.add("not anchored");
                    tally.rows.add(Json.obj()
                            .put("g", g)
                            .put("path_state", "not defined")
                            .put("plant", "not defined")
                            .put("P_g", "not defined")
                            .put("height_P_g", "not defined")
                            .put("d_g", "not defined")
                            .put("claimed_block", "not defined"));
                }
                continue;
            }
            byte[] path = stage == null ? new byte[0] : bytes(stage, "path");
            if (path == null) {
                path = new byte[0];
            }
            if (g == 17) {
                Json.Obj row = Json.obj().put("g", 17);
                if (path.length == 0) {
                    row.put("w17_path", "not fetched by design");
                    row.put("claimed_block", "not defined");
                } else if (v2) {
                    row.put("w17_path", "not defined");
                    byte[] waypoint = bytes(stage, "waypoint", 32);
                    PathBody.Locators loc = PathBody.locatorsFor(path, waypoint);
                    row.put("claimed_block", loc == null ? Json.str("not defined") : blockJson(loc));
                } else {
                    row.put("w17_path", "not defined");
                    row.put("claimed_block", "not defined");
                }
                tally.rows.add(row);
                continue;
            }
            if (path.length == 0) {
                tally.skips.add(g);
                if (v2) {
                    tally.anchors.add("not anchored");
                }
                tally.rows.add(Json.obj()
                        .put("g", g)
                        .put("path_state", "skip")
                        .put("plant", "n/a")
                        .put("P_g", "not defined")
                        .put("height_P_g", "not defined")
                        .put("d_g", "not defined")
                        .put("claimed_block", "not defined"));
                continue;
            }
            if (!v2) {
                tally.rows.add(Json.obj()
                        .put("g", g)
                        .put("path_state", "present")
                        .put("plant", "mismatch")
                        .put("P_g", "not defined")
                        .put("height_P_g", "not defined")
                        .put("d_g", "not defined")
                        .put("claimed_block", "not defined"));
                continue;
            }
            byte[] waypoint = stage == null ? null : bytes(stage, "waypoint", 32);
            PathBody.Locators loc = PathBody.locatorsFor(path, waypoint);
            if (loc == null) {
                tally.anchors.add("not anchored");
                tally.present++;
                tally.rows.add(Json.obj()
                        .put("g", g)
                        .put("path_state", "present")
                        .put("plant", "mismatch")
                        .put("P_g", "not defined")
                        .put("height_P_g", "not defined")
                        .put("d_g", "not defined")
                        .put("claimed_block", "not defined"));
                continue;
            }
            Json claimed = blockJson(loc);
            Long beaconH = stage == null ? null : uint(stage, "beacon_height");
            long origin = beaconH == null ? 0L : beaconH;
            boolean inWindow = heightInWindow(origin, loc.blockHeight);
            boolean interrupt = interruptHeader(top, stages, loc.blockHash, loc.blockHeight);
            String plant = "not read";
            String anchor = "not anchored";
            if (inWindow && interrupt) {
                boolean ok = chain == null || chain.ok(loc);
                plant = ok ? "match" : "mismatch";
                anchor = ok ? "anchored" : "not anchored";
            }
            tally.anchors.add(anchor);
            tally.present++;
            tally.blockHash[g - 1] = loc.blockHash;
            tally.blockHeight[g - 1] = loc.blockHeight;
            if ("anchored".equals(anchor)) {
                tally.anchored++;
                tally.anchoredAt.add(g - 1);
            }
            Json pg = Json.str("not defined");
            Json heightPg = Json.str("not defined");
            Json dg = Json.str("not defined");
            if ("match".equals(plant)) {
                pg = claimed;
                if (!Hike.aboveJson(loc.blockHeight)) {
                    heightPg = Json.num(loc.blockHeight);
                }
                dg = plantGap(headers[g], loc.blockHeight);
            }
            tally.rows.add(Json.obj()
                    .put("g", g)
                    .put("path_state", "present")
                    .put("plant", plant)
                    .put("P_g", pg)
                    .put("height_P_g", heightPg)
                    .put("d_g", dg)
                    .put("claimed_block", claimed));
        }
        return tally;
    }

    private static Json blockJson(PathBody.Locators loc) {
        return Json.obj().put("block_hash", Bytes.displayHex(loc.blockHash)).put("block_height", loc.blockHeight);
    }

    private static Json plantGap(Header header, long plantHeight) {
        Json comparison = header.json.fields().get("index_comparison");
        Json stored = header.json.fields().get("stored_height");
        if (!"match".equals(Json.asString(comparison)) || !(stored instanceof Json.Num) || Hike.aboveJson(plantHeight)) {
            return Json.str("not defined");
        }
        return Json.num(plantHeight - Long.parseLong(Json.asNumber(stored)));
    }

    private static boolean heightInWindow(long beaconHeight, long inclusion) {
        long origin = Hike.anchorWindowOrigin(beaconHeight);
        for (int off : Hike.anchorOffsetsAfterPost()) {
            if (off < 0) {
                return false;
            }
            long sum = origin + Integer.toUnsignedLong(off);
            if (Long.compareUnsigned(sum, origin) < 0) {
                return false;
            }
            if (sum == inclusion) {
                return true;
            }
        }
        return false;
    }

    private static boolean interruptHeader(Cbor.Val top, List<Cbor.Val> stages, byte[] hash, long height) {
        if (stages != null) {
            for (int g = 1; g < stages.size() && g < Hike.STAGES; g++) {
                byte[] beacon = bytes(stages.get(g), "beacon", 32);
                Long stored = uint(stages.get(g), "beacon_height");
                if (beacon != null && stored != null && stored == height && Bytes.eq(beacon, hash)) {
                    return true;
                }
            }
        }
        byte[] last = bytes(top, "last_interrupt", 32);
        Long stored = uint(top, "last_interrupt_height");
        return last != null && stored != null && stored == height && Bytes.eq(last, hash);
    }

    /** Rule A5. Sections 14.1–14.6 are not applied. */
    private static String earlierLine(String kind) {
        return Json.obj().put("version", Hike.EARLIER_VERSION).put("kind", kind).compact() + "\n";
    }

    /**
     * No usable hike. shape_item is exactly ["malformed cbor"].
     * Counts and results are not defined. six_leg and six_leg_runs are not read.
     */
    private static String undecoded(Profile profile) {
        Json notDefined = Json.str("not defined");
        Json notRead = Json.str("not read");
        Json.Obj header = Json.obj()
                .put("stored_hash", notDefined)
                .put("stored_height", notDefined)
                .put("index_comparison", "not read")
                .put("T", notRead);
        Json.Arr headers = Json.arr();
        for (int i = 0; i < 19; i++) {
            headers.add(header);
        }
        Json.Arr checkpoints = Json.arr();
        for (int g = 0; g < Hike.STAGES; g++) {
            checkpoints.add(Json.obj().put("g", g).put("legs", notDefined));
        }
        Json.Arr waypoints = Json.arr();
        for (int g = 1; g < Hike.STAGES; g++) {
            waypoints.add(Json.obj().put("g", g).put("result", "not defined"));
        }
        Json.Arr pathRows = Json.arr();
        Json.Arr anchors = Json.arr();
        for (int g = 1; g < Hike.STAGES; g++) {
            if (g == 17) {
                pathRows.add(Json.obj().put("g", 17).put("w17_path", "not defined").put("claimed_block", notDefined));
            } else {
                pathRows.add(Json.obj()
                        .put("g", g)
                        .put("path_state", "not defined")
                        .put("plant", "not defined")
                        .put("P_g", notDefined)
                        .put("height_P_g", notDefined)
                        .put("d_g", notDefined)
                        .put("claimed_block", notDefined));
                anchors.add("not anchored");
            }
        }
        Json.Arr perStage = Json.arr();
        for (int g = 0; g < Hike.STAGES; g++) {
            perStage.add(Json.obj().put("g", g).put("checkpoint_count", notDefined).put("span", notRead));
        }
        Json.Obj analysis = Json.obj()
                .put("whole_hike_legs", notDefined)
                .put("whole_hike_span", notRead)
                .put("per_stage", perStage)
                .put("legs_outside", notDefined);
        Json.Obj fixed = Json.obj().put("N", notDefined).put("S", notDefined).put("delta_h", notDefined);
        Json.Obj out = Json.obj();
        out.put("profile", profile.name);
        if ("conformance-small".equals(profile.name)) {
            out.put("sample_bytes", profile.sampleBytes);
            out.put("slot_bytes", profile.slotBytes);
            out.put("terrain_bytes", profile.terrainBytes);
            out.put("steps_per_leg", profile.stepsPerLeg);
        }
        out.put("shape", "mismatch");
        out.put("shape_item", Json.arr().add("malformed cbor"));
        out.put("headers", headers);
        out.put("seed_value", "not defined");
        out.put("checkpoints", checkpoints);
        out.put("waypoints", waypoints);
        out.put("paths", pathRows);
        out.put("anchors", anchors);
        out.put("anchored_total", 0);
        out.put("skip_list", Json.arr());
        out.put("a", notDefined);
        out.put("b", notDefined);
        out.put("N", notDefined);
        out.put("S", notDefined);
        out.put("delta_h", notDefined);
        out.put("fixed_start", fixed);
        out.put("P_b", notDefined);
        out.put("height_P_b", notDefined);
        out.put("index_comparison_P_b", notDefined);
        out.put("T_P_b", notDefined);
        out.put("height_I_a_minus_1", notDefined);
        out.put("T_I_a_minus_1", notDefined);
        out.put("six_leg", notRead);
        out.put("six_leg_runs", notRead);
        out.put("analysis", analysis);
        return out.compact() + "\n";
    }

    /** Required fields present and typed. Shape notes do not make a hike unusable. */
    private static boolean usable(Cbor.Val top) {
        if (text(top, "kind") == null || text(top, "fill") == null) {
            return false;
        }
        for (String name : new String[] {"slot_bytes", "n_slots", "sample_bytes", "steps_per_leg"}) {
            if (uint(top, name) == null) {
                return false;
            }
        }
        if (bytes(top, "public_key", 32) == null || bytes(top, "seed_beacon", 32) == null) {
            return false;
        }
        List<Cbor.Val> rows = stages(top);
        if (rows == null) {
            return false;
        }
        for (Cbor.Val stage : rows) {
            if (!stageUsable(stage)) {
                return false;
            }
        }
        return true;
    }

    private static boolean stageUsable(Cbor.Val stage) {
        if (stage == null || stage.kind != Cbor.Val.Kind.MAP) {
            return false;
        }
        boolean pathKey = stage.map.containsKey("path");
        String[] expect = pathKey
                ? new String[] {"path", "beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results"}
                : new String[] {"beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results"};
        if (!keys(stage, expect)) {
            return false;
        }
        if (bytes(stage, "beacon", 32) == null || bytes(stage, "waypoint", 32) == null) {
            return false;
        }
        if (pathKey && bytes(stage, "path") == null) {
            return false;
        }
        Long legCount = uint(stage, "leg_count");
        if (legCount == null || uint(stage, "beacon_height") == null) {
            return false;
        }
        List<Cbor.Val> checks = array(stage, "checkpoints");
        List<Cbor.Val> legs = array(stage, "leg_results");
        if (checks == null || legs == null || badWidth(checks, 32) || badWidth(legs, 8)) {
            return false;
        }
        return legCount.longValue() == checks.size() && checks.size() == legs.size();
    }

    private static Json.Obj sixLeg(long[] indices, List<Cbor.Val> stages, Terrain terrain, byte[] computedSeed, int[] counts) {
        Json.Arr indexArr = Json.arr();
        Json.Arr results = Json.arr();
        for (long index : indices) {
            indexArr.add(index);
            int[] located = locate(counts, index);
            String token = "mismatch";
            if (located != null && terrain != null) {
                int g = located[0];
                int j = located[1];
                Cbor.Val stage = stages.get(g);
                List<Cbor.Val> stored = array(stage, "leg_results");
                List<Cbor.Val> checkpoints = array(stage, "checkpoints");
                // Section 14.6: leg 0 starts from the waypoint (or the computed seed on
                // stage 0); leg j >= 1 starts from the stored checkpoints[j - 1] alone.
                // Section 11.2 reads only its first 8 bytes. A previous that is not a
                // byte string of at least 8 bytes recomputes nothing: mismatch.
                byte[] previous = j == 0
                        ? (g == 0 ? computedSeed : bytes(stage, "waypoint", 32))
                        : (checkpoints != null && j - 1 < checkpoints.size()
                                && checkpoints.get(j - 1).kind == Cbor.Val.Kind.BYTES
                                ? checkpoints.get(j - 1).bytes : null);
                if (previous != null && previous.length < 8) {
                    previous = null;
                }
                if (stored != null && j < stored.size() && stored.get(j).kind == Cbor.Val.Kind.BYTES && stored.get(j).bytes.length == 8 && previous != null) {
                    long recomputed = terrain.leg(previous);
                    if (recomputed == Bytes.u64le(stored.get(j).bytes)) {
                        token = "match";
                    }
                }
            }
            results.add(token);
        }
        return Json.obj().put("indices", indexArr).put("results", results);
    }

    private static int[] locate(int[] counts, long index) {
        long start = 0;
        for (int g = 0; g < counts.length; g++) {
            if (index < start + counts[g]) {
                return new int[] {g, (int) (index - start)};
            }
            start += counts[g];
        }
        return null;
    }

    public static Drawn drawSix(byte[] stream, long legTotal) {
        if (legTotal < 6) {
            return Drawn.token("not defined");
        }
        if (stream == null) {
            stream = new byte[0];
        }
        long mod = modPow64(legTotal);
        long limit = mod == 0 ? 0 : -mod;
        List<Long> chosen = new ArrayList<>();
        int pos = 0;
        while (chosen.size() < 6) {
            if (pos + 8 > stream.length) {
                return Drawn.token("not read");
            }
            long draw = Bytes.u64le(stream, pos);
            pos += 8;
            if (mod == 0 || Long.compareUnsigned(draw, limit) < 0) {
                long index = Long.remainderUnsigned(draw, legTotal);
                if (!chosen.contains(index)) {
                    chosen.add(index);
                }
            }
        }
        long[] indices = new long[chosen.size()];
        for (int i = 0; i < chosen.size(); i++) {
            indices[i] = chosen.get(i);
        }
        return Drawn.indices(indices);
    }

    /** 2^64 mod divisor. Zero when the divisor is a power of two. */
    public static long modPow64(long divisor) {
        if (divisor > 0 && (divisor & (divisor - 1)) == 0) {
            return 0;
        }
        long remainder = Long.remainderUnsigned(-1L, divisor);
        long mod = remainder + 1;
        if (mod == divisor) {
            return 0;
        }
        return mod;
    }

    private static Json span(Long later, Long earlier) {
        if (later == null || earlier == null) {
            return Json.str("not read");
        }
        return Json.num(later - earlier);
    }

    private static Long timeAt(Header[] headers, int index) {
        if (index < 0 || index >= headers.length) {
            return null;
        }
        return headers[index].time;
    }

    private static boolean badWidth(List<Cbor.Val> items, int width) {
        for (Cbor.Val item : items) {
            if (item == null || item.kind != Cbor.Val.Kind.BYTES || item.bytes.length != width) {
                return true;
            }
        }
        return false;
    }

    private static List<Cbor.Val> stages(Cbor.Val top) {
        Cbor.Val value = top.map.get("stages");
        if (value == null || value.kind != Cbor.Val.Kind.ARRAY) {
            return null;
        }
        return value.array;
    }

    private static boolean keys(Cbor.Val map, String... expect) {
        if (map == null || map.kind != Cbor.Val.Kind.MAP || map.map.size() != expect.length) {
            return false;
        }
        for (String key : expect) {
            if (!map.map.containsKey(key)) {
                return false;
            }
        }
        return true;
    }

    private static byte[] bytes(Cbor.Val map, String key, int width) {
        byte[] raw = bytes(map, key);
        if (raw == null || raw.length != width) {
            return null;
        }
        return raw;
    }

    private static byte[] bytes(Cbor.Val map, String key) {
        if (map == null || map.kind != Cbor.Val.Kind.MAP) {
            return null;
        }
        Cbor.Val value = map.map.get(key);
        if (value == null || value.kind != Cbor.Val.Kind.BYTES) {
            return null;
        }
        return value.bytes;
    }

    private static String text(Cbor.Val map, String key) {
        if (map == null || map.kind != Cbor.Val.Kind.MAP) {
            return null;
        }
        Cbor.Val value = map.map.get(key);
        if (value == null || value.kind != Cbor.Val.Kind.TEXT) {
            return null;
        }
        return value.text;
    }

    private static Long uint(Cbor.Val map, String key) {
        if (map == null || map.kind != Cbor.Val.Kind.MAP) {
            return null;
        }
        Cbor.Val value = map.map.get(key);
        if (value == null || value.kind != Cbor.Val.Kind.UINT) {
            return null;
        }
        return value.uint;
    }

    private static List<Cbor.Val> array(Cbor.Val map, String key) {
        if (map == null || map.kind != Cbor.Val.Kind.MAP) {
            return null;
        }
        Cbor.Val value = map.map.get(key);
        if (value == null || value.kind != Cbor.Val.Kind.ARRAY) {
            return null;
        }
        return value.array;
    }

    private static final class Header {
        final Json.Obj json;
        final Long time;

        Header(Json.Obj json, Long time) {
            this.json = json;
            this.time = time;
        }
    }

    private static final class PathTally {
        final Json.Arr rows = Json.arr();
        final Json.Arr skips = Json.arr();
        final Json.Arr anchors = Json.arr();
        final List<Integer> anchoredAt = new ArrayList<>();
        final byte[][] blockHash = new byte[16][];
        final long[] blockHeight = new long[16];
        int anchored;
        int present;
        boolean plantReadNotRead;
    }

    private static final class BookFields {
        Json a;
        Json b;
        Json n;
        Json s;
        Json delta;
        Json fixedN;
        Json fixedS;
        Json fixedDelta;
        Json pb;
        Json heightPb;
        Json cmpPb;
        Json tPb;
        Json heightI;
        Json tI;
        Json outside;

        static BookFields token(String token) {
            Json value = Json.str(token);
            BookFields book = new BookFields();
            book.a = value;
            book.b = value;
            book.n = value;
            book.s = value;
            book.delta = value;
            book.fixedN = value;
            book.fixedS = value;
            book.fixedDelta = value;
            book.pb = value;
            book.heightPb = value;
            book.cmpPb = value;
            book.tPb = value;
            book.heightI = value;
            book.tI = value;
            book.outside = value;
            return book;
        }
    }

    private static final class Compared {
        String cmp = "not read";
        Json height = Json.str("not defined");
        Json time = Json.str("not read");
    }

    private static Json blockJson(byte[] hash, long height) {
        if (hash == null || hash.length != 32 || Hike.aboveJson(height)) {
            return Json.str("not defined");
        }
        return Json.obj().put("block_hash", Bytes.displayHex(hash)).put("block_height", height);
    }

    public static final class Drawn {
        public final long[] indices;
        public final String token;

        private Drawn(long[] indices, String token) {
            this.indices = indices;
            this.token = token;
        }

        static Drawn indices(long[] indices) {
            return new Drawn(indices, null);
        }

        static Drawn token(String token) {
            return new Drawn(null, token);
        }
    }
}
