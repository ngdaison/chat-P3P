using System.Windows;
using P2PChatApp.Services;

namespace P2PChatApp;

public partial class PromptDialog : Window
{
    public string InputText { get; private set; } = "";

    public PromptDialog(string title, string prompt, string defaultValue = "")
    {
        InitializeComponent();
        Title = title;
        TxtPromptMessage.Text = prompt;
        TxtInput.Text = defaultValue;
        TxtInput.SelectAll();
        TxtInput.Focus();
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        WindowsNative.EnableMicaAndDarkMode(this);
    }

    private void BtnOk_Click(object sender, RoutedEventArgs e)
    {
        InputText = TxtInput.Text.Trim();
        DialogResult = true;
        Close();
    }

    private void BtnCancel_Click(object sender, RoutedEventArgs e)
    {
        DialogResult = false;
        Close();
    }

    public static string? Show(Window owner, string title, string prompt, string defaultValue = "")
    {
        var dlg = new PromptDialog(title, prompt, defaultValue)
        {
            Owner = owner
        };
        return dlg.ShowDialog() == true ? dlg.InputText : null;
    }
}
