// Converters.cs
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;

namespace P2PChatApp;

public static class Converters
{
    public static readonly IValueConverter BoolToVis = new BooleanToVisibilityConverter();
    public static readonly IValueConverter InverseBoolToVis = new InverseBooleanToVisibilityConverter();
}

public class InverseBooleanToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is bool b)
        {
            return b ? Visibility.Collapsed : Visibility.Visible;
        }
        return Visibility.Visible;
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is Visibility v)
        {
            return v != Visibility.Visible;
        }
        return false;
    }
}
