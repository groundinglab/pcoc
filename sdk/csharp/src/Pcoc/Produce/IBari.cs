namespace Pcoc.Produce;

/// §13.1
[Spec("§13.1", "Produce sends one POST {bari-url}/waypoint as an asynchronous event and does not follow a redirect.")]
public interface IBari
{
    [Spec("§13.1", "Produce sends one POST {bari-url}/waypoint as an asynchronous event and does not follow a redirect.")]
    PostResult PostWaypoint(byte[] waypoint, ulong nonce, byte[] feeHeader);

    [Spec("§13.1", "An ask sends GET {bari-url}/path/{leaf} and does not follow a redirect.")]
    AskResult GetPath(byte[] waypoint);
}

/// §13.1
[Spec("§13.1", "A transitory send failure is a POST that received no HTTP status line")]
public readonly struct PostResult
{
    public bool HasStatus { get; }
    public int Status { get; }

    [Spec("§13.1", "A transitory send failure is a POST that received no HTTP status line")]
    public PostResult(bool hasStatus, int status)
    {
        HasStatus = hasStatus;
        Status = status;
    }
}

/// §13.1
[Spec("§13.1", "Status 200: a checked path is a 200 response whose bytes are a path for that waypoint.")]
public readonly struct AskResult
{
    public bool HasStatus { get; }
    public int Status { get; }
    public byte[] Body { get; }

    [Spec("§13.1", "Status 200: a checked path is a 200 response whose bytes are a path for that waypoint.")]
    public AskResult(bool hasStatus, int status, byte[] body)
    {
        HasStatus = hasStatus;
        Status = status;
        Body = body;
    }
}
