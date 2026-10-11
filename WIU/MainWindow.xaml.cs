// MainWindow.xaml.cs
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using Microsoft.Win32;
using P2PChatApp.CoreBridge;
using P2PChatApp.Models;
using P2PChatApp.Services;

namespace P2PChatApp;

public partial class MainWindow : Window
{
    private readonly P2pNodeService _node = new();
    private readonly DispatcherTimer _pollTimer = new();

    private ContactDto? _selectedContact;
    private GroupDto? _selectedGroup;
    private List<MessageDto> _allMessages = new();
    private List<ContactDto> _allContacts = new();
    private int _boundPort = 7001;

    public MainWindow()
    {
        InitializeComponent();
        Loaded += MainWindow_Loaded;
        Closing += MainWindow_Closing;
        StateChanged += MainWindow_StateChanged;
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        // Enable native Windows 11 Mica backdrop & Immersive Dark Mode
        WindowsNative.EnableMicaAndDarkMode(this);
    }

    private void MainWindow_StateChanged(object? sender, EventArgs e)
    {
        bool isMaximized = WindowState == WindowState.Maximized;
        if (IconMaximize != null)
        {
            IconMaximize.Text = isMaximized ? "\uE923" : "\uE922";
        }

        if (RootGrid != null)
        {
            RootGrid.Margin = isMaximized ? new Thickness(7) : new Thickness(0);
        }

        bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
        if (BtnMaximize != null)
        {
            BtnMaximize.ToolTip = isMaximized ? (isEn ? "Restore Down" : "Khôi phục cửa sổ") : (isEn ? "Maximize" : "Phóng to");
        }
    }

    private void MainWindow_Loaded(object sender, RoutedEventArgs e)
    {
        // 0. Language Service Subscription & Initialization
        LanguageService.Instance.LanguageChanged += () =>
        {
            Dispatcher.Invoke(() => ApplyLanguage(LanguageService.Instance.CurrentLanguage));
        };
        CmbLanguage.SelectedIndex = LanguageService.Instance.CurrentLanguage == AppLanguage.English ? 1 : 0;
        ApplyLanguage(LanguageService.Instance.CurrentLanguage);

        // 1. Start the P2P Chat Node with smart port fallback
        string dataDir = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "p2p_data_win");
        bool started = false;

        for (int port = 7001; port <= 7020; port++)
        {
            if (_node.Start(dataDir, $"127.0.0.1:{port}", true))
            {
                started = true;
                _boundPort = port;
                break;
            }
        }

        bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
        if (started)
        {
            NodeStatusDot.Fill = (System.Windows.Media.Brush)FindResource("OnlineGreenBrush");
            NodeStatusText.Text = isEn ? $"Active (Port {_boundPort})" : $"Đang hoạt động (Cổng {_boundPort})";
        }
        else
        {
            NodeStatusDot.Fill = System.Windows.Media.Brushes.Crimson;
            NodeStatusText.Text = isEn ? "Port binding error" : "Lỗi gán cổng mạng";
        }

        // 2. Wire Events from Rust Core
        _node.MessageReceived += OnMessageReceived;
        _node.PeerConnected += OnPeerConnected;
        _node.PeerDisconnected += OnPeerDisconnected;
        _node.FileOfferReceived += OnFileOfferReceived;
        _node.FileCompleted += OnFileCompleted;
        _node.GroupMessageReceived += OnGroupMessageReceived;
        _node.PeerDiscovered += OnPeerDiscovered;
        _node.NoticeReceived += OnNoticeReceived;

        // 3. Populate Initial Data
        TxtShortPeerId.Text = _node.PeerId.Length > 12 ? _node.PeerId[..12] + "..." : _node.PeerId;
        TxtFullPeerId.Text = _node.PeerId;
        TxtFullPubKey.Text = _node.PublicKey;

        // Compute local Smart Invite Code
        string localIp = "127.0.0.1";
        try
        {
            var host = System.Net.Dns.GetHostEntry(System.Net.Dns.GetHostName());
            foreach (var ip in host.AddressList)
            {
                if (ip.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork && !System.Net.IPAddress.IsLoopback(ip))
                {
                    localIp = ip.ToString();
                    break;
                }
            }
        }
        catch { }
        TxtMySmartInvite.Text = $"{_node.PublicKey}@{localIp}:{_boundPort}";

        RefreshContacts();
        RefreshGroups();
        RefreshMessages();
        RefreshDiscoveredDevices();

