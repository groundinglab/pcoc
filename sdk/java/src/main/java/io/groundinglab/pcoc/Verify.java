package io.groundinglab.pcoc;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * §14 verifier. Measurements only. One terrain fill per call, then §11.1 on every
 * recomputed step. Rule A5 returns before the fill.
 */
public final class Verify {
    private static final long JSON_MAX = 9007199254740991L;
    private static final BigInteger JSON_MAX_I = BigInteger.valueOf(JSON_MAX);

    private Verify() {}

    /** §14.7. The Rule A5 record, compact, no trailing newline. */
    public static String unknownKindJson(String kindText) {
        Json.Obj o = new Json.Obj();
        o.add("kind", Json.quote("unknown kind"));
        o.add("kind_text", Json.quote(kindText));
        return o.build();
    }

    public static final class Report {
        public final boolean decoded;
        public final boolean unknownKind;
        /** Compact record. The CLI adds one trailing LF. Null when not decoded. */
        public final String json;
        public final String shape;
        public final String seedValue;
        public final boolean checkpointsMatch;
        public final boolean waypointsMatch;
        public final String sixLeg;
        public final List<String> sixResults;
        public final int anchoredTotal;
        public final boolean anchorsNotAnchored;
        public final boolean pathsSkip;

        private Report(boolean decoded, boolean unknownKind, String json, String shape, String seedValue,
                       boolean checkpointsMatch, boolean waypointsMatch, String sixLeg, List<String> sixResults,
                       int anchoredTotal, boolean anchorsNotAnchored, boolean pathsSkip) {
            this.decoded = decoded;
            this.unknownKind = unknownKind;
            this.json = json;
            this.shape = shape;
            this.seedValue = seedValue;
            this.checkpointsMatch = checkpointsMatch;
            this.waypointsMatch = waypointsMatch;
            this.sixLeg = sixLeg;
            this.sixResults = sixResults;
            this.anchoredTotal = anchoredTotal;
            this.anchorsNotAnchored = anchorsNotAnchored;
            this.pathsSkip = pathsSkip;
        }

        static Report undecoded() {
            return new Report(false, false, null, null, null, false, false, null, new ArrayList<String>(), 0, false, false);
        }

        static Report unknown(String kindText) {
            return new Report(true, true, unknownKindJson(kindText), null, null, false, false, null,
                    new ArrayList<String>(), 0, false, false);
        }
    }

    /**
     * §14 release path. The six-leg draw reads the operating-system CSPRNG.
     * This method has no byte-source parameter.
     */
    public static Report verify(byte[] object, Profile profile, Index index) {
        return verifyAt(object, profile, index, null);
    }

    /**
     * §14.6 / §16.1 rig path. {@code draw} is the supplied byte source (a label chain
     * or finite injected bytes). It is not the release path.
     */
    public static Report verifyRig(byte[] object, Profile profile, Index index, Draw.Source draw) {
        if (draw == null) {
            throw new IllegalArgumentException("draw");
        }
        return verifyAt(object, profile, index, draw);
    }

    /**
     * §7.3 decode order, then Rule A5, then section 14.
     * {@code draw} null selects the operating-system CSPRNG inside the six-leg check.
     */
    private static Report verifyAt(byte[] object, Profile profile, Index index, Draw.Source draw) {
        Cbor.Parsed parsed = Cbor.parse(object);
        // First: not one definite-length CBOR data item, including empty input (§7.3).
        if (parsed.error != null || parsed.item == null) {
            return measure(parsed, profile, index == null ? Index.EMPTY : index, draw);
        }
        // Second: a well-formed item that is not a map, or a map with no kind text, is a decode failure.
        if (parsed.item.kind != Cbor.Kind.MAP || parsed.kindText == null) {
            return Report.undecoded();
        }
        // Third: a kind text that is not KIND is Rule A5. A malformed value never reaches this.
        if (!Const.KIND.equals(parsed.kindText)) {
            return Report.unknown(parsed.kindText);
        }
        return measure(parsed, profile, index == null ? Index.EMPTY : index, draw);
    }

