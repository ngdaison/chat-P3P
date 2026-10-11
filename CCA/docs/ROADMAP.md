# P2P Chat Core — Kế Hoạch Phát Triển (Future Roadmap)

Tài liệu này vạch ra lộ trình phát triển kỹ thuật sau khi phiên bản MVP Cốt Lõi (P2P Chat Core MVP) đã hoàn thành và được kiểm thử thành công.

---

## Giai đoạn 1: MVP Cốt Lõi (Đã Hoàn Thành 100%)

- [x] Nhân Rust độc lập, **0 dependency bên ngoài** (chỉ dùng Rust `std`).
- [x] Triển khai mật mã chuẩn RFC: SHA-256, HMAC-SHA256, HKDF, ChaCha20, Poly1305, AEAD, Curve25519/X25519.
- [x] Giao thức nhị phân riêng có magic `P2CH`, phân tích cú pháp an toàn chống tràn bộ nhớ.
- [x] Quản lý danh tính phi tập trung dựa trên cặp khóa, Peer ID = `SHA256(PublicKey)`.
- [x] Kết nối P2P đa node qua TCP, bắt tay hai chiều, kiểm tra tính hợp lệ của khóa.
- [x] Khám phá mạng cơ bản qua Peer Exchange và sổ địa chỉ tự động kết nối lại (exponential backoff).
- [x] Tin nhắn mã hóa đầu cuối (E2EE) với AAD bảo vệ tính toàn vẹn metadata.
- [x] Chống phát lại (Replay protection) bằng bộ đệm `SeenMessageCache`.
- [x] Cơ chế chuyển tiếp và lưu hộ tin nhắn ngoại tuyến (Store-and-Forward Relay) cho phép người nhận offline nhận lại tin nhắn sau đó.
- [x] Động cơ lưu trữ nhị phân cục bộ có checksum Adler-32 và ghi tệp nguyên tử (crash-resilient).
- [x] Ứng dụng dòng lệnh Terminal REPL trực quan với các lệnh `/id`, `/status`, `/connect`, `/add`, `/send`, `/inbox`, `/relay`.
- [x] Bộ kiểm thử toàn diện: 35 Unit tests + 6 Integration tests qua mạng TCP thật.

---

## Giai đoạn 2: Nâng Cấp Mật Mã & Quyền Riêng Tư (Post-MVP Cryptography)

1. **Khóa Phiên Tạm Thời (Ephemeral Keys & Prekeys)**:
   - Triển khai Extended Triple Diffie-Hellman (X3DH) bằng prekey được công bố sẵn trên mạng relay.
   - Giúp tạo bí mật phiên mới cho mỗi cuộc hội thoại kể cả khi người nhận offline.
2. **Double Ratchet Algorithm**:
   - Triển khai thuật toán Double Ratchet để cung cấp **Forward Secrecy** (bảo vệ tin cũ khi mất khóa dài hạn) và **Break-in Recovery** (tự phục hồi tính bảo mật sau khi khóa bị lộ).
3. **Kiểm toán Mật mã Độc lập (Independent Cryptographic Audit)**:
   - Thuê chuyên gia đánh giá khả năng chống tấn công kênh phụ (timing side-channel) và kiểm định mã nguồn.

---

## Giai đoạn 3: Vượt Tường Lửa & Khám Phá Mạng Nâng Cao (NAT & DHT)

1. **NAT Traversal & UDP Hole Punching**:
   - Tự triển khai giao thức STUN đơn giản để xác định loại NAT.
   - Thử nghiệm UDP Hole Punching để hai máy tính sau Router gia đình có thể kết nối trực tiếp không cần mở port thủ công.
   - Hỗ trợ UPnP / PCP tự động yêu cầu Router mở cổng TCP.
2. **Kademlia DHT Hoàn Chỉnh**:
   - Triển khai bảng định tuyến K-bucket theo khoảng cách XOR giữa các Peer ID.
   - Tìm kiếm node và lưu trữ provider records hoàn toàn phi tập trung mà không cần danh sách node ban đầu cố định.

---

## Giai đoạn 4: Truyền File & Phương Tiện (File & Media Transfer)

1. **Phân đoạn File (Chunked Transfer Protocol)**:
   - Cắt tệp tin thành các khối nhỏ 64 KiB hoặc 256 KiB.
   - Xây dựng cây băm Merkle Tree để xác thực toàn vẹn từng khối độc lập.
2. **Tải Song Song Đa Node**:
   - Cho phép tải các khối file đồng thời từ nhiều node khác nhau đang lưu giữ bản mã.
   - Hỗ trợ tạm dừng, tiếp tục tải (resume) khi rớt mạng.

---

## Giai đoạn 5: Trò Chuyện Nhóm (Decentralized Group Chat)

1. **Sender Keys / Messaging Layer Security (MLS)**:
   - Phân phối khóa nhóm mã hóa đầu cuối.
   - Xoay khóa tự động (Key Rotation) khi có thành viên rời nhóm hoặc tham gia mới.
2. **Đồng Bộ Trạng Thái Nhóm Phân Tán**:
   - Thỏa thuận danh sách thành viên không qua máy chủ tập trung.

---

## Giai đoạn 6: Ứng Dụng Đa Nền Tảng (Mobile & Desktop)

1. **FFI C-Bindings**:
   - Xuất khẩu thư viện C ABI (`p2pchat_core.so` / `.dll` / `.dylib`) từ nhân Rust.
   - Cho phép tích hợp trực tiếp vào ứng dụng Android (Kotlin / JNI), iOS (Swift), và Desktop (Tauri / Flutter).
2. **Cổng Thông Báo Đẩy Tùy Chọn (Optional Push Gateway)**:
   - Cho phép người dùng tự dựng gateway đẩy thông báo FCM/APNs cá nhân khi ứng dụng trên điện thoại bị hệ điều hành đóng ngắt kết nối nền.
