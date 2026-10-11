// CoreBridge/NativeMethods.cs
using System.Runtime.InteropServices;

namespace P2PChatApp.CoreBridge;

internal static class NativeMethods
{
    private const string DllName = "p2pchat.dll";

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr p2p_node_start(string dataDir, string listenAddr, int isRelay);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void p2p_node_stop(IntPtr handle);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int p2p_node_get_identity(
        IntPtr handle,
        byte[] outPeerId,
        nuint maxIdLen,
        byte[] outPubkey,
        nuint maxKeyLen);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int p2p_node_connect_peer(IntPtr handle, string addrStr);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int p2p_node_add_contact(IntPtr handle, string name, string pubkeyHex);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int p2p_node_list_contacts_json(IntPtr handle, byte[] outBuf, nuint maxLen);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int p2p_node_send_chat(IntPtr handle, string contactName, string text);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int p2p_node_get_messages_json(IntPtr handle, byte[] outBuf, nuint maxLen);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int p2p_node_send_file(IntPtr handle, string contactName, string filePath);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int p2p_node_get_file_transfers_json(IntPtr handle, byte[] outBuf, nuint maxLen);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int p2p_node_create_group(
        IntPtr handle,
        string groupName,
        byte[] outGroupIdHex,
        nuint maxIdLen);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int p2p_node_add_group_member(
        IntPtr handle,
        string groupIdHex,
        string contactName);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern int p2p_node_send_group_chat(
        IntPtr handle,
        string groupIdHex,
        string text);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int p2p_node_list_groups_json(IntPtr handle, byte[] outBuf, nuint maxLen);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int p2p_node_get_network_stats_json(IntPtr handle, byte[] outBuf, nuint maxLen);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int p2p_node_poll_event(IntPtr handle, byte[] outBuf, nuint maxLen);
}