    private static Report measure(Cbor.Parsed parsed, Profile profile, Index index, Draw.Source draw) {
        boolean malformed = parsed.error != null;
        Cbor.Item root = parsed.item;
        boolean rootMap = root != null && root.kind == Cbor.Kind.MAP;

        Stage[] stages = new Stage[Const.STAGES];
        for (int g = 0; g < Const.STAGES; g++) {
            stages[g] = Stage.missing();
        }
        byte[] publicKey = null;
        byte[] seedBeacon = null;
        byte[] lastInterrupt = null;
        Long lastHeight = null;
        boolean lastHeightOver = false;
        Long slotBytes = null;
        Long nSlots = null;
        Long sampleBytes = null;
        Long steps = null;
        String fill = null;
        boolean fieldSet = !rootMap;
        boolean heightRange = false;
        int stagesLen = -1;

        if (rootMap) {
            fieldSet = badTop(root);
            fill = text(root, "fill");
            publicKey = bytesOf(root, "public_key");
            seedBeacon = bytesOf(root, "seed_beacon");
            lastInterrupt = bytesOf(root, "last_interrupt");
            Cbor.Item lh = root.field("last_interrupt_height");
            if (lh != null && lh.kind == Cbor.Kind.UINT) {
                if (over(lh)) {
                    lastHeightOver = true;
                    heightRange = true;
                } else {
                    lastHeight = lh.uint.longValue();
                }
            }
            slotBytes = uintOk(root, "slot_bytes");
            nSlots = uintOk(root, "n_slots");
            sampleBytes = uintOk(root, "sample_bytes");
            steps = uintOk(root, "steps_per_leg");
            if (anyOver(root)) {
                heightRange = true;
            }
            Cbor.Item st = root.field("stages");
            if (st != null && st.kind == Cbor.Kind.ARRAY) {
                stagesLen = st.array.size();
                for (int g = 0; g < st.array.size() && g < Const.STAGES; g++) {
                    stages[g] = readStage(st.array.get(g), g == 0);
                    if (stages[g].fieldSet) {
                        fieldSet = true;
                    }
                    if (stages[g].heightOver) {
                        heightRange = true;
                    }
                }
            }
        }

        boolean stage0Path = stages[0].pathKey;
        boolean w17 = !(stages[17].present && stages[17].pathKey && stages[17].path != null && stages[17].path.length == 0);
        boolean constant = !Const.KIND.equals(parsed.kindText) || !"dag2".equals(fill)
                || slotBytes == null || slotBytes != profile.slotBytes
                || nSlots == null || nSlots != profile.nSlots
                || sampleBytes == null || sampleBytes != profile.sampleBytes
                || steps == null || steps != profile.stepsPerLeg;
        boolean stagesLength = stagesLen != Const.STAGES;
        boolean legCount = false;
        boolean cpWidth = false;
        boolean lrWidth = false;
        for (int g = 0; g < Const.STAGES; g++) {
            Stage s = stages[g];
            if (!s.present || s.checkpoints == null || s.legResults == null) {
                legCount = true;
                cpWidth = true;
                lrWidth = true;
                continue;
            }
            int nCp = s.checkpoints.length;
            int nLr = s.legResults.length;
            if (s.legCount == null || s.legCount < 1 || s.legCount != nCp || s.legCount != nLr) {
                legCount = true;
            }
            if (s.checkpointBadType) {
                cpWidth = true;
            }
            if (s.legBadType) {
                lrWidth = true;
            }
            for (byte[] c : s.checkpoints) {
                if (c == null || c.length != 32) {
                    cpWidth = true;
                }
            }
            for (byte[] r : s.legResults) {
                if (r == null || r.length != 8) {
                    lrWidth = true;
                }
            }
        }
        boolean seedBeaconBad = seedBeacon == null || seedBeacon.length != 32
                || stages[0].beacon == null || !Bytes.eq(stages[0].beacon, seedBeacon);
        boolean lastBad = lastInterrupt == null || lastInterrupt.length != 32
                || stages[17].beacon == null || Bytes.eq(lastInterrupt, stages[17].beacon)
                || lastHeight == null || stages[17].beaconHeight == null || lastHeight <= stages[17].beaconHeight;
        boolean heightOrder = !heightsIncrease(stages, lastHeight);
        boolean hashDiffer = !hashesDiffer(stages, lastInterrupt);

        List<String> items = new ArrayList<String>();
        if (malformed) {
            items.add("malformed cbor");
        } else {
            if (fieldSet) {
                items.add("field set");
            }
            if (stage0Path) {
                items.add("stage-0 path key");
            }
            if (w17) {
                items.add("W_17 path");
            }
            if (constant) {
                items.add("constant");
            }
            if (stagesLength) {
                items.add("stages length");
            }
            if (legCount) {
                items.add("leg_count");
            }
            if (cpWidth) {
                items.add("checkpoint width");
            }
            if (lrWidth) {
                items.add("leg result width");
            }
            if (seedBeaconBad) {
                items.add("seed beacon");
            }
            if (lastBad) {
                items.add("last_interrupt");
            }
            if (heightOrder) {
                items.add("header height order");
            }
            if (hashDiffer) {
                items.add("header hash differ");
            }
            if (heightRange || lastHeightOver) {
                items.add("height_range");
            }
        }
        String shape = items.isEmpty() ? "match" : "mismatch";

        Sha256 sha = new Sha256();
        byte[] seed = null;
        long[] terrain = null;
        if (publicKey != null && publicKey.length == 32 && seedBeacon != null && seedBeacon.length == 32) {
            seed = Seed.derive(sha, publicKey, seedBeacon);
            terrain = Terrain.fill(profile, Seed.lanes(seed));
        }
        String seedValue = (seed != null && stages[0].waypoint != null && Bytes.eq(stages[0].waypoint, seed))
                ? "match" : "mismatch";

        Reads reads = new Reads(index);
        HeaderM[] headers = new HeaderM[19];
        for (int k = 0; k < 18; k++) {
            headers[k] = header(reads, stages[k].beacon, stages[k].beaconHeight);
        }
        headers[18] = header(reads, lastInterrupt != null && lastInterrupt.length == 32 ? lastInterrupt : null,
                lastHeight);

        boolean cpMatch = true;
        int cpSeen = 0;
        String[] cpJson = new String[Const.STAGES];
        for (int g = 0; g < Const.STAGES; g++) {
            cpJson[g] = checkpointStage(sha, profile, terrain, seed, stages[g], g);
            if (stages[g].checkpoints == null) {
                cpMatch = false;
            } else {
                for (int j = 0; j < stages[g].checkpoints.length; j++) {
                    cpSeen++;
                    String result = legResultToken(sha, profile, terrain, seed, stages[g], g, j);
                    if (!"match".equals(result)) {
                        cpMatch = false;
                    }
                }
            }
        }
        if (cpSeen == 0) {
            cpMatch = false;
        }

        boolean wpMatch = true;
        String[] wpJson = new String[17];
        String[] wpTok = new String[17];
        for (int g = 0; g < 17; g++) {
            wpTok[g] = waypointToken(sha, profile, terrain, stages[g], stages[g + 1]);
            if (!"match".equals(wpTok[g])) {
                wpMatch = false;
            }
            Json.Obj o = new Json.Obj();
            o.add("g", Integer.toString(g + 1));
            o.add("result", Json.quote(wpTok[g]));
            wpJson[g] = o.build();
        }

        PathM[] paths = new PathM[17];
        String[] anchorTok = new String[16];
        int anchored = 0;
        boolean allSkip = true;
        boolean allNot = true;
        List<Integer> skip = new ArrayList<Integer>();
        List<Boolean> issuedMiss = new ArrayList<Boolean>();
        int validated = 0;
        for (int g = 1; g <= 17; g++) {
            paths[g - 1] = pathOf(sha, reads, stages, lastInterrupt, lastHeight, g, headers, issuedMiss);
            if (g <= 16) {
                // Erratum 2 (E02). A malformed CBOR value has no path byte string.
                // path_state and plant are not defined. skip is only an empty path
                // on a decoded stage (Rule A4.5), so this g is not a skip_list entry.
                if (malformed) {
                    paths[g - 1].pathState = "not defined";
                    paths[g - 1].plant = "not defined";
                }
                anchorTok[g - 1] = paths[g - 1].anchor;
                if ("anchored".equals(paths[g - 1].anchor)) {
                    anchored++;
                    allNot = false;
                }
                if (!"skip".equals(paths[g - 1].pathState)) {
                    allSkip = false;
                } else {
                    skip.add(g);
                }
                if (paths[g - 1].validated) {
                    validated++;
                }
            }
        }

        Book book = bookends(stages, paths, headers, reads, validated, issuedMiss, anchored);

        Six six = sixLeg(sha, profile, terrain, seed, stages, draw);

        String analysis = analysis(stages, headers, book, malformed);

        Json.Obj rec = new Json.Obj();
        rec.add("profile", Json.quote(profile.name));
        if (!profile.isProduction()) {
            rec.add("sample_bytes", Integer.toString(profile.sampleBytes));
            rec.add("slot_bytes", Integer.toString(profile.slotBytes));
            rec.add("terrain_bytes", Integer.toString(profile.terrainBytes));
            rec.add("steps_per_leg", Integer.toString(profile.stepsPerLeg));
        }
        rec.add("shape", Json.quote(shape));
        Json.Arr shapeItem = new Json.Arr();
        for (String it : items) {
            shapeItem.add(Json.quote(it));
        }
        rec.add("shape_item", shapeItem.build());
        Json.Arr headerArr = new Json.Arr();
        for (HeaderM h : headers) {
            headerArr.add(h.json());
        }
        rec.add("headers", headerArr.build());
        rec.add("seed_value", Json.quote(seedValue));
        Json.Arr cpArr = new Json.Arr();
        for (int g = 0; g < Const.STAGES; g++) {
            cpArr.add(cpJson[g]);
        }
        rec.add("checkpoints", cpArr.build());
        Json.Arr wpArr = new Json.Arr();
        for (String w : wpJson) {
            wpArr.add(w);
        }
        rec.add("waypoints", wpArr.build());
        Json.Arr pathArr = new Json.Arr();
        for (PathM p : paths) {
            pathArr.add(p.json());
        }
        rec.add("paths", pathArr.build());
        Json.Arr anArr = new Json.Arr();
        for (String a : anchorTok) {
            anArr.add(Json.quote(a));
        }
        rec.add("anchors", anArr.build());
        rec.add("anchored_total", Integer.toString(anchored));
        Json.Arr skipArr = new Json.Arr();
        for (int g : skip) {
            skipArr.add(Integer.toString(g));
        }
        rec.add("skip_list", skipArr.build());
        rec.add("a", book.a);
        rec.add("b", book.b);
        rec.add("N", book.n);
        rec.add("S", book.s);
        rec.add("delta_h", book.delta);
        rec.add("fixed_start", book.fixed);
        rec.add("P_b", book.pb);
        rec.add("height_P_b", book.heightPb);
        rec.add("index_comparison_P_b", book.cmpPb);
        rec.add("T_P_b", book.tPb);
        rec.add("height_I_a_minus_1", book.heightIa);
        rec.add("T_I_a_minus_1", book.tIa);
        rec.add("six_leg", six.json);
        rec.add("six_leg_runs", six.runs);
        rec.add("analysis", analysis);

        String sixTok = six.token;
        if (sixTok == null && six.results != null) {
            sixTok = "match";
            for (String r : six.results) {
                if (!"match".equals(r)) {
                    sixTok = "mismatch";
                    break;
                }
            }
        }
        return new Report(true, false, rec.build(), shape, seedValue, cpMatch, wpMatch, sixTok,
                six.results == null ? new ArrayList<String>() : six.results, anchored, allNot, allSkip);
    }

