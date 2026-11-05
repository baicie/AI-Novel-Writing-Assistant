-- P17.4.1：伏笔 As-Of 折叠按 plot_id 取最近事件，补覆盖索引。
CREATE INDEX IF NOT EXISTS plot_thread_event_plot
  ON plot_thread_event(plot_id, chapter_order, created_at);
