# P15.4 Code Review

## P1 — No real batch Unknown request-count harness

- No hanging HTTP provider, no cross-process request counter; cannot prove no automatic resend.
- Status BLOCKED; no code change.

## P2 — Reject path now Native-tested

- `batch-reject.spec.mjs` creates 3 chapters, rejects chapter 1 candidate, asserts Batch does not open chapter 2/3 and chapter 1 has no official revision.
- Commit `862c61db`.
- Residual: no Native regenerate/repair continuation.

## P2 — Tauri WDIO single-window limitation

- Probe: handles `['main']`; `newWindow` returns `main`; no second label.
- Impact: real two-window Native automation unavailable with current driver. Backend CAS tests remain; not a product defect proof.

## Verified inherited

- Native backup restore restart (P15.3).
- Batch open→bind real SIGKILL and Index worker SIGKILL/FTS (P15.2).
