# P2P Chat Core

> **Nhân ứng dụng chat phi tập trung (P2P), mã hóa đầu cuối (E2EE), viết bằng Rust từ con số 0 với 0 THƯ VIỆN BÊN THỨ BA (Zero Dependencies).**

---

## 1. Triết lý Thiết kế

Dự án được xây dựng dựa trên nguyên tắc tương tự mạng Bitcoin:
- **Không có máy chủ trung tâm bắt buộc**: Không ai sở hữu mạng lưới.
- **Không có khóa quản trị (No Admin Keys / No Backdoor)**: Không một cá nhân hay tổ chức nào có thể tắt hoặc kiểm duyệt toàn mạng.
- **Không Blockchain / Token / Đào Coin**: Tin nhắn được trao đổi trực tiếp ngang hàng hoặc lưu hộ tạm thời, không tiêu tốn tài nguyên đào coin.
- **Người sáng lập có thể rời đi mà mạng vẫn tiếp tục vận hành**.
- **100% Rust Standard Library (`std`)**: Không phụ thuộc `tokio`, `serde`, `ring`, `rand`, `libp2p` hay bất kỳ crate nào trong `Cargo.toml`.

---

## 2. Tính năng Hoàn thành trong Phiên bản MVP

1. **Khởi tạo danh tính mật mã độc lập (F01, F08)**:
   - Mỗi Node sở hữu cặp khóa X25519 cho mã hóa và cặp khóa Ed25519 cho chữ ký số.
   - Peer ID = `SHA256(PublicKey)`.
   - **Identity Proof binding**: Chữ ký số Ed25519 liên kết chặt chẽ với khóa công khai X25519 của node, chống giả mạo danh tính.
2. **Giao thức nhị phân riêng (`P2CH`) (F02, F14)**:
   - Đóng gói nhị phân có tiêu đề 12 byte, giới hạn 512 KiB (đáp ứng truyền khối tệp 256 KiB có mã hóa AEAD).
   - Xác thực độ dài trước khi cấp phát bộ nhớ, chống tấn công cạn kiệt tài nguyên (DoS / memory exhaustion).
3. **Mạng P2P qua TCP & Kiểm soát lưu lượng (F03, F13)**:
   - Bắt tay 2 chiều (`HELLO`/`HELLO_ACK`), quản lý kết nối, ngăn chặn kết nối trùng lặp, giới hạn 64 peers.
   - **Token Bucket Rate Limiting**: Giới hạn tốc độ và tạo backpressure cho từng địa chỉ socket từ xa.
4. **Khám phá node (Peer Discovery) (F04)**:
   - Trao đổi danh sách node qua `PEER_LIST_REQUEST`/`RESPONSE`, sổ địa chỉ tự động kết nối lại (exponential backoff).
5. **Mật mã đầu cuối chuẩn quốc tế (E2EE) (F05, F08)**:
   - **X25519 (RFC 7748)**: Trao đổi khóa Diffie-Hellman trên Curve25519.
   - **Ed25519 (RFC 8032)**: Chữ ký số đường cong Edwards xoắn trên $\mathbb{F}_{2^{255}-19}$.
   - **HKDF-SHA256 (RFC 5869)**: Dẫn xuất khóa riêng cho từng tin nhắn.
   - **ChaCha20-Poly1305 (RFC 8439)**: Mã hóa xác thực (AEAD) với dữ liệu liên kết (AAD).
   - **SHA-256 (FIPS 180-4) & SHA-512 (RFC 6234) & HMAC (RFC 2104)**: Băm và xác thực dữ liệu.
   - **CSPRNG**: Đọc nguồn ngẫu nhiên từ hệ điều hành (`RtlGenRandom` trên Windows / `/dev/urandom` trên Unix).