    private static String checkpointStage(Sha256 sha, Profile profile, long[] terrain, byte[] seed, Stage stage, int g) {
        Json.Obj o = new Json.Obj();
        o.add("g", Integer.toString(g));
        if (stage.checkpoints == null) {
            o.add("legs", Json.quote("not defined"));
            return o.build();
        }
        Json.Arr legs = new Json.Arr();
        for (int j = 0; j < stage.checkpoints.length; j++) {
            String result = legResultToken(sha, profile, terrain, seed, stage, g, j);
            Json.Obj leg = new Json.Obj();
            leg.add("j", Integer.toString(j));
            leg.add("result", Json.quote(result));
            legs.add(leg.build());
        }
        o.add("legs", legs.build());
        return o.build();
    }

    /** §14.3. Does not run a leg. */
    private static String legResultToken(Sha256 sha, Profile profile, long[] terrain, byte[] seed, Stage stage, int g, int j) {
        if (stage.checkpoints == null || j >= stage.checkpoints.length) {
            return "not defined";
        }
        byte[] storedCp = stage.checkpoints[j];
        byte[] leg = (stage.legResults != null && j < stage.legResults.length) ? stage.legResults[j] : null;
        if (leg == null || leg.length != 8) {
            return "mismatch";
        }
        byte[] stageLimb = g == 0 ? seed : stage.waypoint;
        byte[] previous = j == 0 ? stageLimb : (j - 1 < stage.checkpoints.length ? stage.checkpoints[j - 1] : null);
        if (terrain == null || stageLimb == null || stageLimb.length != 32 || previous == null || previous.length != 32
                || storedCp == null) {
            return "not defined";
        }
        int off = Leg.sampleOffset(previous, profile.offsetModulus);
        byte[] sample = Terrain.window(terrain, off, profile.sampleBytes);
        byte[] got = Checkpoint.digest(sha, previous, stageLimb, sample, leg);
        return Bytes.eq(got, storedCp) ? "match" : "mismatch";
    }

