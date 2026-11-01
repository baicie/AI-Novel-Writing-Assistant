# 章节作者工作台（P17）

## Background

章节编辑器原本只解决「写这一章」：Plate 编辑器 + 选区 AI + 候选 Diff + 诊断 + 本地草稿。整本小说的导航、运行状态与恢复入口散落在其他页面，作者要来回跳。

P17 的目标不是重写编辑器，而是让它成为整本小说的统一作者工作台。

## Decision

1. 编辑器、选区 AI、光标续写、候选 Diff、诊断、本地草稿、版本 CAS **全部复用**，不引入第二套编辑器引擎。
2. 左侧新增「整本小说章节导航」，只消费已加载的 `novel detail.chapters`，不新增查询。
3. 右侧把原有 `ChapterEditorDirectorPanel` 提升为四页签工作区：AI 修改 / 审校问题 / 人物与世界观 / 运行与恢复。
4. 「运行与恢复」的动作**严格取自后端 `legalActions`**，未识别的动作直接忽略；前端不得臆造入口，也不得直接改数据库跳状态。
5. `legalActions` 是**语义意图**，不是命令名。后端只回答「这个状态下作者可以做什么」，不回答「该调哪个命令」。前端必须再做一层「意图 → 真实 Tauri 命令」的翻译，翻译结果由门点类型和候选稿是否就绪共同决定；按动作名直连同名命令会丢掉正文冲突保护。
6. 运行面板是**章节级**视图，不是书级视图。作者在第三章的编辑器里看到的运行必须是第三章自己的运行；本章没有运行就给空态并引导去全书运行记录，不回退到「全书最近一次运行」。

## Current Rule

- 章节切换统一走 `shouldBlockChapterSwitch(isDirty, confirm)`：有未保存修改必须先得到用户明确同意；按钮与 `Alt+←/→` 共用同一套判定。
- 章节导航上的「待处理 / 生成中」标记来自 `buildRunAttention(listRuns(novelId))`：按章节取最近一次 Run，只有 `waiting_approval / paused / failed` 才标为待处理。
- 人物与世界观面板只读现有数据，不生成新的记忆事实。
- 「AI 修改」页签内的组件与状态保持原样，改动只能通过 Shell 传入的 props。

### 选区改写：两条运行环境，一条 diff 实现

影响范围：章节编辑器选区 AI 改写、光标续写、候选 Diff 与部分采纳、桌面端 axios 适配层。

选区改写是「局部改写」，与整章导演流水线是两件事，不能互相替代：

| | 选区改写 | 整章导演流水线（`open_run`） |
|---|---|---|
| 入口 | `rewrite_chapter_selection` | `open_run` |
| 是否建 Run | 否 | 是 |
| 是否写候选表 / 动正文 | 都不 | 都会 |
| 产出 | 候选文本，交回编辑器 | 章节草稿 + 门点 |
| 作者如何采纳 | Diff 里逐处挑选 | 门点确认整章 |

桌面端与网页端走不同实现，由 `hasDesktopShell()` 判定，**不是**由用户偏好或 localStorage 开关判定：

- 网页端：`POST /novels/:id/chapters/:cid/editor/ai-revision-preview`；
- 桌面端：`rewrite_chapter_selection` IPC 命令。桌面的 axios 适配层没有改写路由，继续发 HTTP 只会落到 `unsupported()`，用户看到「这个功能暂未接入桌面本地内核」。

**diff 必须只有一份实现**，位于 `shared/types/chapterEditor/document.ts` 的 `buildChapterEditorDiffChunks`。原因是采纳路径 `applySelectedChapterEditorDiff` 会按分片重建原文与候选，两者不一致就整段抛「候选与原文的对比不完整」。服务端与桌面端各写一套分词或分片规则，会在中文标点、空白、英文单词边界上产生细微差异，表现为「AI 改写完但采纳不了」。该文件的单测锁住了这条往返契约。

职责切分：Rust 侧只做受控模型调用并返回候选文本，候选 id 与 diff 由前端用共享实现生成。这样 diff 只有一份，`src-tauri` 也不必引入 uuid 与一套等价的 LCS。

