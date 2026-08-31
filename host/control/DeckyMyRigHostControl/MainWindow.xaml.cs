using DeckyMyRigHostControl.Core;
using Microsoft.UI.Xaml;
using System.Diagnostics;
using System.Text.Json;
using System.Security.Cryptography;

namespace DeckyMyRigHostControl;

public sealed partial class MainWindow : Window
{
    private readonly DispatcherTimer timer = new() { Interval = TimeSpan.FromSeconds(1) };
    private readonly HostUpdateService updates = new();
    public HostControlViewModel ViewModel { get; } = new(new NamedPipeManagementClient());

    public MainWindow()
    {
        InitializeComponent();
        ViewModel.PropertyChanged += (_, _) => UpdateControls();
        timer.Tick += async (_, _) =>
        {
            ViewModel.Tick();
            if (ViewModel.ServiceStatus == "Unavailable" && DateTimeOffset.UtcNow.Second % 5 == 0)
            {
                await ViewModel.RefreshAsync();
            }
        };
        timer.Start();
        Activated += async (_, _) =>
        {
            await ViewModel.RefreshAsync();
            UpdateControls();
        };
        Closed += (_, _) => timer.Stop();
        UpdateControls();
    }

    private async void GeneratePairingCode_Click(object sender, RoutedEventArgs e)
    {
        await ViewModel.GenerateCodeAsync();
        UpdateControls();
    }

    private async void UpdateHost_Click(object sender, RoutedEventArgs e)
    {
        UpdateHostButton.IsEnabled = false;
        UpdateHostButton.Content = "Checking…";
        try
        {
            var installer = await updates.DownloadUpdateAsync(ViewModel.HostVersion, WindowsSignatureVerifier.IsTrusted);
            if (installer is null)
            {
                UpdateStatusText.Text = $"Host {ViewModel.HostVersion} is up to date.";
                return;
            }
            Process.Start(new ProcessStartInfo
            {
                FileName = installer,
                UseShellExecute = true,
                Verb = "runas",
            });
        }
        catch (Exception error) when (error is HttpRequestException or IOException or InvalidOperationException or OperationCanceledException or CryptographicException or System.ComponentModel.Win32Exception or UnauthorizedAccessException or JsonException)
        {
            ErrorText.Text = $"The host update could not be installed safely: {error.Message}";
            ErrorBorder.Visibility = Visibility.Visible;
        }
        finally
        {
            UpdateHostButton.IsEnabled = true;
            UpdateHostButton.Content = "Check for host updates";
        }
    }

    private void UpdateControls()
    {
        ServiceStatusText.Text = ViewModel.ServiceStatus;
        HostPortText.Text = ViewModel.Port is 0 ? "—" : ViewModel.Port.ToString();
        HostVersionText.Text = ViewModel.HostVersion;
        UpdateStatusText.Text = ViewModel.UpdateStatus;
        PairingStatusText.Text = ViewModel.PairingStatus;
        PairingCodeText.Text = ViewModel.PairingCode;
        PairingExpirationText.Text = ViewModel.Expiration;
        GeneratePairingCodeButton.IsEnabled = ViewModel.CanGenerate;
        ErrorText.Text = ViewModel.ErrorMessage ?? string.Empty;
        ErrorBorder.Visibility = ViewModel.ErrorMessage is null ? Visibility.Collapsed : Visibility.Visible;
    }
}
