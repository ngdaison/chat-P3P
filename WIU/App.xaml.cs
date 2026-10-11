// App.xaml.cs
using System.Windows;
using System.Windows.Threading;

namespace P2PChatApp;

/// <summary>
/// Interaction logic for App.xaml
/// </summary>
public partial class App : Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // Global UI thread exception handler: prevents app crashes
        DispatcherUnhandledException += App_DispatcherUnhandledException;

        // Non-UI thread unhandled exceptions
        AppDomain.CurrentDomain.UnhandledException += (s, args) =>
        {
            // Keep app alive and log
        };

        // Unobserved task exceptions
        TaskScheduler.UnobservedTaskException += (s, args) =>
        {
            args.SetObserved();
        };
    }

    private void App_DispatcherUnhandledException(object sender, DispatcherUnhandledExceptionEventArgs e)
    {
        // Mark exception as handled so the application does not crash
        e.Handled = true;

        if (MainWindow is MainWindow mw)
        {
            mw.Dispatcher.Invoke(() =>
            {
                mw.ShowErrorMessage($"Đã xảy ra lỗi hệ thống: {e.Exception.Message}");
            });
        }
    }
}