坐标约定：编辑器坐标是 **UTF-16 偏移**，Rust 字符串是 UTF-8。`rewrite_chapter_selection` 用 `slice_utf16` 按 UTF-16 取子串再与 `selectedText` 逐字比对，中文正文下按字节切会错位。前后端都要在发请求前确认选区仍指向作者看到的那段文字，对不上就报错重选，绝不猜测位置。

### 运行动作到后端命令的映射

影响范围：章节作者工作台「运行与恢复」页签、运行恢复入口、前端运行状态投影。

后端按运行控制状态给出 `legal_actions`，取值集合是封闭的：

| 运行状态 | 后端 `legal_actions` |
|---|---|
| `waiting_approval` 且门点已打开 | `confirm` / `reject` / `regenerate` / `confirm_after_edit` |
| `paused`（安全停点） | `cancel`；质量修复上限停点且质量提醒都已处理时追加 `resume_quality_debt` |
| `paused`（普通暂停） | `resume` / `cancel` |
| `running` / `queued` | `cancel` |
| 其他（成功 / 失败 / 已取消） | 空 |

前端把这些意图翻译成真实 Tauri 命令。翻译表是契约本身，不是实现细节：

| 后端意图 | 条件 | 渲染动作 | 真实 Tauri 命令 |
|---|---|---|---|
| `confirm` | 章节确认门点（`gateId === "approval"`）且候选稿就绪 | 确认候选正文 | `confirm_candidate_after_edit_run` |
| `confirm` | 其他门点，或候选稿未就绪 | 确认这一步 | `approve_run` |
| `confirm_after_edit` | 候选稿就绪 | 确认候选正文（与上面合并，不重复渲染） | `confirm_candidate_after_edit_run` |
| `confirm_after_edit` | 候选稿未就绪 | **不渲染** | — |
| `reject` | — | 退回重写 | `reject_run` |
| `regenerate` | — | 重新生成 | `regenerate_run` |
| `resume` | — | 继续这次创作 | `resume_run`（`acceptCandidate: false`） |
| `resume_quality_debt` | — | 接受质量提醒并继续 | `resume_run_with_quality_debt`（必须带 reason） |
| `cancel` | — | 取消这次创作 | `cancel_run` |
| 未识别的字符串 | — | **不渲染** | — |

硬规则：

1. **候选正文确认必须走候选 CAS 命令。** 调用必须同时携带 `baseCandidateId` + `baseContentSha256` + `content` + `expectedVersion`，不得用 `approve_run` 绕过。原因在存储层：`confirm_candidate_after_edit_run` 会把 `baseCandidateId` / `baseContentSha256` 与库里最新候选比对，不一致就返回冲突（「候选正文已经变化或已被其他窗口修改」）。`approve_run` 只校验 `expectedVersion`（运行控制版本），对正文内容没有任何比对——另一个窗口或自动导演刚写入的新候选会被静默放过，作者确认的是自己看到的旧正文，落库的是别人的新正文。`expectedVersion` 保护的是运行进度，`contentSha256` 保护的是正文本身，两者不能互相替代。
2. **`confirm_after_edit` 在没有候选稿时不渲染。** 后端在 `waiting_approval` 下无条件把四个动作都列出来，并不检查候选稿是否已经落库；而候选 CAS 命令在没有候选时必定返回冲突。渲染它等于给作者一个点了必然失败的按钮。「候选稿就绪」的判定是 `id` + `contentSha256` + 非空 `content` 三者齐备，缺一个就按未就绪处理。
3. **`regenerate` 不得退化成 `resume_run`。** 两者语义相反：`regenerate_run` 走 `decide_gate` 的重做分支，丢弃当前草稿重新生成；`resume_run` 接着当前进度往下走。混用会让作者以为重写了，实际把不想要的草稿推进到了下一步。

未识别动作一律不渲染，而不是降级到某个「最接近」的命令。后端新增动作时，前端的正确行为是少一个按钮（作者可以去运行记录处理），而不是多一个会失败或语义错误的按钮。

### 运行面板与章节的隔离

影响范围：章节作者工作台右侧面板、前端查询缓存键、运行动作的启用条件。

运行是章节级实体（`RunSummary.chapterId`），面板必须全程保持这个归属：

