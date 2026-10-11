# P2P Chat Core — Tài Liệu Bảo Mật (Security Specification)

## 1. Tiêu Chuẩn Thuật Toán Mật Mã

Dự án chỉ triển khai các thuật toán mật mã đã được chuẩn hóa quốc tế và kiểm chứng bằng test vector chính thức:

| Chức năng | Thuật toán | Tiêu chuẩn tham chiếu |
|---|---|---|
| Khóa trao đổi (Key Agreement) | X25519 (Montgomery Ladder) | RFC 7748 |
| Dẫn xuất khóa (Key Derivation) | HKDF-SHA256 | RFC 5869 |
| Mã hóa dòng (Stream Cipher) | ChaCha20 (256-bit key, 96-bit nonce) | RFC 8439 |
| Kiểm tra toàn vẹn (MAC) | Poly1305 (One-time authenticator) | RFC 8439 |
| Mã hóa xác thực (AEAD) | ChaCha20-Poly1305 | RFC 8439 |
| Mã băm (Hashing) | SHA-256 | FIPS 180-4 / RFC 6234 |
| Mã xác thực thông điệp (MAC) | HMAC-SHA256 | RFC 2104 / RFC 4231 |
| Nguồn ngẫu nhiên an toàn | OS CSPRNG (RtlGenRandom / /dev/urandom) | Platform OS API |

---

## 2. Quy trình Mã hóa Đầu Cuối (E2EE Pipeline)

1. **Thỏa thuận Khóa Chung (Shared Secret)**:
   - Người gửi tính bí mật chung $S = \text{X25519}(\text{sk}_{\text{sender}}, \text{pk}_{\text{recipient}})$.
   - Kiểm tra điểm yếu: Nếu $S = [0; 32]$ (tấn công điểm cấp thấp / small subgroup attack), hệ thống từ chối ngay lập tức (`WeakSharedSecret`).
2. **Dẫn xuất Khóa cho từng Tin nhắn (Key Derivation)**:
   - Sinh `salt` ngẫu nhiên 16 byte từ CSPRNG hệ điều hành.
   - Sử dụng HKDF-SHA256 với nhãn định danh giao thức `p2pchat-mvp-v1/message`:
     $$\text{MessageKey} = \text{HKDF}(\text{salt}, S, \text{"p2pchat-mvp-v1/message"}, 32)$$
3. **Mã hóa và Xác thực (AEAD)**:
   - Sinh `nonce` ngẫu nhiên 12 byte từ CSPRNG hệ điều hành. Không bao giờ tái sử dụng nonce.
   - Xây dựng AAD (Authenticated Associated Data) ràng buộc siêu dữ liệu bất biến:
     $$\text{AAD} = \text{Version} \parallel \text{MessageID} \parallel \text{SenderPeerID} \parallel \text{RecipientPeerID} \parallel \text{CreatedAt} \parallel \text{ExpiresAt}$$
   - Tạo khóa Poly1305 một lần từ khối ChaCha20 với counter = 0.
   - Mã hóa nội dung bản rõ với ChaCha20 counter = 1.
   - Tính toán nhãn xác thực 16 byte (`auth_tag`) trên AAD và Ciphertext.
4. **Giải mã phía Người Nhận**:
   - Tính toán cùng bí mật $S$ và dẫn xuất $\text{MessageKey}$.
   - Tái tạo AAD và tính toán nhãn xác thực kỳ vọng.
   - So sánh nhãn xác thực bằng **hàm so sánh thời gian không đổi (`constant_time::ct_eq`)**.
   - Nếu nhãn không khớp, bản ghi bị từ chối và bản mã bị hủy bỏ, không giải mã.

---

## 3. Các Giới Hạn Bảo Mật Phải Công Bố Rõ Ràng

1. **Chưa có Forward Secrecy hoàn chỉnh**:
   - MVP sử dụng cặp khóa tĩnh để dẫn xuất khóa phiên. Nếu khóa bí mật dài hạn của người dùng bị lộ và kẻ tấn công đã lưu trữ toàn bộ bản mã truyền trên mạng, tin nhắn trong quá khứ có thể bị giải mã.
   - Phiên bản tương lai cần bổ sung khóa phiên tạm thời (Ephemeral Diffie-Hellman), Prekeys (X3DH) và cơ chế Double Ratchet.
2. **Lộ diện Siêu dữ liệu đối với Node Trung Gian (Metadata Exposure)**:
   - Node chuyển tiếp và lưu hộ (Relay) **không thể đọc nội dung bản rõ**, nhưng có thể biết:
     - Địa chỉ IP mạng của node kết nối.
     - Peer ID người gửi và Peer ID người nhận trong tiêu đề phong bì.
     - Thời gian tạo và kích thước gói tin.
3. **Chưa qua kiểm toán độc lập**:
   - Đây là bản triển khai mẫu phục vụ nghiên cứu và kiểm thử mô hình P2P phi tập trung, chưa được các công ty kiểm toán bảo mật đánh giá chuyên sâu về rò rỉ kênh phụ (side-channel analysis). Không nên sử dụng cho thông tin tối mật cấp quốc gia.
