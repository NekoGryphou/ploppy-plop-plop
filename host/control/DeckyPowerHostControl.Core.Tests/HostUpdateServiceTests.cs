using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using DeckyPowerHostControl.Core;

namespace DeckyPowerHostControl.Core.Tests;

[TestClass]
public sealed class HostUpdateServiceTests
{
    [TestMethod]
    public async Task DownloadRequiresMatchingChecksumAndSignature()
    {
        var installer = Encoding.UTF8.GetBytes("signed installer fixture");
        var checksum = Convert.ToHexString(SHA256.HashData(installer)).ToLowerInvariant();
        var handler = new FixtureHandler(checksum, installer);
        var service = new HostUpdateService(new HttpClient(handler));

        var path = await service.DownloadUpdateAsync("1.2.3", candidate => File.ReadAllBytes(candidate).SequenceEqual(installer));

        Assert.IsNotNull(path);
        Assert.IsTrue(path!.EndsWith("DeckyPowerHost-1.3.0-Setup.exe", StringComparison.Ordinal));
        File.Delete(path);
    }

    [TestMethod]
    public async Task DownloadDeletesAnInstallerWithAnInvalidPublisherSignature()
    {
        var installer = Encoding.UTF8.GetBytes("untrusted installer fixture");
        var checksum = Convert.ToHexString(SHA256.HashData(installer)).ToLowerInvariant();
        var service = new HostUpdateService(new HttpClient(new FixtureHandler(checksum, installer)));

        await Assert.ThrowsExactlyAsync<InvalidDataException>(() => service.DownloadUpdateAsync("1.2.3", _ => false));

        Assert.IsFalse(File.Exists(Path.Combine(Path.GetTempPath(), "DeckyPowerHost-1.3.0-Setup.exe")));
    }

    private sealed class FixtureHandler(string checksum, byte[] installer) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            if (request.RequestUri!.AbsolutePath.EndsWith("release-manifest.json", StringComparison.Ordinal))
            {
                var json = JsonSerializer.Serialize(new
                {
                    schemaVersion = 1,
                    version = "1.3.0",
                    host = new
                    {
                        url = "https://github.com/NekoGryphou/Decky-My-Rig/releases/download/v1.3.0/DeckyPowerHost-Setup.exe",
                        sha256 = checksum,
                    },
                });
                return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(json) });
            }
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK) { Content = new ByteArrayContent(installer) });
        }
    }
}