    /** §12.2 recompute of W_{g+1}. No leg. */
    private static String waypointToken(Sha256 sha, Profile profile, long[] terrain, Stage cur, Stage next) {
        if (!cur.present || !next.present || terrain == null) {
            return "not defined";
        }
        if (cur.waypoint == null || cur.waypoint.length != 32 || next.beacon == null || next.beacon.length != 32
                || next.waypoint == null || next.waypoint.length != 32 || cur.checkpoints == null) {
            return "not defined";
        }
        for (byte[] c : cur.checkpoints) {
            if (c == null || c.length != 32) {
                return "not defined";
            }
        }
        byte[] fold = Waypoint.fold(sha, cur.checkpoints);
        int slot = Waypoint.slotIndex(next.beacon, profile.nSlots);
        byte[] slotSample = Terrain.slotBytes(terrain, slot, profile.wordsPerSlot);
        byte[] w = Waypoint.compute(sha, next.beacon, slotSample, cur.waypoint, fold);
        return Bytes.eq(w, next.waypoint) ? "match" : "mismatch";
    }

    private static HeaderM header(Reads reads, byte[] hash, Long height) {
        HeaderM m = new HeaderM();
        if (hash == null || hash.length != 32) {
            m.hash = "not defined";
            m.height = "not defined";
            m.cmp = "not read";
            m.t = "not read";
            return m;
        }
        m.hash = Bytes.hex(hash);
        if (height == null) {
            m.height = "not defined";
            m.cmp = "not read";
            m.t = "not read";
            return m;
        }
        m.height = Long.toString(height);
        Doc doc = reads.at(height);
        if (doc == null || doc.header == null) {
            m.cmp = "not read";
            m.t = "not read";
            return m;
        }
        if (!Bytes.eq(doc.header.hashNatural, hash) || doc.header.height != height) {
            m.cmp = "mismatch";
            m.t = "not read";
            return m;
        }
        m.cmp = "match";
        m.t = doc.header.time == null ? "not read" : Long.toString(doc.header.time);
        return m;
    }

    private static PathM pathOf(Sha256 sha, Reads reads, Stage[] stages, byte[] lastInterrupt, Long lastHeight,
                                int g, HeaderM[] headers, List<Boolean> issuedMiss) {
        Stage stage = stages[g];
        PathM m = new PathM();
        m.g = g;
        byte[] raw = stage.path;
        if (g == 17) {
            m.w17 = true;
            if (raw != null && raw.length == 0) {
                m.w17Tok = "not fetched by design";
                m.claimed = Json.quote("not defined");
            } else {
                m.w17Tok = "not defined";
                PathBody body = raw == null ? null : PathBody.decode(raw);
                m.claimed = body == null ? Json.quote("not defined") : claimed(body);
            }
            return m;
        }
        if (raw == null || raw.length == 0) {
            m.pathState = "skip";
            m.plant = "n/a";
            m.anchor = "not anchored";
            m.pg = Json.quote("not defined");
            m.heightPg = Json.quote("not defined");
            m.dg = Json.quote("not defined");
            m.claimed = Json.quote("not defined");
            return m;
        }
        m.pathState = "present";
        m.anchor = "not anchored";
        PathBody body = PathBody.decode(raw);
        byte[] waypoint = stage.waypoint;
        if (body == null || waypoint == null || !body.validateFor(sha, waypoint)) {
            m.plant = "mismatch";
            m.pg = Json.quote("not defined");
            m.heightPg = Json.quote("not defined");
            m.dg = Json.quote("not defined");
            m.claimed = Json.quote("not defined");
            return m;
        }
        m.validated = true;
        m.claimed = claimed(body);
        m.body = body;
        Long h = stages[g].beaconHeight;
        boolean window = h != null && (body.blockHeight == h + 2 || body.blockHeight == h + 3 || body.blockHeight == h + 4);
        boolean interrupt = window && storedInterrupt(stages, lastInterrupt, lastHeight, body.blockHash, body.blockHeight);
        if (!window || !interrupt) {
            m.plant = "not read";
            m.pg = Json.quote("not defined");
            m.heightPg = Json.quote("not defined");
            m.dg = Json.quote("not defined");
            m.issued = false;
            return m;
        }
        m.issued = true;
        Doc doc = reads.at(body.blockHeight);
        if (doc == null) {
            issuedMiss.add(Boolean.TRUE);
            m.plant = "not read";
            m.pg = Json.quote("not defined");
            m.heightPg = Json.quote("not defined");
            m.dg = Json.quote("not defined");
            return m;
        }
        if (!plantMatch(doc, body)) {
            m.plant = "mismatch";
            m.pg = Json.quote("not defined");
            m.heightPg = Json.quote("not defined");
            m.dg = Json.quote("not defined");
            return m;
        }
        m.plant = "match";
        m.anchor = "anchored";
        m.pg = claimed(body);
        m.heightPg = Long.toString(body.blockHeight);
        if ("match".equals(headers[g].cmp) && headers[g].heightJson()) {
            long ih = Long.parseLong(headers[g].height);
            m.dg = Long.toString(body.blockHeight - ih);
        } else {
            m.dg = Json.quote("not defined");
        }
        return m;
    }

    private static boolean storedInterrupt(Stage[] stages, byte[] lastInterrupt, Long lastHeight, byte[] hash, long height) {
        for (int g = 1; g <= 17; g++) {
            if (stages[g].beacon != null && stages[g].beaconHeight != null
                    && stages[g].beaconHeight == height && Bytes.eq(stages[g].beacon, hash)) {
                return true;
            }
        }
        return lastInterrupt != null && lastHeight != null && lastHeight == height && Bytes.eq(lastInterrupt, hash);
    }

    private static boolean plantMatch(Doc doc, PathBody body) {
        if (doc.network == null || !("main".equals(doc.network) || "synthetic".equals(doc.network))) {
            return false;
        }
        if (doc.plants.isEmpty()) {
            return false;
        }
        for (Plant p : doc.plants) {
            if (p.ok && Bytes.eq(p.root, body.root) && Bytes.eq(p.txid, body.txid) && Bytes.eq(p.blockHash, body.blockHash)
                    && p.vout == body.vout && p.blockHeight == body.blockHeight) {
                return true;
            }
        }
        return false;
    }

