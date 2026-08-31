using System.Security.Cryptography;
using System.Text.Json;
using System.Text.RegularExpressions;

namespace DeckyPowerHostControl.Core;

public sealed record ReleaseArtifact(string Url, string Sha256);
public sealed record ReleaseManifest(int SchemaVersion, string Version, ReleaseArtifact Host);

public sealed class HostUpdateService(HttpClient? httpClient = null)
{
    private const long MaximumInstallerBytes = 256L * 1024 * 1024;
    private static readonly Regex StrictVersion = new(@"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$", RegexOptions.CultureInvariant);
    private static readonly Uri ManifestUri = new("https://github.com/NekoGryphou/Decky-My-Rig/releases/latest/download/release-manifest.json");
    private readonly HttpClient http = httpClient ?? new HttpClient { Timeout = TimeSpan.FromSeconds(30) };

    public async Task<string?> DownloadUpdateAsync(string installedVersion, Func<string, bool> verifySignature, CancellationToken cancellationToken = default)
    {
        if (!StrictVersion.IsMatch(installedVersion) || !Version.TryParse(installedVersion, out var installed)) throw new InvalidDataException("The installed host version is invalid.");
        using var manifestResponse = await http.GetAsync(ManifestUri, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
        manifestResponse.EnsureSuccessStatusCode();
        await using var manifestStream = await manifestResponse.Content.ReadAsStreamAsync(cancellationToken);
        var manifest = await JsonSerializer.DeserializeAsync<ReleaseManifest>(manifestStream, new JsonSerializerOptions { PropertyNameCaseInsensitive = true }, cancellationToken)
            ?? throw new InvalidDataException("The release manifest is empty.");
        if (manifest.SchemaVersion != 1 || !StrictVersion.IsMatch(manifest.Version) || !Version.TryParse(manifest.Version, out var available))
            throw new InvalidDataException("The release manifest version is invalid.");
        if (available <= installed) return null;
        if (manifest.Host is null || !Uri.TryCreate(manifest.Host.Url, UriKind.Absolute, out var installerUri) || installerUri.Scheme != Uri.UriSchemeHttps || !installerUri.Host.Equals("github.com", StringComparison.OrdinalIgnoreCase) || !installerUri.AbsolutePath.StartsWith("/NekoGryphou/Decky-My-Rig/releases/download/", StringComparison.Ordinal))
            throw new InvalidDataException("The release manifest contains an untrusted installer location.");
        if (manifest.Host.Sha256.Length != 64 || !manifest.Host.Sha256.All(Uri.IsHexDigit))
            throw new InvalidDataException("The release manifest contains an invalid installer checksum.");

        var destination = Path.Combine(Path.GetTempPath(), $"DeckyPowerHost-{manifest.Version}-Setup.exe");
        try
        {
            using var response = await http.GetAsync(installerUri, HttpCompletionOption.ResponseHeadersRead, cancellationToken);
            response.EnsureSuccessStatusCode();
            if (response.Content.Headers.ContentLength is > MaximumInstallerBytes)
                throw new InvalidDataException("The host installer exceeds the allowed size.");
            await using (var input = await response.Content.ReadAsStreamAsync(cancellationToken))
            await using (var output = new FileStream(destination, FileMode.Create, FileAccess.Write, FileShare.None, 81920, FileOptions.Asynchronous | FileOptions.WriteThrough))
            {
                var buffer = new byte[81920];
                long total = 0;
                int read;
                while ((read = await input.ReadAsync(buffer, cancellationToken)) > 0)
                {
                    total += read;
                    if (total > MaximumInstallerBytes) throw new InvalidDataException("The host installer exceeds the allowed size.");
                    await output.WriteAsync(buffer.AsMemory(0, read), cancellationToken);
                }
            }
            await using var downloaded = File.OpenRead(destination);
            var checksum = Convert.ToHexString(await SHA256.HashDataAsync(downloaded, cancellationToken));
            if (!CryptographicOperations.FixedTimeEquals(Convert.FromHexString(checksum), Convert.FromHexString(manifest.Host.Sha256)))
                throw new InvalidDataException("The downloaded installer checksum does not match the release manifest.");
            if (!verifySignature(destination))
                throw new InvalidDataException("Windows could not verify the installer publisher signature.");
            return destination;
        }
        catch
        {
            try { File.Delete(destination); } catch (IOException) { }
            throw;
        }
    }
}
