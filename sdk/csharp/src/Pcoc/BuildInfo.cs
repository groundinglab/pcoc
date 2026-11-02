namespace Pcoc;

/// §16.2
public static class BuildInfo
{
    public const string SdkVersion = "2.3.1";
    public const string Language = "csharp";

    /// §16.2
    public static string KernelName() => "scalar";

    /// §7.4
    public static string Sha256Backend() => Sha256.BackendName();
}
