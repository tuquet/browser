---
name: specter-browser
description: >
  Manage Antidetect Chromium runtimes, hardware emulation seeds, profile sandboxes, and proxy routing.
  Trigger: /specter-browser, "specter browser", "install chromium", "stealth browser", "clean browser".
argument-hint: "[status|install|list|use|clean|path]"
license: MIT
---

# Specter Browser Sandbox (`specter-browser`)

Dedicated Antidetect Chromium v148 LTS runtime manager. Isolated profile sandboxes and deterministic hardware fingerprinting with zero identity bleed.

## Code & Specification Pointers (Code-as-Docs)

Read native source definitions on demand:
- Engine & CLI Commands: [browser/src/lib.rs](../../src/lib.rs)
- Sandbox Profile Manager: [browser/src/profile.rs](../../src/profile.rs)
- Runner Browser Driver: [runner/src/drivers/browser.rs](../../../runner/src/drivers/browser.rs)
- Stealth Flags & Invariants: [Stealth Flags Reference](./references/stealth_flags.md)

## Native Commands

```powershell
# 1. Inspect installed runtimes, active version, and sandbox count
specter browser status

# 2. List available versions or install golden LTS v148
specter browser list
specter browser install 148

# 3. Switch active runtime version
specter browser use 148

# 4. Print raw executable path for automation drivers
specter browser path

# 5. Clean caches & purge stale profile locks
specter browser clean
```

## Output Contract

Report status strictly:
```text
[BROWSER] Engine: Chromium Antidetect v148 LTS
• Binary:    <path> (<READY|MISSING>)
• Footprint: <size> MB on disk
• Sandboxes: <N> profiles in ~/.specter/browser/profiles/
• Stealth:   <Active | Missing>
• Network:   <Direct | SOCKS5 Proxy 127.0.0.1:1080>
Verdict: Stealth Engine operational. / Missing: run 'specter browser install'.
```

## Boundaries

Do NOT launch or inspect host personal browsers (Chrome, Edge, Brave). Only manage isolated runtimes and profiles under `~/.specter/browser/`. One-shot execution.
