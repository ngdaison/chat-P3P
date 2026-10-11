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

        if (LanguageService.Instance.CurrentLanguage == AppLanguage.English)
        {
            BtnCancel.Content = "Cancel";
            BtnOk.Content = "OK";
        }
        else
        {
            BtnCancel.Content = "Hủy bỏ";
            BtnOk.Content = "Xác nhận";
        }
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        try
        {
            WindowsNative.EnableMicaAndDarkMode(this);
        }
        catch { }
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

    public static string? Show(Window? owner, string title, string prompt, string defaultValue = "")
    {
        try
        {
            var dlg = new PromptDialog(title, prompt, defaultValue);
            if (owner != null && owner.IsLoaded && owner.IsVisible)
            {
                dlg.Owner = owner;
            }
            return dlg.ShowDialog() == true ? dlg.InputText : null;
        }
        catch
        {
            return null;
        }
    }
}
