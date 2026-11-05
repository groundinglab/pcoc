namespace Pcoc.Produce;

/// §13.1
[Spec("§13.1", "After a transitory send failure the producer resends the identical request exactly once, in the background, 30 seconds after that transport failure, and then drops the waypoint if the resend fails.")]
public interface IClock
{
    [Spec("§13.1", "After a transitory send failure the producer resends the identical request exactly once, in the background, 30 seconds after that transport failure, and then drops the waypoint if the resend fails.")]
    long NowSeconds { get; }
}
