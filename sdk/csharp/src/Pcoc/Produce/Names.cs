namespace Pcoc.Produce;
using Pcoc;

/// §13.1
[Spec("§13.1", "ANCHOR_WINDOW_OFFSETS = 2, 3, 4")]
public static class Names
{
    [Spec("§13.1", "ANCHOR_WINDOW_OFFSETS = 2, 3, 4")]
    public static readonly int[] ANCHOR_WINDOW_OFFSETS = { 2, 3, 4 };

    [Spec("§9", "`HEADER_PEERS_MIN` is 2.")]
    public const int HEADER_PEERS_MIN = 2;

    [Spec("§9", "`HEADER_PEER_SILENCE_SECONDS` is 120.")]
    public const int HEADER_PEER_SILENCE_SECONDS = 120;

    [Spec("§13.1", "After a transitory send failure the producer resends the identical request exactly once, in the background, 30 seconds after that transport failure, and then drops the waypoint if the resend fails.")]
    public const int ResendDelaySeconds = 30;

    [Spec("§13.1", "The difficulty is 21 leading zero bits.")]
    public const int HashcashBits = 21;
}
