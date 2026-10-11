# P2P Chat Core — Mô Hình Đe Dọa (Threat Model)

Mạng P2P hoạt động trong môi trường mở và không tin cậy. Dưới đây là phân tích chi tiết các vector tấn công và cơ chế phòng vệ của hệ thống:

---

## 1. Tấn công Thư rác & Tràn Gói tin (Spam & Flooding)

- **Nguy cơ**: Node xấu gửi hàng ngàn tin nhắn hoặc gói tin liên tục để gây nghẽn mạng và cạn kiệt tài nguyên node khác.
- **Biện pháp phòng thủ**:
  - Hàng đợi tin nhắn và bộ đệm nhận có giới hạn cứng.
  - Bộ đệm `SeenMessageCache` chặn phát tán lại các tin nhắn đã thấy.
  - Giới hạn kích thước khung truyền tối đa 64 KiB.
  - Ngắt kết nối socket nếu nhận dữ liệu bất thường hoặc timeout.

---

## 2. Tấn công Danh tính Ảo (Sybil Attack)

- **Nguy cơ**: Kẻ tấn công tự sinh hàng triệu cặp khóa để tạo ra số lượng lớn node giả nhằm áp đảo mạng P2P.
- **Thực tế công bố**: Trong mạng P2P mở không có phí tạo danh tính hay blockchain đào coin, không thể ngăn chặn 100% việc tạo khóa ảo.
- **Biện pháp phòng vệ trong MVP**:
  - Áp dụng chính sách danh bạ tin cậy cục bộ (`ContactBook`): Chỉ những liên hệ được người dùng chủ động thêm (`/add`) hoặc xác minh mã nhận dạng (`fingerprint`) mới được hiển thị tên xác thực.
  - Giới hạn số lượng kết nối tối đa trên mỗi node (`max_peers = 64`).

---

## 3. Tấn công Phát lại (Replay Attack)

- **Nguy cơ**: Kẻ tấn công ghi lại một phong bì mã hóa hợp lệ trên đường truyền và gửi lại nhiều lần để quấy rối người nhận.
- **Biện pháp phòng thủ**:
  - Mỗi phong bì có `message_id` ngẫu nhiên 32 byte.
  - Bộ đệm `SeenMessageCache` lưu trữ và loại bỏ ngay lập tức các tin nhắn trùng lặp.
  - Trường `expires_at` và `created_at` ngăn chặn việc phát lại tin nhắn đã hết hạn (TTL).

---

## 4. Tấn công Gói tin Sai Định Dạng (Malformed Packet / Memory Exhaustion)

- **Nguy cơ**: Gửi frame với độ dài khai báo cực lớn (ví dụ 4 GB) để ép bên nhận cấp phát bộ nhớ và bị tràn RAM (OOM Crash).
- **Biện pháp phòng thủ**:
  - Kiểm tra độ dài nghiêm ngặt: nếu `payload_len > 65,536` byte, parser từ chối ngay lập tức trước khi gọi hàm cấp phát `vec![]`.
  - Kiểm tra Magic bytes `P2CH` và phiên bản giao thức trước khi xử lý phần thân.
  - Xử lý lỗi an toàn bằng `Result` và `Option`, tuyệt đối không panic khi gặp dữ liệu mạng không hợp lệ.

---

## 5. Tấn công Cạn kiệt Bộ nhớ Lưu hộ (Storage Exhaustion)

- **Nguy cơ**: Gửi khối lượng lớn tin nhắn nhờ Relay lưu hộ nhằm chiếm hết dung lượng đĩa của node trung gian.
- **Biện pháp phòng thủ**:
  - Áp dụng hạn ngạch lưu trữ nghiêm ngặt: `relay_storage_limit_bytes` (mặc định 50 MB) và `max_messages` (1,000 tin).
  - Khi đầy bộ nhớ, tự động dọn dẹp các tin nhắn quá hạn; nếu vẫn đầy thì từ chối tin nhắn mới một cách an toàn mà không làm gián đoạn node.

---

## 6. Tấn công Giả mạo & Sửa đổi Dữ liệu (Tampering / MitM)

- **Nguy cơ**: Node trung gian trên đường truyền hoặc Relay cố tình sửa đổi ciphertext hoặc metadata.
- **Biện pháp phòng thủ**:
  - Sử dụng ChaCha20-Poly1305 AEAD với AAD ràng buộc toàn bộ metadata quan trọng.
  - Mọi hành vi sửa đổi dù chỉ 1 bit trong ciphertext, auth tag hay AAD đều khiến hàm giải mã báo lỗi `AuthenticationFailed` và hủy bỏ dữ liệu.