    private static String claimed(PathBody body) {
        Json.Obj o = new Json.Obj();
        o.add("block_hash", Json.quote(Bytes.hex(Bytes.reverse(body.blockHash))));
        o.add("block_height", Long.toString(body.blockHeight));
        return o.build();
    }

    private static Book bookends(Stage[] stages, PathM[] paths, HeaderM[] headers, Reads reads,
                                 int validated, List<Boolean> issuedMiss, int anchored) {
        Book b = new Book();
        if (validated < 2) {
            b.all("not defined");
            b.legsOutside = Json.quote("not defined");
            return b;
        }
        if (!issuedMiss.isEmpty()) {
            b.all("not read");
            b.legsOutside = Json.quote("not read");
            return b;
        }
        if (anchored < 2) {
            b.all("not defined");
            b.legsOutside = Json.quote("not defined");
            return b;
        }
        int a = -1;
        int bIdx = -1;
        for (int g = 1; g <= 16; g++) {
            if ("anchored".equals(paths[g - 1].anchor)) {
                if (a < 0) {
                    a = g;
                }
                bIdx = g;
            }
        }
        int n = 0;
        for (int g = a; g <= bIdx - 1; g++) {
            n += stages[g].checkpoints == null ? 0 : stages[g].checkpoints.length;
        }
        b.a = Integer.toString(a);
        b.b = Integer.toString(bIdx);
        b.n = Integer.toString(n);
        PathBody pb = paths[bIdx - 1].body;
        if ("not read".equals(headers[a].cmp) || pb == null) {
            b.all("not read");
            b.legsOutside = Json.quote("not read");
            return b;
        }
        Doc pbDoc = reads.at(pb.blockHeight);
        String cmp;
        String tPb;
        String heightPb;
        if (pbDoc == null || pbDoc.header == null) {
            b.all("not read");
            b.legsOutside = Json.quote("not read");
            return b;
        }
        boolean hashEq = Bytes.eq(pbDoc.header.hashDisplay, Bytes.reverse(pb.blockHash));
        if (hashEq && pbDoc.header.height == pb.blockHeight) {
            cmp = "match";
            heightPb = Long.toString(pbDoc.header.height);
            tPb = pbDoc.header.time == null ? Json.quote("not read") : Long.toString(pbDoc.header.time);
        } else {
            cmp = "mismatch";
            heightPb = Json.quote("not defined");
            tPb = Json.quote("not read");
        }
        b.pb = claimed(pb);
        b.cmpPb = Json.quote(cmp);
        b.heightPb = heightPb;
        b.tPb = tPb;
        if ("match".equals(headers[a].cmp)) {
            b.heightIa = headers[a].height;
            b.tIa = "not read".equals(headers[a].t) ? Json.quote("not read") : headers[a].t;
        } else {
            b.heightIa = Json.quote("not defined");
            b.tIa = Json.quote("not read");
        }
        boolean tOk = !"not read".equals(strip(b.tPb)) && !"not read".equals(strip(b.tIa))
                && !b.tPb.startsWith("\"") && !b.tIa.startsWith("\"");
        if (tOk) {
            b.s = Long.toString(Long.parseLong(b.tPb) - Long.parseLong(b.tIa));
        } else {
            b.s = Json.quote("not read");
        }
        if ("match".equals(cmp) && "match".equals(headers[a].cmp)) {
            b.delta = Long.toString(Long.parseLong(heightPb) - Long.parseLong(headers[a].height));
        } else {
            b.delta = Json.quote("not defined");
        }
        int outside = 0;
        for (int g = 0; g < Const.STAGES; g++) {
            if (g < a || g > bIdx - 1) {
                outside += stages[g].checkpoints == null ? 0 : stages[g].checkpoints.length;
            }
        }
        b.legsOutside = Integer.toString(outside);
        int fsN = 0;
        for (int g = 1; g <= bIdx - 1; g++) {
            fsN += stages[g].checkpoints == null ? 0 : stages[g].checkpoints.length;
        }
        String fsS;
        if ("match".equals(headers[1].cmp) && !b.tPb.startsWith("\"") && !"not read".equals(headers[1].t)) {
            fsS = Long.toString(Long.parseLong(stripNum(b.tPb)) - Long.parseLong(headers[1].t));
        } else if ("not read".equals(headers[1].cmp) || b.tPb.equals(Json.quote("not read"))) {
            fsS = Json.quote("not read");
        } else {
            fsS = Json.quote("not read");
        }
        String fsD;
        if ("match".equals(headers[1].cmp) && "match".equals(cmp)) {
            fsD = Long.toString(Long.parseLong(heightPb) - Long.parseLong(headers[1].height));
        } else if ("not read".equals(headers[1].cmp) || pbDoc.header == null) {
            fsD = Json.quote("not read");
        } else {
            fsD = Json.quote("not defined");
        }
        b.fixed = fixedObj(Integer.toString(fsN), fsS, fsD);
        return b;
    }

    private static String strip(String raw) {
        if (raw.startsWith("\"") && raw.endsWith("\"")) {
            return raw.substring(1, raw.length() - 1);
        }
        return raw;
    }

    private static String stripNum(String raw) {
        return raw;
    }

