// Converters.cs
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Media;

namespace P2PChatApp;

public static class Converters
{
    public static readonly IValueConverter BoolToVis = new BooleanToVisibilityConverter();
    public static readonly IValueConverter InverseBoolToVis = new InverseBooleanToVisibilityConverter();
    public static readonly IValueConverter Initials = new InitialsConverter();
    public static readonly IValueConverter AvatarBrush = new AvatarBrushConverter();
    public static readonly IValueConverter NullOrEmptyToVis = new NullOrEmptyToVisibilityConverter();
}

public class InverseBooleanToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        return value is bool b && b ? Visibility.Collapsed : Visibility.Visible;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
    {
        return value is Visibility v && v != Visibility.Visible;
    }
}

public class InitialsConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is string s && !string.IsNullOrWhiteSpace(s))
        {
            var parts = s.Trim().Split(' ', StringSplitOptions.RemoveEmptyEntries);
            if (parts.Length == 1)
            {
                return parts[0].Substring(0, Math.Min(2, parts[0].Length)).ToUpperInvariant();
            }
            if (parts.Length >= 2)
            {
                return $"{char.ToUpperInvariant(parts[0][0])}{char.ToUpperInvariant(parts[1][0])}";
            }
        }
        return "?";
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) => throw new NotImplementedException();
}

public class AvatarBrushConverter : IValueConverter
{
    private static readonly SolidColorBrush[] Palette = new[]
    {
        new SolidColorBrush(Color.FromRgb(0x00, 0x78, 0xD4)), // Blue
        new SolidColorBrush(Color.FromRgb(0x10, 0x7C, 0x41)), // Green
        new SolidColorBrush(Color.FromRgb(0x87, 0x64, 0xB8)), // Purple
        new SolidColorBrush(Color.FromRgb(0xCA, 0x50, 0x10)), // Orange
        new SolidColorBrush(Color.FromRgb(0x03, 0x83, 0x87)), // Teal
        new SolidColorBrush(Color.FromRgb(0xC2, 0x39, 0xB3)), // Magenta
        new SolidColorBrush(Color.FromRgb(0x49, 0x82, 0x05)), // Lime
    };

    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is string s && !string.IsNullOrEmpty(s))
        {
            int hash = Math.Abs(s.GetHashCode());
            return Palette[hash % Palette.Length];
        }
        return Palette[0];
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) => throw new NotImplementedException();
}

public class NullOrEmptyToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        bool isEmpty = value == null || (value is string s && string.IsNullOrWhiteSpace(s));
        bool invert = parameter is string p && p.Equals("invert", StringComparison.OrdinalIgnoreCase);
        if (invert)
        {
            return isEmpty ? Visibility.Visible : Visibility.Collapsed;
        }
        return isEmpty ? Visibility.Collapsed : Visibility.Visible;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) => throw new NotImplementedException();
}
