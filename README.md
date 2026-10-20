<div align="center">
  <img src="https://tuquet.com/icons/browser.svg" width="76" height="76" alt="Browser Logo" />
  <h1>Browser Engine</h1>
  <p><strong>C++ Native Anti-Detect Engine & Deterministic PRNG Hardware Emulation</strong></p>

  <p>
    <a href="https://docs.tuquet.com/en/browser/"><img src="https://img.shields.io/badge/Docs-VitePress%20Hub-blue.svg" alt="Documentation Hub" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-Tokio-orange.svg" alt="Rust" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License" /></a>
  </p>

  <p>
    <strong><a href="https://docs.tuquet.com/en/browser/">📖 Đọc toàn bộ tài liệu kỹ thuật tại Documentation Hub &rarr;</a></strong>
  </p>
</div>

---

## 📌 Tổng Quan (Overview)

**Browser** là subsystem điều khiển và cô lập trình duyệt anti-detect ở tầng native C++ và Rust. Thay vì sử dụng các plugin JavaScript dễ bị phát hiện bởi Cloudflare Turnstile, DataDome hoặc CreepJS, engine nhúng trực tiếp nhân C++ Chromium LTS v148 với cơ chế sinh hạt giống phần cứng giả lập tất định (Deterministic PRNG Seed).

* **Lưu trữ SSOT**: Toàn bộ profile, nhị phân runtime và extension được quản lý tập trung tại `~/.tuquet/browser/`.
* **Zero-Leak WebRTC & Canvas**: Khử hoàn toàn rò rỉ IP và vết định danh phần cứng qua GPU WebGL, AudioContext và Canvas 2D.

## ⚡ Sử Dụng Nhanh (Quickstart)

```bash
# Khởi chạy một phiên profile cô lập với hạt giống tất định
tq browser launch --profile lab1 --seed 133742

# Kiểm tra trạng thái runtime và danh sách profile
tq browser status
```

## 📚 Tài Liệu Kỹ Thuật Tập Trung (SSOT)

Toàn bộ đặc tả cờ CLI v148, ma trận phần cứng, cấu trúc thư mục và runbook kiểm nghiệm được bảo trì duy nhất tại Documentation Hub:

👉 **[https://docs.tuquet.com/en/browser/](https://docs.tuquet.com/en/browser/)**
