# P2P Chat Core — Kiến Trúc Hệ Thống (Architecture)

## 1. Giới thiệu và Triết lý Thiết kế

**P2P Chat Core** là nhân ứng dụng trò chuyện phi tập trung (peer-to-peer) hoàn toàn viết bằng ngôn ngữ Rust, **không sử dụng bất kỳ thư viện bên thứ ba nào (zero external dependencies)**.

Dự án hoạt động theo nguyên tắc tương tự mạng Bitcoin:
- Không máy chủ trung tâm (no central servers).
- Không tài khoản quản trị (no admin keys, no master backdoor).
- Không blockchain, không coin hay token.
- Mã nguồn mở hoàn toàn, giao thức mở.
- Tự động duy trì và phục hồi khi các node rời mạng hoặc thay đổi IP.

---

## 2. Kiến trúc Đa tầng (Layered Architecture)

```text
+-----------------------------------------------------------+
|               TERMINAL APPLICATION (CLI REPL)             |
+-----------------------------------------------------------+
                             |
                             v
+-----------------------------------------------------------+
|                      NODE RUNTIME                         |
|   (Lifecycle, Configuration, Background Maintenance)     |
+-----------------------------------------------------------+
       |                           |                     |
       v                           v                     v
+---------------+        +-------------------+    +----------------+
| MESSAGE ROUTER|        | IDENTITY SYSTEM   |    | RELAY STORE    |
| - Deduplication        | - KeyPair (X25519)|    | - TTL Pruning  |
| - Envelope E2EE        | - Peer ID (SHA256)|    | - Quota Guard  |
| - Delivery State       | - Contact Book    |    | - Store-Forward|
+---------------+        +-------------------+    +----------------+
       |                           |                     |
       +---------------------------+---------------------+
                                   |
                                   v
+-----------------------------------------------------------+
|                    CRYPTO SUBSYSTEM                       |
|  SHA-256 | HMAC-SHA256 | HKDF-SHA256 | ChaCha20 | Poly1305 |
|             ChaCha20-Poly1305 AEAD | X25519              |
+-----------------------------------------------------------+
                                   |
                                   v
+-----------------------------------------------------------+
|                   P2P NETWORKING LAYER                    |
| - Peer Manager (Session registry, Handshake, Max Peers)   |
| - Peer Discovery & Address Book (Backoff Reconnect)       |
| - Binary Framer (P2CH magic, 64 KiB bounds check)         |
+-----------------------------------------------------------+
                                   |
                                   v
+-----------------------------------------------------------+
|                 TCP TRANSPORT / OPERATING SYSTEM          |
|              (std::net::TcpListener, TcpStream)           |
+-----------------------------------------------------------+
```

---

## 3. Các Thành phần Cốt lõi (Core Components)

### 3.1. Identity Subsystem (`src/identity/`)
- Quản lý danh tính mật mã không phụ thuộc bên thứ ba.
- Mỗi người dùng sở hữu cặp khóa X25519 được sinh ngẫu nhiên từ CSPRNG hệ điều hành.
- **Peer ID** được tính xác định từ `SHA256(PublicKey)`.
- Danh bạ (`ContactBook`) lưu tên cục bộ, public key và fingerprint xác thực ngoài luồng (out-of-band).

### 3.2. Cryptography Subsystem (`src/crypto/`)
- Viết thuần 100% bằng Rust Standard Library từ các đặc tả RFC chính thức:
  - **FIPS 180-4 / RFC 6234**: SHA-256.
  - **RFC 2104 / RFC 4231**: HMAC-SHA256.
  - **RFC 5869**: HKDF-SHA256 (Extract & Expand).
  - **RFC 8439**: ChaCha20 stream cipher & Poly1305 MAC & ChaCha20-Poly1305 AEAD.
  - **RFC 7748**: X25519 Montgomery ladder scalar multiplication trên Curve25519.
  - **CSPRNG**: Windows `RtlGenRandom` / Unix `/dev/urandom`.

### 3.3. Binary Protocol & Framing (`src/network/frame.rs`, `src/network/protocol.rs`)
- Magic bytes cố định: `b"P2CH"`.
- Phiên bản: `u16` (big-endian), MVP version = 1.
- Kích thước frame tối đa: 64 KiB.
- Bộ phân tích cú pháp xác thực giới hạn độ dài trước khi cấp phát bộ nhớ, ngăn chặn tấn công từ chối dịch vụ (DoS / memory exhaustion).

### 3.4. Peer Management & Discovery (`src/network/peer_manager.rs`, `src/network/discovery.rs`)
- Bắt tay 2 chiều (`HELLO` / `HELLO_ACK`) kiểm tra tính hợp lệ của Peer ID so với Public Key.
- Chặn kết nối trùng lặp với cùng một Peer ID.
- Giới hạn số lượng kết nối tối đa (`max_peers = 64`).
- Sổ địa chỉ (`PeerAddressBook`) với thuật toán giãn cách số mũ (exponential backoff: 1s -> 2s -> 4s -> ... -> 60s) để tự động thử lại khi mất kết nối.
- Cơ chế Peer Exchange: trao đổi danh sách các node đã biết qua `PEER_LIST_REQUEST` / `PEER_LIST_RESPONSE`.