    private static Six sixLeg(Sha256 sha, Profile profile, long[] terrain, byte[] seed, Stage[] stages, Draw.Source draw) {
        Six six = new Six();
        int total = 0;
        int[] start = new int[Const.STAGES + 1];
        for (int g = 0; g < Const.STAGES; g++) {
            start[g] = total;
            if (stages[g].checkpoints == null) {
                six.token = "not defined";
                six.json = Json.quote("not defined");
                six.runs = "[]";
                return six;
            }
            total += stages[g].checkpoints.length;
        }
        start[Const.STAGES] = total;
        if (total < Const.SIX_LEG) {
            six.token = "not defined";
            six.json = Json.quote("not defined");
            six.runs = "[]";
            return six;
        }
        Draw.Source source = draw == null ? Draw.csprng() : draw;
        Draw.Six got = Draw.six(total, source);
        if (got.notRead || got.indices == null) {
            six.token = "not read";
            six.json = Json.quote("not read");
            six.runs = Json.quote("not read");
            return six;
        }
        List<String> results = new ArrayList<String>();
        Json.Arr idx = new Json.Arr();
        Json.Arr res = new Json.Arr();
        for (int q : got.indices) {
            idx.add(Integer.toString(q));
            int g = 0;
            while (g < Const.STAGES && q >= start[g + 1]) {
                g++;
            }
            int j = q - start[g];
            String token = recomputeLeg(sha, profile, terrain, seed, stages[g], g, j);
            results.add(token);
            res.add(Json.quote(token));
        }
        Json.Obj one = new Json.Obj();
        one.add("indices", idx.build());
        one.add("results", res.build());
        six.json = one.build();
        Json.Arr runs = new Json.Arr();
        runs.add(six.json);
        six.runs = runs.build();
        six.results = results;
        return six;
    }

    /** §14.6. Full slot on every step via {@link Leg#legResult}. */
    private static String recomputeLeg(Sha256 sha, Profile profile, long[] terrain, byte[] seed, Stage stage, int g, int j) {
        if (terrain == null || stage.legResults == null || j >= stage.legResults.length || stage.legResults[j] == null
                || stage.legResults[j].length != 8) {
            return "mismatch";
        }
        byte[] previous;
        if (j == 0) {
            previous = g == 0 ? seed : stage.waypoint;
        } else {
            previous = stage.checkpoints != null && j - 1 < stage.checkpoints.length ? stage.checkpoints[j - 1] : null;
        }
        if (previous == null || previous.length < 8) {
            return "mismatch";
        }
        long got = Leg.legResult(terrain, profile, sha, Bytes.getLe64(previous, 0));
        long stored = Bytes.getLe64(stage.legResults[j], 0);
        return got == stored ? "match" : "mismatch";
    }

    private static String analysis(Stage[] stages, HeaderM[] headers, Book book, boolean malformed) {
        Long whole = sum(stages, 1, 17);
        // Stages 1–17. Stage 0 is before the first interrupt and is not an admission leg.
        Long hike = sum(stages, 1, 17);
        Json.Arr per = new Json.Arr();
        for (int g = 0; g < Const.STAGES; g++) {
            Json.Obj o = new Json.Obj();
            o.add("g", Integer.toString(g));
            // Erratum 2 (E02). A missing stage is not a count of zero, and a
            // malformed CBOR value has no checkpoint list to count.
            if (malformed || stages[g].checkpoints == null) {
                o.add("checkpoint_count", Json.quote("not defined"));
            } else {
                o.add("checkpoint_count", Integer.toString(stages[g].checkpoints.length));
            }
            o.add("span", span(headers, g));
            per.add(o.build());
        }
        String wholeSpan = subT(headers[18], headers[1]);
        // Same integer: nTime(I_17) − nTime(I_0). Median-time-past is not this span.
        String hikeSpan = wholeSpan;
        Json.Obj o = new Json.Obj();
        o.add("whole_hike_legs", malformed || whole == null ? Json.quote("not defined") : Long.toString(whole));
        o.add("whole_hike_span", wholeSpan);
        o.add("per_stage", per.build());
        o.add("legs_outside", book.legsOutside);
        o.add("hike_legs", malformed || hike == null ? Json.quote("not defined") : Long.toString(hike));
        o.add("hike_span", hikeSpan);
        return o.build();
    }

    private static Long sum(Stage[] stages, int from, int to) {
        long n = 0;
        for (int g = from; g <= to; g++) {
            if (stages[g].checkpoints == null) {
                return null;
            }
            n += stages[g].checkpoints.length;
        }
        return n;
    }

    private static String span(HeaderM[] headers, int g) {
        if (g == 0) {
            return subT(headers[1], headers[0]);
        }
        if (g < 17) {
            return subT(headers[g + 1], headers[g]);
        }
        return subT(headers[18], headers[17]);
    }

    private static String subT(HeaderM hi, HeaderM lo) {
        if (hi == null || lo == null || "not read".equals(hi.t) || "not read".equals(lo.t)) {
            return Json.quote("not read");
        }
        try {
            return Long.toString(Long.parseLong(hi.t) - Long.parseLong(lo.t));
        } catch (NumberFormatException e) {
            return Json.quote("not read");
        }
    }

    private static boolean heightsIncrease(Stage[] stages, Long lastHeight) {
        Long prev = null;
        for (int g = 0; g < Const.STAGES; g++) {
            Long h = stages[g].beaconHeight;
            if (h == null) {
                return false;
            }
            if (prev != null && h <= prev) {
                return false;
            }
            prev = h;
        }
        return lastHeight != null && prev != null && lastHeight > prev;
    }

    private static boolean hashesDiffer(Stage[] stages, byte[] last) {
        byte[] prev = null;
        for (int g = 0; g < Const.STAGES; g++) {
            byte[] h = stages[g].beacon;
            if (h == null || h.length != 32) {
                return false;
            }
            if (prev != null && Bytes.eq(prev, h)) {
                return false;
            }
            prev = h;
        }
        return last != null && last.length == 32 && !Bytes.eq(prev, last);
    }

