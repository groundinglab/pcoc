namespace Pcoc;

/// §9
public sealed class IndexDoc
{
    public bool Document { get; init; }
    public bool HeaderOk { get; init; }
    public byte[]? NaturalHash { get; init; }
    public long Height { get; init; }
    public bool TimeOk { get; init; }
    public long Time { get; init; }
    public string? Network { get; init; }
    public System.Collections.Generic.List<PlantFact> Plants { get; init; } = new();

    /// §9
    public readonly struct PlantFact
    {
        public byte[] Root { get; }
        public byte[] Txid { get; }
        public long Vout { get; }
        public byte[] BlockHash { get; }
        public long BlockHeight { get; }
        public bool Ok { get; }

        public PlantFact(byte[] root, byte[] txid, long vout, byte[] blockHash, long blockHeight, bool ok)
        {
            Root = root;
            Txid = txid;
            Vout = vout;
            BlockHash = blockHash;
            BlockHeight = blockHeight;
            Ok = ok;
        }
    }
}

/// §9
public interface IIndex
{
    /// §9
    IndexDoc? Get(long height);
}
