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

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int p2p_node_get_discovered_peers_json(IntPtr handle, byte[] outBuf, nuint maxLen);

    [DllImport(DllName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int p2p_node_trigger_discovery_scan(IntPtr handle);

    static NativeMethods()
    {
        NativeLibrary.SetDllImportResolver(typeof(NativeMethods).Assembly, ResolveDll);
    }

    private static IntPtr ResolveDll(string libraryName, System.Reflection.Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (libraryName.Equals(DllName, StringComparison.OrdinalIgnoreCase) || libraryName.Equals("p2pchat", StringComparison.OrdinalIgnoreCase))
        {
            // 1. Try application base directory
            string baseDir = AppDomain.CurrentDomain.BaseDirectory;
            string localDll = System.IO.Path.Combine(baseDir, DllName);
            if (System.IO.File.Exists(localDll) && NativeLibrary.TryLoad(localDll, out IntPtr hLocal))
            {
                return hLocal;
            }

            // 2. Try extract from embedded manifest resource
            try
            {
                string tempDir = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "p2pchat_embedded");
                System.IO.Directory.CreateDirectory(tempDir);
                string targetDll = System.IO.Path.Combine(tempDir, DllName);

                using var stream = assembly.GetManifestResourceStream("P2PChatApp.p2pchat.dll");
                if (stream != null)
                {
                    bool needWrite = true;
                    if (System.IO.File.Exists(targetDll))
                    {
                        var fi = new System.IO.FileInfo(targetDll);
                        if (fi.Length == stream.Length)
                        {
                            needWrite = false;
                        }
                    }

                    if (needWrite)
                    {
                        try
                        {
                            using var fs = new System.IO.FileStream(targetDll, System.IO.FileMode.Create, System.IO.FileAccess.Write, System.IO.FileShare.Read);
                            stream.CopyTo(fs);
                            fs.Flush();
                        }
                        catch
                        {
                            // If file is locked or out of space, attempt loading targetDll directly
                        }
                    }

                    if (NativeLibrary.TryLoad(targetDll, out IntPtr hTemp))
                    {
                        return hTemp;
                    }
                }
            }
            catch
            {
                // Fallback to default loader
            }
        }
        return IntPtr.Zero;
    }
}
