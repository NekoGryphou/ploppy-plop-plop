using Microsoft.UI.Xaml;

namespace DeckyPowerHostControl;

public partial class App : Application
{
    private Window? window;
    public App()
    {
        UnhandledException += (_, args) => WriteStartupError(args.Exception);
        try
        {
            InitializeComponent();
        }
        catch (Exception error)
        {
            WriteStartupError(error);
            throw;
        }
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            window = new MainWindow();
            window.Activate();
        }
        catch (Exception error)
        {
            WriteStartupError(error);
            throw;
        }
    }

    private static void WriteStartupError(Exception error)
    {
        try
        {
            var directory = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "DeckyPowerHostControl");
            Directory.CreateDirectory(directory);
            File.WriteAllText(Path.Combine(directory, "startup-error.log"), error.ToString());
        }
        catch
        {
            // Startup diagnostics must never replace the original WinUI exception.
        }
    }
}
