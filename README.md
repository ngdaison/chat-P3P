# P2P Chat Core (`chat-P3P`)

> **Nhân ứng dụng trò chuyện phi tập trung (P2P), mã hóa đầu cuối (E2EE), viết bằng Rust từ con số 0 với 100% Rust Standard Library (`std`) — Tuyệt đối KHÔNG sử dụng thư viện bên thứ ba (Zero External Dependencies).**

[![Rust](https://img.shields.io/badge/rust-edition%202024-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Dependencies](https://img.shields.io/badge/dependencies-0%20external-brightgreen.svg)](Cargo.toml)
[![Tests](https://img.shields.io/badge/tests-57%20passed-success.svg)](tests/)

---

## 1. Triết lý Thiết kế

Dự án được xây dựng và thiết kế theo các nguyên tắc cốt lõi tương tự mạng Bitcoin:

- **Không có máy chủ trung tâm bắt buộc**: Người dùng tự vận hành node, kết nối trực tiếp ngang hàng qua TCP.
- **Không có khóa quản trị (No Admin Keys / No Backdoor)**: Không bất kỳ cá nhân hay tổ chức nào có thể can thiệp, tắt, kiểm duyệt hoặc giải mã tin nhắn trên toàn mạng.
- **Không Blockchain / Coin / Token / Đào Coin**: Hệ thống chỉ tập trung vào truyền thông tin và dữ liệu mã hóa phân tán, không có sổ cái nặng nề, không tốn tài nguyên đào coin.
- **Sống sót độc lập**: Người sáng lập có thể rời đi mà mạng lưới vẫn tiếp tục tồn tại và vận hành bền bỉ.
- **100% Rust Standard Library (`std`)**: Không phụ thuộc `tokio`, `serde`, `ring`, `rand`, `libp2p` hay bất kỳ crate nào trong `Cargo.toml`. Toàn bộ thuật toán mã hóa (X25519, Ed25519, ChaCha20-Poly1305, SHA-256, SHA-512, HKDF, HMAC) và giao thức mạng đều được tự lập trình chuẩn theo các đặc tả RFC quốc tế.

---

## 2. Cấu trúc Dự án

Toàn bộ mã nguồn nhân Core của hệ thống được tổ chức hoàn chỉnh tại thư mục `/CCA`:

```text
chat-P3P/
├── Cargo.toml                  # Root Cargo Workspace điều phối /CCA
├── .gitignore                  # Bỏ qua tệp nhị phân và dữ liệu tạm
├── LICENSE                     # Giấy phép mã nguồn mở MIT
├── README.md                   # Tài liệu hướng dẫn chính của dự án
└── CCA/                        # THƯ MỤC CHỨA TOÀN BỘ NHÂN P2P CORE
    ├── Cargo.toml              # 0 dependencies! (100% Rust Standard Library)
    ├── Cargo.lock              # Lockfile nội bộ
    ├── README.md               # Tài liệu chi tiết nội bộ Core
    ├── docs/                   # Tài liệu kỹ thuật chuyên sâu
    │   ├── ARCHITECTURE.md     # Kiến trúc hệ thống đa tầng
    │   ├── PROTOCOL.md         # Đặc tả giao thức nhị phân P2CH
    │   ├── SECURITY.md         # Phân tích an toàn mật mã & giới hạn
    │   ├── THREAT_MODEL.md     # Mô hình đe dọa và giải pháp phòng vệ
    │   ├── STORAGE.md          # Định dạng lưu trữ nhị phân có Adler-32
    │   ├── TESTING.md          # Bảng RFC Test Vectors
    │   └── ROADMAP.md          # Lộ trình phát triển mở rộng
    ├── src/
    │   ├── lib.rs              # Export toàn bộ Core API
    │   ├── main.rs             # Điểm khởi chạy ứng dụng CLI
    │   ├── config.rs           # Cấu hình node và tham số vận hành
    │   ├── error.rs            # Hệ thống phân loại lỗi nội bộ
    │   ├── types.rs            # Hằng số giao thức, Adler-32, Hex utils
    │   ├── events/             # Hệ thống Event Bus nội bộ
    │   │   └── mod.rs          # EventBus Pub/Sub & định nghĩa NodeEvent
    │   ├── crypto/             # Các thuật toán mật mã tự lập trình chuẩn RFC
    │   │   ├── constant_time.rs# So sánh mảng byte thời gian không đổi (ct_eq)
    │   │   ├── random.rs       # CSPRNG hệ điều hành (RtlGenRandom / /dev/urandom)
    │   │   ├── sha256.rs       # SHA-256 (FIPS 180-4)
    │   │   ├── sha512.rs       # SHA-512 (FIPS 180-4 / RFC 6234)
    │   │   ├── ed25519.rs      # Chữ ký số Ed25519 đường cong Edwards xoắn (RFC 8032)
    │   │   ├── hmac.rs         # HMAC-SHA256 (RFC 2104 / RFC 4231)
    │   │   ├── hkdf.rs         # HKDF-SHA256 Extract & Expand (RFC 5869)
    │   │   ├── chacha20.rs     # ChaCha20 Stream Cipher (RFC 8439)
    │   │   ├── poly1305.rs     # Poly1305 One-Time Authenticator (RFC 8439)
    │   │   ├── aead.rs         # ChaCha20-Poly1305 AEAD có AAD (RFC 8439)
    │   │   └── x25519.rs       # Curve25519 Montgomery Ladder DH (RFC 7748)
    │   ├── identity/           # Hệ thống danh tính mật mã
    │   │   ├── keypair.rs      # X25519 & Ed25519 KeyPair, Identity Proof binding
    │   │   ├── peer_id.rs      # Peer ID = SHA256(PublicKey)
    │   │   └── contacts.rs     # Danh bạ bạn bè, quản lý khóa và Fingerprint
    │   ├── network/            # Tầng mạng P2P & Giao thức nhị phân
    │   │   ├── frame.rs        # Đóng gói khung nhị phân P2CH (tối đa 512 KiB)
    │   │   ├── protocol.rs     # Mã lệnh và payload nhị phân (0x01..0x42)
    │   │   ├── rate_limiter.rs # Token Bucket rate limiting per remote socket
    │   │   ├── connection.rs   # Wrapper quản lý luồng socket TCP
    │   │   ├── discovery.rs    # Sổ địa chỉ và lịch trình tái kết nối backoff
    │   │   └── peer_manager.rs # Quản lý phiên kết nối, bắt tay 2 chiều
    │   ├── messaging/          # Tin nhắn mã hóa đầu cuối E2EE
    │   │   ├── envelope.rs     # MessageEnvelope có AAD chống giả mạo
    │   │   ├── message.rs      # Mô hình tin nhắn & trạng thái chuyển phát
    │   │   ├── dedup.rs        # Bộ đệm chống phát lại SeenMessageCache
    │   │   └── router.rs       # Định tuyến và phân phối tin nhắn
    │   ├── files/              # Hệ thống truyền tệp đa phương tiện
    │   │   ├── chunk.rs        # Phân đoạn 256 KiB & mã hóa AEAD từng khối
    │   │   ├── manifest.rs     # FileManifest ký số Ed25519 & bọc khóa X25519
    │   │   └── transfer_manager.rs # Quản lý gửi/nhận tệp, hỗ trợ resume (.part)
    │   ├── groups/             # Trò chuyện nhóm phân tán mã hóa
    │   │   ├── group.rs        # Mô hình nhóm, Epoch xoay khóa (Forward Secrecy)
    │   │   └── group_manager.rs# Phân phối khóa 1-1 & xác thực thành viên
    │   ├── relay/              # Lưu hộ tin nhắn ngoại tuyến
    │   │   └── store.rs        # RelayStore quản lý hạn ngạch dung lượng và TTL
    │   ├── storage/            # Lưu trữ đĩa cứng bền vững
    │   │   ├── record.rs       # Bản ghi nhị phân có checksum Adler-32
    │   │   └── engine.rs       # Ghi tệp nguyên tử (Atomic write & sync)
    │   ├── node/               # Runtime điều phối
    │   │   └── runtime.rs      # Vòng đời Node và luồng ngầm bảo trì
    │   └── cli/                # Giao diện dòng lệnh
    │       ├── commands.rs     # Bộ phân tích tham số CLI
    │       └── terminal.rs     # Interactive REPL shell có Event Bus thời gian thực
    └── tests/
        └── integration_tests.rs# 8 bài kiểm thử tích hợp mạng đa node qua TCP loopback
```

---

## 3. Các Tính năng Nổi bật

### 3.1. Danh tính & Mật mã Tự do (Sovereign Identity)
- Cặp khóa X25519 sinh ngẫu nhiên từ CSPRNG hệ điều hành.
- **Peer ID** định danh duy nhất xác định từ `SHA256(PublicKey)`.
- **Identity Proof (F08)**: Tự động ký xác thực ràng buộc giữa khóa mã hóa X25519 và khóa chữ ký số Ed25519, ngăn chặn kẻ trung gian mạo danh danh tính.

### 3.2. Truyền Tệp & Đa phương tiện Phân đoạn (F14, F15, F16)
- Tệp tin được chia nhỏ thành các khối kích thước chuẩn 256 KiB (`CHUNK_SIZE = 262,144 bytes`).
- Mỗi khối được mã hóa bằng khóa tệp đối xứng riêng biệt qua ChaCha20-Poly1305 với nonce suy biến từ chỉ số khối.
- Bản kê khai `FileManifest` chứa mã băm SHA-256 từng khối, mã băm toàn tệp, chữ ký Ed25519 và khóa tệp đã được bọc bằng X25519 DH.
- Hỗ trợ tải tiếp khi mất kết nối qua tệp đệm `.part`.

### 3.3. Trò chuyện Nhóm Mã hóa Động (Encrypted Group Chat - F17)
- Hoàn toàn phi tập trung, không cần server trung gian.
- **Epoch Key Rotation**: Khóa đối xứng của nhóm tự động xoay khi có thành viên gia nhập hoặc rời nhóm, bảo đảm tính bí mật tiến về sau (**Forward Secrecy**).
- Phân phối khóa Epoch an toàn qua kênh mã hóa 1-1 cho mọi thành viên hợp lệ.
- Cơ chế chuyển tiếp gossip trong mạng overlay bảo đảm tin nhắn nhóm đến tay tất cả thành viên kết nối.

### 3.4. Kiểm soát Lưu lượng (Token Bucket Rate Limiting - F13)
- Thuật toán Token Bucket độc lập cho từng địa chỉ IP/socket từ xa.
- Giới hạn tốc độ và tạo backpressure tự nhiên, bảo vệ node khỏi các cuộc tấn công DoS hoặc flood gói tin.

### 3.5. Lưu hộ Tin nhắn Ngoại tuyến (Store-and-Forward Relay - F06)
- Node trung gian lưu trữ phong bì đã mã hóa cho bạn bè đang offline.
- Node relay **chỉ lưu bản mã (ciphertext)**, không có khóa giải mã.
- Tự động kiểm soát hạn ngạch lưu trữ (Storage Quota: mặc định 50 MB, 1000 tin) và thời gian hết hạn (TTL).
- Khi người nhận online trở lại, tin nhắn được tự động truy vấn và kéo về giải mã.

---

## 4. Hướng dẫn Biên dịch & Chạy Kiểm thử

### 4.1. Yêu cầu Hệ thống
- Đã cài đặt **Rust** và công cụ **Cargo** (khuyến nghị phiên bản 1.80 trở lên).
- Không cần kết nối Internet để tải crate (vì `Cargo.toml` có 0 external dependencies).

### 4.2. Biên dịch Dự án
```bash
# Biên dịch phiên bản debug
cargo build

# Biên dịch phiên bản release tối ưu hóa cao
cargo build --release
```
*Tệp nhị phân độc lập chỉ nặng ~670 KB, không phụ thuộc DLL/thư viện bên ngoài.*

### 4.3. Chạy Toàn bộ Kiểm thử (57 Tests)
```bash
# Chạy tất cả unit tests và integration tests
cargo test

# Kiểm tra quy chuẩn mã nguồn với Clippy
cargo clippy --all-targets -- -D warnings

# Kiểm tra định dạng code chuẩn
cargo fmt --check
```

---

## 5. Hướng dẫn Sử dụng Giao diện Dòng lệnh (CLI REPL)

### 5.1. Bắt đầu Node Mới
```bash
# Khởi tạo thư mục dữ liệu và cặp khóa danh tính mới
cargo run -- init --data ./my_node

# Chạy node lắng nghe trên cổng 7001
cargo run -- run --data ./my_node --listen 127.0.0.1:7001
```

### 5.2. Danh mục Lệnh trong Interactive Terminal

| Lệnh | Mô tả Chức năng |
|---|---|
| `/id` | Hiển thị Peer ID và Public Key của node |
| `/status` | Xem trạng thái kết nối, địa chỉ nghe và relay |
| `/peers` | Liệt kê các node đang kết nối trực tiếp |
| `/connect <IP:PORT>` | Kết nối chủ động tới một node khác |
| `/routes` | Xem bảng định tuyến và danh sách địa chỉ đã biết |
| `/network-stats` | Xem thống kê lưu lượng, số peer, rate limiting |
| `/debug on\|off` | Bật/tắt chế độ ghi nhật ký chẩn đoán |
| `/add <TÊN> <PUBKEY_HEX>` | Thêm bạn bè vào danh bạ từ khóa công khai hex |
| `/contacts` | Liệt kê danh bạ và Fingerprint xác thực ngoài luồng |
| `/send <TÊN> <TIN_NHẮN>` | Mã hóa E2EE và gửi tin nhắn trực tiếp |
| `/inbox` | Xem các tin nhắn đã nhận và giải mã thành công |
| `/outbox` | Xem lịch sử tin nhắn đã gửi và trạng thái chuyển phát |
| `/file-send <TÊN> <ĐƯỜNG_DẪN>` | Gửi tệp mã hóa phân đoạn 256 KiB |
| `/file-status` | Xem danh sách và tiến độ các tệp đang gửi/nhận |
| `/file-cancel <FILE_ID_HEX>` | Hủy phiên truyền tệp |
| `/group-create <TÊN_NHÓM>` | Tạo nhóm trò chuyện mã hóa mới |
| `/group-list` | Xem danh sách các nhóm đã tham gia và Epoch hiện tại |
| `/group-add <ID_NHÓM> <TÊN>` | Thêm bạn vào nhóm và tự động xoay khóa Epoch |
| `/group-remove <ID_NHÓM> <TÊN>`| Xóa thành viên khỏi nhóm và xoay khóa bảo mật |
| `/group-send <ID_NHÓM> <TIN>` | Gửi tin nhắn mã hóa tới toàn bộ thành viên nhóm |
| `/relay on\|off` | Bật hoặc tắt chức năng lưu hộ tin nhắn cho mạng |
| `/relay-status` | Xem dung lượng và số lượng tin nhắn đang lưu hộ |
| `/storage` | Xem chi tiết dung lượng lưu trữ trên đĩa cứng |
| `/quit` | Đóng kết nối an toàn, lưu trạng thái và thoát |

---

## 6. Kịch bản Thử nghiệm Mẫu 3 Node (Alice - Relay - Bob)

Mở 3 cửa sổ dòng lệnh riêng biệt:

### Cửa sổ 1 — Alice (Port 7001)
```bash
cargo run -- init --data ./alice
cargo run -- run --data ./alice --listen 127.0.0.1:7001
```

### Cửa sổ 2 — Relay Node (Port 7002, kết nối Alice)
```bash
cargo run -- init --data ./relay
cargo run -- run --data ./relay --listen 127.0.0.1:7002 --peer 127.0.0.1:7001 --relay
```

### Cửa sổ 3 — Bob (Port 7003, kết nối Relay)
```bash
cargo run -- init --data ./bob
cargo run -- run --data ./bob --listen 127.0.0.1:7003 --peer 127.0.0.1:7002
```

### Các bước thao tác:
1. **Kết bạn**: Gõ `/id` tại Alice và Bob để lấy Public Key. Gõ `/add Bob <PUBKEY_BOB>` ở Alice, và `/add Alice <PUBKEY_ALICE>` ở Bob.
2. **Nhắn tin trực tiếp**: Alice gõ `/send Bob Xin chao Bob!`. Bob sẽ nhận thông báo thời gian thực từ Event Bus và có thể xem lại tại `/inbox`.
3. **Thử nghiệm Offline Relay**:
   - Bob gõ `/quit` để tắt node.
   - Alice gửi tin: `/send Bob Tin nhan luu ho khi Bob offline!`. Alice nhận thông báo `Stored by 1 relay node(s)`.
   - Relay gõ `/relay-status` thấy `Stored Count: 1 envelopes` (Relay chỉ giữ bản mã).
   - Alice tắt node: gõ `/quit`.
   - Bob khởi động lại: `cargo run -- run --data ./bob --listen 127.0.0.1:7003 --peer 127.0.0.1:7002`.
   - Bob gõ `/inbox`: Tin nhắn từ Relay tự động kéo về và giải mã thành công!
4. **Thử nghiệm Truyền tệp**:
   - Alice gõ: `/file-send Bob ./tai_lieu.pdf`.
   - Bob nhận thông báo đề nghị nhận tệp, tự động kéo từng khối 256 KiB, đối chiếu băm SHA-256 và lưu vào thư mục `downloads/`.
5. **Thử nghiệm Chat Nhóm**:
   - Alice gõ: `/group-create NhomDev`.
   - Alice thêm Bob: `/group-add <ID_NHÓM> Bob`.
   - Alice gửi tin nhóm: `/group-send <ID_NHÓM> Chao cac ban trong nhom!`.
   - Bob lập tức nhận và giải mã tin nhắn nhóm.

---

## 7. Giấy phép (License)

Dự án được phân phối dưới giấy phép mã nguồn mở **MIT License**. Mọi cá nhân, nhóm và tổ chức đều có toàn quyền nghiên cứu, sử dụng, phát triển và tích hợp độc lập.
