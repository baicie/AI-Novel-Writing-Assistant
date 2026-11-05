-- P17.4：伏笔生命周期与剧情因果
-- 只做增量扩展：既有 plot_thread 继续有效，新字段与事件表用于状态机与可追溯历史。

ALTER TABLE plot_thread ADD COLUMN version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE plot_thread ADD COLUMN last_development_chapter_order INTEGER;

CREATE TABLE IF NOT EXISTS plot_thread_event (
  id TEXT PRIMARY KEY,
  plot_id TEXT NOT NULL REFERENCES plot_thread(id),
  novel_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  chapter_id TEXT NOT NULL,
  chapter_order INTEGER NOT NULL,
  source_revision INTEGER NOT NULL,
  evidence TEXT NOT NULL,
  from_status TEXT,
  to_status TEXT NOT NULL,
  command_id TEXT NOT NULL UNIQUE,
  request_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS plot_thread_event_novel
  ON plot_thread_event(novel_id, chapter_order, created_at);

CREATE TABLE IF NOT EXISTS plot_causality_edge (
  id TEXT PRIMARY KEY,
  novel_id TEXT NOT NULL,
  from_kind TEXT NOT NULL,
  from_id TEXT NOT NULL,
  to_kind TEXT NOT NULL,
  to_id TEXT NOT NULL,
  relation TEXT NOT NULL,
  evidence TEXT NOT NULL,
  source_chapter_id TEXT NOT NULL,
  source_revision INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'confirmed',
  version INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE(novel_id, from_kind, from_id, to_kind, to_id, relation)
);
CREATE INDEX IF NOT EXISTS plot_causality_edge_novel
  ON plot_causality_edge(novel_id, status);
