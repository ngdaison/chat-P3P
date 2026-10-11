// CoreBridge/P2pNodeService.cs
using System.IO;
using System.Text;
using System.Text.Json;
using P2PChatApp.Models;

namespace P2PChatApp.CoreBridge;

public class P2pNodeService : IDisposable
{
    private IntPtr _handle = IntPtr.Zero;
    private readonly object _lock = new();
    private CancellationTokenSource? _pollCts;
    private Task? _pollTask;

    public bool IsRunning => _handle != IntPtr.Zero;
    public string PeerId { get; private set; } = "";
    public string PublicKey { get; private set; } = "";

    public event Action<MessageDto>? MessageReceived;
    public event Action<string, string>? PeerConnected;
    public event Action<string>? PeerDisconnected;
    public event Action<string, string, string, ulong>? FileOfferReceived;
    public event Action<string, uint, uint, ulong>? FileProgress;
    public event Action<string, string, string>? FileCompleted;
    public event Action<string, string>? FileFailed;
    public event Action<string, string, string, string, long>? GroupMessageReceived;
    public event Action<string, string, string, bool>? GroupMembershipChanged;
    public event Action<string>? NoticeReceived;

    public bool Start(string dataDir, string listenAddr, bool isRelay)
    {
        lock (_lock)
        {
            if (IsRunning)
                return true;

            Directory.CreateDirectory(dataDir);
            _handle = NativeMethods.p2p_node_start(dataDir, listenAddr, isRelay ? 1 : 0);
            if (_handle == IntPtr.Zero)
                return false;

            RefreshIdentity();

            _pollCts = new CancellationTokenSource();
            _pollTask = Task.Run(() => EventLoop(_pollCts.Token));
            return true;
        }
    }

    public void Stop()
    {
        lock (_lock)
        {
            if (!IsRunning)
                return;

            _pollCts?.Cancel();
            try { _pollTask?.Wait(1000); } catch { }

            NativeMethods.p2p_node_stop(_handle);
            _handle = IntPtr.Zero;
        }
    }

    private void RefreshIdentity()
    {
        if (!IsRunning) return;

        byte[] idBuf = new byte[128];
        byte[] keyBuf = new byte[128];
        int res = NativeMethods.p2p_node_get_identity(_handle, idBuf, (nuint)idBuf.Length, keyBuf, (nuint)keyBuf.Length);
        if (res == 0)
        {
            PeerId = Encoding.ASCII.GetString(idBuf).TrimEnd('\0');
            PublicKey = Encoding.ASCII.GetString(keyBuf).TrimEnd('\0');
        }
    }

    public bool ConnectPeer(string addr)
    {
        lock (_lock)
        {
            if (!IsRunning) return false;
            return NativeMethods.p2p_node_connect_peer(_handle, addr) == 0;
        }
    }

    public bool AddContact(string name, string pubkeyHex)
    {
        lock (_lock)
        {
            if (!IsRunning) return false;
            return NativeMethods.p2p_node_add_contact(_handle, name, pubkeyHex) == 0;
        }
    }

    public List<ContactDto> GetContacts()
    {
        lock (_lock)
        {
            if (!IsRunning) return new();
            byte[] buf = new byte[65536];
            int len = NativeMethods.p2p_node_list_contacts_json(_handle, buf, (nuint)buf.Length);
            if (len <= 0) return new();

            string json = Encoding.UTF8.GetString(buf, 0, len);
            try
            {
                return JsonSerializer.Deserialize<List<ContactDto>>(json) ?? new();
            }
            catch
            {
                return new();
            }
        }
    }

    public bool SendChatMessage(string contactName, string text)
    {
        lock (_lock)
        {
            if (!IsRunning) return false;
            return NativeMethods.p2p_node_send_chat(_handle, contactName, text) == 0;
        }
    }

    public List<MessageDto> GetMessages()
    {
        lock (_lock)
        {
            if (!IsRunning) return new();
            byte[] buf = new byte[131072];
            int len = NativeMethods.p2p_node_get_messages_json(_handle, buf, (nuint)buf.Length);
            if (len <= 0) return new();

            string json = Encoding.UTF8.GetString(buf, 0, len);
            try
            {
                return JsonSerializer.Deserialize<List<MessageDto>>(json) ?? new();
            }
            catch
            {
                return new();
            }
        }
    }

    public bool SendFile(string contactName, string filePath)
    {
        lock (_lock)
        {
            if (!IsRunning) return false;
            return NativeMethods.p2p_node_send_file(_handle, contactName, filePath) == 0;
        }
    }

    public List<string> GetFileTransfers()
    {
        lock (_lock)
        {
            if (!IsRunning) return new();
            byte[] buf = new byte[32768];
            int len = NativeMethods.p2p_node_get_file_transfers_json(_handle, buf, (nuint)buf.Length);
            if (len <= 0) return new();

            string json = Encoding.UTF8.GetString(buf, 0, len);
            try
            {
                return JsonSerializer.Deserialize<List<string>>(json) ?? new();
            }
            catch
            {
                return new();
            }
        }
    }

    public (bool Success, string GroupIdHex) CreateGroup(string groupName)
    {
        lock (_lock)
        {
            if (!IsRunning) return (false, "");
            byte[] gidBuf = new byte[128];
            int res = NativeMethods.p2p_node_create_group(_handle, groupName, gidBuf, (nuint)gidBuf.Length);
            if (res == 0)
            {
                string gid = Encoding.ASCII.GetString(gidBuf).TrimEnd('\0');
                return (true, gid);
            }
            return (false, "");
        }
    }

