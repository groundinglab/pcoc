using System;
using System.Runtime.InteropServices;
using System.Security.Cryptography;

namespace Pcoc;

/// <summary>
/// SHA-256 with a hardware-instruction fast path. On Linux, .NET routes
/// managed SHA256 through OpenSSL, which selects SHA-NI on hosts that have
/// it; calling libcrypto's one-shot SHA256() directly keeps those same
/// instructions and drops the per-call EVP/setup overhead — a measurable win
/// on the section 11.1 seal, which is one single-block hash per step. The
/// bytes are identical either way; the managed path is the fallback where
/// libcrypto is absent (SPEC 11.1 permits a software SHA-256 only there).
/// </summary>
public static unsafe class Sha256Fast
{
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate byte* Sha256OneShot(byte* d, nuint n, byte* md);

    private static readonly Sha256OneShot? _native = Load();

    /// <summary>True when a libcrypto one-shot SHA256 is bound.</summary>
    public static bool NativeAvailable => _native is not null;

    /// <summary>Which SHA-256 backend the hot path uses, for reporting.</summary>
    public static string Backend => _native is not null ? "libcrypto SHA256 (SHA-NI)" : "managed SHA256";

    [DllImport("libdl.so.2", EntryPoint = "dlopen", CharSet = CharSet.Ansi)]
    private static extern IntPtr DlOpen(string filename, int flags);

    [DllImport("libdl.so.2", EntryPoint = "dlsym", CharSet = CharSet.Ansi)]
    private static extern IntPtr DlSym(IntPtr handle, string symbol);

    private static Sha256OneShot? Load()
    {
        string[] libs = { "libcrypto.so.3", "libcrypto.so.1.1", "libcrypto.so" };
        foreach (string lib in libs)
        {
            IntPtr h = DlOpen(lib, 2); // RTLD_NOW
            if (h == IntPtr.Zero) continue;
            IntPtr sym = DlSym(h, "SHA256");
            if (sym != IntPtr.Zero)
                return Marshal.GetDelegateForFunctionPointer<Sha256OneShot>(sym);
        }
        return null;
    }

    /// <summary>Computes SHA-256(data) into dst (32 bytes).</summary>
    public static void Hash(ReadOnlySpan<byte> data, Span<byte> dst)
    {
        if (_native is not null)
        {
            fixed (byte* p = data)
            fixed (byte* m = dst)
            {
                if (_native(p, (nuint)data.Length, m) is not null)
                    return;
            }
        }
        SHA256.TryHashData(data, dst, out _);
    }
}
