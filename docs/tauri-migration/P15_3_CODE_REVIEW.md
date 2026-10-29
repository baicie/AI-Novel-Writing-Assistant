# P15.3 Code Review

## P1 — Restore request had no real restart proof

- Fix: two-phase Native E2E creates backup, alters data, requests restore, SIGKILLs the app, starts a new process on the same data dir; startup applies staged DB before SQLite/Runtime; chapter content and protection backup verified.
- Commit: `f3d49d61`.

## P2 — Tauri WDIO exposes only one window

- Probe: `getWindowHandles()` => `['main']`; `newWindow` returns `{handle:'main'}` and handles unchanged.
- Impact: real two-window Native E2E cannot be implemented with current driver. Two-window writer safety remains backend-only.
- No production code defect proven; it is a test-driver capability blocker.

## P2 — Batch Unknown/Reject/Repair

- No hanging mock provider with request counter, no Batch Native reject/repair test.
- Existing single-chapter workflows unchanged.

## P2 — OS Save Dialog manual

- Production uses Rust-side Tauri dialog and atomic writer; real OS interaction remains MANUAL_NOT_RUN.
