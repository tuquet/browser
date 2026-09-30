# 🌐 Tuquet Browser

> **Tuquet Browser** (`tuquet-browser`) là phân hệ trình duyệt chuyên biệt trong hệ sinh thái Tuquet, chịu trách nhiệm quản lý vòng đời Chromium Runtime, Multi-Profile Sandbox, Chrome DevTools Protocol (CDP) và động cơ tự động hóa trình duyệt chống phát hiện (Antidetect Engine).

---

## 📦 Nhiệm vụ & Tính năng

1. **Zero Host Scanning & Dedicated Runtime**:
   * Tự động tải và đồng bộ các bản build Open-Source Chromium LTS chính thức từ CDN tốc độ cao.
   * Cách ly hoàn toàn với trình duyệt cá nhân trên máy trạm (`~/.tuquet/runtimes/chromium-<platform>`).
2. **Multi-Profile Sandbox**:
   * Quản lý phân vùng lưu trữ user data riêng biệt cho từng profile.
   * Tự động dọn dẹp các cache phình to (`GPUCache`, `ShaderCache`, `Crashpad`) và mở khóa singleton lock files (`SingletonLock`).
3. **CDP & Extension Driver**:
   * Quản lý tiến trình Chromium child process, tự động phát hiện cổng debug WebSocket CDP (`/json/version`).
   * Hỗ trợ nạp động tiện ích mở rộng (MV3 runner extension) cho từng phiên làm việc.
4. **Proxy & Antidetect Architecture**:
   * Hỗ trợ cấu hình proxy độc lập (SOCKS5 / HTTP) trên từng phiên làm việc.
   * Lộ trình mở rộng can thiệp làm giả vân tay phần cứng (Canvas, WebGL, AudioContext, Client Hints).

---

## 🚀 Sử dụng Crate trong Rust

Thêm vào `Cargo.toml`:
```toml
[dependencies]
tuquet-browser = { git = "https://github.com/tuquet/browser.git", branch = "main" }
```

### Ví dụ tải và kiểm tra trạng thái Runtime:
```rust
use tuquet_browser::{get_runtime_status, download_chromium_runtime};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let status = get_runtime_status();
    println!("Chromium installed: {}", status.installed);

    if !status.installed {
        let path = download_chromium_runtime(false, None).await?;
        println!("Chromium downloaded to: {}", path);
    }

    Ok(())
}
```

---

## 📄 License
Phát hành theo giấy phép [MIT](LICENSE).