        // 4. Background refresh timer (every 2s for discovered devices & transfers)
        _pollTimer.Interval = TimeSpan.FromSeconds(2.0);
        _pollTimer.Tick += (s, ev) =>
        {
            RefreshDiscoveredDevices();
            RefreshFileTransfers();
        };
        _pollTimer.Start();
    }

    private void MainWindow_Closing(object? sender, System.ComponentModel.CancelEventArgs e)
    {
        _pollTimer.Stop();
        _node.Stop();
    }

    // --- Dynamic Multi-language Switcher ---

    private void CmbLanguage_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (CmbLanguage == null || !IsLoaded) return;
        var newLang = CmbLanguage.SelectedIndex == 1 ? AppLanguage.English : AppLanguage.Vietnamese;
        LanguageService.Instance.SetLanguage(newLang);
    }

    private void ApplyLanguage(AppLanguage lang)
    {
        bool isEn = lang == AppLanguage.English;

        // Window Title & Header
        Title = isEn ? "P2P Chat — Decentralized Messaging App" : "P2P Chat — Ứng Dụng Trò Chuyện Phi Tập Trung";
        TxtTitleTag.Text = isEn ? "DECENTRALIZED · 100% RUST" : "PHI TẬP TRUNG · 100% RUST";
        BtnQuickCopyId.Content = isEn ? "📋 Copy ID" : "📋 Sao chép ID";
        BtnMinimize.ToolTip = isEn ? "Minimize" : "Thu nhỏ";
        BtnClose.ToolTip = isEn ? "Close" : "Đóng";
        bool isMaximized = WindowState == WindowState.Maximized;
        BtnMaximize.ToolTip = isMaximized ? (isEn ? "Restore Down" : "Khôi phục cửa sổ") : (isEn ? "Maximize" : "Phóng to");

        // Status pill
        if (NodeStatusText != null)
        {
            NodeStatusText.Text = isEn ? $"Active (Port {_boundPort})" : $"Đang hoạt động (Cổng {_boundPort})";
        }

        // Navigation Sidebar
        LblNavChannels.Text = isEn ? "CHANNELS" : "KÊNH TRÒ CHUYỆN";
        NavDirectChat.Content = isEn ? "💬  Direct Messages" : "💬  Tin nhắn trực tiếp";
        NavGroupChat.Content = isEn ? "👥  Secure Groups" : "👥  Nhóm bảo mật";
        LblNavNetworking.Text = isEn ? "NETWORK & SECURITY" : "MẠNG & BẢO MẬT";
        NavContacts.Content = isEn ? "📖  Contacts Book" : "📖  Danh bạ liên hệ";
        NavFiles.Content = isEn ? "📁  File Transfers" : "📁  Truyền tệp tin";
        NavNearby.Content = isEn ? "📡  Nearby Devices" : "📡  Thiết bị lân cận";
        NavSettings.Content = isEn ? "⚙️  Settings" : "⚙️  Cài đặt";

        // Direct Chat View
        BtnAddContactQuick.Content = isEn ? "+ Add" : "+ Thêm";
        LblEmptyHeroTitle.Text = isEn ? "End-to-End Encrypted P2P Chat" : "Trò Chuyện Mã Hóa Đầu Cuối P2P";
        LblEmptyHeroSub.Text = isEn ? "Zero Central Server · Absolute Privacy · 100% Rust Core" : "Không qua máy chủ · Tuyệt đối bảo mật · 100% Rust Core";
        LblEmptyHeroDesc.Text = isEn ? "Select a contact from the left sidebar or click '+ Add' to start an encrypted conversation." : "Chọn một người liên hệ từ danh sách bên trái hoặc nhấn '+ Thêm' để bắt đầu trò chuyện an toàn.";
        LblVerifiedBadge.Text = isEn ? "✓ E2EE VERIFIED" : "✓ ĐÃ XÁC THỰC E2EE";
        BtnSendFileQuick.Content = isEn ? "📎 Send File" : "📎 Gửi tệp";
        BtnAttachClip.ToolTip = isEn ? "Attach file to send" : "Đính kèm tệp gửi đi";
        BtnSendMessage.ToolTip = isEn ? "Send message (Enter)" : "Gửi tin nhắn (Enter)";

        // Group Chat View
        LblGroupHeader.Text = isEn ? "ENCRYPTED CHAT GROUPS" : "NHÓM TRÒ CHUYỆN MÃ HÓA";
        BtnCreateGroup.Content = isEn ? "+ New Group" : "+ Tạo nhóm";
        BtnAddGroupMember.Content = isEn ? "+ Add Member" : "+ Thêm thành viên";
        if (_selectedGroup == null)
        {
            TxtActiveGroupName.Text = isEn ? "Select or create a new group" : "Chọn hoặc tạo nhóm mới";
        }

        // Contacts View
        LblMyIdentityTitle.Text = isEn ? "YOUR CRYPTOGRAPHIC IDENTITY" : "DANH TÍNH MÃ HÓA CỦA BẠN";
        LblMyIdentityDesc.Text = isEn ? "Share your Public Key with peers so they can add you to their secure contacts." : "Chia sẻ Khóa công khai của bạn để bạn bè có thể thêm bạn trực tiếp vào danh bạ P2P an toàn.";
        LblPubKeyTitle.Text = isEn ? "X25519 Public Key (64-char hex):" : "Khóa công khai X25519 (64 ký tự hex):";
        BtnCopyPubKey.Content = isEn ? "Copy Key" : "Sao chép Khóa";
        LblPeerIdTitle.Text = isEn ? "Peer ID (SHA-256 Identifier):" : "Mã định danh Peer ID (SHA-256):";
        BtnCopyPeerId.Content = isEn ? "Copy ID" : "Sao chép ID";
        LblAddContactTitle.Text = isEn ? "Add New Peer to Contacts" : "Thêm Bạn Mới Vào Danh Bạ";
        LblAddContactDesc.Text = isEn ? "Enter a nickname and paste your peer's Public Key or Smart Invite Code (auto-connects)." : "Nhập tên gợi nhớ và dán Khóa công khai hoặc Mã mời thông minh của bạn bè (tự động kết nối).";
        BtnAddContact.Content = isEn ? "+ Add Contact" : "+ Thêm Liên Hệ";
        LblSavedContacts.Text = isEn ? "SAVED CONTACTS" : "DANH SÁCH LIÊN HỆ ĐÃ LƯU";

        // File Transfers View
        LblSendFileTitle.Text = isEn ? "Send Segmented Encrypted File (256 KiB)" : "Gửi Tệp Tin Mã Hóa Phân Đoạn (256 KiB)";
        LblSendFileDesc.Text = isEn ? "Files are chunked into 256 KiB pieces, encrypted with ChaCha20-Poly1305 and Ed25519 signed." : "Tệp được chia thành các khối 256 KiB, mã hóa ChaCha20-Poly1305 với chữ ký số Ed25519.";
        BtnBrowseFile.Content = isEn ? "Browse..." : "Chọn tệp...";
        BtnSendFileAction.Content = isEn ? "Send File" : "Gửi Tệp";
        LblActiveTransfers.Text = isEn ? "FILE TRANSFERS & DOWNLOADS" : "DANH SÁCH TRUYỀN TỆP & TẢI VỀ";
        BtnOpenDownloads.Content = isEn ? "Open Downloads Folder" : "Mở thư mục Tải về";

        // Nearby Devices View
        LblNearbyHeader.Text = isEn ? "Nearby Devices & Auto-Discovery" : "Thiết Bị Lân Cận & Tự Động Kết Nối";
        LblNearbySub.Text = isEn ? "Auto-discover peers on the same Wi-Fi/LAN and chat instantly without entering IP." : "Tự động dò tìm bạn bè trong cùng mạng Wi-Fi/LAN và kết nối tức thì không cần nhập IP.";
        LblRadarStatus.Text = isEn ? "Wi-Fi/LAN radar is actively scanning" : "Ra-đa quét Wi-Fi/LAN đang hoạt động";
        LblRadarSub.Text = isEn ? "Broadcasting and listening for peer beacon signals on local network" : "Đang tự động phát và bắt sóng beacon ngang hàng P2P trong mạng nội bộ";
        BtnScanNearby.Content = isEn ? "🔄 Scan Now" : "🔄 Quét Lại Ngay";
        LblInviteTitle.Text = isEn ? "YOUR SMART P2P INVITE CODE" : "MÃ MỜI P2P THÔNG MINH CỦA BẠN";
        BtnCopySmartInvite.Content = isEn ? "📋 Copy Invite Code" : "📋 Sao chép Mã Mời";
        LblInviteDesc.Text = isEn ? "💡 Send this code to a friend. When pasted into Contacts, both computers connect automatically!" : "💡 Bạn chỉ cần gửi mã này cho bạn bè. Bạn bè dán vào mục Danh bạ là hai máy tự động kết nối và chat ngay lập tức!";
        LblNearbyListTitle.Text = isEn ? "DEVICES FOUND ON LOCAL NETWORK" : "THIẾT BỊ PHÁT HIỆN TRONG MẠNG LAN";
        LblNoNearbyTitle.Text = isEn ? "No other devices detected on local network yet" : "Chưa phát hiện thiết bị nào khác trong mạng";
        LblNoNearbyDesc.Text = isEn ? "When friends open P2P Chat on the same Wi-Fi or LAN, their device will appear here for 1-click messaging." : "Khi bạn bè mở ứng dụng P2P Chat trong cùng mạng Wi-Fi hoặc mạng LAN, thiết bị sẽ tự động xuất hiện tại đây để bạn nhắn tin chỉ với 1 click.";

        // Settings View
        LblSettingsHeader.Text = isEn ? "System Settings & Preferences" : "Cài Đặt & Ngôn Ngữ Hệ Thống";
        LblLangTitle.Text = isEn ? "Interface Language" : "Ngôn ngữ giao diện (Interface Language)";
        LblLangDesc.Text = isEn ? "Choose your preferred display language for the application." : "Tùy chọn ngôn ngữ hiển thị cho toàn bộ ứng dụng.";
        LblTechTitle.Text = isEn ? "Architecture & Core Technologies" : "Kiến trúc & Công nghệ Lõi";
        LblTech1.Text = isEn ? "• Pure Rust Core: 100% Rust Standard Library (0 external crates)" : "• Nhân Rust Core thuần: 100% Rust Standard Library (0 thư viện bên ngoài)";
        LblTech2.Text = isEn ? "• Native Windows 11 UI: DWM Mica, Windows Caption Buttons, Core Audio" : "• Giao diện Windows 11 Native: DWM Mica, chuẩn Caption Button Windows, Core Audio";
        LblTech3.Text = isEn ? "• Cryptography: X25519 DH, Ed25519 Signatures, ChaCha20-Poly1305 AEAD" : "• Mật mã học: X25519 DH, chữ ký số Ed25519, mã hóa ChaCha20-Poly1305 AEAD";
        LblTech4.Text = isEn ? "• Decentralized Mesh: Bitcoin-like Gossip Discovery, Store-and-Forward Relay" : "• Mạng phân tán: Bitcoin-like Gossip Discovery, Store-and-Forward Relay";

        // Bottom Status Bar
        TxtStatusBar.Text = isEn ? "Ready. Distributed P2P node is operational." : "Sẵn sàng. Node P2P phân tán đang hoạt động.";
        TxtAppVersion.Text = isEn ? "WIU v1.0 · Pure Rust Core" : "WIU v1.0 · Nhân Rust Thuần";

        RefreshDiscoveredDevices();
    }

    // --- Window Chrome Caption Buttons ---

    private void BtnMinimize_Click(object sender, RoutedEventArgs e)
    {
        WindowState = WindowState.Minimized;
    }

    private void BtnMaximize_Click(object sender, RoutedEventArgs e)
    {
        WindowState = WindowState == WindowState.Maximized ? WindowState.Normal : WindowState.Maximized;
    }

    private void BtnClose_Click(object sender, RoutedEventArgs e)
    {
        Close();
    }

    // --- Event Handlers from Rust Core ---

    private void OnMessageReceived(MessageDto msg)
    {
        Dispatcher.Invoke(() =>
        {
            WindowsNative.PlayNotificationSound();
            _allMessages.Add(msg);
            UpdateCurrentChatView();
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtStatusBar.Text = isEn ? $"New encrypted message from {msg.Sender}" : $"Tin nhắn mã hóa mới từ {msg.Sender}";
        });
    }

    private void OnPeerConnected(string peerId, string addr)
    {
        Dispatcher.Invoke(() =>
        {
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtStatusBar.Text = isEn ? $"Connected to peer: {addr}" : $"Đã kết nối với node: {addr}";
            RefreshDiscoveredDevices();
        });
    }

    private void OnPeerDisconnected(string peerId)
    {
        Dispatcher.Invoke(() =>
        {
            string shortId = peerId.Length > 8 ? peerId[..8] : peerId;
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtStatusBar.Text = isEn ? $"Peer disconnected: {shortId}" : $"Node đã ngắt kết nối: {shortId}";
            RefreshDiscoveredDevices();
        });
    }

    private void OnFileOfferReceived(string fileId, string sender, string fileName, ulong fileSize)
    {
        Dispatcher.Invoke(() =>
        {
            WindowsNative.PlayNotificationSound();
            string shortSender = sender.Length > 8 ? sender[..8] : sender;
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtStatusBar.Text = isEn ? $"Receiving '{fileName}' ({fileSize / 1024} KB) from {shortSender}" : $"Đang nhận '{fileName}' ({fileSize / 1024} KB) từ {shortSender}";
            RefreshFileTransfers();
        });
    }

    private void OnFileCompleted(string fileId, string fileName, string path)
    {
        Dispatcher.Invoke(() =>
        {
            WindowsNative.PlayNotificationSound();
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtStatusBar.Text = isEn ? $"File transfer completed: {fileName}" : $"Truyền tệp hoàn tất: {fileName}";
            RefreshFileTransfers();
        });
    }

    private void OnGroupMessageReceived(string groupId, string groupName, string sender, string text, long timestamp)
    {
        Dispatcher.Invoke(() =>
        {
            WindowsNative.PlayNotificationSound();
            if (_selectedGroup != null && _selectedGroup.GroupId.Equals(groupId, StringComparison.OrdinalIgnoreCase))
            {
                string shortSender = sender.Length > 8 ? sender[..8] : sender;
                AppendGroupMessage(shortSender, text, timestamp);
            }
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtStatusBar.Text = isEn ? $"Group message in '{groupName}'" : $"Tin nhắn nhóm trong '{groupName}'";
        });
    }

    private void OnNoticeReceived(string msg)
    {
        Dispatcher.Invoke(() =>
        {
            TxtStatusBar.Text = msg;
        });
    }

    // --- Navigation Switching ---

    private void Nav_Checked(object sender, RoutedEventArgs e)
    {
        if (ViewDirectChat == null) return;

        ViewDirectChat.Visibility = NavDirectChat.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        ViewGroupChat.Visibility = NavGroupChat.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        ViewContacts.Visibility = NavContacts.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        ViewFiles.Visibility = NavFiles.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        ViewNearby.Visibility = NavNearby.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        ViewSettings.Visibility = NavSettings.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        if (NavNearby.IsChecked == true)
        {
            RefreshDiscoveredDevices();
        }
    }

    private void BtnNavToAddContact_Click(object sender, RoutedEventArgs e)
    {
        NavContacts.IsChecked = true;
        TxtNewContactName.Focus();
    }

    // --- Direct Messages ---

    private void RefreshContacts()
    {
        _allContacts = _node.GetContacts();
        ApplyContactFilter();

        LstAllContacts.ItemsSource = _allContacts;
        CmbFileContact.ItemsSource = _allContacts;
        CmbFileContact.DisplayMemberPath = "Name";
        if (_allContacts.Count > 0 && CmbFileContact.SelectedIndex < 0)
        {
            CmbFileContact.SelectedIndex = 0;
        }
    }

    private void TxtSearchContact_TextChanged(object sender, TextChangedEventArgs e)
    {
        ApplyContactFilter();
    }

    private void ApplyContactFilter()
    {
        string filter = TxtSearchContact.Text.Trim();
        if (string.IsNullOrEmpty(filter))
        {
            LstChatContacts.ItemsSource = _allContacts;
        }
        else
        {
            LstChatContacts.ItemsSource = _allContacts
                .Where(c => c.Name.Contains(filter, StringComparison.OrdinalIgnoreCase) ||
                            c.Fingerprint.Contains(filter, StringComparison.OrdinalIgnoreCase))
                .ToList();
        }
    }

    private void RefreshMessages()
    {
        _allMessages = _node.GetMessages();
        UpdateCurrentChatView();
    }

    private void LstChatContacts_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (LstChatContacts.SelectedItem is ContactDto contact)
        {
            _selectedContact = contact;
            ChatEmptyState.Visibility = Visibility.Collapsed;
            ChatActivePane.Visibility = Visibility.Visible;

            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtChatContactName.Text = contact.Name;
            TxtChatContactFingerprint.Text = (isEn ? "Fingerprint: " : "Mã xác thực: ") + contact.Fingerprint;
            TxtChatHeaderInitials.Text = contact.Name.Length > 0 ? contact.Name[..1].ToUpperInvariant() : "?";

            UpdateCurrentChatView();
            TxtMessageInput.Focus();
        }
    }

    private void UpdateCurrentChatView()
    {
        if (_selectedContact == null)
        {
            ChatEmptyState.Visibility = Visibility.Visible;
            ChatActivePane.Visibility = Visibility.Collapsed;
            ItemsMessages.ItemsSource = null;
            return;
        }

        var filtered = _allMessages.Where(m =>
            m.Sender.Equals(_selectedContact.Name, StringComparison.OrdinalIgnoreCase) ||
            m.Recipient.Equals(_selectedContact.Name, StringComparison.OrdinalIgnoreCase)).ToList();

        ItemsMessages.ItemsSource = filtered;
        ScrollMessages.ScrollToBottom();
    }

    private void BtnSendMessage_Click(object sender, RoutedEventArgs e)
    {
        SendMessage();
    }

    private void TxtMessageInput_KeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter)
        {
            SendMessage();
        }
    }

    private void SendMessage()
    {
        if (_selectedContact == null) return;

        string text = TxtMessageInput.Text.Trim();
        if (string.IsNullOrEmpty(text)) return;

        bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
        bool success = _node.SendChatMessage(_selectedContact.Name, text);
        if (success)
        {
            TxtMessageInput.Text = "";
            RefreshMessages();
            TxtStatusBar.Text = isEn ? $"Encrypted message sent to {_selectedContact.Name}" : $"Đã gửi tin nhắn mã hóa tới {_selectedContact.Name}";
        }
        else
        {
            TxtStatusBar.Text = isEn
                ? $"Could not deliver directly to {_selectedContact.Name}. Queued for relay distribution."
                : $"Không thể chuyển trực tiếp tới {_selectedContact.Name}. Đã lưu hàng đợi gửi qua mạng Relay.";
            RefreshMessages();
        }
    }

    // --- Group Chat ---

    private void RefreshGroups()
    {
        var groups = _node.GetGroups();
        LstGroups.ItemsSource = groups;
    }

    private void LstGroups_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (LstGroups.SelectedItem is GroupDto grp)
        {
            _selectedGroup = grp;
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtActiveGroupName.Text = isEn
                ? $"{grp.Name} (Security Epoch {grp.Epoch}) · {grp.MemberCount} member(s)"
                : $"{grp.Name} (Kỷ nguyên bảo mật Epoch {grp.Epoch}) · {grp.MemberCount} thành viên";
            BtnAddGroupMember.Visibility = Visibility.Visible;
            PanelGroupMessages.Children.Clear();
            TxtGroupMessageInput.Focus();
        }
    }

    private void BtnCreateGroup_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            string title = isEn ? "Create Group" : "Tạo nhóm mới";
            string prompt = isEn ? "Enter encrypted group name:" : "Nhập tên nhóm trò chuyện mã hóa:";
            string? groupName = PromptDialog.Show(this, title, prompt, "DevTeam");
            if (!string.IsNullOrWhiteSpace(groupName))
            {
                var res = _node.CreateGroup(groupName.Trim());
                if (res.Success)
                {
                    RefreshGroups();
                    TxtStatusBar.Text = isEn ? $"Encrypted group '{groupName}' created." : $"Đã tạo nhóm trò chuyện '{groupName}'.";
                }
            }
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }

    private void BtnAddGroupMember_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            if (_selectedGroup == null) return;

            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            string title = isEn ? "Add Member" : "Thêm thành viên";
            string prompt = isEn ? "Enter contact nickname to add to group:" : "Nhập tên gợi nhớ của liên hệ cần thêm vào nhóm:";
            string? contactName = PromptDialog.Show(this, title, prompt, "");
            if (!string.IsNullOrWhiteSpace(contactName))
            {
                bool ok = _node.AddGroupMember(_selectedGroup.GroupId, contactName.Trim());
                if (ok)
                {
                    RefreshGroups();
                    TxtStatusBar.Text = isEn ? $"Added '{contactName}' to group. Epoch symmetric key rotated." : $"Đã thêm '{contactName}' vào nhóm. Khóa đối xứng Epoch đã được luân phiên bảo mật.";
                }
                else
                {
                    TxtStatusBar.Text = isEn ? $"Failed to add '{contactName}' to group. Ensure contact exists." : $"Không thể thêm '{contactName}' vào nhóm. Hãy chắc chắn liên hệ đã tồn tại trong danh bạ.";
                }
            }
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }

    private void BtnSendGroupMessage_Click(object sender, RoutedEventArgs e)
    {
        SendGroupMessage();
    }

    private void TxtGroupMessageInput_KeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter)
        {
            SendGroupMessage();
        }
    }

    private void SendGroupMessage()
    {
        if (_selectedGroup == null) return;
        string text = TxtGroupMessageInput.Text.Trim();
        if (string.IsNullOrEmpty(text)) return;

        bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
        bool ok = _node.SendGroupChat(_selectedGroup.GroupId, text);
        if (ok)
        {
            AppendGroupMessage(isEn ? "Me" : "Tôi", text, DateTimeOffset.UtcNow.ToUnixTimeSeconds());
            TxtGroupMessageInput.Text = "";
            TxtStatusBar.Text = isEn ? $"Group message sent to {_selectedGroup.Name}" : $"Đã gửi tin nhắn nhóm tới {_selectedGroup.Name}";
        }
    }

    private void AppendGroupMessage(string sender, string text, long timestamp)
    {
        bool isSelf = sender == "Me" || sender == "Tôi";
        var border = new Border
        {
            Background = isSelf ? (System.Windows.Media.Brush)FindResource("OutgoingBubbleBrush")
                                 : (System.Windows.Media.Brush)FindResource("CardBgBrush"),
            BorderBrush = isSelf ? null : (System.Windows.Media.Brush)FindResource("BorderBrush"),
            BorderThickness = isSelf ? new Thickness(0) : new Thickness(1),
            CornerRadius = isSelf ? new CornerRadius(14, 14, 2, 14) : new CornerRadius(14, 14, 14, 2),
            Padding = new Thickness(14, 9, 14, 9),
            Margin = new Thickness(0, 4, 0, 4),
            MaxWidth = 520,
            HorizontalAlignment = isSelf ? HorizontalAlignment.Right : HorizontalAlignment.Left
        };

        var sp = new StackPanel();
        if (!isSelf)
        {
            sp.Children.Add(new TextBlock
            {
                Text = sender,
                FontWeight = FontWeights.SemiBold,
                FontSize = 11,
                Foreground = (System.Windows.Media.Brush)FindResource("CyanBrush"),
                Margin = new Thickness(0, 0, 0, 3)
            });
        }
        sp.Children.Add(new TextBlock
        {
            Text = text,
            FontSize = 13,
            TextWrapping = TextWrapping.Wrap,
            Foreground = System.Windows.Media.Brushes.White
        });

        border.Child = sp;
        PanelGroupMessages.Children.Add(border);
        ScrollGroupMessages.ScrollToBottom();
    }

    // --- Contacts Management ---

    private void BtnAddContact_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            string name = TxtNewContactName.Text.Trim();
            string pubkey = TxtNewContactPubKey.Text.Trim();
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;

            if (string.IsNullOrEmpty(pubkey))
            {
                TxtStatusBar.Text = isEn ? "Please enter Public Key or Smart Invite Code." : "Vui lòng dán Khóa công khai hoặc Mã mời thông minh.";
                return;
            }

            if (string.IsNullOrEmpty(name))
            {
                string shortKey = pubkey.Contains('@') ? pubkey.Split('@')[0] : pubkey;
                if (shortKey.Length > 8) shortKey = shortKey[..8];
                name = (isEn ? "Peer_" : "Bạn_") + shortKey;
            }

            bool ok = _node.AddContact(name, pubkey);
            if (ok)
            {
                TxtNewContactName.Text = "";
                TxtNewContactPubKey.Text = "";
                RefreshContacts();
                TxtStatusBar.Text = isEn ? $"Contact '{name}' added successfully. Connecting..." : $"Đã thêm liên hệ '{name}' thành công. Đang tự động kết nối...";

                var added = _allContacts.FirstOrDefault(c => c.Name == name);
                if (added != null)
                {
                    _selectedContact = added;
                    NavDirectChat.IsChecked = true;
                    LstChatContacts.SelectedItem = added;
                }
            }
            else
            {
                TxtStatusBar.Text = isEn ? "Invalid Public Key hex format (must be 64 valid hex characters) or invalid invite code." : "Định dạng khóa hex không hợp lệ (phải đúng 64 ký tự hex) hoặc mã mời không hợp lệ.";
            }
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }

    private void BtnChatWithContact_Click(object sender, RoutedEventArgs e)
    {
        if (sender is Button btn && btn.Tag is ContactDto c)
        {
            _selectedContact = c;
            NavDirectChat.IsChecked = true;
            LstChatContacts.SelectedItem = c;
        }
    }

    // --- File Transfers ---

    private void BtnBrowseFile_Click(object sender, RoutedEventArgs e)
    {
        bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
        var dlg = new OpenFileDialog
        {
            Title = isEn ? "Select File to Send via P2P" : "Chọn tệp tin cần gửi qua P2P"
        };
        if (dlg.ShowDialog() == true)
        {
            TxtFilePath.Text = dlg.FileName;
        }
    }

    private void BtnSendFileQuick_Click(object sender, RoutedEventArgs e)
    {
        NavFiles.IsChecked = true;
        if (_selectedContact != null)
        {
            CmbFileContact.SelectedItem = _selectedContact;
        }
        BtnBrowseFile_Click(sender, e);
    }

    private void BtnSendFileAction_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            if (CmbFileContact.SelectedItem is not ContactDto contact)
            {
                TxtStatusBar.Text = isEn ? "Please select a recipient contact." : "Vui lòng chọn người nhận trong danh bạ.";
                return;
            }

            string path = TxtFilePath.Text.Trim();
            if (!File.Exists(path))
            {
                TxtStatusBar.Text = isEn ? "Selected file does not exist on disk." : "Tệp được chọn không tồn tại trên ổ đĩa.";
                return;
            }

            bool ok = _node.SendFile(contact.Name, path);
            if (ok)
            {
                TxtStatusBar.Text = isEn ? $"Encrypted transfer initiated for '{Path.GetFileName(path)}'." : $"Đã khởi tạo truyền tệp mã hóa '{Path.GetFileName(path)}'.";
                TxtFilePath.Text = "";
                RefreshFileTransfers();
            }
            else
            {
                TxtStatusBar.Text = isEn ? "Failed to start file transfer. Verify connection status." : "Không thể bắt đầu truyền tệp. Vui lòng kiểm tra kết nối mạng.";
            }
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }

    private void RefreshFileTransfers()
    {
        var transfers = _node.GetFileTransfers();
        LstFileTransfers.ItemsSource = transfers;
    }

    private void BtnOpenDownloads_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            string downloadsDir = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "p2p_data_win", "downloads");
            Directory.CreateDirectory(downloadsDir);
            WindowsNative.OpenInExplorer(downloadsDir);
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }

    // --- Nearby Devices & Auto-Discovery ---

    private void OnPeerDiscovered(DiscoveredPeerDto peer)
    {
        Dispatcher.Invoke(() =>
        {
            RefreshDiscoveredDevices();
        });
    }

    private void RefreshDiscoveredDevices()
    {
        try
        {
            var peers = _node.GetDiscoveredPeers();
            LstDiscoveredDevices.ItemsSource = peers;
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtDiscoveredCount.Text = isEn ? $"{peers.Count} devices" : $"{peers.Count} thiết bị";
            PnlNoNearby.Visibility = peers.Count == 0 ? Visibility.Visible : Visibility.Collapsed;

            var stats = _node.GetNetworkStats();
            TxtPeerCount.Text = isEn
                ? $"{stats.ConnectedPeers.Count} peer(s) connected"
                : $"{stats.ConnectedPeers.Count} node đang kết nối";
        }
        catch { }
    }

    private void BtnScanNearby_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            _node.TriggerDiscoveryScan();
            RefreshDiscoveredDevices();
            bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
            TxtStatusBar.Text = isEn ? "Sent local discovery beacon to Wi-Fi/LAN." : "Đã phát sóng quét tìm kiếm lân cận trên Wi-Fi/LAN.";
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }

    private void BtnCopySmartInvite_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            if (!string.IsNullOrEmpty(TxtMySmartInvite.Text))
            {
                SafeSetClipboardText(TxtMySmartInvite.Text);
                bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
                TxtStatusBar.Text = isEn ? "Smart invite code copied to clipboard!" : "Đã sao chép Mã Mời thông minh vào khay nhớ tạm!";
            }
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }

    private void BtnAddDiscoveredPeer_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            if (sender is Button btn && btn.Tag is DiscoveredPeerDto peer)
            {
                bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
                string defaultName = string.IsNullOrWhiteSpace(peer.Name) ? $"Peer_{peer.Address.Replace(':', '_')}" : peer.Name;
                
                // Automatically add as contact and connect
                bool ok = _node.AddContact(defaultName, peer.SmartInviteCode);
                if (ok)
                {
                    RefreshContacts();
                    TxtStatusBar.Text = isEn ? $"Added {defaultName} and connected!" : $"Đã thêm {defaultName} vào danh bạ và kết nối thành công!";
                    // Switch to direct chat with them
                    var added = _allContacts.FirstOrDefault(c => c.Name == defaultName);
                    if (added != null)
                    {
                        LstChatContacts.SelectedItem = added;
                    }
                    NavDirectChat.IsChecked = true;
                }
            }
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }

    public void ShowErrorMessage(string message)
    {
        if (TxtStatusBar != null)
        {
            TxtStatusBar.Text = message;
        }
    }

    private bool SafeSetClipboardText(string text)
    {
        if (string.IsNullOrEmpty(text)) return false;
        for (int i = 0; i < 10; i++)
        {
            try
            {
                Clipboard.SetDataObject(text, true);
                return true;
            }
            catch (System.Runtime.InteropServices.COMException)
            {
                System.Threading.Thread.Sleep(50);
            }
            catch
            {
                break;
            }
        }
        return false;
    }

    private void BtnCopyMyId_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            if (!string.IsNullOrEmpty(_node.PeerId))
            {
                bool copied = SafeSetClipboardText(_node.PeerId);
                bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
                TxtStatusBar.Text = copied
                    ? (isEn ? "Peer ID copied to clipboard!" : "Đã sao chép Peer ID vào khay nhớ tạm!")
                    : (isEn ? "Could not access clipboard." : "Không thể truy cập khay nhớ tạm (đang bận).");
            }
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }

    private void BtnCopyPubKey_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            if (!string.IsNullOrEmpty(_node.PublicKey))
            {
                bool copied = SafeSetClipboardText(_node.PublicKey);
                bool isEn = LanguageService.Instance.CurrentLanguage == AppLanguage.English;
                TxtStatusBar.Text = copied
                    ? (isEn ? "Public Key copied to clipboard!" : "Đã sao chép Khóa công khai vào khay nhớ tạm!")
                    : (isEn ? "Could not access clipboard." : "Không thể truy cập khay nhớ tạm (đang bận).");
            }
        }
        catch (Exception ex)
        {
            TxtStatusBar.Text = $"Lỗi: {ex.Message}";
        }
    }
}