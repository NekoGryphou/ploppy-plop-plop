using DeckyPowerHostControl.Core;

namespace DeckyPowerHostControl.Core.Tests;

[TestClass]
public sealed class HostControlViewModelTests
{
    [DataTestMethod]
    [DataRow("compatible", "Host 1.2.8 • Plugin 1.2.1 • Compatible")]
    [DataRow("update_host", "Update this host to match plugin 1.2.1.")]
    [DataRow("update_plugin", "Update the Decky plugin to match host 1.2.8.")]
    [DataRow("incompatible", "Host and plugin major versions are incompatible. Update the older component.")]
    public async Task RefreshExplainsVersionCompatibility(string status, string expected)
    {
        var state = new ManagementState(true, null, true, 48100, true, null, 0, "1.2.8", "1.2.1", status);
        var viewModel = new HostControlViewModel(new FakeClient(state));

        await viewModel.RefreshAsync();

        Assert.AreEqual(expected, viewModel.UpdateStatus);
    }

    [TestMethod]
    public async Task RefreshDisplaysServicePortPairingAndCountdown()
    {
        var clock = new FakeClock(new DateTimeOffset(2026, 8, 25, 12, 0, 0, TimeSpan.Zero));
        var viewModel = new HostControlViewModel(new FakeClient(new(true, null, true, 48100, false, "483921", 272, "0.2.0")), clock);
        await viewModel.RefreshAsync();
        Assert.AreEqual("Running", viewModel.ServiceStatus);
        Assert.AreEqual(48100, viewModel.Port);
        Assert.AreEqual("483 921", viewModel.PairingCode);
        Assert.AreEqual("04:32", viewModel.Expiration);
        Assert.AreEqual("Not paired", viewModel.PairingStatus);
        Assert.AreEqual("0.2.0", viewModel.HostVersion);
    }

    [TestMethod]
    public async Task GenerateReplacesCodeAndCountdownExpires()
    {
        var clock = new FakeClock(DateTimeOffset.UnixEpoch);
        var client = new FakeClient(new(true, null, true, 47991, true, "739104", 5));
        var viewModel = new HostControlViewModel(client, clock);
        await viewModel.GenerateCodeAsync();
        Assert.AreEqual("739 104", viewModel.PairingCode);
        Assert.AreEqual("Paired", viewModel.PairingStatus);
        clock.Now = clock.Now.AddSeconds(6); viewModel.Tick();
        Assert.AreEqual("Expired", viewModel.Expiration);
        Assert.AreEqual(1, client.GenerateCalls);
    }

    [TestMethod]
    public async Task ConnectionErrorProducesPersistentUiErrorState()
    {
        var viewModel = new HostControlViewModel(new FailingClient());
        await viewModel.RefreshAsync();
        Assert.AreEqual("Unavailable", viewModel.ServiceStatus);
        Assert.IsNotNull(viewModel.ErrorMessage);
        Assert.IsFalse(viewModel.IsBusy);
    }

    [TestMethod]
    public async Task MalformedServiceResponseProducesUiErrorInsteadOfEscapingEventHandler()
    {
        var viewModel = new HostControlViewModel(new MalformedClient());
        await viewModel.RefreshAsync();
        Assert.AreEqual("Unavailable", viewModel.ServiceStatus);
        Assert.IsNotNull(viewModel.ErrorMessage);
        Assert.IsTrue(viewModel.CanGenerate);
    }

    [TestMethod]
    public async Task ServiceOperationErrorKeepsServiceAndConfigVisible()
    {
        var viewModel = new HostControlViewModel(new FakeClient(new(false, "Pairing state could not be saved.", true, 48100, false, null, 0)));
        await viewModel.GenerateCodeAsync();
        Assert.AreEqual("Running", viewModel.ServiceStatus);
        Assert.AreEqual(48100, viewModel.Port);
        StringAssert.Contains(viewModel.ErrorMessage, "could not be saved");
        Assert.IsTrue(viewModel.CanGenerate);
    }

    private sealed class FakeClock(DateTimeOffset now) : IClock { public DateTimeOffset Now = now; public DateTimeOffset UtcNow => Now; }
    private sealed class FakeClient(ManagementState state) : IManagementClient
    {
        public int GenerateCalls { get; private set; }
        public Task<ManagementState> GetServiceInfoAsync(CancellationToken cancellationToken = default) => Task.FromResult(state);
        public Task<ManagementState> GetPairingStateAsync(CancellationToken cancellationToken = default) => Task.FromResult(state);
        public Task<ManagementState> GeneratePairingCodeAsync(CancellationToken cancellationToken = default) { GenerateCalls++; return Task.FromResult(state); }
    }
    private sealed class FailingClient : IManagementClient
    {
        private static Task<ManagementState> Fail() => Task.FromException<ManagementState>(new IOException("pipe unavailable"));
        public Task<ManagementState> GetServiceInfoAsync(CancellationToken cancellationToken = default) => Fail();
        public Task<ManagementState> GetPairingStateAsync(CancellationToken cancellationToken = default) => Fail();
        public Task<ManagementState> GeneratePairingCodeAsync(CancellationToken cancellationToken = default) => Fail();
    }
    private sealed class MalformedClient : IManagementClient
    {
        private static Task<ManagementState> Fail() => Task.FromException<ManagementState>(new System.Text.Json.JsonException("invalid response"));
        public Task<ManagementState> GetServiceInfoAsync(CancellationToken cancellationToken = default) => Fail();
        public Task<ManagementState> GetPairingStateAsync(CancellationToken cancellationToken = default) => Fail();
        public Task<ManagementState> GeneratePairingCodeAsync(CancellationToken cancellationToken = default) => Fail();
    }
}
