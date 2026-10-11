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
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        // Enable native Windows 11 Mica backdrop & Immersive Dark Mode
        WindowsNative.EnableMicaAndDarkMode(this);
    }

    private void MainWindow_Loaded(object sender, RoutedEventArgs e)
    {
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

        if (started)
        {
            NodeStatusDot.Fill = (System.Windows.Media.Brush)FindResource("OnlineGreenBrush");
            NodeStatusText.Text = $"Active (Port {_boundPort})";
        }
        else
        {
            NodeStatusDot.Fill = System.Windows.Media.Brushes.Crimson;
            NodeStatusText.Text = "Port binding error";
        }

        // 2. Wire Events from Rust Core
        _node.MessageReceived += OnMessageReceived;
        _node.PeerConnected += OnPeerConnected;
        _node.PeerDisconnected += OnPeerDisconnected;
        _node.FileOfferReceived += OnFileOfferReceived;
        _node.FileCompleted += OnFileCompleted;
        _node.GroupMessageReceived += OnGroupMessageReceived;
        _node.NoticeReceived += OnNoticeReceived;

        // 3. Populate Initial Data
        TxtShortPeerId.Text = _node.PeerId.Length > 12 ? _node.PeerId[..12] + "..." : _node.PeerId;
        TxtFullPeerId.Text = _node.PeerId;
        TxtFullPubKey.Text = _node.PublicKey;

        RefreshContacts();
        RefreshGroups();
        RefreshMessages();
        RefreshNetworkStats();

        // 4. Background refresh timer (every 2s for stats & transfers)
        _pollTimer.Interval = TimeSpan.FromSeconds(2.0);
        _pollTimer.Tick += (s, ev) =>
        {
            RefreshNetworkStats();
            RefreshFileTransfers();
        };
        _pollTimer.Start();
    }

    private void MainWindow_Closing(object? sender, System.ComponentModel.CancelEventArgs e)
    {
        _pollTimer.Stop();
        _node.Stop();
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
            TxtStatusBar.Text = $"New encrypted message from {msg.Sender}";
        });
    }

    private void OnPeerConnected(string peerId, string addr)
    {
        Dispatcher.Invoke(() =>
        {
            TxtStatusBar.Text = $"Connected to peer: {addr}";
            RefreshNetworkStats();
        });
    }

    private void OnPeerDisconnected(string peerId)
    {
        Dispatcher.Invoke(() =>
        {
            string shortId = peerId.Length > 8 ? peerId[..8] : peerId;
            TxtStatusBar.Text = $"Peer disconnected: {shortId}";
            RefreshNetworkStats();
        });
    }

    private void OnFileOfferReceived(string fileId, string sender, string fileName, ulong fileSize)
    {
        Dispatcher.Invoke(() =>
        {
            WindowsNative.PlayNotificationSound();
            string shortSender = sender.Length > 8 ? sender[..8] : sender;
            TxtStatusBar.Text = $"Receiving '{fileName}' ({fileSize / 1024} KB) from {shortSender}";
            RefreshFileTransfers();
        });
    }

    private void OnFileCompleted(string fileId, string fileName, string path)
    {
        Dispatcher.Invoke(() =>
        {
            WindowsNative.PlayNotificationSound();
            TxtStatusBar.Text = $"File transfer completed: {fileName}";
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
            TxtStatusBar.Text = $"Group message in '{groupName}'";
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
        ViewNetwork.Visibility = NavNetwork.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        ViewSettings.Visibility = NavSettings.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
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

            TxtChatContactName.Text = contact.Name;
            TxtChatContactFingerprint.Text = $"Fingerprint: {contact.Fingerprint}";
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

        bool success = _node.SendChatMessage(_selectedContact.Name, text);
        if (success)
        {
            TxtMessageInput.Text = "";
            RefreshMessages();
            TxtStatusBar.Text = $"Encrypted message sent to {_selectedContact.Name}";
        }
        else
        {
            TxtStatusBar.Text = $"Could not deliver directly to {_selectedContact.Name}. Queued for relay distribution.";
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
            TxtActiveGroupName.Text = $"{grp.Name} (Security Epoch {grp.Epoch}) · {grp.MemberCount} member(s)";
            BtnAddGroupMember.Visibility = Visibility.Visible;
            PanelGroupMessages.Children.Clear();
            TxtGroupMessageInput.Focus();
        }
    }

    private void BtnCreateGroup_Click(object sender, RoutedEventArgs e)
    {
        string? groupName = PromptDialog.Show(this, "Create Group", "Enter encrypted group name:", "DevTeam");
        if (!string.IsNullOrWhiteSpace(groupName))
        {
            var res = _node.CreateGroup(groupName.Trim());
            if (res.Success)
            {
                RefreshGroups();
                TxtStatusBar.Text = $"Encrypted group '{groupName}' created.";
            }
        }
    }

    private void BtnAddGroupMember_Click(object sender, RoutedEventArgs e)
    {
        if (_selectedGroup == null) return;

        string? contactName = PromptDialog.Show(this, "Add Member", "Enter contact nickname to add to group:", "");
        if (!string.IsNullOrWhiteSpace(contactName))
        {
            bool ok = _node.AddGroupMember(_selectedGroup.GroupId, contactName.Trim());
            if (ok)
            {
                RefreshGroups();
                TxtStatusBar.Text = $"Added '{contactName}' to group. Epoch symmetric key rotated.";
            }
            else
            {
                TxtStatusBar.Text = $"Failed to add '{contactName}' to group. Ensure contact exists.";
            }
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

        bool ok = _node.SendGroupChat(_selectedGroup.GroupId, text);
        if (ok)
        {
            AppendGroupMessage("Me", text, DateTimeOffset.UtcNow.ToUnixTimeSeconds());
            TxtGroupMessageInput.Text = "";
            TxtStatusBar.Text = $"Group message sent to {_selectedGroup.Name}";
        }
    }

    private void AppendGroupMessage(string sender, string text, long timestamp)
    {
        var border = new Border
        {
            Background = sender == "Me" ? (System.Windows.Media.Brush)FindResource("OutgoingBubbleBrush")
                                         : (System.Windows.Media.Brush)FindResource("CardBgBrush"),
            BorderBrush = sender == "Me" ? null : (System.Windows.Media.Brush)FindResource("BorderBrush"),
            BorderThickness = sender == "Me" ? new Thickness(0) : new Thickness(1),
            CornerRadius = sender == "Me" ? new CornerRadius(14, 14, 2, 14) : new CornerRadius(14, 14, 14, 2),
            Padding = new Thickness(14, 9, 14, 9),
            Margin = new Thickness(0, 4, 0, 4),
            MaxWidth = 520,
            HorizontalAlignment = sender == "Me" ? HorizontalAlignment.Right : HorizontalAlignment.Left
        };

        var sp = new StackPanel();
        if (sender != "Me")
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
        string name = TxtNewContactName.Text.Trim();
        string pubkey = TxtNewContactPubKey.Text.Trim();

        if (string.IsNullOrEmpty(name) || string.IsNullOrEmpty(pubkey))
        {
            TxtStatusBar.Text = "Please enter both Contact Nickname and 64-char Hex Public Key.";
            return;
        }

        bool ok = _node.AddContact(name, pubkey);
        if (ok)
        {
            TxtNewContactName.Text = "";
            TxtNewContactPubKey.Text = "";
            RefreshContacts();
            TxtStatusBar.Text = $"Contact '{name}' added successfully.";
        }
        else
        {
            TxtStatusBar.Text = "Invalid Public Key hex format (must be 64 valid hex characters).";
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
        var dlg = new OpenFileDialog
        {
            Title = "Select File to Send via P2P"
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
        if (CmbFileContact.SelectedItem is not ContactDto contact)
        {
            TxtStatusBar.Text = "Please select a recipient contact.";
            return;
        }

        string path = TxtFilePath.Text.Trim();
        if (!File.Exists(path))
        {
            TxtStatusBar.Text = "Selected file does not exist on disk.";
            return;
        }

        bool ok = _node.SendFile(contact.Name, path);
        if (ok)
        {
            TxtStatusBar.Text = $"Encrypted transfer initiated for '{Path.GetFileName(path)}'.";
            TxtFilePath.Text = "";
            RefreshFileTransfers();
        }
        else
        {
            TxtStatusBar.Text = "Failed to start file transfer. Verify connection status.";
        }
    }

    private void RefreshFileTransfers()
    {
        var transfers = _node.GetFileTransfers();
        LstFileTransfers.ItemsSource = transfers;
    }

    private void BtnOpenDownloads_Click(object sender, RoutedEventArgs e)
    {
        string downloadsDir = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "p2p_data_win", "downloads");
        Directory.CreateDirectory(downloadsDir);
        WindowsNative.OpenInExplorer(downloadsDir);
    }

    // --- Network & Relay Dashboard ---

    private void BtnConnectPeer_Click(object sender, RoutedEventArgs e)
    {
        string addr = TxtConnectPeerAddr.Text.Trim();
        if (string.IsNullOrEmpty(addr)) return;

        bool ok = _node.ConnectPeer(addr);
        if (ok)
        {
            TxtStatusBar.Text = $"TCP Handshake initiated to {addr}.";
            RefreshNetworkStats();
        }
        else
        {
            TxtStatusBar.Text = $"Failed to connect to {addr}. Check remote node status.";
        }
    }

    private void RefreshNetworkStats()
    {
        var stats = _node.GetNetworkStats();
        TxtMetricConnected.Text = stats.ConnectedPeers.Count.ToString();
        TxtMetricKnown.Text = stats.KnownAddresses.Count.ToString();
        TxtMetricRelay.Text = $"{stats.RelayStoredCount} msgs ({stats.RelayStoredBytes / 1024} KB)";
        TxtPeerCount.Text = $"{stats.ConnectedPeers.Count} peer(s) connected";
    }

    private void BtnCopyMyId_Click(object sender, RoutedEventArgs e)
    {
        if (!string.IsNullOrEmpty(_node.PeerId))
        {
            Clipboard.SetText(_node.PeerId);
            TxtStatusBar.Text = "Peer ID copied to clipboard!";
        }
    }

    private void BtnCopyPubKey_Click(object sender, RoutedEventArgs e)
    {
        if (!string.IsNullOrEmpty(_node.PublicKey))
        {
            Clipboard.SetText(_node.PublicKey);
            TxtStatusBar.Text = "Public Key copied to clipboard!";
        }
    }
}