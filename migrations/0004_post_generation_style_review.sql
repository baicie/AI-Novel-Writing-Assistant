-- 0004: 保留“生成后风格复审”开关，避免桌面迁移时静默丢失作者设置。
ALTER TABLE novel ADD COLUMN post_generation_style_review_enabled INTEGER NOT NULL DEFAULT 0;
