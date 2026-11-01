/**
 * 章节编辑器的 Diff 分片只有一套实现，放在 shared 里。
 *
 * 原因：`applySelectedChapterEditorDiff` 在采纳候选时会校验分片能逐字还原原文与候选文本。
 * 如果云端和桌面端各自实现一套分词/LCS，两边产生的分片就无法互相校验，
 * 作者在一端生成、在另一端采纳时会直接报「对比不完整」。
 */
export { buildChapterEditorDiffChunks } from "@ai-novel/shared/types/chapterEditor/document";
