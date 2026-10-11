# P2P Chat Core — Đặc Tả Giao Thức Nhị Phân (Binary Protocol Specification)

## 1. Cấu trúc Khung Truyền (Binary Frame Structure)

Mọi gói tin trao đổi qua kết nối TCP đều bắt đầu bằng tiêu đề cố định 12 byte:

```text
+------------------------+-----------------------------------------------+
| Offset | Độ dài        | Mô tả                                         |
+------------------------+-----------------------------------------------+
| 0..4   | 4 bytes       | Magic Bytes: b"P2CH" (0x50, 0x32, 0x43, 0x48) |
| 4..6   | 2 bytes (u16) | Protocol Version: 1 (Big-Endian)              |
| 6      | 1 byte  (u8)  | Message Type ID                               |
| 7      | 1 byte  (u8)  | Flags (mặc định 0x00)                         |
| 8..12  | 4 bytes (u32) | Payload Length (N bytes, Big-Endian)          |
| 12..N  | N bytes       | Payload dữ liệu                               |
+------------------------+-----------------------------------------------+
```

### Giới hạn Kỹ thuật:
- Byte order: **Big-Endian**.
- Kích thước payload tối đa (`MAX_FRAME_SIZE`): **524,288 bytes (512 KiB)** (đáp ứng khối tệp 256 KiB có mã hóa).
- Mọi gói tin có Magic khác `P2CH`, phiên bản không được hỗ trợ, hoặc độ dài > 512 KiB sẽ bị ngắt kết nối ngay lập tức mà không cấp phát vùng nhớ.

---

## 2. Bảng Định Danh Loại Thông Điệp (Message Types)

| ID | Tên Mã Lệnh | Ý Nghĩa / Mục Đích |
|---|---|---|
| `0x01` | `HELLO` | Khởi tạo bắt tay phiên kết nối mới |
| `0x02` | `HELLO_ACK` | Xác nhận và hoàn tất phiên bắt tay |
| `0x03` | `PEER_LIST_REQUEST` | Yêu cầu danh sách các node lân cận |
| `0x04` | `PEER_LIST_RESPONSE` | Trả lời danh sách địa chỉ IP:Port các node đã biết |
| `0x05` | `PING` | Nhịp tim kiểm tra trạng thái sống còn |
| `0x06` | `PONG` | Phản hồi nhịp tim |
| `0x10` | `MESSAGE` | Gói tin nhắn mã hóa đầu cuối (trực tiếp hoặc chuyển tiếp) |
| `0x11` | `MESSAGE_RECEIVED` | Xác nhận nhận gói tin nhắn |
| `0x20` | `RELAY_STORE` | Yêu cầu Node trung gian lưu hộ bản mã cho người nhận offline |
| `0x21` | `RELAY_STORED` | Node trung gian xác nhận đã lưu bản mã vào bộ nhớ |
| `0x22` | `MESSAGE_FETCH_REQUEST`| Người nhận truy vấn danh sách tin nhắn lưu hộ dành cho mình |
| `0x23` | `MESSAGE_FETCH_RESPONSE`| Trả lời các phong bì tin nhắn lưu hộ |
| `0x24` | `DELIVERY_RECEIPT` | Biên nhận đầu cuối xác nhận người nhận đã giải mã thành công |
| `0x30` | `FILE_MANIFEST` | Bản kê khai tệp (tên, kích thước, băm khối, chữ ký Ed25519) |
| `0x31` | `FILE_CHUNK_REQUEST` | Yêu cầu một khối tệp theo chỉ số |
| `0x32` | `FILE_CHUNK_RESPONSE` | Khối dữ liệu 256 KiB mã hóa ChaCha20-Poly1305 |
| `0x40` | `GROUP_KEY_DISTRIBUTION`| Phân phối khóa đối xứng nhóm đã bọc cho từng thành viên |
| `0x41` | `GROUP_MESSAGE` | Tin nhắn trò chuyện nhóm mã hóa AEAD theo Epoch |
| `0x42` | `GROUP_MEMBERSHIP_UPDATE`| Thông báo thêm/bớt thành viên có chữ ký Ed25519 của quản trị |
| `0xFE` | `ERROR` | Thông báo lỗi giao thức |
| `0xFF` | `GOODBYE` | Đóng kết nối an toàn có thông báo |

---

## 3. Cấu trúc Payload Chi tiết

### 3.1. `HELLO` (`0x01`)
- `version`: `u16` (2 bytes)
- `peer_id`: `[u8; 32]` (32 bytes)
- `public_key`: `[u8; 32]` (32 bytes)
- `listen_port`: `u16` (2 bytes)
- `features`: `u32` (4 bytes: Bit 0 = Relay, Bit 1 = Storage, Bit 2 = Discovery)
*Tổng độ dài cố định: 72 bytes.*
*Quy tắc nghiệm thu:* Bên nhận tính `SHA256(public_key)` và so sánh với `peer_id`. Nếu không khớp, từ chối kết nối ngay lập tức.

### 3.2. `HELLO_ACK` (`0x02`)
- `status`: `u8` (0 = Chấp nhận, 1 = Từ chối/Không tương thích)
- `peer_id`: `[u8; 32]` (32 bytes)
- `public_key`: `[u8; 32]` (32 bytes)
*Tổng độ dài cố định: 65 bytes.*