    public bool AddGroupMember(string groupIdHex, string contactName)
    {
        lock (_lock)
        {
            if (!IsRunning) return false;
            return NativeMethods.p2p_node_add_group_member(_handle, groupIdHex, contactName) == 0;
        }
    }

    public bool SendGroupChat(string groupIdHex, string text)
    {
        lock (_lock)
        {
            if (!IsRunning) return false;
            return NativeMethods.p2p_node_send_group_chat(_handle, groupIdHex, text) == 0;
        }
    }

    public List<GroupDto> GetGroups()
    {
        lock (_lock)
        {
            if (!IsRunning) return new();
            byte[] buf = new byte[65536];
            int len = NativeMethods.p2p_node_list_groups_json(_handle, buf, (nuint)buf.Length);
            if (len <= 0) return new();

            string json = Encoding.UTF8.GetString(buf, 0, len);
            try
            {
                return JsonSerializer.Deserialize<List<GroupDto>>(json) ?? new();
            }
            catch
            {
                return new();
            }
        }
    }

    public NetworkStatsDto GetNetworkStats()
    {
        lock (_lock)
        {
            if (!IsRunning) return new();
            byte[] buf = new byte[32768];
            int len = NativeMethods.p2p_node_get_network_stats_json(_handle, buf, (nuint)buf.Length);
            if (len <= 0) return new();

            string json = Encoding.UTF8.GetString(buf, 0, len);
            try
            {
                return JsonSerializer.Deserialize<NetworkStatsDto>(json) ?? new();
            }
            catch
            {
                return new();
            }
        }
    }

    private void EventLoop(CancellationToken ct)
    {
        byte[] buf = new byte[16384];

        while (!ct.IsCancellationRequested)
        {
            int status = 0;
            lock (_lock)
            {
                if (_handle != IntPtr.Zero)
                {
                    status = NativeMethods.p2p_node_poll_event(_handle, buf, (nuint)buf.Length);
                }
            }

            if (status > 0)
            {
                string json = Encoding.UTF8.GetString(buf).TrimEnd('\0');
                Array.Clear(buf, 0, buf.Length);
                ProcessEvent(json);
            }
            else
            {
                Thread.Sleep(50);
            }
        }
    }

    private void ProcessEvent(string json)
    {
        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;
            if (!root.TryGetProperty("event", out var evProp)) return;

            string evType = evProp.GetString() ?? "";
            switch (evType)
            {
                case "message_received":
                    var msg = new MessageDto
                    {
                        Id = root.GetProperty("id").GetString() ?? "",
                        Sender = root.GetProperty("sender").GetString() ?? "",
                        Recipient = "Me",
                        Content = root.GetProperty("content").GetString() ?? "",
                        Timestamp = root.GetProperty("timestamp").GetInt64(),
                        IsOutgoing = false,
                        Status = "Delivered"
                    };
                    MessageReceived?.Invoke(msg);
                    break;

                case "peer_connected":
                    PeerConnected?.Invoke(
                        root.GetProperty("peer_id").GetString() ?? "",
                        root.GetProperty("address").GetString() ?? "");
                    break;

                case "peer_disconnected":
                    PeerDisconnected?.Invoke(root.GetProperty("peer_id").GetString() ?? "");
                    break;

                case "file_offer":
                    FileOfferReceived?.Invoke(
                        root.GetProperty("file_id").GetString() ?? "",
                        root.GetProperty("sender").GetString() ?? "",
                        root.GetProperty("file_name").GetString() ?? "",
                        root.GetProperty("file_size").GetUInt64());
                    break;

                case "file_progress":
                    FileProgress?.Invoke(
                        root.GetProperty("file_id").GetString() ?? "",
                        root.GetProperty("completed").GetUInt32(),
                        root.GetProperty("total").GetUInt32(),
                        root.GetProperty("bytes").GetUInt64());
                    break;

                case "file_completed":
                    FileCompleted?.Invoke(
                        root.GetProperty("file_id").GetString() ?? "",
                        root.GetProperty("file_name").GetString() ?? "",
                        root.GetProperty("path").GetString() ?? "");
                    break;

                case "file_failed":
                    FileFailed?.Invoke(
                        root.GetProperty("file_id").GetString() ?? "",
                        root.GetProperty("reason").GetString() ?? "");
                    break;

                case "group_message":
                    GroupMessageReceived?.Invoke(
                        root.GetProperty("group_id").GetString() ?? "",
                        root.GetProperty("group_name").GetString() ?? "",
                        root.GetProperty("sender").GetString() ?? "",
                        root.GetProperty("content").GetString() ?? "",
                        root.GetProperty("timestamp").GetInt64());
                    break;

                case "group_membership":
                    GroupMembershipChanged?.Invoke(
                        root.GetProperty("group_id").GetString() ?? "",
                        root.GetProperty("group_name").GetString() ?? "",
                        root.GetProperty("member").GetString() ?? "",
                        root.GetProperty("added").GetBoolean());
                    break;

                case "notice":
                    NoticeReceived?.Invoke(root.GetProperty("message").GetString() ?? "");
                    break;
            }
        }
        catch
        {
            // Ignore malformed json safely
        }
    }

    public void Dispose()
    {
        Stop();
    }
}