    private static boolean badTop(Cbor.Item map) {
        String[] keys = {"fill", "kind", "last_interrupt", "last_interrupt_height", "n_slots", "public_key",
                "sample_bytes", "seed_beacon", "slot_bytes", "stages", "steps_per_leg"};
        if (map.fields.size() != keys.length) {
            return true;
        }
        for (String k : keys) {
            if (map.field(k) == null) {
                return true;
            }
        }
        if (!is(map, "fill", Cbor.Kind.TEXT) || !is(map, "kind", Cbor.Kind.TEXT) || !is(map, "stages", Cbor.Kind.ARRAY)
                || !is(map, "n_slots", Cbor.Kind.UINT) || !is(map, "slot_bytes", Cbor.Kind.UINT)
                || !is(map, "sample_bytes", Cbor.Kind.UINT) || !is(map, "steps_per_leg", Cbor.Kind.UINT)
                || !is(map, "last_interrupt_height", Cbor.Kind.UINT)) {
            return true;
        }
        byte[] pk = bytesOf(map, "public_key");
        byte[] sb = bytesOf(map, "seed_beacon");
        byte[] li = bytesOf(map, "last_interrupt");
        return pk == null || pk.length != 32 || sb == null || sb.length != 32 || li == null || li.length != 32;
    }

    private static Stage readStage(Cbor.Item item, boolean stage0) {
        Stage s = new Stage();
        if (item == null || item.kind != Cbor.Kind.MAP) {
            s.fieldSet = true;
            return s;
        }
        s.present = true;
        s.pathKey = item.field("path") != null;
        s.path = bytesOf(item, "path");
        s.beacon = bytesOf(item, "beacon");
        s.waypoint = bytesOf(item, "waypoint");
        Cbor.Item bh = item.field("beacon_height");
        if (bh != null && bh.kind == Cbor.Kind.UINT) {
            if (over(bh)) {
                s.heightOver = true;
            } else {
                s.beaconHeight = bh.uint.longValue();
            }
        }
        Cbor.Item lc = item.field("leg_count");
        if (lc != null && lc.kind == Cbor.Kind.UINT && !over(lc) && lc.uint.bitLength() <= 31) {
            s.legCount = lc.uint.longValue();
        }
        s.checkpoints = byteArray(item.field("checkpoints"), s, true);
        s.legResults = byteArray(item.field("leg_results"), s, false);
        s.fieldSet = badStage(item, stage0);
        return s;
    }

    private static byte[][] byteArray(Cbor.Item item, Stage s, boolean checkpoint) {
        if (item == null || item.kind != Cbor.Kind.ARRAY) {
            return null;
        }
        byte[][] out = new byte[item.array.size()][];
        for (int i = 0; i < out.length; i++) {
            Cbor.Item el = item.array.get(i);
            if (el.kind != Cbor.Kind.BYTES) {
                if (checkpoint) {
                    s.checkpointBadType = true;
                } else {
                    s.legBadType = true;
                }
                out[i] = null;
            } else {
                out[i] = el.bytes;
            }
        }
        return out;
    }

    private static boolean badStage(Cbor.Item map, boolean stage0) {
        int expect = stage0 ? 6 : 7;
        if (map.fields.size() != expect) {
            return true;
        }
        String[] keys = stage0
                ? new String[] {"beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results"}
                : new String[] {"path", "beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results"};
        for (String k : keys) {
            if (map.field(k) == null) {
                return true;
            }
        }
        if (stage0 && map.field("path") != null) {
            return true;
        }
        if (!is(map, "beacon", Cbor.Kind.BYTES) || !is(map, "waypoint", Cbor.Kind.BYTES)
                || !is(map, "beacon_height", Cbor.Kind.UINT) || !is(map, "leg_count", Cbor.Kind.UINT)
                || !is(map, "checkpoints", Cbor.Kind.ARRAY) || !is(map, "leg_results", Cbor.Kind.ARRAY)) {
            return true;
        }
        if (!stage0 && !is(map, "path", Cbor.Kind.BYTES)) {
            return true;
        }
        byte[] beacon = bytesOf(map, "beacon");
        byte[] wp = bytesOf(map, "waypoint");
        return beacon == null || beacon.length != 32 || wp == null || wp.length != 32;
    }

    private static boolean anyOver(Cbor.Item item) {
        if (item == null) {
            return false;
        }
        if (item.kind == Cbor.Kind.UINT) {
            return over(item);
        }
        if (item.kind == Cbor.Kind.ARRAY) {
            for (Cbor.Item c : item.array) {
                if (anyOver(c)) {
                    return true;
                }
            }
        }
        if (item.kind == Cbor.Kind.MAP) {
            for (Cbor.Field f : item.fields) {
                if (anyOver(f.value)) {
                    return true;
                }
            }
        }
        return false;
    }

    private static boolean over(Cbor.Item item) {
        return item.uint.compareTo(JSON_MAX_I) > 0;
    }

    private static boolean is(Cbor.Item map, String key, Cbor.Kind kind) {
        Cbor.Item v = map.field(key);
        return v != null && v.kind == kind;
    }

    private static String text(Cbor.Item map, String key) {
        Cbor.Item v = map.field(key);
        return v != null && v.kind == Cbor.Kind.TEXT ? v.text : null;
    }

    private static byte[] bytesOf(Cbor.Item map, String key) {
        Cbor.Item v = map.field(key);
        return v != null && v.kind == Cbor.Kind.BYTES ? v.bytes : null;
    }

    private static Long uintOk(Cbor.Item map, String key) {
        Cbor.Item v = map.field(key);
        if (v == null || v.kind != Cbor.Kind.UINT || over(v)) {
            return null;
        }
        return v.uint.longValue();
    }

    private static String fixedObj(String n, String s, String d) {
        Json.Obj o = new Json.Obj();
        o.add("N", n);
        o.add("S", s);
        o.add("delta_h", d);
        return o.build();
    }

    private static final class Stage {
        boolean present;
        boolean fieldSet;
        boolean pathKey;
        boolean heightOver;
        boolean checkpointBadType;
        boolean legBadType;
        byte[] path;
        byte[] beacon;
        byte[] waypoint;
        Long beaconHeight;
        Long legCount;
        byte[][] checkpoints;
        byte[][] legResults;

        static Stage missing() {
            Stage s = new Stage();
            s.fieldSet = true;
            return s;
        }
    }

    private static final class HeaderM {
        String hash;
        String height;
        String cmp;
        String t;

