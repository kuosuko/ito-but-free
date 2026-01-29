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

## License
MIT
