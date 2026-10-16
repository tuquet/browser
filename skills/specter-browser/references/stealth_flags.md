# Tuquet Browser: C++ Antidetect Stealth & Flag Reference

## Architectural Invariants

1. **Storage Pillar**: All browser data resolves strictly to the single canonical root `~/.specter/browser/`:
   - **Antidetect Chromium Runtimes**: `~/.specter/browser/runtimes/<version>/` (e.g. `stealth/` or `v148.0.7778.215/`)
   - **User Sandboxes**: `~/.specter/browser/profiles/<profile_id>/`
   - **Extension Bundles**: `~/.specter/browser/extensions/`

2. **Zero Identity Bleed**: Never inspect, hook, or launch personal host browsers (Chrome, Edge, Brave). Every workflow session binds to a scoped, disposable sandbox directory (`--user-data-dir`).

3. **C++ Level Stealth Defenses**:
   - **Zero Prototype Poisoning**: Canvas, Audio, WebGL, and Client Hints are patched directly inside Blink/V8 at the native level, preserving `Function.prototype.toString` and avoiding JavaScript-level getter traps (0 Lies on CreepJS).
   - **Deterministic PRNG Seed**: The `--fingerprint=<seed>` flag produces consistent, reproducible hardware characteristics for the same seed.
   - **Automated Anti-Bot Evasion**: Native `navigator.webdriver = false`, `fakeShadowRoot` for closed Shadow DOM traversal, and Chrome DevTools Protocol (CDP) evasion.

4. **Version Pinning Guardrail (Golden LTS v148)**:
   - Exclusively bind to **`v148.0.7778.215`**.
   - Strictly reject upstream Chromium 150 (`150.0.7871.186`) due to open memory bugs in canvas readback (`SIGSEGV` / `Crashpad_NotConnectedToHandler` in `getImageData()`/`readPixels()`, GitHub Issues #94 & #95).

5. **Version Switching & Profile Compatibility Mechanism**:
   - Switching versions via `specter browser use <version>` updates the active version in `~/.specter/browser/browser.json`.
   - Existing sandboxes in `~/.specter/browser/profiles/` remain 100% functional.
   - Stale profile lock files (`SingletonLock`, `SingletonCookie`, `SingletonSocket`) are automatically purged before launch.

6. **Network Mesh Integration**:
   - Route browser network traffic through Specter Bridge (`--proxy-server=socks5://127.0.0.1:1080`).
   - Always enforce `--disable-non-proxied-udp` when routing through proxies to eliminate real IP leaks via WebRTC STUN queries.

---

## CLI Flag Reference (v148 LTS)

| Flag | Value / Example | Architectural Purpose |
| :--- | :--- | :--- |
| `--fingerprint=<seed>` | `133742` (32-bit int) | **Core Deterministic Seed**. Powers all PRNG noise algorithms. |
| `--fingerprint-platform=<os>` | `windows`, `macos`, `linux` | Spoofs `navigator.platform` and Client Hints OS headers. |
| `--fingerprint-platform-version=<ver>` | `"10.0.0"`, `"15.2.0"` | Specifies the detailed operating system version string. |
| `--fingerprint-brand=<brand>` | `Chrome`, `Edge`, `Opera` | Customizes browser brand in `navigator.userAgentData`. |
| `--fingerprint-brand-version=<ver>` | `148.0.7778.215` | Sets the brand version string. |
| `--fingerprint-hardware-concurrency=<n>` | `8`, `16` | CPU core count in `navigator.hardwareConcurrency`. |
| `--timezone="<tz>"` | `"Asia/Ho_Chi_Minh"`, `"UTC"` | Native C++ timezone override via `Intl.DateTimeFormat`. |
| `--lang=<locale>` | `vi-VN`, `en-US` | Sets internal browser UI language. |
| `--accept-lang=<locales>` | `vi-VN,vi,en-US,en` | HTTP `Accept-Language` header and `navigator.languages`. |
| `--proxy-server="<proto>://<ip>:<port>"` | `socks5://127.0.0.1:1080` | Directs traffic through SOCKS5 proxy via `specter bridge`. |
| `--disable-non-proxied-udp` | *(Flag)* | Disables non-proxied UDP to prevent real IP leaks via WebRTC STUN. |
| `--disable-spoofing=<list>` | `font,audio` | Selectively disables spoofing for specified subsystems. |
| `--user-data-dir=<path>` | `~/.specter/browser/profiles/p1` | Absolute path to isolated profile directory. |
| `--no-first-run` | *(Flag)* | Skips first-run wizard. |
| `--no-default-browser-check` | *(Flag)* | Disables default browser check dialog. |

### Deprecated Flags (Chrome 144+)
- Do NOT use `--fingerprint-gpu-vendor` or `--fingerprint-gpu-renderer` (auto-simulated by seed in Chrome 144+).
- Do NOT use `--disable-gpu-fingerprint` (replaced by `--disable-spoofing=gpu`).
- Avoid passing `--disable-gpu-sandbox` or `--no-sandbox` during desktop interactive execution.

---

## Identity Verification Checklist

- **`https://iphey.com`**: Status must be "Pass", Trustworthy score "High", and 0 correlation warnings between isolated sessions.
- **`https://bot.sannysoft.com`**: `navigator.webdriver` must report `false`, CDP protocol must remain undetected.
- **`https://creepjs-api.web.app`**: "Lies" count must be `0` (clean Blink/V8 native execution).
- **`https://www.browserscan.net`**: Authenticity score must achieve 100%.