        boolean heightJson() {
            return height != null && !height.startsWith("not");
        }

        String json() {
            Json.Obj o = new Json.Obj();
            o.add("stored_hash", hash.startsWith("not") ? Json.quote(hash) : Json.quote(hash));
            o.add("stored_height", heightJson() ? height : Json.quote(height));
            o.add("index_comparison", Json.quote(cmp));
            o.add("T", "not read".equals(t) ? Json.quote(t) : t);
            return o.build();
        }
    }

    private static final class PathM {
        int g;
        boolean w17;
        String w17Tok;
        String pathState;
        String plant;
        String anchor;
        String pg;
        String heightPg;
        String dg;
        String claimed;
        boolean validated;
        boolean issued;
        PathBody body;

        String json() {
            Json.Obj o = new Json.Obj();
            o.add("g", Integer.toString(g));
            if (w17) {
                o.add("w17_path", Json.quote(w17Tok));
                o.add("claimed_block", claimed);
                return o.build();
            }
            o.add("path_state", Json.quote(pathState));
            o.add("plant", Json.quote(plant));
            o.add("P_g", pg);
            o.add("height_P_g", heightPg);
            o.add("d_g", dg);
            o.add("claimed_block", claimed);
            return o.build();
        }
    }

    private static final class Book {
        String a;
        String b;
        String n;
        String s;
        String delta;
        String fixed;
        String pb;
        String heightPb;
        String cmpPb;
        String tPb;
        String heightIa;
        String tIa;
        String legsOutside;

        void all(String token) {
            String q = Json.quote(token);
            a = q;
            b = q;
            n = q;
            s = q;
            delta = q;
            pb = q;
            heightPb = q;
            cmpPb = q;
            tPb = q;
            heightIa = q;
            tIa = q;
            fixed = fixedObj(q, q, q);
        }
    }

    private static final class Six {
        String token;
        String json;
        String runs;
        List<String> results;
    }

    private static final class Reads {
        final Index index;
        final Map<Long, Doc> cache = new HashMap<Long, Doc>();
        final Map<Long, Boolean> seen = new HashMap<Long, Boolean>();

        Reads(Index index) {
            this.index = index;
        }

        Doc at(long height) {
            if (seen.containsKey(height)) {
                return cache.get(height);
            }
            seen.put(height, Boolean.TRUE);
            byte[] raw = index.get(height);
            if (raw == null) {
                cache.put(height, null);
                return null;
            }
            Json.Val val = Json.parse(raw);
            Json.ObjMap obj = val == null ? null : val.asObj();
            if (obj == null) {
                cache.put(height, null);
                return null;
            }
            Doc doc = Doc.parse(obj);
            cache.put(height, doc);
            return doc;
        }
    }

    private static final class Doc {
        Header header;
        String network;
        final List<Plant> plants = new ArrayList<Plant>();

        static Doc parse(Json.ObjMap obj) {
            Doc d = new Doc();
            Json.Val net = obj.get("network");
            if (net != null && net.asStr() != null) {
                d.network = net.asStr().value;
            }
            Json.Val header = obj.get("header");
            if (header != null && header.asObj() != null) {
                d.header = Header.parse(header.asObj());
            }
            Json.Val plants = obj.get("plants");
            if (plants != null && plants.asArr() != null) {
                for (Json.Val p : plants.asArr().list) {
                    if (p.asObj() != null) {
                        d.plants.add(Plant.parse(p.asObj()));
                    }
                }
            }
            Json.Val plant = obj.get("plant");
            if (plant != null && plant.asObj() != null) {
                d.plants.add(Plant.parse(plant.asObj()));
            }
            return d;
        }
    }

    private static final class Header {
        byte[] hashNatural;
        byte[] hashDisplay;
        long height;
        Long time;

        static Header parse(Json.ObjMap obj) {
            Json.Val hash = obj.get("hash");
            Json.Val height = obj.get("height");
            if (hash == null || hash.asStr() == null || height == null || height.asNum() == null) {
                return null;
            }
            byte[] display = hex32(hash.asStr().value);
            Long h = height.asNum().jsonUint();
            if (display == null || h == null) {
                return null;
            }
            Header x = new Header();
            x.hashDisplay = display;
            x.hashNatural = Bytes.reverse(display);
            x.height = h;
            Json.Val time = obj.get("time");
            if (time != null && time.asNum() != null) {
                Long t = time.asNum().jsonUint();
                if (t != null && t <= 0xffffffffL) {
                    x.time = t;
                }
            }
            return x;
        }
    }

    private static final class Plant {
        boolean ok;
        byte[] root;
        byte[] txid;
        byte[] blockHash;
        long vout;
        long blockHeight;

        static Plant parse(Json.ObjMap obj) {
            Plant p = new Plant();
            byte[] root = hex32(str(obj, "root"));
            byte[] txid = hex32(str(obj, "txid"));
            byte[] block = hex32(str(obj, "block_hash"));
            Long vout = uint(obj, "vout");
            Long height = uint(obj, "block_height");
            if (root == null || txid == null || block == null || vout == null || height == null
                    || vout > 0xffffffffL || height > 0xffffffffL) {
                return p;
            }
            p.ok = true;
            p.root = root;
            p.txid = Bytes.reverse(txid);
            p.blockHash = Bytes.reverse(block);
            p.vout = vout;
            p.blockHeight = height;
            return p;
        }
    }

    private static String str(Json.ObjMap obj, String key) {
        Json.Val v = obj.get(key);
        return v != null && v.asStr() != null ? v.asStr().value : null;
    }

    private static Long uint(Json.ObjMap obj, String key) {
        Json.Val v = obj.get(key);
        if (v == null || v.asNum() == null) {
            return null;
        }
        return v.asNum().jsonUint();
    }

    private static byte[] hex32(String s) {
        if (s == null || s.length() != 64) {
            return null;
        }
        try {
            return Bytes.parseHex(s);
        } catch (IllegalArgumentException e) {
            return null;
        }
    }
}