6. **Chuyển tiếp và Lưu hộ tin nhắn ngoại tuyến (Store-and-Forward Relay) (F06)**:
   - Node trung gian lưu trữ phong bì đã mã hóa (chỉ lưu ciphertext, không thể đọc nội dung).
   - Người nhận offline khi online trở lại sẽ tự động kéo tin nhắn về giải mã.
   - Hạn ngạch lưu trữ (Storage Quota: mặc định 50 MB, 1000 tin) và tự động dọn dẹp tin nhắn hết hạn (TTL).
7. **Truyền tệp & truyền thông đa phương tiện mã hóa (F14, F15, F16)**:
   - Chia nhỏ tệp thành các khối chuẩn 256 KiB, mã hóa ChaCha20-Poly1305 độc lập từng khối.
   - Bản kê khai tệp (`FileManifest`) mang chữ ký Ed25519, băm SHA-256 từng khối và băm toàn tệp.
   - Khóa đối xứng của tệp được bọc bảo vệ (key wrapping) bằng X25519 DH.
   - Hỗ trợ tải tiếp khi mất mạng (resume) qua tệp đệm `.part`.
8. **Trò chuyện nhóm mã hóa phân tán (Group Chat) (F17)**:
   - Quản lý nhóm theo Epoch, tự động xoay khóa đối xứng khi thêm/xóa thành viên (Forward Secrecy).
   - Phân phối khóa Epoch 1-1 qua X25519 cho mọi thành viên hợp lệ.
   - Mã hóa tin nhắn nhóm bằng ChaCha20-Poly1305 và cơ chế gossip broadcast trong mạng overlay.
9. **Event Bus & Hệ thống thông báo nội bộ (F18, F19)**:
   - Bus sự kiện Publisher-Subscriber phi tập trung, phát thông báo sự kiện thời gian thực ra giao diện CLI.
10. **Lưu trữ dữ liệu cục bộ bền vững (F07)**:
   - Định dạng bản ghi nhị phân có checksum Adler-32.
   - Ghi tệp nguyên tử (ghi file tạm `.tmp`, `sync_all`, rồi đổi tên) chống hỏng file khi sập nguồn.
11. **Giao diện dòng lệnh trực quan (Terminal REPL) (F09, F10)**:
   - Đầy đủ các nhóm lệnh tin nhắn cá nhân, chuyển tệp, trò chuyện nhóm, trạng thái mạng, relay và chẩn đoán.

---

## 3. Cấu trúc Dự án

