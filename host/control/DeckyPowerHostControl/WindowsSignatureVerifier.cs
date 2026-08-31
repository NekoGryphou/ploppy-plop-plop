using System.Runtime.InteropServices;
using System.Reflection;
using System.Security.Cryptography.X509Certificates;

namespace DeckyPowerHostControl;

internal static class WindowsSignatureVerifier
{
    private static readonly Guid ActionGenericVerifyV2 = new("00AAC56B-CD44-11d0-8CC2-00C04FC295EE");

    public static bool IsTrusted(string path)
    {
        var expectedThumbprint = Assembly.GetExecutingAssembly()
            .GetCustomAttributes<AssemblyMetadataAttribute>()
            .SingleOrDefault(attribute => attribute.Key == "DeckySigningCertificateThumbprint")?.Value;
        if (string.IsNullOrWhiteSpace(expectedThumbprint) || expectedThumbprint == "UNCONFIGURED") return false;
        var file = new WinTrustFileInfo(path);
        var filePointer = Marshal.AllocHGlobal(Marshal.SizeOf<WinTrustFileInfo>());
        try
        {
            Marshal.StructureToPtr(file, filePointer, false);
            var data = new WinTrustData(filePointer);
            if (WinVerifyTrust(IntPtr.Zero, ActionGenericVerifyV2, ref data) != 0) return false;
            using var signer = new X509Certificate2(X509Certificate.CreateFromSignedFile(path));
            return signer.Thumbprint.Equals(expectedThumbprint, StringComparison.OrdinalIgnoreCase);
        }
        finally
        {
            Marshal.DestroyStructure<WinTrustFileInfo>(filePointer);
            Marshal.FreeHGlobal(filePointer);
        }
    }

    [DllImport("wintrust.dll", ExactSpelling = true, PreserveSig = true)]
    private static extern int WinVerifyTrust(IntPtr window, [MarshalAs(UnmanagedType.LPStruct)] Guid action, ref WinTrustData data);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct WinTrustFileInfo
    {
        public uint Size;
        public string FilePath;
        public IntPtr FileHandle;
        public IntPtr KnownSubject;
        public WinTrustFileInfo(string path) { Size = (uint)Marshal.SizeOf<WinTrustFileInfo>(); FilePath = path; FileHandle = IntPtr.Zero; KnownSubject = IntPtr.Zero; }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct WinTrustData
    {
        public uint Size;
        public IntPtr PolicyCallbackData;
        public IntPtr SipClientData;
        public uint UiChoice;
        public uint RevocationChecks;
        public uint UnionChoice;
        public IntPtr FileInfo;
        public uint StateAction;
        public IntPtr StateData;
        public string? UrlReference;
        public uint ProviderFlags;
        public uint UiContext;
        public WinTrustData(IntPtr fileInfo) { Size = (uint)Marshal.SizeOf<WinTrustData>(); PolicyCallbackData = IntPtr.Zero; SipClientData = IntPtr.Zero; UiChoice = 2; RevocationChecks = 1; UnionChoice = 1; FileInfo = fileInfo; StateAction = 0; StateData = IntPtr.Zero; UrlReference = null; ProviderFlags = 0x100; UiContext = 0; }
    }
}
