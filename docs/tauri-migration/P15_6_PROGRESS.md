# P15.6 Progress

Start HEAD: `cc4889e9`

| Gate | Status | Commit | Evidence |
| --- | --- | --- | --- |
| Editor-B Rendering | **VERIFIED** | 01543406 | real editor-b loads chapter editor, contenteditable accepts input, closes cleanly |
| Dual-writer Draft | **VERIFIED（核心）** | 68375d33 | A/B independent drafts; official CAS conflict; B draft retained; SIGKILL restart finds `desktop-editor-b` draft |
| Batch Repair/Confirm Edit | **PARTIAL** | inherited | Confirm After Edit Native existing PASS; Repair/Repair Exhausted Native not run |
| Native Index Restart | **PARTIAL** | inherited | backend SIGKILL/FTS PASS; Native two-process not rerun |
| Backup WAL Matrix | **PARTIAL** | inherited | Native restart restore PASS; full WAL boundary matrix not run |
| Shared Author Journey | **BLOCKED** | — | no single-fixture end-to-end |
| Desktop Performance | **NOT_STARTED** | — | no 100/500/1000 desktop metrics |
| Long Story Consistency | **NOT_STARTED** | — | no 30–50 chapter fixture |
| Migration / Save | **BLOCKED / PARTIAL** | inherited | early schema matrix and OS dialog manual |
| Final Production Audit | **PARTIAL** | — | final suite/build running |

## G1 root cause

`WebviewUrl::App("index.html?writer=...#/...")` was not the same entry URL as the main window; the app root loaded but the chapter route did not mount. Using a fragment URL on the app root (`#/novels/...?...`) makes the second window load the same HashRouter entry and render `NovelChapterEditor`.

## G2 evidence

- `multiwindow.spec.mjs`: editor-b input accepted; two drafts with client IDs `desktop-editor-b` and main writer; stale official save rejected; B draft remains.
- `run-tauri-multiwindow-draft-e2e.sh`: A/B drafts written, app SIGKILLed, new process lists `desktop-editor-b` draft.
- Writer identity uses `writer` query in the hash; sessionStorage remains fallback for single-window use.
