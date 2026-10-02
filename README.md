<div align="center">
  <img src="./assets/logo.svg" width="76" height="76" alt="Browser Logo" />
  <h1>Browser</h1>
  <p><strong>High-Performance Headless Web Scraping &amp; Dedicated Chromium Automation Runtime</strong></p>

  <p>
    <a href="https://github.com/tuquet/scoop-bucket"><img src="https://img.shields.io/badge/Scoop-tuquet%2Fscoop--bucket-blue.svg" alt="Scoop Bucket" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-Tokio%2FCDP-orange.svg" alt="Rust" /></a>
    <img src="https://img.shields.io/badge/Runtime-Chromium%20LTS-brightgreen.svg" alt="Chromium" />
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License" /></a>
  </p>
</div>

---

> **Browser** (`tuquet-browser`) is a specialized browser engine module within the automation ecosystem, responsible for Chromium LTS runtime lifecycle management, multi-profile sandboxing, Chrome DevTools Protocol (CDP) orchestration, and antidetect stealth automation.

## 📦 Core Responsibilities & Capabilities

1. **Zero Host Scanning & Dedicated Runtime**:
   * Automatically downloads and synchronizes official Open-Source Chromium LTS builds from high-speed CDNs.
   * Completely isolated from personal workstation host browsers (`~/.tuquet/runtimes/chromium-<platform>`).
2. **Multi-Profile Sandbox**:
   * Manages dedicated user data directories for each isolated automation profile.
   * Automatically cleans bloated caches (`GPUCache`, `ShaderCache`, `Crashpad`) and unlocks singleton lock files (`SingletonLock`).
3. **CDP & Extension Driver**:
   * Supervises Chromium child processes and auto-discovers WebSocket CDP debugging endpoints (`/json/version`).
   * Supports dynamic loading of Manifest V3 runner extensions per session.
4. **Proxy & Antidetect Architecture**:
   * Supports isolated proxy routing (SOCKS5 / HTTP) per browser instance.
   * Hardware fingerprint spoofing roadmap (Canvas, WebGL, AudioContext, Client Hints).

---

## 🚀 Rust Crate Usage

Add to `Cargo.toml`:
```toml
[dependencies]
tuquet-browser = { git = "https://github.com/tuquet/browser.git", branch = "main" }
```

### Runtime Download & Status Check Example:
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

## 🌐 Ecosystem

Part of the **Automation & Agent Ecosystem**:

- [Automa](https://github.com/tuquet/automa) — Native Chrome/Edge Desktop UI Automation Browser.
- [Runner](https://github.com/tuquet/runner) — High-Performance Distributed Process Supervision Engine in Rust.
- [Browser](https://github.com/tuquet/browser) — High-Performance Headless Web Scraping & Stealth Automation Core.
- [Cloud](https://github.com/tuquet/cloud) — Enterprise Orchestration & Real-time Task Control Plane.
- [CLI](https://github.com/tuquet/cli) — Developer Ergonomic CLI & Unified Command Center.
- [Lib](https://github.com/tuquet/lib) — Monorepo for Shared Enterprise UI & Utilities (`vue-ui`, `vue-table`, `md-export`, `extension-runner`, `lunar`).
- [Scoop Bucket](https://github.com/tuquet/scoop-bucket) — Official Windows Scoop Distribution Channel.

---

## 📄 License

Distributed under the [MIT License](LICENSE).

---

<div align="center">
  <samp>
    <a href="https://tuquet.github.io">Portfolio</a> •
    <a href="https://tuquet.github.io/cv">CV &amp; Resume</a> •
    <a href="https://tuquet.github.io/automa">Automa Studio</a> •
    <a href="https://tuquet.github.io/lib">Component Lab</a> •
    <a href="https://github.com/tuquet/scoop-bucket">Scoop Bucket</a>
  </samp>
</div>