```text
p2p-chat-core/
├── Cargo.toml                  # 0 dependencies! (100% Rust std)
├── LICENSE                     # MIT License
├── README.md                   # Hướng dẫn tổng quan
├── docs/
│   ├── ARCHITECTURE.md         # Kiến trúc hệ thống đa tầng
│   ├── PROTOCOL.md             # Đặc tả giao thức nhị phân và byte-layout
│   ├── SECURITY.md             # Đặc tả mật mã và giới hạn bảo mật
│   ├── THREAT_MODEL.md         # Phân tích mô hình đe dọa và cách phòng vệ
│   ├── STORAGE.md              # Kiến trúc lưu trữ nhị phân và ghi an toàn
│   ├── TESTING.md              # Bảng test vector RFC và kiểm thử
│   └── ROADMAP.md              # Lộ trình phát triển tính năng tương lai
├── src/
│   ├── lib.rs                  # Thư viện Core API
│   ├── main.rs                 # Điểm khởi chạy CLI
│   ├── config.rs               # Cấu hình NodeConfig
│   ├── error.rs                # Hệ thống Error types
│   ├── types.rs                # Tiện ích Hex, Timestamp, Checksum
│   ├── events/                 # Event Bus & Realtime Events
│   │   └── mod.rs              # EventBus pub/sub & NodeEvent
│   ├── crypto/                 # Mật mã thuần Rust chuẩn RFC
│   │   ├── constant_time.rs    # So sánh thời gian không đổi ct_eq
│   │   ├── random.rs           # OS CSPRNG
│   │   ├── sha256.rs           # SHA-256 (FIPS 180-4)
│   │   ├── sha512.rs           # SHA-512 (FIPS 180-4 / RFC 6234)
│   │   ├── ed25519.rs          # Ed25519 Chữ ký số (RFC 8032)
│   │   ├── hmac.rs             # HMAC-SHA256 (RFC 2104)
│   │   ├── hkdf.rs             # HKDF-SHA256 (RFC 5869)
│   │   ├── chacha20.rs         # ChaCha20 (RFC 8439)
│   │   ├── poly1305.rs         # Poly1305 (RFC 8439)
│   │   ├── aead.rs             # ChaCha20-Poly1305 AEAD (RFC 8439)
│   │   └── x25519.rs           # Curve25519 Montgomery Ladder (RFC 7748)
│   ├── identity/               # Quản lý danh tính
│   │   ├── keypair.rs          # X25519 & Ed25519 KeyPair, Identity Proof
│   │   ├── peer_id.rs          # Peer ID = SHA256(PublicKey)
│   │   └── contacts.rs         # Sổ danh bạ và Fingerprint
│   ├── network/                # Mạng P2P
│   │   ├── frame.rs            # Khung truyền nhị phân P2CH (512 KiB)
│   │   ├── protocol.rs         # Mã lệnh và payload giao thức
│   │   ├── rate_limiter.rs     # Token Bucket rate limiter per socket
│   │   ├── connection.rs       # Socket TCP wrapper
│   │   ├── discovery.rs        # Sổ địa chỉ và Reconnect scheduler
│   │   └── peer_manager.rs     # Quản lý phiên kết nối và bắt tay
│   ├── messaging/              # Mã hóa tin nhắn
│   │   ├── envelope.rs         # MessageEnvelope E2EE & AAD
│   │   ├── message.rs          # Mô hình tin nhắn & trạng thái
│   │   ├── dedup.rs            # Bộ đệm chống phát lại SeenMessageCache
│   │   └── router.rs           # Định tuyến và phân phối tin nhắn
│   ├── files/                  # Truyền tệp đa phương tiện
│   │   ├── chunk.rs            # Phân đoạn 256 KiB & mã hóa AEAD khối
│   │   ├── manifest.rs         # FileManifest ký Ed25519 & bọc khóa X25519
│   │   └── transfer_manager.rs # FileTransferManager điều phối luồng tệp & resume
│   ├── groups/                 # Trò chuyện nhóm mã hóa
│   │   ├── group.rs            # Mô hình nhóm, Epoch xoay khóa
│   │   └── group_manager.rs    # Phân phối khóa đối xứng & xác thực thành viên
│   ├── relay/                  # Lưu hộ tin nhắn
│   │   └── store.rs            # RelayStore quản lý quota và TTL
│   ├── storage/                # Lưu trữ đĩa cứng
│   │   ├── record.rs           # Bản ghi nhị phân có checksum Adler-32
│   │   └── engine.rs           # Ghi tệp nguyên tử (Atomic writer)
│   ├── node/                   # Nhân Runtime
│   │   └── runtime.rs          # Vòng đời Node và luồng ngầm
│   └── cli/                    # Ứng dụng dòng lệnh
│       ├── commands.rs         # Phân tích tham số CLI
│       └── terminal.rs         # Interactive REPL shell có Event Bus
└── tests/
    └── integration_tests.rs    # Kiểm thử tích hợp mạng đa node qua TCP loopback
```

---

## 4. Hướng dẫn Biên dịch và Chạy Kiểm thử

### Yêu cầu:
- Đã cài đặt Rust (Rust 1.80+ stable).
- Không cần kết nối Internet để tải package (Zero dependencies).

```bash
# 1. Biên dịch phiên bản debug
cargo build

# 2. Biên dịch bản release tối ưu
cargo build --release

# 3. Chạy toàn bộ 35 unit tests và 6 integration tests
cargo test

# 4. Kiểm tra quy chuẩn clippy (zero warnings)
cargo clippy --all-targets -- -D warnings

# 5. Kiểm tra quy chuẩn định dạng mã nguồn
cargo fmt --check
```

