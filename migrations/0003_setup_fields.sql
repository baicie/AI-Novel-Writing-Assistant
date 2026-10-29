-- 0003: 阶段 2 setup 模块字段扩展
-- 在原有的 novel/world_fact/character 等聚焦章节生产的 schema 上，补齐 setup 流程需要的基础字段
-- 与三张查表（world / genre / story_mode）。
--
-- 范围：novel base 字段、world 实体、查表。
-- 不在范围：character 子结构（dynamics/influence/mind 等）放到 0004；world 子结构
-- （axioms / layers / structure / generation snapshots）放到 0005。

ALTER TABLE novel ADD COLUMN narrative_form TEXT NOT NULL DEFAULT 'long_novel';
ALTER TABLE novel ADD COLUMN target_audience TEXT NOT NULL DEFAULT '';
ALTER TABLE novel ADD COLUMN book_selling_point TEXT NOT NULL DEFAULT '';
ALTER TABLE novel ADD COLUMN competing_feel TEXT NOT NULL DEFAULT '';
ALTER TABLE novel ADD COLUMN first_30_chapter_promise TEXT NOT NULL DEFAULT '';
ALTER TABLE novel ADD COLUMN commercial_tags_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE novel ADD COLUMN genre_id TEXT;
ALTER TABLE novel ADD COLUMN primary_story_mode_id TEXT;
ALTER TABLE novel ADD COLUMN secondary_story_mode_id TEXT;
ALTER TABLE novel ADD COLUMN world_id TEXT;
ALTER TABLE novel ADD COLUMN writing_mode TEXT NOT NULL DEFAULT 'original';
ALTER TABLE novel ADD COLUMN project_mode TEXT NOT NULL DEFAULT 'ai_led';
ALTER TABLE novel ADD COLUMN creation_experience TEXT NOT NULL DEFAULT 'simple';
ALTER TABLE novel ADD COLUMN narrative_pov TEXT;
ALTER TABLE novel ADD COLUMN pace_preference TEXT;
ALTER TABLE novel ADD COLUMN style_tone TEXT NOT NULL DEFAULT '';
ALTER TABLE novel ADD COLUMN emotion_intensity TEXT;
ALTER TABLE novel ADD COLUMN ai_freedom TEXT;
ALTER TABLE novel ADD COLUMN default_chapter_length INTEGER;
ALTER TABLE novel ADD COLUMN estimated_chapter_count INTEGER;
ALTER TABLE novel ADD COLUMN project_status TEXT NOT NULL DEFAULT 'not_started';
ALTER TABLE novel ADD COLUMN storyline_status TEXT NOT NULL DEFAULT 'not_started';
ALTER TABLE novel ADD COLUMN outline_status TEXT NOT NULL DEFAULT 'not_started';
ALTER TABLE novel ADD COLUMN resource_ready_score INTEGER NOT NULL DEFAULT 0;

CREATE TABLE world (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  overview_summary TEXT NOT NULL DEFAULT '',
  source_template_id TEXT,
  owner_novel_id TEXT REFERENCES novel(id),
  status TEXT NOT NULL DEFAULT 'draft',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX world_owner_novel ON world(owner_novel_id);

CREATE TABLE genre (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  description TEXT NOT NULL DEFAULT '',
  is_active INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE story_mode (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  description TEXT NOT NULL DEFAULT '',
  is_active INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0
);

-- 查表初始值：避免前端一上来就拿到空列表。后续内容是 build-time seed 补，迁移只兜底结构。
INSERT INTO genre(id, name, slug, description, sort_order) VALUES
  ('genre-fantasy', '玄幻', 'fantasy', '以修炼、等级、异能为核心的长篇幻想题材。', 10),
  ('genre-urban', '都市', 'urban', '以现代都市生活为背景的题材。', 20),
  ('genre-history', '历史', 'history', '以历史背景与人物命运为线索的题材。', 30),
  ('genre-scifi', '科幻', 'scifi', '围绕未来科技与文明冲突展开的题材。', 40),
  ('genre-romance', '言情', 'romance', '以情感关系为主线的题材。', 50),
  ('genre-mystery', '悬疑', 'mystery', '以悬念与解谜推动情节的题材。', 60);

INSERT INTO story_mode(id, name, slug, description, sort_order) VALUES
  ('mode-growth', '成长升级', 'growth', '主角由弱到强、以修炼/技能/职级推进的叙事节奏。', 10),
  ('mode-face-slap', '打脸反转', 'face-slap', '以身份落差和反转爽感为驱动力的叙事节奏。', 20),
  ('mode-mystery', '悬疑揭秘', 'mystery', '以谜题、伏笔和真相揭露推进的叙事节奏。', 30),
  ('mode-romance', '情感推进', 'romance', '以关系变化和情感冲突推进的叙事节奏。', 40),
  ('mode-power', '权谋博弈', 'power', '以势力对抗、策略与政治博弈推进的叙事节奏。', 50);
