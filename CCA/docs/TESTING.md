# P2P Chat Core — Tài Liệu Kiểm Thử (Testing & Test Vectors)

## 1. Hướng Dẫn Thực Thi Kiểm Thử

Dự án có thể chạy kiểm thử độc lập mà không cần kết nối Internet hoặc tải thêm dependency:

```bash
# Chạy toàn bộ unit test và integration test
cargo test

# Chạy riêng các bài kiểm thử đơn vị mật mã và giao thức
cargo test --lib

# Chạy riêng các bài kiểm thử tích hợp đa node qua TCP loopback
cargo test --test integration_tests

# Kiểm tra quy chuẩn mã nguồn không có cảnh báo
cargo clippy --all-targets -- -D warnings

# Kiểm tra chuẩn định dạng mã nguồn
cargo fmt --check
```

---

## 2. Bảng Tổng Hợp Kiểm Thử Mật Mã Chuẩn Quốc Tế

| Module | Mục kiểm tra | Nguồn Test Vector | Kết quả |
|---|---|---|---|
| `crypto::sha256` | Chuỗi rỗng `""` | NIST / RFC 6234 | **PASSED** |
| `crypto::sha256` | Chuỗi `"abc"` | NIST / RFC 6234 | **PASSED** |
| `crypto::sha256` | Chuỗi 56 byte `abcdbcdecdef...` | NIST / RFC 6234 | **PASSED** |
| `crypto::hmac` | RFC 4231 Case 1 (20 byte key, data "Hi There") | RFC 4231 Section 4.2 | **PASSED** |
| `crypto::hmac` | RFC 4231 Case 2 (key "Jefe") | RFC 4231 Section 4.2 | **PASSED** |
| `crypto::hmac` | RFC 4231 Case 3 (20 byte 0xaa, 50 byte 0xdd) | RFC 4231 Section 4.4 | **PASSED** |
| `crypto::hkdf` | RFC 5869 Case 1 (Extract & Expand) | RFC 5869 Section A.1 | **PASSED** |
| `crypto::hkdf` | RFC 5869 Case 2 (Extract & Expand 82 bytes) | RFC 5869 Section A.2 | **PASSED** |
| `crypto::chacha20` | RFC 8439 Section 2.4.2 Encryption | RFC 8439 Section 2.4.2 | **PASSED** |
| `crypto::poly1305` | RFC 8439 Section 2.5.2 Authenticator | RFC 8439 Section 2.5.2 | **PASSED** |
| `crypto::aead` | RFC 8439 Section 2.8.2 AEAD Authenticated Encryption | RFC 8439 Section 2.8.2 | **PASSED** |
| `crypto::x25519` | RFC 7748 Section 5.2 Alice/Bob Key Agreement | RFC 7748 Section 5.2 | **PASSED** |
| `crypto::constant_time` | So sánh thời gian không đổi `ct_eq` | Custom boundary tests | **PASSED** |
| `crypto::random` | Sinh ngẫu nhiên CSPRNG hệ điều hành | OS Random Integrity | **PASSED** |

---

## 3. Các Bài Kiểm Thử Tích Hợp Đa Node (Integration Tests)

Các bài kiểm thử trong `tests/integration_tests.rs` sử dụng mạng TCP thật trên giao tiếp loopback (`127.0.0.1`):

1. **`test_two_nodes_direct_connection`**:
   - Khởi động Node A (port 17001) và Node B (port 17002).
   - Node B kết nối tới Node A qua TCP.
   - Xác minh bắt tay nhị phân hai chiều (`HELLO` / `HELLO_ACK`) thành công.
   - Xác minh cả 2 node đều nhận diện và lưu Peer ID của nhau trong sổ quản lý kết nối.
2. **`test_direct_encrypted_messaging`**:
   - Alice và Bob thêm nhau vào danh bạ bằng Public Key.
   - Alice mã hóa và gửi tin nhắn trực tiếp qua kết nối TCP.
   - Bob nhận gói tin, xác thực nhãn AEAD và giải mã chính xác bản rõ.
   - Xác minh trên wire không có bất kỳ byte văn bản thô nào.
3. **`test_offline_relay_store_and_forward`**:
   - Khởi động Relay Node R (port 17022).
   - Alice trực tuyến và gửi tin nhắn cho Bob khi **Bob đang hoàn toàn ngoại tuyến**.
   - Relay R lưu trữ phong bì đã mã hóa vào RelayStore.
   - Alice tắt hoàn toàn (`alice.stop()`).
   - Bob khởi động sau đó, kết nối tới Relay R và gửi yêu cầu `MESSAGE_FETCH_REQUEST`.
   - Bob nhận lại bản mã từ Relay R và giải mã thành công bản rõ mà không cần Alice phải online cùng lúc.
4. **`test_malformed_packets_rejected_without_crash`**:
   - Gửi gói tin có Magic bytes rác, phiên bản sai, và frame khai báo kích thước vượt quá 64 KiB (>100 KiB).
   - Xác minh TCP Listener từ chối an toàn mà không bị crash, không panic và vẫn tiếp tục phục vụ các kết nối hợp lệ sau đó.
5. **`test_replay_protection`**:
   - Thử nghiệm gửi lại cùng một `message_id` nhiều lần.
   - Bộ đệm `SeenMessageCache` chặn phát lại thành công.
6. **`test_persistence_across_restarts`**:
   - Khởi động Node, thêm liên hệ danh bạ, tắt Node.
   - Khởi động lại Node trỏ về cùng thư mục dữ liệu.
   - Xác minh Identity (KeyPair, Peer ID) và danh bạ được phục hồi nguyên vẹn.
