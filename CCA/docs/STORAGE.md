# P2P Chat Core — Kiến Trúc Lưu Trữ Cục Bộ (Local Storage Engine)

## 1. Nguyên Tắc Thiết Kế

- Hoàn toàn tự triển khai bằng chuẩn I/O của ngôn ngữ Rust (`std::fs`, `std::io`).
- **Không sử dụng SQLite, RocksDB, Sled hay bất kỳ thư viện ngoài nào**.
- Không phụ thuộc định dạng văn bản cồng kềnh như JSON hay XML cho cơ sở dữ liệu cốt lõi.
- Đảm bảo tính nguyên tử (atomic) và an toàn trước sự cố sập nguồn điện (crash resilience).

---

## 2. Định Dạng Bản Ghi Nhị Phân (Binary Record Format)

Mỗi mục dữ liệu được ghi tuần tự dưới dạng bản ghi nhị phân chuẩn hóa có kiểm tra toàn vẹn:

```text
+----------------------------------------------------+
| Field              | Size (bytes) | Mô tả          |
+----------------------------------------------------+
| magic              | 4            | b"P2RC" (0x50, 0x32, 0x52, 0x43) |
| version            | 2 (u16 BE)   | Phiên bản bản ghi (1) |
| record_type        | 2 (u16 BE)   | Loại bản ghi (1..5) |
| payload_length     | 4 (u32 BE)   | Chiều dài Payload (bytes) |
| payload            | N bytes      | Dữ liệu bản ghi |
| checksum           | 4 (u32 BE)   | Checksum Adler-32 của Payload |
+----------------------------------------------------+
```

### Các Loại Bản Ghi (`record_type`):
1. `TYPE_IDENTITY (0x0001)`: Cặp khóa danh tính người dùng.
2. `TYPE_CONTACT (0x0002)`: Liên hệ danh bạ và Public Key.
3. `TYPE_PEER_ADDR (0x0003)`: Địa chỉ IP:Port các node P2P đã biết.
4. `TYPE_CHAT_MESSAGE (0x0004)`: Lịch sử tin nhắn gửi và nhận.
5. `TYPE_RELAY_ENVELOPE (0x0005)`: Các phong bì tin nhắn lưu hộ (chỉ lưu ciphertext).

---

## 3. Cơ Chế Ghi An Toàn Nguyên Tử (Atomic Write)

Để tránh tình trạng file bị hỏng nửa chừng khi máy tính bị tắt đột ngột:
1. Tạo một tệp tạm thời `<filename>.tmp` trong thư mục dữ liệu.
2. Ghi toàn bộ dữ liệu kèm checksum Adler-32 vào tệp tạm.
3. Gọi lệnh `writer.flush()` và `writer.get_ref().sync_all()` để ép hệ điều hành ghi dữ liệu vật lý xuống đĩa cứng.
4. Đóng tệp và sử dụng hàm đổi tên nguyên tử `std::fs::rename(tmp_path, target_path)`.
5. Nếu quá trình ghi tệp bị lỗi hoặc dừng đột ngột, tệp chính trước đó vẫn nguyên vẹn và không bị hư hại.

---

## 4. Bảo Vệ Dữ Liệu và Quyền Riêng Tư

- Tệp `relay.dat` chỉ lưu trữ bản mã `ciphertext` và metadata đã mã hóa, node trung gian không thể đọc nội dung này.
- Tệp `identity.dat` lưu khóa bí mật người dùng, cần được phân quyền đọc nghiêm ngặt ở cấp hệ điều hành (chỉ cho phép user hiện tại đọc).
- Tệp `messages.dat` lưu trữ lịch sử tin nhắn đã giải mã trên thiết bị cá nhân. Nếu thiết bị bị xâm nhập vật lý hoặc nhiễm mã độc, nội dung này có thể bị truy cập. Phiên bản tương lai có thể bổ sung mã hóa ổ đĩa bằng mật khẩu cục bộ (AES-GCM / Argon2).
