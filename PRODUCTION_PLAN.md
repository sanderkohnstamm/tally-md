# Tally.md Production Plan

## Context

Tally.md is at v0.2.0 — functional on desktop (Tauri 2.x) and iOS (SwiftUI) with git sync, 8 themes, and item cycling. The app needs hardening for production release: security fixes, App Store compliance, robust sync, settings migration, test coverage, code signing, and auto-updates. This plan splits the work into 7 phases ordered by priority.

## Phase Dependencies

```
Phase 1 (Security) ─────────────────────────┐
Phase 2 (iOS App Store) ── parallel with 1 ──┤
Phase 3 (Settings Migration) ────────────────┤
Phase 4 (Git Sync Robustness) ── needs 3 ───┤
Phase 5 (Test Coverage) ── covers 1-4 ───────┤
Phase 6 (Desktop Release) ── needs 1 ────────┤
Phase 7 (Polish) ── after all above ─────────┘
```

---

## Phase 1: Security and Crash Prevention

**Why:** CSP is disabled on desktop (XSS → code execution). iOS has force unwraps that crash.

### Desktop — Enable CSP
- **File:** `desktop/src-tauri/tauri.conf.json`
- Change `"csp": null` → `"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"`
- `unsafe-inline` needed for CodeMirror + theme CSS variable injection

### iOS — Fix force unwraps
- **File:** `ios/Tally.md/Tally.md/Engine/DateHeaderEngine.swift`
  - Lines 25-26: `dates.first!` / `dates.last!` → `guard let`
  - Line 40: `calendar.date(byAdding:)!` → `guard let` + break
- **File:** `ios/Tally.md/Tally.md/Engine/ItemMovementEngine.swift`
  - Line 284: `parts.last!` → `guard let`

### iOS — Add error logging to silent failures
- **File:** `ios/Tally.md/Tally.md/Services/FileService.swift`
  - Add `import os` and `os_log` on `try?` write failures
- **File:** `ios/Tally.md/Tally.md/Services/GitService.swift`
  - Add `os_log` on `try?` API failures in `initRepo`, `pull`, `push`

### Verify
- Desktop: `npm run dev`, open DevTools, confirm no CSP violations during editing/theming
- iOS: Build with `OS_ACTIVITY_MODE=debug`, confirm no crashes with empty/malformed done.md

---

## Phase 2: iOS App Store Readiness

**Why:** Multiple blockers that cause automatic App Store rejection.

### Fix deployment target
- **File:** `ios/Tally.md/Tally.md.xcodeproj/project.pbxproj`
- Change `IPHONEOS_DEPLOYMENT_TARGET = 26.2` → `16.0` (both build configs)

### Add PrivacyInfo.xcprivacy (required since May 2024)
- **Create:** `ios/Tally.md/Tally.md/PrivacyInfo.xcprivacy`
- Declare: `NSPrivacyTracking = false`, empty tracking domains
- Declare `NSPrivacyAccessedAPITypes` for file timestamp APIs
- Add to Xcode target membership

### Add entitlements file
- **Create:** `ios/Tally.md/Tally.md/Tally.md.entitlements`
- Add `keychain-access-groups`: `$(AppIdentifierPrefix)com.sanderkohnstamm.tallymd`
- Reference in Xcode build settings

### Add accessibility labels
- **File:** `ios/.../Views/ActionBarView.swift`
  - `.accessibilityLabel("Move item to \(backLabel)")` on back button
  - `.accessibilityLabel("Move item to \(forwardLabel)")` on forward button
  - `.accessibilityLabel("Pull from remote")` / `"Push to remote"` on git buttons
- **File:** `ios/.../Views/PaneView.swift`
  - `.accessibilityLabel("\(pane.label) pane, \(isExpanded ? "expanded" : "collapsed")")`
- **File:** `ios/.../ContentView.swift`
  - `.accessibilityLabel("Settings")` on gear button

### Support Dynamic Type in editor
- **File:** `ios/.../Views/MarkdownTextView.swift`
- Line 20: Replace fixed `.monospacedSystemFont(ofSize: 15)` with:
  ```swift
  UIFont.monospacedSystemFont(
      ofSize: UIFont.preferredFont(forTextStyle: .body).pointSize,
      weight: .regular
  )
  ```
- Add `textView.adjustsFontForContentSizeCategory = true`

### Verify
- `xcodebuild -showBuildSettings | grep IPHONEOS_DEPLOYMENT_TARGET` → 16.0
- Accessibility Inspector: all buttons have labels
- Change system text size → editor text scales
- Archive build succeeds without PrivacyInfo warnings

---

## Phase 3: Settings Versioning and Migration

**Why:** Settings schema changes between versions silently reset user config to defaults.

### Desktop
- **File:** `desktop/src-tauri/src/settings.rs`
- Add `pub version: u32` field with `#[serde(default)]` (old files default to 0)
- Set current version to `1` in `Default` impl
- Add `fn migrate(settings: &mut Settings)` that chains: 0→1 sets version
- Call `migrate()` after `load()`, re-save if version changed

### iOS
- **File:** `ios/.../Models/Settings.swift`
- Add `var version: Int` with CodingKeys `"version"`, default 0 via custom decoder
- Add `static func migrate(_ settings: inout AppSettings)`
- Call from `FileService.loadSettings()`, re-save if changed

### Verify
- Delete settings.json, launch → defaults with version=1
- Create settings.json without `version` field → migrates to 1 on load

---

