# WIU — Windows Interface Unit (`P2PChatApp`)

> **Ứng dụng Desktop Windows Bản địa (Native Windows Client) dành cho mạng trò chuyện phi tập trung P2P Chat Core.**  
> Viết bằng C# .NET 9 Desktop kết hợp trực tiếp với nhân mật mã & mạng thuần Rust (`p2pchat.dll`) qua cơ chế C-ABI FFI hiệu năng cao.

---

## 1. Triết lý Thiết kế: 100% Windows Native

Ứng dụng **WIU** được xây dựng với phương châm **tối đa hóa hiệu năng và trải nghiệm người dùng trên Windows**, tuyệt đối **không sử dụng Electron, Chromium nhúng, WebView2 hay các công nghệ lai (hybrid)** gây tốn RAM.

* **Hiệu ứng Kính Mờ Windows 11 (Mica Backdrop & Acrylic)**: Tích hợp trực tiếp DWM API (`dwmapi.dll` - `DwmSetWindowAttribute`) với hiệu ứng `DWMSBT_MAINWINDOW` và `DWMWCP_ROUND`, giúp cửa sổ phản chiếu hình nền Desktop theo thời gian thực và tự động bo tròn góc chuẩn Windows 11.
* **Giao diện Fluent Dark Mode**: Bật cờ `DWMWA_USE_IMMERSIVE_DARK_MODE` đồng bộ thanh tiêu đề và bảng màu Dark Theme hiện đại với hệ điều hành.
* **Âm thanh Hệ thống Windows (Core Audio)**: Sử dụng `System.Media.SystemSounds.Asterisk` phát âm thanh thông báo chuẩn khi nhận tin nhắn hoặc truyền tệp thành công.
* **Tích hợp Windows Shell & File Explorer**:
  * Hộp thoại chọn tệp chuẩn Windows `Microsoft.Win32.OpenFileDialog`.
  * Nút "Open Downloads Folder" mở trực tiếp Windows Explorer qua `explorer.exe /select` trỏ đúng vào thư mục tệp tin đã nhận.
* **Tiết kiệm Tài nguyên Hệ thống**: Toàn bộ ứng dụng (bao gồm cả nhân mạng P2P và giao diện) chỉ tiêu tốn ~35–50 MB RAM khi hoạt động, khởi động tức thì trong < 0.5 giây.

---

## 2. Kiến trúc Cầu nối (C-ABI FFI Bridge)

Giao diện Windows kết nối trực tiếp với nhân Rust `CCA` thông qua thư viện liên kết động `p2pchat.dll`:

```
┌────────────────────────────────────────────────────────┐
│                   WIU (Windows Client)                 │
│  - MainWindow.xaml (Fluent Navigation & Views)         │
│  - WindowsNative.cs (DWM Mica, Shell, System Sounds)   │
│  - P2pNodeService.cs (Background Polling Event Loop)   │
└───────────────────────────┬────────────────────────────┘
                            │ P/Invoke C-ABI (0ms Latency)
┌───────────────────────────▼────────────────────────────┐
│                  p2pchat.dll (CCA Core)                │
│  - p2p_node_start() / p2p_node_stop()                  │
│  - p2p_node_send_chat() / p2p_node_send_file()         │
│  - p2p_node_create_group() / p2p_node_poll_event()     │
│  - 100% Pure Rust Standard Library (0 External Crates) │
└────────────────────────────────────────────────────────┘
```

---

## 3. Các Phân hệ Màn hình (Views)

1. **Direct Messages (Trò chuyện 1-1)**:
   * Danh sách người liên hệ bên trái kèm Fingerprint xác thực an toàn.
   * Bong bóng tin nhắn Fluent Design: tin gửi đi màu Xanh Accent (`#0078D4`), tin nhận màu bề mặt thẻ xám (`#2B2B2B`).
   * Phím tắt `Enter` gửi tin nhắn nhanh.