---

## 5. Hướng dẫn Trình diễn Thực tế 3 Node (Demo Kịch Bản)

Mở 3 cửa sổ Terminal riêng biệt:

### Terminal 1 — Alice
```bash
# Bước 1: Khởi tạo danh tính Alice
cargo run -- init --data ./alice

# Bước 2: Chạy Node Alice lắng nghe ở cổng 7001
cargo run -- run --data ./alice --listen 127.0.0.1:7001
```

### Terminal 2 — Relay (Node Lưu Hộ)
```bash
# Bước 1: Khởi tạo danh tính Relay
cargo run -- init --data ./relay

# Bước 2: Chạy Node Relay ở cổng 7002, kết nối đến Alice và bật cờ --relay
cargo run -- run --data ./relay --listen 127.0.0.1:7002 --peer 127.0.0.1:7001 --relay
```

### Terminal 3 — Bob
```bash
# Bước 1: Khởi tạo danh tính Bob
cargo run -- init --data ./bob

# Bước 2: Chạy Node Bob ở cổng 7003 và kết nối đến Relay
cargo run -- run --data ./bob --listen 127.0.0.1:7003 --peer 127.0.0.1:7002
```

---

### Kịch bản Thao tác Kiểm Thử:

1. **Kiểm tra kết nối 3 Node**:
   - Tại Terminal của Alice, gõ: `/peers` (Thấy Relay đã kết nối).
   - Tại Terminal của Relay, gõ: `/peers` (Thấy cả Alice và Bob đã kết nối).
2. **Trao đổi Public Key và Thêm Danh Bạ**:
   - Tại Terminal của Alice, gõ `/id` để lấy Public Key của Alice.
   - Tại Terminal của Bob, gõ `/id` để lấy Public Key của Bob.
   - Alice thêm Bob vào danh bạ:
     ```text
     /add Bob <PUBLIC_KEY_CUA_BOB>
     ```
   - Bob thêm Alice vào danh bạ:
     ```text
     /add Alice <PUBLIC_KEY_CUA_ALICE>
     ```
3. **Alice gửi tin nhắn trực tiếp cho Bob**:
   - Tại Terminal của Alice:
     ```text
     /send Bob Xin chao Bob!
     ```
   - Tại Terminal của Bob, gõ:
     ```text
     /inbox
     ```
     -> Bob đọc được tin nhắn đã giải mã: `From Alice: Xin chao Bob!`.
4. **Kiểm thử Người Nhận Offline (Offline Store-and-Forward)**:
   - **Tắt Bob**: Tại Terminal của Bob, gõ `/quit`.
   - **Alice gửi tin nhắn mới cho Bob**:
     ```text
     /send Bob Tin nhan gui khi Bob offline!
     ```
     -> Hệ thống thông báo: `Status: Stored by 1 relay node(s)`.
   - **Kiểm tra tại Relay**:
     Tại Terminal của Relay, gõ: `/relay-status`
     -> Thấy `Stored Count: 1 envelopes`. (Relay **chỉ lưu bản mã**, không đọc được nội dung).
   - **Tắt Alice**: Tại Terminal của Alice, gõ `/quit`. (Lúc này cả Alice và Bob đều đã tắt).
   - **Bob trực tuyến trở lại**:
     Tại Terminal của Bob, khởi động lại:
     ```bash
     cargo run -- run --data ./bob --listen 127.0.0.1:7003 --peer 127.0.0.1:7002
     ```
   - **Bob kiểm tra hộp thư**:
     ```text
     /inbox
     ```
     -> Bob tự động nhận lại tin nhắn từ Relay và giải mã thành công: `From Alice: Tin nhan gui khi Bob offline!`.

---

## 6. Giấy phép (License)

Dự án phát hành dưới giấy phép mã nguồn mở **MIT License**. Mọi cá nhân và tổ chức đều có quyền sử dụng, sửa đổi và phân phối độc lập.
