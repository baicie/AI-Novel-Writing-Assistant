#!/usr/bin/env python3
"""把作者修订稿写回验收 Run 的候选表（工具，不是产品路径）。

按产品 `confirm_candidate_after_edit` 的字段语义插入一条新的
`step_id='author_edit'` 候选，而不是覆盖原候选：
- 原始 draft / bounded_repair 候选保持不动，验收证据仍可复核；
- 不触碰 gate、workflow_control、章节正文与 artifact；
- 只追加一条 checkpoint_saved 事件，便于在 Run 记录里看到这次编辑。

守卫：写入前必须与「当前最新候选的 id 与 sha256」完全一致，否则拒绝写入，
防止覆盖其他窗口或后续运行产生的候选。

用法：
  python3 -I writeback-author-edit-candidate.py \\
      <db> <run_id> <revised.md> <new_candidate_id> <expected_latest_id> <expected_latest_sha>
"""
import datetime
import hashlib
import pathlib
import sqlite3
import sys

db_path, run_id, revised_path, new_id, expect_id, expect_sha = sys.argv[1:7]
content = pathlib.Path(revised_path).read_text()
if not content.strip():
    raise SystemExit("修订稿为空，拒绝写入")
content = content.rstrip("\n")

sha = hashlib.sha256(content.encode()).hexdigest()
now_ms = int(datetime.datetime.now().timestamp() * 1000)

con = sqlite3.connect(db_path)
con.execute("PRAGMA foreign_keys = ON")
cur = con.cursor()

# 1. 确认最新候选仍是预期的那条（防止覆盖别人的编辑）
row = cur.execute(
    "SELECT id, content_sha256, chapter_id FROM chapter_candidate "
    "WHERE run_id = ? ORDER BY updated_at DESC, rowid DESC LIMIT 1",
    (run_id,),
).fetchone()
if row is None:
    raise SystemExit("没有找到候选，拒绝写入")
latest_id, latest_sha, chapter_id = row
if latest_id != expect_id or latest_sha != expect_sha:
    raise SystemExit(f"最新候选已变化：{latest_id} / {latest_sha}，拒绝写入")
if cur.execute("SELECT 1 FROM chapter_candidate WHERE id = ?", (new_id,)).fetchone():
    raise SystemExit("目标候选 id 已存在，拒绝写入")

# 2. 插入 author_edit 候选（不改动原有记录）
cur.execute(
    "INSERT INTO chapter_candidate"
    "(id, chapter_id, run_id, step_id, content, content_sha256, interrupted, created_at, updated_at)"
    " VALUES (?, ?, ?, 'author_edit', ?, ?, 0, ?, ?)",
    (new_id, chapter_id, run_id, content, sha, now_ms, now_ms),
)

# 3. 追加一条 checkpoint_saved 事件（与产品写入路径一致）
seq = cur.execute(
    "SELECT COALESCE(MAX(seq), 0) + 1 FROM workflow_event WHERE run_id = ?",
    (run_id,),
).fetchone()[0]
cur.execute(
    "INSERT INTO workflow_event(run_id, seq, kind, step_id, payload_json, occurred_at)"
    " VALUES (?, ?, 'checkpoint_saved', 'author_edit', ?, ?)",
    (
        run_id,
        seq,
        '{"candidateId":"%s","source":"manual-review-revision"}' % new_id,
        now_ms,
    ),
)

con.commit()
con.close()
print(f"WROTE candidate={new_id} sha={sha} seq={seq} chars={len(content)}")
