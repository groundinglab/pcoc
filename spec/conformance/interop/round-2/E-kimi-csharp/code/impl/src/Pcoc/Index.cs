using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text;

namespace Pcoc;

/// <summary>
/// One section 9 clock read of GET {index-url}/index/{height}. Returns the
/// raw body on a successful read (HTTP 200, no redirect), null otherwise.
/// </summary>
public interface IIndexSource
{
    byte[]? Get(long height);
}

/// <summary>Reads GET {index-url}/index/{height} over HTTP, no redirect, 200 only.</summary>
public sealed class HttpIndexSource : IIndexSource, IDisposable
{
    private readonly HttpClient _client;
    private readonly string _base;

    public HttpIndexSource(string indexUrl)
    {
        _base = indexUrl.TrimEnd('/');
        var handler = new HttpClientHandler { AllowAutoRedirect = false };
        _client = new HttpClient(handler);
    }

    public byte[]? Get(long height)
    {
        try
        {
            using var resp = _client.Send(new HttpRequestMessage(HttpMethod.Get, $"{_base}/index/{height}"));
            if ((int)resp.StatusCode != 200) return null;
            return resp.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult();
        }
        catch
        {
            return null; // no HTTP status, transport failure: an unsuccessful read
        }
    }

    public void Dispose() => _client.Dispose();
}

/// <summary>
/// The synthetic test-plane index of the interop fixtures: serves
/// GET /index/{h} for h = low … high with
/// {"network":"synthetic","header":{"hash":…,"height":h,"time":1700000000+600·h}}.
/// The natural bytes are the beacon stored at that height, or the filler
/// digest SHA-256(ASCII "pcoc-pack-2.2/index/filler/" ‖ decimal h) elsewhere.
/// </summary>
public sealed class SyntheticIndexSource : IIndexSource
{
    private readonly long _low;
    private readonly long _high;
    private readonly Dictionary<long, byte[]> _beaconAt = new();

    public SyntheticIndexSource(long low, long high, IReadOnlyDictionary<long, byte[]> beaconAtHeight)
    {
        _low = low;
        _high = high;
        foreach (var kv in beaconAtHeight) _beaconAt[kv.Key] = kv.Value;
    }

    public static byte[] FillerNatural(long height)
    {
        byte[] label = Encoding.ASCII.GetBytes("pcoc-pack-2.2/index/filler/" + height.ToString(System.Globalization.CultureInfo.InvariantCulture));
        return SHA256.HashData(label);
    }

    public byte[]? Get(long height)
    {
        if (height < _low || height > _high) return null;
        byte[] natural = _beaconAt.TryGetValue(height, out var b) ? b : FillerNatural(height);
        // header.hash is Bitcoin display order: the natural bytes reversed.
        var display = (byte[])natural.Clone();
        Array.Reverse(display);
        var sb = new StringBuilder(160);
        sb.Append("{\"network\":\"synthetic\",\"header\":{\"hash\":\"");
        foreach (byte x in display) sb.Append(x.ToString("x2"));
        sb.Append("\",\"height\":").Append(height);
        long time = 1700000000L + 600L * height;
        sb.Append(",\"time\":").Append(time).Append("}}");
        return Encoding.ASCII.GetBytes(sb.ToString());
    }
}

/// <summary>The parsed result of one header read (Rule A4.2).</summary>
public readonly struct HeaderRead
{
    public HeaderRead(bool success, byte[] naturalHash, long height, long time, bool timeOk)
    {
        Success = success;
        NaturalHash = naturalHash;
        Height = height;
        Time = time;
        TimeOk = timeOk;
    }

    public static HeaderRead Failed => new(false, Array.Empty<byte>(), 0, 0, false);

    /// <summary>The read returned a document with a usable header (section 9).</summary>
    public bool Success { get; }
    public byte[] NaturalHash { get; }
    public long Height { get; }
    public long Time { get; }
    public bool TimeOk { get; }
}

/// <summary>
/// Interprets one section 9 document for a header measurement. The header
/// measurement uses only header.hash, header.height and header.time; a
/// document with no usable header is an unsuccessful header read.
/// </summary>
public static class IndexRead
{
    public const long MaxHeight = (1L << 53) - 1;

    public static HeaderRead Header(byte[]? body)
    {
        JVal.Obj? doc = body is null ? null : JsonDoc.Parse(body);
        if (doc is null) return HeaderRead.Failed;
        if (doc.Find("header") is not JVal.Obj header) return HeaderRead.Failed;
        if (!JsonDoc.TryHex64(header.Find("hash"), out byte[]? display)) return HeaderRead.Failed;
        if (!JsonDoc.TryInteger(header.Find("height"), 0, MaxHeight, out long height)) return HeaderRead.Failed;
        // natural[i] = display[31 − i]: the single reversal rule of section 9.
        var natural = new byte[32];
        for (int i = 0; i < 32; i++) natural[i] = display[31 - i];
        bool timeOk = JsonDoc.TryInteger(header.Find("time"), 0, 0xFFFFFFFFL, out long time);
        return new HeaderRead(true, natural, height, timeOk ? time : 0, timeOk);
    }

