namespace Pcoc.Produce;

/// §9
[Spec("§9", "Observe performs the read in section 9 and returns (natural, height).")]
public interface IHeaderSource
{
    [Spec("§9", "Observe performs the read in section 9 and returns (natural, height).")]
    bool TryObserve(out Tip tip);

    [Spec("§13", "After each checkpoint the producer reads the tip: the latest header its header source already holds.")]
    bool TryTipRead(out Tip tip);

    [Spec("§13.1", "When the post of W_g starts, the producer takes as fee header the 80-byte block header its header source holds as best tip.")]
    bool TryFeeHeader(out byte[] header80);

    [Spec("§13.1", "A waypoint posted at height H is asked when the producer’s header source learns each header whose height is H plus one offset in that list, in list order.")]
    System.Collections.Generic.IReadOnlyList<Tip> LearnedHeaders();
}