2. **Encrypted Groups (Nhóm Trò chuyện Bảo mật)**:
   * Tạo nhóm phi tập trung không cần server.
   * Thêm thành viên với cơ chế tự động xoay khóa bảo mật Epoch Key Rotation.
   * Cửa sổ chat nhóm hiển thị định danh người gửi và thời gian thực.
3. **Contacts (Danh bạ & Fingerprint)**:
   * Xem và sao chép Peer ID, Public Key hex 64 ký tự của bản thân.
   * Thêm bạn mới từ khóa công khai hex.
   * Xem Fingerprint bảo mật ngoài luồng của từng liên hệ.
4. **File Transfers (Truyền Tệp Phân đoạn 256 KiB)**:
   * Chọn người nhận và duyệt tệp từ Windows Explorer.
   * Hiển thị danh sách tiến độ, số khối đã hoàn thành và kích thước tệp.
   * Nút mở thư mục `downloads/` trực tiếp trên máy.
5. **Network & Relay Dashboard (Giám sát Mạng & Lưu hộ)**:
   * Kết nối chủ động tới node P2P khác qua `IP:Port`.
   * Thẻ thống kê: Số Peer đang kết nối, Số địa chỉ đã biết, Dung lượng và số tin nhắn đang lưu hộ ngoại tuyến (Relay Store).
6. **Settings (Cài đặt & Kiến trúc)**:
   * Thông tin chi tiết về mã hóa và kiến trúc phân tán.

---

## 4. Hướng dẫn Biên dịch & Khởi chạy

### 4.1. Yêu cầu Môi trường
* Windows 10 (bản 1809 trở lên) hoặc Windows 11.
* .NET SDK 9.0 (đã cài sẵn).
* Rust Compiler (để build `p2pchat.dll`).

### 4.2. Biên dịch Nhân Rust DLL
```powershell
# Chạy từ thư mục gốc E:\DATA\source\chat
cargo build
# Thư viện p2pchat.dll sẽ được sinh ra tại target/debug/p2pchat.dll
```

### 4.3. Biên dịch & Chạy Ứng dụng Windows
```powershell
# Chuyển vào thư mục WIU
cd E:\DATA\source\chat\WIU

# Biên dịch dự án .NET 9
dotnet build

# Chạy ứng dụng giao diện
dotnet run
# Hoặc chạy trực tiếp file thực thi:
.\bin\Debug\net9.0-windows\P2PChatApp.exe
```

---

## 5. Kịch bản Thử nghiệm Thực tế: Windows App kết nối CLI Node

Bạn có thể mở đồng thời ứng dụng đồ họa Windows và một node dòng lệnh để kiểm tra nhắn tin E2EE hai chiều:

1. **Cửa sổ 1 — Khởi động Windows App (WIU)**:
   * Chạy `.\bin\Debug\net9.0-windows\P2PChatApp.exe` (lắng nghe cổng mặc định 7001).
   * Bấm nút **"📋 Copy My Peer ID"** hoặc vào tab Network để lấy Public Key của GUI.
2. **Cửa sổ 2 — Chạy CLI Node (Bob - Cổng 7002)**:
   ```powershell
   cargo run -- init --data ./test_bob
   cargo run -- run --data ./test_bob --listen 127.0.0.1:7002 --peer 127.0.0.1:7001
   ```
3. **Thao tác Kết nối & Nhắn tin**:
   * Tại cửa sổ Bob, gõ `/id` lấy Public Key của Bob.
   * Trên Windows App, vào tab **Contacts**, nhập tên "Bob" và paste Public Key của Bob rồi bấm **+ Add Contact**.
   * Trên Bob, gõ: `/add Alice <PUBKEY_CỦA_GUI>`.
   * Vào tab **Direct Messages** trên Windows App, chọn "Bob" và gõ: `"Xin chào Bob từ giao diện Windows 11!"` rồi ấn Enter.
   * Cửa sổ Bob lập tức nhận tin nhắn thời gian thực!
   * Bob gõ lại: `/send Alice "Chao Alice, da nhan duoc tin nhan E2EE!"`.
   * Windows App lập tức phát âm thanh thông báo và hiển thị bong bóng tin nhắn trên màn hình!