### 3.5. Encrypted Messaging & Routing (`src/messaging/`)
- Tin nhắn được mã hóa đầu cuối vào `MessageEnvelope`.
- Ràng buộc siêu dữ liệu bất biến (Message ID, Sender ID, Recipient ID, Timestamp, TTL) vào **Authenticated Associated Data (AAD)**.
- `hops_remaining` (TTL số bước nhảy) không nằm trong AAD để các node chuyển tiếp có thể giảm trừ mà không làm sai lệch chữ ký toàn vẹn (auth tag).
- Bộ đệm chống lặp (`SeenMessageCache`) giới hạn dung lượng để ngăn chặn replay attack và lặp gói chuyển tiếp.

### 3.6. Relay & Offline Storage (`src/relay/`)
- Lưu trữ phong bì đã mã hóa cho người nhận đang ngoại tuyến.
- Node chuyển tiếp **chỉ lưu bản mã (ciphertext)**, không thể đọc nội dung bản rõ.
- Áp dụng hạn ngạch dung lượng (Storage Quota: mặc định 50 MB, tối đa 1000 tin nhắn) và tự động dọn dẹp tin nhắn hết hạn (TTL cleanup).
- Người nhận khi trực tuyến trở lại sẽ gửi `MESSAGE_FETCH_REQUEST` để nhận lại toàn bộ tin nhắn lưu hộ.

### 3.7. Local Persistence Engine (`src/storage/`)
- Lưu trữ cục bộ an toàn theo định dạng nhị phân có checksum Adler-32.
- Cơ chế ghi nguyên tử (Atomic write-to-tmp then rename) và `sync_all` bảo vệ dữ liệu khi mất điện hoặc tắt đột ngột.

### 3.8. Pure-Rust Ed25519 Signatures & Identity Proof (`src/crypto/ed25519.rs`, `src/crypto/sha512.rs`)
- Triển khai đầy đủ RFC 8032 (Ed25519) và FIPS 180-4 / RFC 6234 (SHA-512) thuần Rust.
- Đường cong Edwards xoắn: $-x^2 + y^2 = 1 + d x^2 y^2$ trên trường hữu hạn $\mathbb{F}_{2^{255}-19}$.
- **Identity Proof binding**: Do khóa mã hóa 1-on-1 sử dụng X25519 ($u$-coordinate), node tạo chứng chỉ ràng buộc mật mã `identity_proof = sign(x25519_pub)` ký bằng khóa Ed25519. Bên nhận xác thực chữ ký này trước khi tin cậy manifest truyền tệp hoặc cập nhật thành viên nhóm.

### 3.9. Token Bucket Rate Limiting & Backpressure (`src/network/rate_limiter.rs`)
- Cơ chế kiểm soát lưu lượng theo thuật toán Token Bucket độc lập cho từng địa chỉ socket từ xa (`SocketAddr`).
- Mỗi kết nối có dung lượng bucket (`capacity`) và tốc độ nạp thẻ liên tục (`refill_rate`).
- Chống DoS và flood gói tin ở mức kết nối TCP mà không gây nghẽn các node chân chính.

### 3.10. File & Multimedia Transfer Subsystem (`src/files/`)
- Chia nhỏ tệp thành các khối kích thước chuẩn 256 KiB (`CHUNK_SIZE = 262,144 bytes`).
- Mỗi khối được mã hóa độc lập bằng ChaCha20-Poly1305 với khóa đối xứng riêng của tệp và nonce suy biến từ chỉ số khối (`chunk_index`).
- Băm SHA-256 từng khối và băm toàn bộ tệp để đưa vào `FileManifestPayload`.
- Khóa tệp được bọc an toàn (key wrapping) bằng X25519 DH với khóa công khai của người nhận.
- Hỗ trợ tải tiếp khi gián đoạn (resume) thông qua tệp đệm `.part`.

### 3.11. Encrypted Group Chat Subsystem (`src/groups/`)
- Trò chuyện nhóm phân tán không cần máy chủ trung tâm.
- Quản lý vòng đời nhóm theo Epoch: Khi thêm (`add_member`) hoặc xóa (`remove_member`) thành viên, khóa đối xứng của nhóm được xoay (rotate) sang Epoch mới.
- Khóa Epoch mới được bọc 1-kèm-1 bằng X25519 và phân phối đến từng thành viên hợp lệ. Thành viên bị loại bỏ sẽ không nhận được khóa mới, đảm bảo tính bảo mật tiến về sau (Forward Secrecy).
- Tin nhắn nhóm được mã hóa bằng ChaCha20-Poly1305 với AAD liên kết `group_id || epoch || sender_peer_id`.
- Tự động chuyển tiếp (gossip relay) giữa các node trong mạng overlay để đảm bảo mọi thành viên đều nhận được tin nhắn.

### 3.12. Internal Event Bus & Notification Subsystem (`src/events/`)
- Bus sự kiện nội bộ theo mô hình Publisher-Subscriber tách rời hoàn toàn giữa tầng mạng, tầng định tuyến và giao diện người dùng.
- Phát các sự kiện thời gian thực: tin nhắn trực tiếp, biên nhận giao hàng, lời mời nhận tệp, tiến độ tải từng khối, tệp hoàn tất, tin nhắn nhóm, thay đổi thành viên và trạng thái kết nối mạng.
- Giao diện dòng lệnh REPL đăng ký lắng nghe và cập nhật màn hình ngay lập tức mà không làm gián đoạn việc nhập liệu của người dùng.
