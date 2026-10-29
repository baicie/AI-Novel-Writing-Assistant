# P15.6 Code Review

## P1 — editor-b loaded app root but not chapter route

- Files: `src-tauri/src/commands/windows.rs`.
- Root cause: second window used `index.html?writer=...#/...`; main window uses app root + hash. The route did not mount in the second webview.
- Fix: use `#/novels/...?...` as `WebviewUrl::App` fragment, same HashRouter entry.
- Test: `window-command.spec.mjs`, `multiwindow.spec.mjs`; editor input works.
- Commit `01543406`.

## P1 — Writer identity tied to sessionStorage

- File: `useChapterDraft.ts`.
- Fix: `writer` query in hash gives stable `desktop-editor-b`; main falls back to sessionStorage.
- Test: two drafts independent; restart lists second writer draft.
- Commit `a035d43d`, `68375d33`.
- Risk: main writer identity remains session-scoped; only second window has stable identity.

## Verified inherited

- Native Confirm After Edit test exists and passes.
- Batch Unknown request count test PASS.
- Backup restore restart PASS.

## Remaining

- Batch Repair/Repair Exhausted Native.
- Native index restart, full WAL matrix, shared journey, performance, OS dialog.
