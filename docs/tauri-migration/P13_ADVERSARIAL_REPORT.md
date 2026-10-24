# P13.3 Deterministic Adversarial Evaluation

## Dataset

- Dataset Version: `p13-adversarial-v1`
- Context Policy: `director-context-v1`
- Index Version: `knowledge-index-0009`
- Prompt Version: `chapter-draft-v2`
- Model: deterministic ScriptedLlm / local lexical-vector fixtures
- No real paid model

## Cases and metrics

| Case | Result |
| --- | --- |
| 同名不同人 | correct source at rank 1 |
| 冒充者 | correct source at rank 1 |
| 传闻“已死” | correct rumor source at rank 1 |
| 真实“存活” | correct fact source at rank 1 |
| 未来计划 owner | excluded from confirmed character facts |
| 地点事实 | 青阳城 correct source |
| 别名 | 青翼蝠王 correct source |
| 旧设定修改 | 西山新设定 correct source |
| 已故人物回忆 | 生前 correct source |
| Source revision | all returned hits carry sourceRevision |
| 旧章节 revision | old FTS/Vector content not returned |
| 删除知识 | no hits |
| 跨小说 | no leakage |
| Accepted 后问题再次出现 | 不静默重新 open |

```text
P13 adversarial: dataset=p13-adversarial-v1 cases=8 correct=8
recall=1.00 precision=0.88 mrr=0.94 ndcg=0.94
source_revision_accuracy=1.00 future_leak=0
```

## Quality debt identity

- 同一问题不同描述：相同稳定 key。
- 同一描述不同证据：不同稳定 key。
- sourceRef 变化：不同稳定 key。
- accepted 后再次出现：保持 accepted，不静默 open。

## What is NOT measured

- Recall@50/nDCG on a large annotated corpus.
- 50 章伏笔、不可靠叙述者、倒叙、旧设定修改的完整 adversarial set。
- Real model conflict discovery / false positive rate.
- Literature quality.

These must not be inferred from ScriptedLlm clean output.
