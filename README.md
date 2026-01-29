# ito-but-free

A macOS speech-to-text app — Ito, but BYOK (bring your own key). Free.

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

## License
MIT