- **只消费本章运行。** 从 `listRuns(novelId)` 里按 `run.chapterId === 当前章节` 过滤，再按 `updatedAt` 取最近一次。过滤结果为空时给明确空态（引导去「AI 修改」发起，或去全书运行记录查看），**不回退到全书最新运行**。回退会让作者在 A 章界面上操作 B 章的运行，而界面上没有任何提示说当前这个运行不属于本章。
- **查询 key 必须包含 chapterId，且 novelId 必须在 chapterId 之前。** 详情 key 是 `["authoring-run-detail", novelId, chapterId, runId]`。chapterId 不进 key 时，快速切章会命中上一章的缓存条目，面板先闪出别章的状态与动作。顺序也是契约的一部分：动作成功后按 `["authoring-run-detail", novelId]` 前缀失效缓存，依赖 novelId 位于第二位；把 chapterId 挪到前面，前缀失效会静默失配，作者操作后看到的仍是旧状态。
- **详情必须双重匹配才允许驱动界面。** 只有 `detail.chapterId === chapterId && detail.id === latest.id` 同时成立，详情才用于显示状态、理由、用量，并计算可执行动作；否则退回列表态（只显示 `RunSummary` 里的状态与模型）并把动作清空。只比对 `detail.id` 不够：切章后 runId 变了，但旧章节的在途响应可能先到达并落进新章节的渲染路径。动作在不匹配时必须为空数组而不是「暂时禁用」——此时 `expectedVersion` 来自别章详情，带着它发命令就是拿错版本号去改另一个运行。

## Examples

- 有未保存修改时点「下一章」→ 弹确认；拒绝则停留，接受才导航；未保存内容仍可从本地草稿恢复。
- 某章 Run 处于 `paused` → 导航里该章显示「待处理」；进入「运行与恢复」只显示后端允许的动作。
- 运行停在章节确认门点（`gateId === "approval"`）、候选稿已落库 → 后端给出 `confirm` / `reject` / `regenerate` / `confirm_after_edit` 四个意图，面板渲染三个按钮：「确认候选正文」（`confirm` 与 `confirm_after_edit` 合并，走 `confirm_candidate_after_edit_run`）、「退回重写」、「重新生成」。
- 同一个门点、候选稿还没落库 → `confirm` 降级为 `approve_run`（「确认这一步」），`confirm_after_edit` 不渲染。作者看到的按钮少了一个，但每个按钮都能真的执行。
- 运行停在规划阶段的门点（`character_cast` / `volume_strategy`，这两步同样 gateable）→ 没有章节候选正文，`confirm` 走 `approve_run`。
- 质量修复上限停点、审校留下的问题都已逐条处理 → 后端追加 `resume_quality_debt`，面板显示「接受质量提醒并继续」，调用必须带 reason。问题尚未处理完时后端只给 `cancel`，面板也就只有「取消这次创作」。
- 本章从未发起过运行 → 面板显示空态（`data-testid="run-empty"`）并引导去「AI 修改」发起或去全书运行记录查看，不显示任何动作按钮。
- 在第三章按 `Alt+→` 连续切到第五章，第三章的详情响应晚到 → `detail.chapterId` 不等于当前章节，面板退回列表态、动作清空，晚到的响应不参与渲染。

## Failure Modes

- 在导航里自行拼接 Run 状态 → 用户看到不存在的进度，属于禁止行为。
- 把新的业务逻辑继续塞进 781 行的 `ChapterEditorShell` → 应下沉到 `navigation/`、`rightPanel/` 目录下的独立模块。
- 章节切换不检查未保存状态 → 静默丢内容，属于 P0。

动作映射相关：

- **用 `approve_run` 确认候选正文** → 覆盖他人正文。`approve_run` 不比对 `contentSha256`，另一个窗口或自动导演在作者阅读期间写入的新候选会被静默放过；作者按下的是「确认我看到的这一版」，落库的是没看过的那一版，且没有冲突提示。这是不可逆的数据损失，不是显示问题。
- **把 `confirm_after_edit` 直接渲染成按钮** → 候选稿还没落库时，点下去必定撞上「没有可编辑的候选正文」冲突。对本产品的目标用户（写作新手）尤其有害：他们无法区分「我操作错了」「AI 失败了」「这个按钮本来就不该在这儿」，只会认为产品坏了。
- **把 `regenerate` 映射到 `resume_run`** → 作者要求重写，系统却把不想要的草稿推进到了下一步，且运行状态显示「正常继续」。
- **给未识别动作找一个最接近的命令** → 后端语义变更会在前端表现为语义错误的操作，而不是可见的功能缺失。

