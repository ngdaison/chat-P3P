// Services/LanguageService.cs
using System.IO;
using System.Text.Json;

namespace P2PChatApp.Services;

public enum AppLanguage
{
    Vietnamese,
    English
}

public class LanguageService
{
    public static LanguageService Instance { get; } = new();

    public AppLanguage CurrentLanguage { get; private set; } = AppLanguage.Vietnamese;

    public event Action? LanguageChanged;

    private readonly string _configPath;

    public LanguageService()
    {
        string dir = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "p2p_data_win");
        Directory.CreateDirectory(dir);
        _configPath = Path.Combine(dir, "ui_settings.json");
        LoadSetting();
    }

    public void SetLanguage(AppLanguage lang)
    {
        if (CurrentLanguage == lang) return;
        CurrentLanguage = lang;
        SaveSetting();
        LanguageChanged?.Invoke();
    }

    private void LoadSetting()
    {
        try
        {
            if (File.Exists(_configPath))
            {
                string json = File.ReadAllText(_configPath);
                using var doc = JsonDocument.Parse(json);
                if (doc.RootElement.TryGetProperty("language", out var prop))
                {
                    string val = prop.GetString() ?? "vi";
                    CurrentLanguage = val.Equals("en", StringComparison.OrdinalIgnoreCase)
                        ? AppLanguage.English
                        : AppLanguage.Vietnamese;
                }
            }
        }
        catch { }
    }

    private void SaveSetting()
    {
        try
        {
            string json = JsonSerializer.Serialize(new
            {
                language = CurrentLanguage == AppLanguage.English ? "en" : "vi"
            });
            File.WriteAllText(_configPath, json);
        }
        catch { }
    }
}
