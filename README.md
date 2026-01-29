<div align="center">
  <img src="assets/icon.png" alt="ito-but-free icon" width="120" />

  <h1>ito-but-free</h1>

  <p>
    A macOS speech-to-text app — Ito, but BYOK (bring your own key). Free.
  </p>

  <p>
    <em>
      README drafted by a robot butler (Clawdbot) under strict instructions.
      If anything looks too confident, it was probably hallucinating.
    </em>
  </p>
</div>

## Status
Early release / WIP.

## Features
- Push-to-talk / hotkey trigger (including Fn-key mode)
- Transcribe speech → text
- Optional auto-type (requires macOS Accessibility permission)

## Development

```bash
pnpm install
pnpm tauri dev
```

## Build

```bash
pnpm tauri build
```

Note: DMG bundling may fail on some setups; the signed `.app` bundle can still be zipped and shared.

---

## Platform Abstraction Layer

The app uses a platform abstraction layer (`src-tauri/src/platform/`) to isolate OS-specific code. This makes it feasible to add Windows (or other platform) support in the future.

### Architecture

```
src-tauri/src/platform/
├── mod.rs      # Platform trait + facade (current() function)
├── macos.rs    # macOS implementation (CGEventTap, AX APIs)
└── windows.rs  # Windows stubs with TODO notes
```

The `Platform` trait provides:

| Capability | Description | macOS API | Windows API (TODO) |
|------------|-------------|-----------|-------------------|
| `is_accessibility_trusted()` | Check if app has permissions | `AXIsProcessTrusted` | N/A (generally allowed) |
| `request_accessibility_permission()` | Prompt for permissions | `AXIsProcessTrustedWithOptions` | N/A |
| `start_fn_key_listener()` | Listen for Fn key press/release | `CGEventTap` | `SetWindowsHookEx(WH_KEYBOARD_LL)` |
| `type_text()` | Inject text into focused app | `CGEventCreateKeyboardEvent` | `SendInput` with `KEYEVENTF_UNICODE` |
| `start_audio_capture()` | Record from microphone | `cpal` (CoreAudio) | `cpal` (WASAPI) |

### Usage

```rust
use crate::platform;

let p = platform::current();
if p.is_accessibility_trusted() {
    p.type_text("Hello!", Duration::ZERO)?;
}
```

### Windows Implementation Guide

To add Windows support, implement the stubs in `src-tauri/src/platform/windows.rs`:

1. **Key Listening**: Use `SetWindowsHookEx` with `WH_KEYBOARD_LL` and a message pump thread.
2. **Text Injection**: Use `SendInput` with `INPUT_KEYBOARD` and `KEYEVENTF_UNICODE`.
3. **Audio Capture**: The `cpal` crate should work—may need `CoInitializeEx` on the recording thread.
4. **Suggested crate**: [`windows`](https://crates.io/crates/windows) (official Microsoft bindings).

See `windows.rs` for detailed implementation notes and code snippets.

---

## License
MIT