    /// <summary>
    /// The MTP-window time of an additional read (Rule A4.10): the read
    /// returned a document, header.height equals the requested height, and
    /// header.time follows section 9. header.hash is not part of this
    /// measurement, so an unusable hash does not lose the time.
    /// </summary>
    public static bool MtpTime(byte[]? body, long requestedHeight, out long time)
    {
        time = 0;
        JVal.Obj? doc = body is null ? null : JsonDoc.Parse(body);
        if (doc is null) return false;
        if (doc.Find("header") is not JVal.Obj header) return false;
        if (!JsonDoc.TryInteger(header.Find("height"), 0, MaxHeight, out long height)) return false;
        if (height != requestedHeight) return false;
        return JsonDoc.TryInteger(header.Find("time"), 0, 0xFFFFFFFFL, out time);
    }

    /// <summary>
    /// The plant facts of one recorded plant, with txid and block_hash already
    /// reversed once to natural order (section 9).
    /// </summary>
    public readonly struct PlantFacts
    {
        public readonly byte[] Root;
        public readonly byte[] TxidNatural;
        public readonly long Vout;
        public readonly byte[] BlockHashNatural;
        public readonly long BlockHeight;

        public PlantFacts(byte[] root, byte[] txidNatural, long vout, byte[] blockHashNatural, long blockHeight)
        {
            Root = root;
            TxidNatural = txidNatural;
            Vout = vout;
            BlockHashNatural = blockHashNatural;
            BlockHeight = blockHeight;
        }
    }

    public enum PlantOutcome { Match, Mismatch, NotRead }

    /// <summary>
    /// The plant measurement of section 14.3 against one decoded body.
    /// Returns NotRead when the read returned no document; otherwise Match or
    /// Mismatch from network and the recorded plants.
    /// </summary>
    public static PlantOutcome Plant(byte[]? body, PathBody path, out string network)
    {
        network = "";
        JVal.Obj? doc = body is null ? null : JsonDoc.Parse(body);
        if (doc is null) return PlantOutcome.NotRead;
        if (doc.Find("network") is JVal.Str ns) network = ns.Value;
        if (network != "main" && network != "synthetic") return PlantOutcome.Mismatch;
        var plants = new List<PlantFacts>();
        if (doc.Find("plants") is JVal.Arr arr)
            foreach (var item in arr.Items)
                if (item is JVal.Obj po && TryPlantFacts(po, out var f))
                    plants.Add(f);
        if (doc.Find("plant") is JVal.Obj single && TryPlantFacts(single, out var sf))
            plants.Add(sf);
        foreach (var f in plants)
        {
            if (!f.Root.AsSpan().SequenceEqual(path.Root)) continue;
            if (!f.TxidNatural.AsSpan().SequenceEqual(path.Txid)) continue;
            if (f.Vout != path.Vout) continue;
            if (!f.BlockHashNatural.AsSpan().SequenceEqual(path.BlockHash)) continue;
            if (f.BlockHeight != path.BlockHeight) continue;
            return PlantOutcome.Match;
        }
        return PlantOutcome.Mismatch;
    }

    private static bool TryPlantFacts(JVal.Obj plant, out PlantFacts f)
    {
        f = default;
        if (!JsonDoc.TryHex64(plant.Find("root"), out byte[]? root)) return false;
        if (!JsonDoc.TryHex64(plant.Find("txid"), out byte[]? txid)) return false;
        if (!JsonDoc.TryHex64(plant.Find("block_hash"), out byte[]? bh)) return false;
        if (!JsonDoc.TryInteger(plant.Find("vout"), 0, 0xFFFFFFFFL, out long vout)) return false;
        if (!JsonDoc.TryInteger(plant.Find("block_height"), 0, 0xFFFFFFFFL, out long bheight)) return false;
        ReverseInPlace(txid);
        ReverseInPlace(bh);
        f = new PlantFacts(root, txid, vout, bh, bheight);
        return true;
    }

    private static void ReverseInPlace(byte[] b) => Array.Reverse(b);
}

/// <summary>
/// The index reads of one verification, with the reuse section 14 names:
/// the Rule A4.2 header reads (also the MTP beacon documents), the plant
/// reads (also the Rule A4.8 P_b read), and the other MTP window reads.
/// </summary>
public sealed class IndexReads
{
    private readonly IIndexSource _source;
    private readonly Dictionary<long, byte[]?> _headerDocs = new();
    private readonly Dictionary<long, byte[]?> _plantDocs = new();
    private readonly Dictionary<long, byte[]?> _mtpDocs = new();

    public IndexReads(IIndexSource source) { _source = source; }

    private static byte[]? Cached(Dictionary<long, byte[]?> cache, IIndexSource src, long height)
    {
        if (!cache.TryGetValue(height, out byte[]? doc))
        {
            doc = src.Get(height);
            cache[height] = doc;
        }
        return doc;
    }

    /// <summary>Rule A4.2 header read of one stored height.</summary>
    public HeaderRead Header(long height) => IndexRead.Header(Cached(_headerDocs, _source, height));

    /// <summary>Section 14.3 plant read of one height (the same read Rule A4.8 reuses).</summary>
    public byte[]? PlantDoc(long height) => Cached(_plantDocs, _source, height);

    /// <summary>An additional MTP-window read (A4.10): not the beacon's own header.</summary>
    public byte[]? MtpOtherDoc(long height) => Cached(_mtpDocs, _source, height);
}
