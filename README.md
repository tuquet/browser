<div align="center">
  <img src="https://tuquet.github.io/icons/browser.svg" width="80" height="80" alt="Browser Logo" />
  <h1>Tuquet Browser Core (`tuquet-browser`)</h1>
  <p><strong>C++ Native Anti-Detect Engine, Deterministic PRNG Seed Hardware Emulation &amp; Multi-Profile Sandbox Subsystem in Rust</strong></p>

  <p>
    <a href="https://github.com/tuquet/scoop-bucket"><img src="https://img.shields.io/badge/Scoop-tuquet-blue.svg" alt="Scoop Bucket" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-2024%20%7C%20Tokio-orange.svg" alt="Rust 2024" /></a>
    <img src="https://img.shields.io/badge/Engine-C%2B%2B%20Antidetect%20v148%20LTS-brightgreen.svg" alt="C++ Antidetect v148 LTS" />
    <a href="https://github.com/tuquet/skills/blob/main/skills/tuquet-browser/SKILL.md"><img src="https://img.shields.io/badge/Skill-%2Ftuquet--browser-purple.svg" alt="Tuquet Browser Skill" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License" /></a>
  </p>
  <p>
    <strong><a href="https://tuquet.github.io/posts/xay-dung-trinh-duyet-an-danh-antidetect-standard">📖 Technical Guide &amp; Hands-on Lab Post &rarr;</a></strong> &bull;
    <strong><a href="https://github.com/tuquet/skills/blob/main/skills/tuquet-browser/SKILL.md">⚡ Operational Skill Reference (`/tuquet-browser`) &rarr;</a></strong>
  </p>
</div>

---

## 📋 Executive Summary & Strategic Rationale

Traditional browser automation relies on either **host browsers** (personal Chrome/Edge instances) or **generic open-source Chromium** wrapped with JavaScript-level stealth shims (such as `puppeteer-extra-plugin-stealth` or `Object.defineProperty`).

In modern anti-bot environments protected by **Cloudflare Turnstile, DataDome, Kasada, Akamai Bot Manager, CreepJS, or Iphey**, these approaches fail catastrophically:
1. **JavaScript Prototype Poisoning Traps**: Any JavaScript modification to `navigator.webdriver`, `WebGLRenderingContext`, or `CanvasRenderingContext2D` alters prototype chains, triggers getter/setter inspection traps, and flags high "Lies" scores on CreepJS.
2. **CDP Fingerprint Leaks**: Calling Chrome DevTools Protocol (`Runtime.enable`, `Page.enable`) exposes automation hooks to client-side scripts.
3. **Hardware Fingerprint Non-Determinism**: Standard Chromium presents identical hardware hashes across different profiles on the same physical host, leading to immediate multi-accounting correlation bans.

### Why Tuquet Standards on Native C++ Patched Chromium (`adryfish/fingerprint-chromium`)

