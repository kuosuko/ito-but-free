# GroqBara Development Guidelines

## What is this?
GroqBara is a macOS global-hotkey dictation/transcription app. User presses hotkey (or Fn key) → app records audio → sends to Groq Whisper API → auto-types result into the focused app via CGEventTap.

## Architecture
- **Frontend**: React + TypeScript (Vite), in `src/`
- **Backend**: Rust (Tauri v2), in `src-tauri/src/`
- **Platform layer**: `platform/{mod,macos,windows}.rs` — abstracts accessibility, Fn key listening, text injection
- **Recording**: `platform/macos.rs::RecordingSession` — cpal-based mic capture via platform abstraction (runs in dedicated thread because cpal::Stream isn't Send on macOS)
- **Transcription**: `transcribe.rs` — Groq API (Whisper + optional LLM refinement)
- **Settings**: `settings.rs` — JSON file in app config dir

## Key macOS Constraints
- **Accessibility permission** (`AXIsProcessTrustedWithOptions`): Required for both CGEventTap (Fn key listening) and auto-typing. After granting, user must **Cmd+Q restart** the app for CGEventTap to work.
- **Fn key**: Detected via `CGEventTap` watching `flagsChanged` events for the `0x800000` secondary Fn flag.
- **Auto-type**: Uses `CGEventCreateKeyboardEvent` + `CGEventKeyboardSetUnicodeString` to post unicode chunks. Chunks must NOT be dedup-filtered (a previous bug silently dropped repeated text patterns).

## Working Rules
1. **Small steps**: Each change should be buildable and commitable independently.
2. **Build check**: Run `npm run build` (frontend) and `cargo check` in `src-tauri/` (backend) before committing.
3. **Zero warnings**: Keep compiler warnings at zero. Current state: 0 warnings.
4. **Test with `cargo tauri dev`** when possible for runtime verification.
5. **Don't break the hotkey/recording pipeline** — it's the core UX loop.

## Known Pitfalls
- Tauri command names must match exactly between Rust (`#[tauri::command]` fn name) and TypeScript (`invoke("name")`). A mismatch silently fails.
- Settings are loaded from disk on every read (no in-memory cache) — acceptable for the current scale but would need caching if called frequently.
- Sidebar inputs use local state (for edit-on-blur). The `useEffect` sync pattern keeps them in sync with async prop updates.
- The `recording_state` event drives the three-state UI (idle/recording/processing). Both backend and frontend emit these for consistency.
