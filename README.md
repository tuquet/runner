<div align="center">
  <img src="https://tuquet.github.io/icons/runner.svg" width="76" height="76" alt="Runner Logo" />
  <h1>Specter Runner (`specter runner`)</h1>
  <p><strong>Kernel-Level Process Tree Supervisor & Distributed Edge Worker Engine in Rust</strong></p>

  <p>
    <a href="https://tuquet.github.io/docs/runner/"><img src="https://img.shields.io/badge/Docs-VitePress%20Hub-blue.svg" alt="Documentation Hub" /></a>
    <a href="https://github.com/tuquet/scoop-bucket"><img src="https://img.shields.io/badge/Scoop-specter-brightgreen.svg" alt="Scoop" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-Tokio%2FWin32-orange.svg" alt="Rust" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License" /></a>
  </p>

  <p>
    <strong><a href="https://tuquet.github.io/docs/runner/">📖 Đọc toàn bộ tài liệu kỹ thuật tại Documentation Hub &rarr;</a></strong>
  </p>
</div>

---

## 📌 Tổng Quan (Overview)

**Specter Runner** là engine giám sát tiến trình và quản lý worker phân tán được viết bằng Rust thuần túy. Runner giải quyết triệt để vấn đề tiến trình zombie (zombie child processes) khi trình duyệt hoặc worker gặp sự cố bằng cách ràng buộc toàn bộ cây tiến trình con vào Win32 Job Object ở tầng kernel Windows.

* **Zero-Zombie Guarantee**: Đóng hoặc crash supervisor sẽ lập tức thu hồi toàn bộ Chromium và worker con trong <0.1ms.
* **Siêu nhẹ & Hiệu năng cao**: Bộ nhớ chiếm dụng khi rảnh rỗi dưới 10MB RAM, không phụ thuộc vào Node.js hay Electron runtime.

## ⚡ Sử Dụng Nhanh (Quickstart)

```bash
# Khởi chạy Runner Supervisor kết nối với Cloud Control Plane
specter runner start

# Giám sát trạng thái daemon và cây tiến trình đang thực thi
specter runner status
```

## 📚 Tài Liệu Kỹ Thuật Tập Trung (SSOT)

Toàn bộ đặc tả kiến trúc Win32 Job Object, mô hình kết nối Cloud RPC, lưu trữ định danh `~/.specter/system/` và quy trình kiểm thử được bảo trì duy nhất tại Documentation Hub:

👉 **[https://tuquet.github.io/docs/runner/](https://tuquet.github.io/docs/runner/)**
