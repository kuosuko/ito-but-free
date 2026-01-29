### Priority 1: Enhance UI
- Goal: Transition the GroqTranscriber app to mimic a native macOS app in terms of look and feel.
- Planned Changes:
  - Typography: Ensure font type and rendering are consistent with macOS.
  - Color Palette: Utilize macOS-native colors and themes.
  - Spacing/Layouts: Introduce proper margins, paddings, and alignments.
  - Components: Replace HTML-like widgets with native-feel alternatives via libraries (e.g., Radix, Mantine).
  - Transitions: Implement subtle, macOS-style transitions for interactive elements.


### Priority 2: Typing Duplication Bug
- Identified File: auto_type.rs
- Observations:
  1. Text is chunked into 24-character U16 segments (`type_text` -> `post_unicode_chunk`).
  2. CGEvent mechanics (key down/up) could post unintended duplicates under specific scenarios (e.g., retries).
  3. The thread `sleep` mechanism may cause chunks to resend if delays interact poorly with the event lifecycle.

- Debugging Plan:
  1. Log chunk sends and validate chunk boundaries.
  2. Monitor the lifecycle of CGEvent and ensure all chunks are drained once.
  3. Investigate `per_chunk_delay` to account for delay-induced duplication.

- Fix Plan:
  1. Maintain state for already-typed chunks (prevent re-sends).
  2. Serialize typing process with guards to ensure exclusive execution.
  3. Evaluate and optimize `per_chunk_delay` handling.