选区改写相关：

- **在桌面端继续走 HTTP 改写路由** → 适配层没有这条路由，作者点「AI 优化这段」只会看到「这个功能暂未接入桌面本地内核」。对写作新手是「产品坏了」，不是「这里还没做」。
- **把选区改写接到 `open_run`** → 那是整章流水线：它会建 Run、写候选表、推进门点，并且忽略选区，拿回来的是整章草稿。作者只想改一段话，结果整章被重写并进入待确认状态。
- **服务端与桌面端各写一套 diff** → 分片无法重建原文或候选，采纳时整段抛「候选与原文的对比不完整」。表现是「AI 改写成功但点不动采纳」，查起来很费时间，因为两边单独看都对。
- **用字节偏移处理选区** → 中文正文下选区错位，候选被套用到错误的位置，且不一定报错。
- **选区漂移时猜测作者想改哪一段** → 把改写结果写到错误位置。坐标与原文对不上时必须要求重新选中。

章节隔离相关：

- **本章无运行时回退到全书最新运行** → 作者在第三章的界面上确认、退回或取消了第七章的运行。界面上下文全部指向第三章，没有任何线索提示操作对象是另一章。
- **查询 key 不含 chapterId** → 快速切章时面板先渲染上一章的状态与动作按钮。在这个短窗口里按下的动作，打到的是上一章的运行。
- **详情只比对 `detail.id`、不比对 `chapterId`** → 旧章节的在途响应污染新章节的渲染路径；更危险的是 `expectedVersion` 取自别章详情，带着它发命令等于拿错版本号去改另一个运行，而乐观锁校验可能恰好通过。
- **详情不匹配时把动作「禁用」而不是清空** → 禁用只是视觉状态，渲染路径仍然持有一份来自别章的动作列表与版本号，后续状态变化容易把它重新启用。

## Related Modules

- `client/src/pages/novels/NovelChapterEdit.tsx`
- `client/src/pages/novels/components/chapterEditor/ChapterEditorShell.tsx`
- `.../chapterEditor/navigation/`（章节导航）
- `.../chapterEditor/rightPanel/`（统一工作区）
- `.../chapterEditor/rightPanel/panelModel.ts`（`availableRunActions`：意图 → 命令的唯一翻译点）
- `.../chapterEditor/rightPanel/RunRecoveryTab.tsx`（章节隔离、查询 key、命令参数装配）
- `.../chapterEditor/document/useChapterDraft.ts`（本地草稿）
- `client/src/platform/desktopTransport.ts`（Tauri 命令的参数契约）
- `src-tauri/src/commands/run.rs`（命令签名与可选 `expectedVersion`）
- `crates/novel-application/src/director.rs`（`legal_actions` 取值、`decide_gate`、候选确认入口）
- `crates/novel-storage-sqlite/src/workflow.rs`（候选 CAS 比对与冲突返回）
- `crates/novel-workflow/src/plan.rs`（gateable 步骤定义，gate id 的来源）
- `client/src/api/novel/chapters.ts`（`previewChapterAiRevisionWithRouting`：桌面 / 网页分流点）
- `client/src/api/novel/chapterSelectionRewriteModel.ts`（选区坐标校验、候选收敛、意图构造的纯逻辑）
- `shared/types/chapterEditor/document.ts`（`buildChapterEditorDiffChunks` 与 `applySelectedChapterEditorDiff`：唯一 diff 实现与采纳校验）
- `src-tauri/src/commands/editor.rs`（`rewrite_chapter_selection`：选区校验、受控路由、候选收敛）
- `crates/novel-prompt/src/lib.rs`（`chapter_selection_rewrite` 提示词资产）

## Source Documents

- `docs/design/P17-AUTHORING-EXPERIENCE-v2.md`
- `docs/design/P17-ARCHITECTURE-AND-INTERACTIONS.md`
- `docs/reviews/P17.0-IMPLEMENTATION-REVIEW.md`
- `docs/reviews/P17.0.1-CODE-REVIEW-CLOSURE.md`
