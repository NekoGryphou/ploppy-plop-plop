using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Text.Json;

namespace DeckyMyRigHostControl.Core;

public interface IClock { DateTimeOffset UtcNow { get; } }
public sealed class SystemClock : IClock { public DateTimeOffset UtcNow => DateTimeOffset.UtcNow; }

public sealed class HostControlViewModel : INotifyPropertyChanged
{
    private readonly IManagementClient client;
    private readonly IClock clock;
    private DateTimeOffset? expiresAt;
    private string serviceStatus = "Checking…";
    private int port;
    private string pairingCode = "—";
    private string expiration = "—";
    private string pairingStatus = "Not paired";
    private string? errorMessage;
    private bool isBusy;
    private string hostVersion = "—";
    private string updateStatus = "Plugin version not detected yet";

    public HostControlViewModel(IManagementClient client, IClock? clock = null) { this.client = client; this.clock = clock ?? new SystemClock(); }
    public event PropertyChangedEventHandler? PropertyChanged;
    public string ServiceStatus { get => serviceStatus; private set => Set(ref serviceStatus, value); }
    public int Port { get => port; private set => Set(ref port, value); }
    public string PairingCode { get => pairingCode; private set => Set(ref pairingCode, value); }
    public string Expiration { get => expiration; private set => Set(ref expiration, value); }
    public string PairingStatus { get => pairingStatus; private set => Set(ref pairingStatus, value); }
    public string? ErrorMessage { get => errorMessage; private set => Set(ref errorMessage, value); }
    public bool IsBusy { get => isBusy; private set { if (Set(ref isBusy, value)) PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(CanGenerate))); } }
    public bool CanGenerate => !IsBusy;
    public string HostVersion { get => hostVersion; private set => Set(ref hostVersion, value); }
    public string UpdateStatus { get => updateStatus; private set => Set(ref updateStatus, value); }

    public async Task RefreshAsync(CancellationToken cancellationToken = default) => await ExecuteAsync(() => client.GetServiceInfoAsync(cancellationToken));
    public async Task GenerateCodeAsync(CancellationToken cancellationToken = default) => await ExecuteAsync(() => client.GeneratePairingCodeAsync(cancellationToken));

    public void Tick()
    {
        if (expiresAt is null) { Expiration = "—"; return; }
        var remaining = expiresAt.Value - clock.UtcNow;
        if (remaining <= TimeSpan.Zero) { Expiration = "Expired"; return; }
        Expiration = $"{(int)remaining.TotalMinutes:00}:{remaining.Seconds:00}";
    }

    private async Task ExecuteAsync(Func<Task<ManagementState>> action)
    {
        IsBusy = true; ErrorMessage = null;
        try
        {
            Apply(await action());
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or TimeoutException or OperationCanceledException or JsonException)
        {
            ServiceStatus = "Unavailable";
            PairingCode = "—";
            Expiration = "—";
            ErrorMessage = "DeckyMyRigHost could not be contacted. Verify that the Windows service is running.";
        }
        finally { IsBusy = false; }
    }

    private void Apply(ManagementState state)
    {
        ServiceStatus = state.ServiceRunning ? "Running" : "Stopped";
        Port = state.Port;
        HostVersion = state.HostVersion ?? "Unknown";
        UpdateStatus = state.VersionStatus switch
        {
            "compatible" => $"Host {state.HostVersion} • Plugin {state.PluginVersion} • Compatible",
            "update_host" => $"Update this host to match plugin {state.PluginVersion}.",
            "update_plugin" => $"Update the Decky plugin to match host {state.HostVersion}.",
            "incompatible" => "Host and plugin major versions are incompatible. Update the older component.",
            _ => "Plugin version not detected yet. Open the plugin while this PC is online.",
        };
        if (!state.Ok) { ErrorMessage = state.Error ?? "DeckyMyRigHost returned an error."; return; }
        PairingStatus = state.Paired ? "Paired" : "Not paired";
        PairingCode = FormatCode(state.PairingCode);
        expiresAt = state.PairingCode is null ? null : clock.UtcNow.AddSeconds(state.ExpiresInSeconds);
        Tick();
    }

    private static string FormatCode(string? code) => code is { Length: 6 } ? $"{code[..3]} {code[3..]}" : "—";
    private bool Set<T>(ref T field, T value, [CallerMemberName] string? property = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value)) return false;
        field = value; PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(property));
        return true;
    }
}
