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

    public MainWindow()
    {
        InitializeComponent();
        Loaded += MainWindow_Loaded;
        Closing += MainWindow_Closing;
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        // Enable native Windows 11 Mica & Immersive Dark Mode
        WindowsNative.EnableMicaAndDarkMode(this);
    }

    private void MainWindow_Loaded(object sender, RoutedEventArgs e)
    {
        // 1. Start the P2P Chat Node
        string dataDir = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "p2p_data_win");
        bool started = _node.Start(dataDir, "127.0.0.1:7001", true);

        if (!started)
        {
            NodeStatusDot.Fill = System.Windows.Media.Brushes.Crimson;
            NodeStatusText.Text = "Port 7001 busy, fallback 7005";
            _node.Start(dataDir, "127.0.0.1:7005", true);
        }

        // 2. Wire Events
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

        // 4. Background refresh timer (every 2.5s for stats & transfers)
        _pollTimer.Interval = TimeSpan.FromSeconds(2.5);
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
            TxtStatusBar.Text = $"Peer disconnected: {peerId[..8]}";
            RefreshNetworkStats();
        });
    }

    private void OnFileOfferReceived(string fileId, string sender, string fileName, ulong fileSize)
    {
        Dispatcher.Invoke(() =>
        {
            WindowsNative.PlayNotificationSound();
            TxtStatusBar.Text = $"Receiving file '{fileName}' ({fileSize / 1024} KB) from {sender[..8]}";
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
                AppendGroupMessage(sender[..8], text, timestamp);
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

    // --- Navigation ---

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

    // --- Direct Messages ---

    private void RefreshContacts()
    {
        var contacts = _node.GetContacts();
        LstChatContacts.ItemsSource = contacts;
        LstAllContacts.ItemsSource = contacts;

        CmbFileContact.ItemsSource = contacts;
        CmbFileContact.DisplayMemberPath = "Name";
        if (contacts.Count > 0 && CmbFileContact.SelectedIndex < 0)
        {
            CmbFileContact.SelectedIndex = 0;
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
            TxtChatContactName.Text = contact.Name;
            TxtChatContactFingerprint.Text = $"Fingerprint: {contact.Fingerprint}";
            BtnSendFileQuick.Visibility = Visibility.Visible;
            UpdateCurrentChatView();
        }
    }

    private void UpdateCurrentChatView()
    {
        if (_selectedContact == null)
        {
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

    private void TxtMessageInput_KeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (e.Key == Key.Enter)
        {
            SendMessage();
        }
    }

    private void SendMessage()
    {
        if (_selectedContact == null)
        {
            MessageBox.Show("Please select a contact from the list first.", "P2P Chat", MessageBoxButton.OK, MessageBoxImage.Information);
            return;
        }

        string text = TxtMessageInput.Text.Trim();
        if (string.IsNullOrEmpty(text)) return;

        bool success = _node.SendChatMessage(_selectedContact.Name, text);
        if (success)
        {
            TxtMessageInput.Text = "";
            RefreshMessages();
        }
        else
        {
            MessageBox.Show("Failed to send message. Please ensure contact is added and route exists.", "Error", MessageBoxButton.OK, MessageBoxImage.Warning);
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
            TxtActiveGroupName.Text = $"{grp.Name} (Epoch {grp.Epoch}) · {grp.MemberCount} member(s)";
            BtnAddGroupMember.Visibility = Visibility.Visible;
            PanelGroupMessages.Children.Clear();
        }
    }

    private void BtnCreateGroup_Click(object sender, RoutedEventArgs e)
    {
        string? groupName = PromptDialog.Show(this, "Create Group", "Enter new encrypted group name:", "DevTeam");
        if (!string.IsNullOrWhiteSpace(groupName))
        {
            var res = _node.CreateGroup(groupName.Trim());
            if (res.Success)
            {
                RefreshGroups();
                TxtStatusBar.Text = $"Group '{groupName}' created.";
            }
        }
    }

    private void BtnAddGroupMember_Click(object sender, RoutedEventArgs e)
    {
        if (_selectedGroup == null) return;

        string? contactName = PromptDialog.Show(this, "Add Group Member", "Enter contact name to add to this group:", "");
        if (!string.IsNullOrWhiteSpace(contactName))
        {
            bool ok = _node.AddGroupMember(_selectedGroup.GroupId, contactName.Trim());
            if (ok)
            {
                RefreshGroups();
                TxtStatusBar.Text = $"Added '{contactName}' to group. Epoch rotated.";
            }
            else
            {
                MessageBox.Show("Failed to add member to group.", "Error", MessageBoxButton.OK, MessageBoxImage.Warning);
            }
        }
    }

    private void BtnSendGroupMessage_Click(object sender, RoutedEventArgs e)
    {
        SendGroupMessage();
    }

    private void TxtGroupMessageInput_KeyDown(object sender, System.Windows.Input.KeyEventArgs e)
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
        }
    }

    private void AppendGroupMessage(string sender, string text, long timestamp)
    {
        var border = new Border
        {
            Background = new System.Windows.Media.SolidColorBrush(System.Windows.Media.Color.FromRgb(0x2B, 0x2B, 0x2B)),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(12, 8, 12, 8),
            Margin = new Thickness(0, 4, 0, 4),
            MaxWidth = 550,
            HorizontalAlignment = sender == "Me" ? System.Windows.HorizontalAlignment.Right : System.Windows.HorizontalAlignment.Left
        };

        var sp = new StackPanel();
        sp.Children.Add(new TextBlock
        {
            Text = sender,
            FontWeight = FontWeights.SemiBold,
            FontSize = 11,
            Foreground = new System.Windows.Media.SolidColorBrush(System.Windows.Media.Color.FromRgb(0x00, 0xBC, 0xF9))
        });
        sp.Children.Add(new TextBlock
        {
            Text = text,
            FontSize = 13,
            TextWrapping = TextWrapping.Wrap,
            Foreground = System.Windows.Media.Brushes.White,
            Margin = new Thickness(0, 2, 0, 0)
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
            MessageBox.Show("Please enter both Contact Name and Public Key Hex.", "Input Required", MessageBoxButton.OK, MessageBoxImage.Information);
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
            MessageBox.Show("Invalid Public Key Hex format (must be 64 hex characters / 32 bytes).", "Error", MessageBoxButton.OK, MessageBoxImage.Error);
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
            MessageBox.Show("Please select a recipient contact.", "P2P Chat", MessageBoxButton.OK, MessageBoxImage.Information);
            return;
        }

        string path = TxtFilePath.Text.Trim();
        if (!File.Exists(path))
        {
            MessageBox.Show("Selected file does not exist.", "File Error", MessageBoxButton.OK, MessageBoxImage.Warning);
            return;
        }

        bool ok = _node.SendFile(contact.Name, path);
        if (ok)
        {
            TxtStatusBar.Text = $"Transfer initiated for '{Path.GetFileName(path)}'.";
            TxtFilePath.Text = "";
            RefreshFileTransfers();
        }
        else
        {
            MessageBox.Show("Failed to start file transfer. Verify connection with contact.", "Error", MessageBoxButton.OK, MessageBoxImage.Error);
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
            TxtStatusBar.Text = $"Connection initiated to {addr}.";
            RefreshNetworkStats();
        }
        else
        {
            MessageBox.Show($"Failed to connect to {addr}. Ensure remote peer is active.", "Connection Failed", MessageBoxButton.OK, MessageBoxImage.Warning);
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
            MessageBox.Show($"Peer ID copied to clipboard:\n\n{_node.PeerId}", "Copied", MessageBoxButton.OK, MessageBoxImage.Information);
        }
    }
}