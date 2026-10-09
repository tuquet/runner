<div align="center">
  <img src="https://tuquet.com/icons/runner.svg" width="76" height="76" alt="Runner Logo" />
  <h1>Specter Runner (`specter runner`)</h1>
  <p><strong>Kernel-Level Process Tree Supervisor & Distributed Edge Worker Engine in Rust</strong></p>

  <p>
    <a href="https://docs.tuquet.com/en/specter/runner/"><img src="https://img.shields.io/badge/Docs-VitePress%20Hub-blue.svg" alt="Documentation Hub" /></a>
    <a href="https://github.com/tuquet/scoop-bucket"><img src="https://img.shields.io/badge/Scoop-specter-brightgreen.svg" alt="Scoop" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-Tokio%2FWin32-orange.svg" alt="Rust" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License" /></a>
  </p>

  <p>
    <strong><a href="https://docs.tuquet.com/en/specter/runner/">📖 Đọc toàn bộ tài liệu kỹ thuật tại Documentation Hub &rarr;</a></strong>
  </p>
</div>

---

## 📌 Tổng Quan (Overview)

**Specter Runner** là engine giám sát và điều phối tác vụ chạy ngầm được viết bằng Rust thuần túy (native app siêu nhẹ). Runner tự động quản lý vòng đời tiến trình, đảm bảo khi tắt hoặc gặp sự cố thì mọi tiến trình con (trình duyệt, worker) đều được dọn dẹp sạch sẽ tức thì (Zero Zombie), không bao giờ gây đơ máy hay chiếm dụng RAM ngầm.

* **Zero-Zombie Guarantee**: Đóng hoặc crash supervisor sẽ lập tức thu hồi toàn bộ Chromium và worker con trong <0.1ms.
* **Siêu nhẹ & Hiệu năng cao**: Bộ nhớ chiếm dụng khi rảnh rỗi dưới 10MB RAM, ứng dụng native không phụ thuộc vào Node.js hay Electron runtime.

## ⚡ Sử Dụng Nhanh (Quickstart)

```bash
# Khởi chạy Runner Supervisor kết nối với Cloud Control Plane
specter runner start

# Giám sát trạng thái daemon và cây tiến trình đang thực thi
specter runner status
```

## 📚 Tài Liệu Kỹ Thuật Tập Trung (SSOT)

Toàn bộ tài liệu chi tiết, cơ chế dọn dẹp tiến trình tự động, mô hình kết nối Cloud RPC và quy trình kiểm thử được bảo trì duy nhất tại Documentation Hub:

👉 **[https://docs.tuquet.com/en/specter/runner/](https://docs.tuquet.com/en/specter/runner/)**