### 3.3. `PEER_LIST_RESPONSE` (`0x04`)
- `count`: `u16` (2 bytes)
- Danh sách lặp lại từng phần tử:
  - `peer_id`: `[u8; 32]` (32 bytes)
  - `ip_type`: `u8` (4 = IPv4, 6 = IPv6)
  - `ip_bytes`: 4 bytes (nếu IPv4) hoặc 16 bytes (nếu IPv6)
  - `port`: `u16` (2 bytes)

### 3.4. `MESSAGE` (`0x10`) & `RELAY_STORE` (`0x20`)
Payload chứa cấu trúc phong bì nhị phân `MessageEnvelope`:
```text
+----------------------------------------------------+
| Field              | Size (bytes) | Mô tả          |
+----------------------------------------------------+
| message_id         | 32           | Định danh ngẫu nhiên |
| sender_peer_id     | 32           | Peer ID người gửi |
| sender_public_key  | 32           | X25519 Public Key  |
| recipient_peer_id  | 32           | Peer ID người nhận|
| created_at         | 8 (u64 BE)   | Thời điểm tạo (Unix) |
| expires_at         | 8 (u64 BE)   | Thời điểm hết hạn (TTL) |
| hops_remaining     | 1 (u8)       | Số bước nhảy còn lại|
| salt               | 16           | Salt dẫn xuất khóa |
| nonce              | 12           | Nonce ChaCha20     |
| auth_tag           | 16           | Poly1305 Tag       |
| ciphertext_len     | 4 (u32 BE)   | Chiều dài bản mã   |
| ciphertext         | N bytes      | Dữ liệu mã hóa     |
+----------------------------------------------------+
```

### 3.5. `DELIVERY_RECEIPT` (`0x24`)
- `message_id`: `[u8; 32]` (32 bytes)
- `recipient_peer_id`: `[u8; 32]` (32 bytes)
- `timestamp`: `u64` (8 bytes)
*Tổng độ dài: 72 bytes.*

### 3.6. `FILE_MANIFEST` (`0x30`)
- `file_id`: `[u8; 32]` (32 bytes - SHA256 định danh tệp)
- `sender`: `[u8; 32]` (Peer ID người gửi)
- `recipient`: `[u8; 32]` (Peer ID người nhận)
- `sender_ed25519_pub`: `[u8; 32]` (Khóa công khai Ed25519)
- `identity_proof`: `[u8; 64]` (Chữ ký Ed25519 trên X25519 public key)
- `encrypted_file_key`: `48 bytes` (32B ephemeral pub + 32B wrapped key + 16B tag)
- `file_name`: String (2 bytes length prefix + UTF-8 bytes)
- `file_size`: `u64` (8 bytes)
- `chunk_count`: `u32` (4 bytes)
- `file_hash`: `[u8; 32]` (SHA-256 bản rõ toàn bộ tệp)
- `chunk_hashes`: Mảng `chunk_count` phần tử băm SHA-256 của từng khối mã hóa
- `signature`: `[u8; 64]` (Chữ ký Ed25519 của người gửi trên manifest)

### 3.7. `FILE_CHUNK_REQUEST` (`0x31`)
- `file_id`: `[u8; 32]` (32 bytes)
- `chunk_index`: `u32` (4 bytes)
- `requester`: `[u8; 32]` (Peer ID người yêu cầu)
*Tổng độ dài: 68 bytes.*

### 3.8. `FILE_CHUNK_RESPONSE` (`0x32`)
- `file_id`: `[u8; 32]` (32 bytes)
- `chunk_index`: `u32` (4 bytes)
- `chunk_hash`: `[u8; 32]` (32 bytes)
- `data_len`: `u32` (4 bytes)
- `data`: N bytes (bản mã khối 256 KiB + 16 bytes Poly1305 tag)

### 3.9. `GROUP_KEY_DISTRIBUTION` (`0x40`)
- `group_id`: `[u8; 32]` (32 bytes)
- `epoch`: `u64` (8 bytes)
- `group_name`: String (2 bytes length prefix + UTF-8 bytes)
- `creator`: `[u8; 32]` (Peer ID người tạo/quản trị nhóm)
- `recipient`: `[u8; 32]` (Peer ID thành viên nhận khóa)
- `key_len`: `u16` (2 bytes)
- `encrypted_key`: Bản mã khóa đối xứng nhóm bọc bằng X25519 DH

### 3.10. `GROUP_MESSAGE` (`0x41`)
- `group_id`: `[u8; 32]` (32 bytes)
- `epoch`: `u64` (8 bytes)
- `message_id`: `[u8; 32]` (32 bytes)
- `sender`: `[u8; 32]` (Peer ID người gửi)
- `timestamp`: `u64` (8 bytes)
- `nonce`: `[u8; 12]` (12 bytes)
- `ciphertext_len`: `u32` (4 bytes)
- `ciphertext`: N bytes (bản mã ChaCha20 + 16 bytes Poly1305 tag, AAD = `group_id || epoch || sender`)

### 3.11. `GROUP_MEMBERSHIP_UPDATE` (`0x42`)
- `group_id`: `[u8; 32]` (32 bytes)
- `epoch`: `u64` (8 bytes)
- `admin`: `[u8; 32]` (Peer ID người quản trị)
- `admin_ed25519_pub`: `[u8; 32]` (Khóa công khai Ed25519 của quản trị)
- `identity_proof`: `[u8; 64]` (Chứng chỉ ràng buộc Ed25519 và X25519)
- `action`: `u8` (1 = Thêm thành viên, 2 = Xóa thành viên)
- `member`: `[u8; 32]` (Peer ID của thành viên bị thêm/xóa)
- `signature`: `[u8; 64]` (Chữ ký Ed25519 của quản trị trên cập nhật thành viên)