## Phase 4: Git Sync Robustness

**Why:** Silent sync failures, no backoff, no conflict handling on iOS.

### Desktop — Better error surfacing
- **File:** `desktop/src-tauri/src/git_sync.rs`
  - In `commit_and_push`: if push fails after commit, return "Committed locally, push failed: ..." instead of generic error
- **File:** `desktop/src-ui/main.js`
  - Add persistent error banner (reuse conflict bar pattern) that shows until next successful sync
  - Add exponential backoff: track consecutive failures, double sync interval after 3 failures, reset on success

### iOS — Rate limit handling
- **File:** `ios/.../Services/GitService.swift`
  - Check `X-RateLimit-Remaining` header on responses
  - Store `rateLimitResetDate: Date?` property
  - Before requests, check rate limit and throw descriptive error if exceeded
  - Show reset time in status message via AppViewModel

### iOS — Conflict detection (MVP)
- **File:** `ios/.../Services/GitService.swift`
  - In `pull()`: before overwriting local, compare remote vs local content
  - If both differ, save local backup to Documents/.backup/ directory
  - Show conflict warning in UI
- **File:** `ios/.../ViewModels/AppViewModel.swift`
  - Add `@Published var conflictMessage: String?`

### Verify
- Disconnect network after save → error banner appears, clears on reconnect
- Edit same file on two devices, sync both → backup created, user notified
- Make 60+ rapid API calls → graceful rate limit message

---

## Phase 5: Test Coverage

**Why:** 1 unit test exists. Core logic (item movement, dates, settings) needs regression protection.

### Desktop — Rust tests
- **File:** `desktop/src-tauri/src/finished.rs` — expand existing test module:
  - `test_complete_item` (with/without breadcrumb, cursor out of bounds)
  - `test_recover_item` (with breadcrumb returns under correct heading, without)
  - `test_move_forward_back_roundtrip`
  - `test_fill_empty_days` (3-day gap, empty input)
  - `test_reformat_date_headers` (multiple formats, mixed valid/invalid)
  - `test_breadcrumb_for` (nested items, under headings, top-level)
- **File:** `desktop/src-tauri/src/settings.rs` — new test module:
  - Default values, old format deserialization, migration path

### iOS — Swift tests
- **Create:** `ios/Tally.md/TallyMDTests/`
  - `ItemMovementEngineTests.swift` — same cases as Rust tests
  - `DateHeaderEngineTests.swift` — same cases
  - `DateParsingTests.swift` — all 6 formats, invalid strings

### CI — Add test steps
- **File:** `.github/workflows/ci.yml`
  - Add `cargo test` to `lint-rust` job
  - Add `test-ios` job with `xcodebuild test` on macOS runner

### Verify
- `cargo test` — all pass
- `xcodebuild test` — all pass
- CI runs tests on every PR

---

## Phase 6: Desktop Release Pipeline

**Why:** macOS builds aren't code-signed (Gatekeeper warnings), no auto-updater.

### macOS code signing
- **File:** `.github/workflows/release.yml`
- Add env vars for macOS matrix entries: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`
- Store as GitHub repository secrets
- `tauri-apps/tauri-action@v0` handles signing via these env vars

### Tauri auto-updater
- **File:** `desktop/src-tauri/Cargo.toml` — add `tauri-plugin-updater`
- **File:** `desktop/src-tauri/tauri.conf.json` — add updater plugin config:
  ```json
  "plugins": {
    "updater": {
      "endpoints": ["https://github.com/sanderkohnstamm/tally-md/releases/latest/download/latest.json"],
      "pubkey": "<generated-public-key>"
    }
  }
  ```
- **File:** `desktop/src-tauri/src/main.rs` — register updater plugin
- **File:** `desktop/src-ui/main.js` — check for updates on startup, show non-intrusive banner
- Generate signing keys with `tauri signer generate`

### Verify
- Tag release → all 4 platform builds succeed with signed macOS artifacts
- macOS .dmg installs without Gatekeeper warning
- `latest.json` uploaded with correct URLs/signatures
- Existing install detects update on next launch

---

## Phase 7: Polish

**Why:** Quality-of-life improvements, not blocking but improve UX.

### iOS localization prep
- Create `Localizable.strings`, wrap user-facing strings in `NSLocalizedString`
- Files: ContentView, ActionBarView, PaneView, SettingsView, AppViewModel (status messages)
- English only for now, but infrastructure ready

### Crash logging (both platforms)
- Desktop: panic hook writes to `~/.tallymd/crash.log`, show recovery toast on next launch
- iOS: `NSSetUncaughtExceptionHandler` writes to Documents/crash.log

### Desktop accessibility
- Add ARIA labels to all interactive elements in `desktop/ui/index.html`

### Settings sync conflict handling
- When pulling settings.json, merge with local (prefer remote for shared fields, keep local-only)

---

## Summary

| Phase | Priority | Effort | Blocks |
|-------|----------|--------|--------|
| 1. Security & Crashes | CRITICAL | 1 session | Nothing |
| 2. iOS App Store | CRITICAL | 1-2 sessions | Nothing |
| 3. Settings Migration | HIGH | 1 session | Phase 4 |
| 4. Git Sync Robustness | HIGH | 2 sessions | — |
| 5. Test Coverage | MEDIUM | 2 sessions | — |
| 6. Desktop Release | MEDIUM | 1-2 sessions | Phase 1 |
| 7. Polish | LOW | 2 sessions | All above |
