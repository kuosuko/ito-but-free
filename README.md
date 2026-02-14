<div align="center">
  <img src="assets/icon.png" alt="GroqBara icon" width="120" />

  <h1>GroqBara (ito-but-free)</h1>

  <p>
    A cross-platform speech-to-text app — Ito, but BYOK (bring your own key). Free.
  </p>

  <p>
    <strong>macOS</strong> &nbsp;·&nbsp; <strong>Windows</strong>
  </p>
</div>

## Features

- **Push-to-talk / hotkey trigger** — Hold or toggle a global hotkey to record
- **Fn key mode** (macOS) — Use the Fn key as the trigger
- **Groq Whisper transcription** — Fast cloud-based speech-to-text
- **Auto-type** — Automatically types the result into the focused app
- **LLM refinement** — Optionally refine transcripts with a Groq-hosted LLM
- **Mic gain boost** — Amplify quiet microphone input (0.5x–3.0x)
- **Floating recording indicator** — Always-on-top overlay shows recording/transcribing status
- **System tray** — Control recording from the tray icon

## Getting Started

1. Get a free API key from [Groq Console](https://console.groq.com/)
2. Download the latest release for your platform
3. Paste your API key in Settings
4. Press the hotkey (default: `Ctrl+Space` on Windows, `F13` on macOS) and speak

## Development

```bash
pnpm install
pnpm tauri dev
```

## Build

```bash
pnpm tauri build
```

Produces:
- **macOS**: `.dmg` and `.app` bundle
- **Windows**: `.msi` and `.exe` (NSIS) installer

## Platform Details

### Architecture

```
src/                    # React + TypeScript frontend
src-tauri/src/
├── lib.rs              # Tauri commands, hotkey registration, tray menu
├── settings.rs         # JSON settings persistence
├── transcribe.rs       # Groq API (Whisper + LLM refinement)
└── platform/
    ├── mod.rs          # Platform trait + facade
    ├── macos.rs        # macOS: CGEventTap, AX APIs, CoreAudio
    └── windows.rs      # Windows: WH_KEYBOARD_LL, SendInput, WASAPI
```

### Platform Trait

| Capability | macOS | Windows |
|---|---|---|
| Accessibility check | `AXIsProcessTrusted` | No-op (always granted) |
| Fn/trigger key listener | `CGEventTap` (Fn flag) | `WH_KEYBOARD_LL` (Right Alt / F24) |
| Text injection | `CGEventCreateKeyboardEvent` | `SendInput` + `KEYEVENTF_UNICODE` |
| Audio capture | `cpal` (CoreAudio) | `cpal` (WASAPI) |

### macOS Notes

- **Accessibility permission** required for Fn key listening and auto-type. After granting, **Cmd+Q restart** the app.
- Default hotkey: `F13`

### Windows Notes

- No accessibility permission needed
- Default hotkey: `Ctrl+Space`
- Fn key mode uses Right Alt (`VK_RMENU`) or F24 as the trigger key

---

## License
MIT
