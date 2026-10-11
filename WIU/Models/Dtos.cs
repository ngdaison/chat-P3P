// Models/Dtos.cs
using System.Text.Json.Serialization;

namespace P2PChatApp.Models;

public class ContactDto
{
    [JsonPropertyName("name")]
    public string Name { get; set; } = "";

    [JsonPropertyName("peer_id")]
    public string PeerId { get; set; } = "";

    [JsonPropertyName("public_key")]
    public string PublicKey { get; set; } = "";

    [JsonPropertyName("fingerprint")]
    public string Fingerprint { get; set; } = "";
}

public class MessageDto
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = "";

    [JsonPropertyName("sender")]
    public string Sender { get; set; } = "";

    [JsonPropertyName("recipient")]
    public string Recipient { get; set; } = "";

    [JsonPropertyName("content")]
    public string Content { get; set; } = "";

    [JsonPropertyName("timestamp")]
    public long Timestamp { get; set; }

    [JsonPropertyName("is_outgoing")]
    public bool IsOutgoing { get; set; }

    [JsonPropertyName("status")]
    public string Status { get; set; } = "";

    public string FormattedTime => DateTimeOffset.FromUnixTimeSeconds(Timestamp).ToLocalTime().ToString("HH:mm");
}

public class GroupDto
{
    [JsonPropertyName("group_id")]
    public string GroupId { get; set; } = "";

    [JsonPropertyName("name")]
    public string Name { get; set; } = "";

    [JsonPropertyName("creator")]
    public string Creator { get; set; } = "";

    [JsonPropertyName("epoch")]
    public ulong Epoch { get; set; }

    [JsonPropertyName("members")]
    public List<string> Members { get; set; } = new();

    [JsonPropertyName("created_at")]
    public long CreatedAt { get; set; }

    public int MemberCount => Members.Count;
}

public class NetworkStatsDto
{
    [JsonPropertyName("connected_peers")]
    public List<string> ConnectedPeers { get; set; } = new();

    [JsonPropertyName("known_addresses")]
    public List<string> KnownAddresses { get; set; } = new();

    [JsonPropertyName("relay_enabled")]
    public bool RelayEnabled { get; set; }

    [JsonPropertyName("relay_stored_bytes")]
    public ulong RelayStoredBytes { get; set; }

    [JsonPropertyName("relay_stored_count")]
    public ulong RelayStoredCount { get; set; }
}

public class FileTransferItem
{
    public string FileId { get; set; } = "";
    public string FileName { get; set; } = "";
    public string Path { get; set; } = "";
    public ulong FileSize { get; set; }
    public uint CompletedChunks { get; set; }
    public uint TotalChunks { get; set; }
    public ulong BytesTransferred { get; set; }
    public string Status { get; set; } = "In Progress";
    public double ProgressPercent => TotalChunks > 0 ? ((double)CompletedChunks / TotalChunks) * 100.0 : 0.0;
}