Tuquet Browser adopts the pre-compiled C++ Antidetect release ([`adryfish/fingerprint-chromium`](https://github.com/adryfish/fingerprint-chromium)) as its **canonical core engine** instead of generic vanilla Chromium:

| Architectural Metric | Vanilla Open-Source Chromium | JavaScript Stealth Plugins (Puppeteer) | Tuquet C++ Antidetect Engine (`v148 LTS`) |
| :--- | :--- | :--- | :--- |
| **Interception Layer** | None (Raw Chromium) | High-level V8 JS Prototype tampering | **Deep Blink / V8 Native C++ core hooks** |
| **Prototype Integrity** | Clean (but detectable flags) | Compromised (`toString` leaks, prototype trap) | **100% Native Invariance (`toString()` clean)** |
| **Deterministic PRNG** | No (Reflects true host hardware) | Difficult & unstable | **Single `--fingerprint=<seed>` generates 100% stable profile** |
| **WebGL & GPU Emulation** | Raw host GPU specs | Incomplete string spoofing | **Realistic GPU Simulation from verified hardware database** |
| **Device Memory Simulation** | Hardcoded to host RAM | Unstable JS getter override | **Deterministic randomization (8, 16, or 32 GB)** |
| **CDP & Bot Flag Evasion** | Leaks `navigator.webdriver = true` | Patches after script execution | **`navigator.webdriver = false` & `fakeShadowRoot` built-in** |
| **iphey.com / CreepJS Score** | Suspicious / Bot Detected | Moderate / High "Lies" count | **100% Pass / 0 Lies / Trustworthy: High** |

---

## 🏗️ Storage Hierarchy & Single Source of Truth (SSOT)

In strict adherence to Tuquet's **Single Source of Truth (SSOT)**, all browser state, binaries, and runtime environments resolve exclusively to `~/.specter/browser/`:

```text
~/.specter/browser/
├── runtimes/
│   ├── stealth/                 # Official C++ Antidetect Engine (chrome.exe v148 LTS)
│   └── chromium-win64/          # Fallback standard vanilla Chromium LTS binary
├── profiles/                    # Multi-profile isolated user data directories
│   ├── default/                 # Default isolated profile
│   └── profile_<uuid>/          # Ephemeral/persistent sandboxes for concurrent worker sessions
├── extensions/                  # Managed MV3 extension bundles (stealth, ad-blockers)
└── scripts/                     # Verified PowerShell test & launch automation scripts
    ├── launch_lab1.ps1          # Deterministic Seed 133742 (Direct IP)
    ├── launch_lab2_proxy.ps1    # Deterministic Seed 888999 (SOCKS5 Proxy via Tuquet Bridge)
    └── compare_side_by_side.ps1 # Dual-instance side-by-side verification runner
```

---

## 🛡️ Production Stability & Version Pinning (Golden LTS v148)

> [!CAUTION]
> **Upstream Regression Notice (Issue #94 & #95 in Chromium 150):**  
> Upstream release `v150.0.7871.186` (October 2026) introduced a critical segmentation fault (`SIGSEGV` / `SEGV_ACCERR`) in `getImageData()` and `readPixels()`. Because Ungoogled Chromium strips Google Crashpad, renderer tabs immediately crash with `Error code: Crashpad_NotConnectedToHandler`.

> [!IMPORTANT]
> **Production Standard: Pin to `v148.0.7778.215` (`ungoogled-chromium_148.0.7778.215-1.1_windows_x64.zip`)**:
> - **Zero Renderer Crashes**: 100% stable across all Canvas, WebGL, and AudioContext readback tests.
> - **Realistic GPU Parameter Simulation**: Automatic real-world WebGL vendor/renderer calibration based on the seed.
> - **Randomized Device Memory**: Dynamically allocates 8, 16, or 32 GB RAM per identity seed.

---

## ⚙️ CLI Flag Specification Matrix (Official v148 LTS)

When launching the stealth engine via CLI or the `tuquet-browser` Rust crate, configure the following verified flag matrix:

### Core Active Flags

| CLI Flag | Example / Value | Description & Native C++ Behavior |
| :--- | :--- | :--- |
| **`--fingerprint=<seed>`** | `133742` (32-bit int) | **Core Deterministic Seed**. Powers all PRNG noise algorithms (Canvas, WebGL, AudioContext, Font metrics, RAM, CPU). |
| **`--fingerprint-platform=<os>`** | `windows`, `macos`, `linux` | Spoofs `navigator.platform` and Client Hints OS headers. |
| **`--fingerprint-platform-version=<ver>`** | `"10.0.0"`, `"15.2.0"` | Specifies the detailed operating system version string. |
| **`--fingerprint-brand=<brand>`** | `Chrome`, `Edge`, `Opera`, `Vivaldi` | Customizes browser brand in `navigator.userAgentData` (default: `Chromium`). |
| **`--fingerprint-brand-version=<ver>`** | `148.0.7778.215` | Sets the brand version string. |
| **`--fingerprint-hardware-concurrency=<n>`** | `8`, `16` | CPU core count in `navigator.hardwareConcurrency` (auto-derived from seed if omitted). |
| **`--timezone="<tz>"`** | `"Asia/Ho_Chi_Minh"`, `"UTC"` | Native C++ timezone override via `Intl.DateTimeFormat` **without modifying host OS clock**. |
| **`--lang=<locale>`** | `vi-VN`, `en-US` | Sets internal browser UI language. |
| **`--accept-lang=<locales>`** | `vi-VN,vi,en-US,en` | HTTP `Accept-Language` header and `navigator.languages` sequence. |
| **`--proxy-server="<proto>://<ip>:<port>"`** | `socks5://127.0.0.1:1080` | Directs all HTTP/HTTPS/WebSocket traffic through SOCKS5 / HTTP proxy (pairs with `tuquet bridge`). |
| **`--disable-non-proxied-udp`** | *(Flag)* | **Essential WebRTC Guardrail**: Disables non-proxied UDP to prevent real IP leaks via STUN. |
| **`--disable-spoofing=<list>`** | `font,audio` | Selectively disables spoofing for specified subsystems: `font`, `audio`, `canvas`, `clientrects`, `gpu`. |
| **`--user-data-dir=<path>`** | `~/.specter/browser/profiles/p1` | Absolute path to isolated profile directory. |
| **`--no-first-run`** | *(Flag)* | Skips first-run wizard and welcomes. |
| **`--no-default-browser-check`** | *(Flag)* | Disables the default browser prompt. |

### Deprecated Flags (Chrome 144+)

- ❌ `--fingerprint-gpu-vendor` *(Removed; replaced by Realistic GPU Simulation)*
- ❌ `--fingerprint-gpu-renderer` *(Removed; replaced by Realistic GPU Simulation)*
- ❌ `--disable-gpu-fingerprint` *(Removed; use `--disable-spoofing=gpu`)*

### ⚠️ Flag Hygiene Rule
Avoid passing `--disable-gpu-sandbox` or `--no-sandbox` on standard desktop sessions. Unnecessary sandbox flags trigger Chromium's yellow notification bar (*"You are using an unsupported command-line flag"*). Keep CLI invocations clean.

---

## 📦 Rust Crate Architecture (`tuquet-browser`)

`tuquet-browser` provides modular, async Tokio-based primitives for managing antidetect browser lifecycles:

```text
src/
├── resolver.rs      # Prioritizes ~/.specter/browser/runtimes/stealth/chrome.exe over vanilla
├── launcher.rs      # Spawns Chromium under AsyncCommandGroup with clean CLI flags & CDP wait
├── manager.rs       # Active process tracking, graceful shutdown (WM_CLOSE/SIGTERM), lock hygiene
├── coordinator.rs   # Port allocation & multi-browser coordination
├── profile.rs       # Profile sandboxing, cache purging, cookie quarantine
└── extension.rs     # Managed MV3 extension injection without fingerprint disruption
```

### Rust Usage Example

```rust
use tuquet_browser::{BrowserLauncher, BrowserLauncherOptions, resolve_executable_path};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Resolves stealth C++ runtime automatically from ~/.specter/browser/runtimes/stealth/
    let exe_path = resolve_executable_path("stealth").await?;
    
    // 2. Configure deterministic antidetect identity
    let seed = "133742";
    let options = BrowserLauncherOptions {
        executable_path: exe_path,
        user_data_dir: "C:\\Users\\<username>\\.specter\\browser\\profiles\\worker_01".to_string(),
        debugging_port: 9222,
        extension_paths: vec![],
        custom_args: vec![
            format!("--fingerprint={}", seed),
            "--fingerprint-platform=windows".to_string(),
            "--fingerprint-brand=Chrome".to_string(),
            "--fingerprint-brand-version=148.0.7778.215".to_string(),
            "--timezone=Asia/Ho_Chi_Minh".to_string(),
            "--lang=vi-VN".to_string(),
            "--proxy-server=socks5://127.0.0.1:1080".to_string(),
            "--disable-non-proxied-udp".to_string(),
        ],
    };

    // 3. Launch isolated instance and connect CDP
    let mut launcher = BrowserLauncher::new(options);
    let ws_endpoint = launcher.launch().await?;
    println!("Antidetect Browser active! CDP WebSocket: {}", ws_endpoint);

    // Keep session alive...
    // launcher.close().await?;
    Ok(())
}
```

---

## 🧪 Verification Benchmarks & Hands-on Lab

In hands-on verification tests ([read full technical post](https://tuquet.github.io/posts/xay-dung-trinh-duyet-an-danh-antidetect-standard)), dual-instance side-by-side executions confirmed:

| Verification Suite | Test Objective | Observed Result |
| :--- | :--- | :--- |
| **`https://iphey.com`** | IP & Hardware Correlation | **Pass / Trustworthy: High** &bull; Left Window (Direct: `198.51.100.24`) vs Right Window (Proxy: `203.0.113.88`) with 0 warnings. |
| **`https://bot.sannysoft.com`** | CDP & Bot Heuristics | `navigator.webdriver = false` &bull; Chrome DevTools Protocol undetected &bull; Clean Chrome brand headers. |
| **`https://creepjs-api.web.app`** | Fingerprint Integrity | **0 Lies Detected** &bull; Native Blink Canvas noise &bull; Consistent WebGL parameters. |
| **`https://www.browserscan.net`** | Browser Authenticity | **100% Genuine Browser Score** &bull; No JavaScript prototype proxy traps. |

### Running the Verification Scripts

Tuquet provides verified PowerShell runner scripts in `~/.specter/browser/scripts/`:

```powershell
# 1. Launch Seed 133742 (Direct Network)
powershell -ExecutionPolicy Bypass -File "$HOME\.specter\browser\scripts\launch_lab1.ps1"

# 2. Launch Seed 888999 (SOCKS5 Proxy via Tuquet Bridge 127.0.0.1:1080)
powershell -ExecutionPolicy Bypass -File "$HOME\.specter\browser\scripts\launch_lab2_proxy.ps1"

# 3. Launch both side-by-side for comparison
powershell -ExecutionPolicy Bypass -File "$HOME\.specter\browser\scripts\compare_side_by_side.ps1"
```

---

## ⚡ Operational Control & Engine Management

Engine search, version switching, inspection, and lifecycle hygiene are managed directly via Tuquet CLI and **[Tuquet Skills](https://github.com/tuquet/skills)**:

```powershell
# 1. Search available upstream releases from curated manifest
tuquet browser search

# 2. List locally installed engines and active selection
tuquet browser list

# 3. Switch active runtime engine (preserves 100% profile state)
tuquet browser use stealth
tuquet browser use chromium-win64

# 4. Install engine from curated manifest (defaults to Golden LTS v148)
tuquet browser install
tuquet browser install v148.0.7778.215 --engine stealth

# 5. Inspect runtime readiness, active engine, and storage footprint
tuquet browser status

# 6. Print raw executable path for headless automation drivers
tuquet browser path

# 7. Purge dead GPUCache, ShaderCache, and SingletonLock files
tuquet browser clean
```

### 🔄 Automated Release Watcher & Manifest (`manifest.json`)

To eliminate rate limits on `api.github.com` and ensure reproducible, enterprise-grade runtime downloads:
- **Curated Manifest (`manifest.json`)**: Tracks upstream releases from [`adryfish/fingerprint-chromium`](https://github.com/adryfish/fingerprint-chromium) with platform assets, SHA256 checksums, and stability tags (`Golden LTS`, `Archive`, `Buggy`).
- **CI Release Watcher (`.github/workflows/sync-releases.yml`)**: Automated cron runs every 6 hours using `scripts/sync-manifest.mjs` to fetch new upstream tags and submit pull requests/commits.
- **Embedded Offline Fallback**: The `tuquet-browser` crate bundles `manifest.json` via `include_str!`, ensuring all search, listing, and validation logic runs 100% offline.

> 💡 **AI Agent Quick Execution**: In Antigravity, Claude Code, or Cursor, invoke:  
> **`/tuquet-browser [status|search|list|use|install|path|clean]`**

---

## 📄 License

Distributed under the [MIT License](LICENSE).
