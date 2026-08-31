using System.IO.Pipes;
using System.Buffers.Binary;
using System.Text.Json;

namespace DeckyMyRigHostControl.Core;

public sealed record ManagementState(bool Ok, string? Error, bool ServiceRunning, int Port, bool Paired, string? PairingCode, ulong ExpiresInSeconds, string? HostVersion = null, string? PluginVersion = null, string? VersionStatus = null);

public interface IManagementClient
{
    Task<ManagementState> GetServiceInfoAsync(CancellationToken cancellationToken = default);
    Task<ManagementState> GetPairingStateAsync(CancellationToken cancellationToken = default);
    Task<ManagementState> GeneratePairingCodeAsync(CancellationToken cancellationToken = default);
}

public sealed class NamedPipeManagementClient : IManagementClient
{
    private const string DefaultPipeName = "DeckyMyRigHostControl";
    private static readonly JsonSerializerOptions JsonOptions = new() { PropertyNameCaseInsensitive = true };
    private readonly string pipeName;

    public NamedPipeManagementClient(string pipeName = DefaultPipeName) => this.pipeName = pipeName;

    public Task<ManagementState> GetServiceInfoAsync(CancellationToken cancellationToken = default) => SendAsync("get_service_info", cancellationToken);
    public Task<ManagementState> GetPairingStateAsync(CancellationToken cancellationToken = default) => SendAsync("get_pairing_state", cancellationToken);
    public Task<ManagementState> GeneratePairingCodeAsync(CancellationToken cancellationToken = default) => SendAsync("generate_pairing_code", cancellationToken);

    private async Task<ManagementState> SendAsync(string operation, CancellationToken cancellationToken)
    {
        await using var pipe = new NamedPipeClientStream(".", pipeName, PipeDirection.InOut, PipeOptions.Asynchronous);
        await pipe.ConnectAsync(2000, cancellationToken);
        var request = JsonSerializer.SerializeToUtf8Bytes(new { operation }, JsonOptions);
        var requestLength = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(requestLength, (uint)request.Length);
        await pipe.WriteAsync(requestLength, cancellationToken);
        await pipe.WriteAsync(request, cancellationToken);
        await pipe.FlushAsync(cancellationToken);
        var responseLengthBytes = new byte[4];
        await pipe.ReadExactlyAsync(responseLengthBytes, cancellationToken);
        var responseLength = BinaryPrimitives.ReadUInt32LittleEndian(responseLengthBytes);
        if (responseLength is 0 or > 65536) throw new IOException("DeckyMyRigHost returned an invalid management response length.");
        var responseBytes = new byte[checked((int)responseLength)];
        await pipe.ReadExactlyAsync(responseBytes, cancellationToken);
        var response = JsonSerializer.Deserialize<ManagementState>(responseBytes, JsonOptions);
        return response ?? throw new IOException("DeckyMyRigHost returned an empty management response.");
    }
}
