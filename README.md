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

### Windows Implementation Status

Initial Windows support is implemented via the platform layer in
`src-tauri/src/platform/windows.rs`:

1. **Key Listening**: Implemented with `SetWindowsHookExW(WH_KEYBOARD_LL, ...)` on a
   background thread with a message pump. The default trigger key is **Right Alt**
   (`VK_RMENU`), with **F24** supported as an alternative (for users who bind it
   via tools like AutoHotkey).
2. **Text Injection**: Implemented using `SendInput` with `INPUT_KEYBOARD` and
   `KEYEVENTF_UNICODE`, so arbitrary Unicode text can be auto-typed into the
   focused application.
3. **Audio Capture**: Implemented using the same `cpal`-based pipeline as macOS,
   writing a temporary WAV file that feeds the Groq transcription API.
4. **Crate**: Uses the official [`windows`](https://crates.io/crates/windows)
   bindings for Win32 APIs.

Limitations / Notes:
- There is currently **no UI to change the Windows trigger key**; it is hard-coded
  to Right Alt / F24. The global hotkey (default `F13`) remains fully configurable
  and is the recommended trigger mechanism on Windows.
- Unlike macOS, there is no explicit Accessibility permission prompt; the
  "Accessibility" checks in the app are effectively no-ops on Windows.

---

## License
MIT
