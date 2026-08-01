# ForgeCAD Luna Goal 模式持续执行指南

版本：2026-08-01
状态：开发执行合同；不属于当前用户功能，不证明任何运行时能力
适用对象：使用 Luna 或其他长程开发模型持续推进 ForgeCAD 的维护者

## 1. 本文唯一负责什么

本文只负责“后续模型如何持续工作而不偏离产品真值”。它不重新定义当前能力、任务状态或架构：

- 产品边界以 [PRODUCT_DEFINITION.md](PRODUCT_DEFINITION.md) 为准；
- 当前状态以 [DOCUMENTATION_STATUS.md](DOCUMENTATION_STATUS.md) 为准；
- 当前任务以 [CODEX_TASK_INDEX.md](CODEX_TASK_INDEX.md) 为准；
- 当前工作区与已知失败以 [CODEX_HANDOFF.md](CODEX_HANDOFF.md) 为准；
- 目标架构以 [DESIGN.md](DESIGN.md) 和 Accepted ADR 为准；
- 当前用户可用能力只以 [USER_GUIDE.md](USER_GUIDE.md) 为准。

Luna 是开发执行者，不是 ForgeCAD 产品中的 Provider、Skill、几何执行器或资产真值。后续可以更换开发模型，本文约束仍然有效。不得因为开发模型擅长长程推理，就把它的文字判断当成代码证据、视觉评分、真实 Provider 成功或产品发布证明。

## 2. 持续 Goal 的唯一总目标

建议在 Goal 模式中使用下面这一个总目标，不要同时建立多个竞争 Goal：

> 在小团队预算内，把 Forge Studio 收敛为轻量、类别开放、高自由度、参考图条件化、外观优先、可编辑的通用 3D Agent：当前先完成 U004 的 DeepSeek 会话/缓存、通用表示、Appearance Compiler 和无遮挡高质量工作台，稳定展示唯一 3D 结果；之后再进入 U005 跨类别真实输入、时间/成本和真人质量门。

2026-08-01 路线覆盖：必须先完整阅读 ADR-0022，再读 ADR-0021/0020 中仍有效的 typed program、1+1 和成本边界。U001、U001A、U002、U003 与 U004A 已完成；`FGC-U004` 是唯一 `in_progress` 父任务，详细第一阶段合同位于 [U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md](U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md)。C111B/E005 已冻结为 procedural hard-surface 回归，其未完成 reference/human 事实不变；任何把它们写成通用质量通过的内容都错误。

Goal 是持续方向，不是完成状态。只有 `CODEX_TASK_INDEX.md` 中当前父任务退出，才可以推进 U005。父任务层面，任何一轮只允许一个任务为 `in_progress`。用户已明确要求四个 Luna 执行 U004；允许 W1–W3 在独立 worktree、互斥文件范围内并行，W4 单一集成，但父任务仍只有 U004 一个 `in_progress`。

## 3. 每次续跑的强制启动协议

Luna 每次收到自动续跑或新上下文后，先执行：

```bash
git status -sb
git diff --check
git rev-parse --short HEAD
```

然后按固定顺序完整阅读：

1. `AGENTS.md`；
2. `docs/DOCUMENTATION_MAP.md`；
3. `docs/DOCUMENTATION_STATUS.md`；
4. `docs/CODEX_HANDOFF.md`；
5. `docs/CODEX_EXECUTION_PLAN.md`；
6. `docs/CODEX_TASK_INDEX.md`；
7. `docs/AUTHORITATIVE_STATE.md`；
8. `docs/USER_GUIDE.md`；
9. `docs/DESIGN.md`；
10. 当前任务卡列出的合同、Schema、fixture、实现和测试。

U004 四 Luna 还必须把 `docs/U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md` 放在第 7 项之后完整阅读；每个 Luna 只执行其中自己的 W1/W2/W3/W4 卡。

续跑时不得从聊天摘要直接推断仓库现状。若文档与代码冲突，先用只读命令确定事实，再修复权威文档；不得选择更乐观的说法。

### 脏工作区规则

- 所有既有修改默认属于用户或并行工作；
- 不运行 `git reset --hard`、`git checkout --` 或广域删除；
- 先用 `git diff -- <path>` 判断重叠，再做最小补丁；
- 与当前任务无关的失败记录为 `KNOWN FAIL`，不得借机重构；
- 需要覆盖同一文件的并行大改时，停止并报告精确冲突，而不是猜测合并意图。

### 四个 Luna 的 worktree 协议

- 四个 Luna 从同一 baseline commit 建立 `codex/u004-provider-context`、`codex/u004-appearance-quality`、`codex/u004-workbench-shell`、`codex/u004-integration-gates`；
- W1 只拥有 Provider/context，W2 只拥有表示/外观编译，W3 只拥有工作台前端，W4 只拥有共享文档、package/lock、跨轨 Gate 与最终证据；
- W1–W3 不修改任务状态和 handoff，不相互 cherry-pick；
- W4 只合并已有 focused PASS 的 commit，按 W1→W2→W3 顺序运行回归，不能用 `ours/theirs` 覆盖整文件；
- 某条轨道失败时只退回该 Luna，其他轨道可继续，但 U004 不得标 `done`；
- 第一阶段不以导出作为目标，但不得删除或破坏现有 Snapshot/Export 真值链。

## 4. 产品方向护栏

后续设计必须同时满足下列判断：

### 4.1 服务对象

- 非专业 3D 创作者、独立开发者和小型游戏/影视/UGC 团队；
- 会表达创意、会提供图片，但不愿安装和学习完整 DCC/CAD 管线；
- 需要快速得到外观精致、可继续改、可导出且来源清楚的通用 3D 资产；
- 企业用户可要求本地项目数据、可替换 Provider 和审计 lineage。

### 4.2 第一分布

入口类别从 U002 起不设对象白名单。机械臂、机器人、工业设备和虚构硬表面道具只是当前最成熟的程序化表示与回归分布；角色、生物、植物、软体和环境同样进入 `SubjectProfile`，再由 `RepresentationPlan` 选择现有能力。没有可执行表示时必须返回 typed limitation，不得回退到机械臂模板。

### 4.3 第一阶段不做

- 把“类别开放入口”写成“任意类别已经达到生产质量”；
- 在 U004 前承诺角色、人物、动物、毛发、布料或扫描级重建；
- B-Rep、STEP、制造尺寸、结构/安全/适航/认证结论；
- 训练基础 3D 模型、自建常驻 GPU 集群；
- 默认安装 CUDA、ComfyUI、Blender 或大体积模型权重；
- 用固定整机模板、英雄角度、三角形数量或高分辨率纹理冒充视觉质量；
- 让用户在多个失败候选中替 Agent 做质量筛选。

## 5. 产品运行时不可破坏的合同

### 5.1 单一状态真值

`ActiveDesignSnapshot@1` 必须把活动版本、选择、预览、质量和导出绑定到同一 revision。前端 localStorage、Provider 输出、临时图片、旧 Concept 或网络响应都不能成为第二版本头。

### 5.2 唯一永久写入路径

```text
typed intent / reference evidence
→ Rust validate / normalize
→ versioned typed design source
→ restricted compile
→ GLB/PBR readback
→ fixed-view hard gates
→ at most one typed in-place patch
→ one transient result
→ user preview / confirm
→ immutable child version
→ ActiveDesignSnapshot / export / restart
```

失败、取消、超时、迟到响应、过期 revision、hash 不符和用户拒绝必须产生零永久版本副作用。

### 5.3 受限执行

模型不能提交任意 Python、JavaScript、shell、URL 或文件路径。Rust 拥有工具注册、权限、预算、状态和版本；Python 只编译 Rust 已验证的受限几何 IR。

### 5.4 单 renderer

工作台只维护一个 Three.js renderer/context。docked 与 focus 只是同一 canvas 的布局状态；参考视图、候选视图和质量 UI 不能偷偷创建第二上下文。

### 5.5 Provider 可替换

产品 Provider 只能通过版本化 adapter 和受限合同进入。任何具体模型名、URL、价格、上下文长度和能力都属于可漂移配置，实施前必须查当前官方资料并用真实 preflight 验证。Luna 作为开发模型不得被写入产品运行时依赖。

## 6. 视觉质量验收合同

每个视觉任务都必须给出一份可检查的 `VisualAcceptanceContract`。在正式 Schema 实现前，任务文档至少包含下列字段：

```text
brief_id
domain_distribution
authorized_reference_ids
intended_use
must_show
must_not_show
macro_claims
meso_claims
micro_claims
material_claims
fixed_view_ids
triangle_budget
texture_budget
latency_budget
provider_call_budget
human_review_protocol
failure_and_stop_conditions
```

每条图片 claim 必须标记：

- `observed`：在授权参考的具体视图中可见；
- `inferred`：根据外观语义推断，但参考没有直接证明；
- `unknown`：信息不足，系统不能假装确定。

质量按六层检查：

| 层级 | 检查内容 | 不能替代它的指标 |
| --- | --- | --- |
| Macro | 轮廓、比例、姿态、重心、负空间 | triangle 数、背景和 bloom |
| Meso | 部件层级、连接、关节、壳体过渡、面板节奏 | inventory 文案、固定名称 |
| Micro | 倒角、紧固、线束夹、Decal、磨损、微表面 | 统一噪声、只提高纹理分辨率 |
| PBR | zone、UV/tangent、五通道消费、roughness 逻辑 | 只有 base color 的“材质” |
| Presentation | 八视图、色彩管理、接触阴影、无掩盖 | 单张英雄图或代理评分 |
| Usability | GLB/readback、Part、选择、修改、版本、导出 | 仅能看的截图或视频 |

确定性 Gate 和真人 Gate 分开：自动 Gate 通过只说明工件可进入评审；独立真人 `4/5` 才证明目标用途的主观质量。开发模型、VLM 或同一作者不能代替独立评分者。

## 7. 当前阶段和依赖顺序

权威状态以任务索引为准。ADR-0022 冻结的当前顺序是：

```text
U001 产品与文档迁移
→ U002 类别开放理解、特征合同和表示规划
→ U003 通用设计源、外观绑定与统一 lineage
→ U004A DeepSeek / 千问唯一 AI Provider
→ U004 procedural / deformable / local-hybrid 能力路由
→ U005 跨类别真实输入、质量、时间、成本和真人盲评
```

### 阶段 0：U001/U001A 产品与文档迁移

统一类别开放目标、当前 Alpha 边界、文档生命周期和安全能力边界。该阶段不增加运行时生成类别，也不能把 ADR 目标写入用户指南为已实现。

### 阶段 1：U002 类别开放入口

实现 `SubjectProfile@1`、`VisualFeatureContract@1`、`RepresentationPlan@1` 与统一 sealed multimodal author request。Domain Pack 只可提供知识提示和回归数据，不能决定对象是否准入；未知对象不得静默转成 C111、机械臂或未来武器。

### 阶段 2：U003 通用设计源与外观绑定

建立 `UniversalAssetSource@1`，让部件、轮廓、结构、材质、投影和 detail claim 都能追到输入证据、设计源、编译结果、GLB/PBR readback 与验收视图。Provider 输出仍不能成为第二版本真值。

### 阶段 3：U004 多表示能力路由

把现有 procedural hard-surface、受限 parametric/deformable 和 local-hybrid 纳入同一 capability-gated 路由、预算、CAS、版本和 readback。千问只负责视觉证据/比较，DeepSeek 只负责受限程序 author/patch；缺少能力、预算或合法输入时 fail closed，不安装本地大权重，不调用第三方远程 Mesh API，不开放任意代码。

### 阶段 4：U005 跨类别质量与商业验证

用冻结的八类未见任务、真实单图/多视图/纯文本/已有资产输入、首轮与最多一次 patch、端到端时间、可变成本、失败分类和独立真人盲评判断每种表示能力的成熟度。C111B、E005 和 M108B 只作为机械回归子集，不阻塞类别开放入口，也不能替代跨类别结果。

## 8. 90 天目标与判定

90 天计划由 [ADR-0022](ADR/0022-universal-reference-conditioned-3d-agent.md) 统领，并继承 ADR-0020/0021 的成本和 `1 + 1` 约束；它只用于判断产品方向，不授权跳过依赖。目标证据：

- 八类冻结未见任务，机械硬表面结果单独保留为回归切片；
- 首次合成真人 `≥4/5` 达 70%，最多一次 typed patch 后达 85%；
- 结果中位数 `<5 分钟`、P90 `<10 分钟`；
- 严重回归率 `<10%`；
- 可变成本不高于实收收入的 25%；
- 至少 5 家付费伙伴，第 4 周核心闭环周留存不少于 50%。

这些数字在完成真实测量前必须标为 `target`。不得写入 `USER_GUIDE` 或能力矩阵为当前能力。

## 9. 成本控制协议

每个会触发联网或付费的任务必须先记录：

```text
experiment_id:
authorized_by:
provider_and_model:
max_requests:
max_repair_attempts: 1
max_tokens_or_images:
max_wall_time:
max_estimated_cost:
cache_key:
stop_conditions:
redacted_evidence_path:
```

默认规则：

- 没有明确授权时网络调用数为 0；
- 本地 Schema、范围、预算、缓存和 deterministic hard gate 优先；
- 不对失败自动重试；只有错误分类证明可重试且仍在预算内时允许一次显式重试；
- 同一意图最多一次 typed patch，之后明确失败；
- 图片先做低成本证据提取，只有审美比较需要时升级更强视觉模型；
- 1K PBR 是默认生产起点，2K/4K 必须有用途、设备和成本理由；
- Provider 响应、图片和大型工件进入受限对象库，日志只保存脱敏 hash、用量和错误类别。

## 10. 每个原子任务的执行循环

### 10.1 领取

1. 从任务索引选择唯一 `ready` ID；
2. 检查全部依赖确为 `done`；
3. 把该任务改为唯一 `in_progress`；
4. 记录任务前 commit、工作区状态和已有失败；
5. 完整读取任务卡列出的入口与合同。

若本轮只有分析或文档设计，不要伪造 `in_progress`。只有真正开始实现并能持续推进退出条件时领取。

### 10.2 基线

- 先运行最小 focused baseline；
- 再运行任务卡要求的聚合 Gate；
- 把既有失败分类为任务内、并行工作或已知发布阻断；
- 基线本身失败时先确定是否阻止当前任务，不能默认修所有失败。

### 10.3 实现

- 先合同、validator 和负向 fixture，再运行时和 UI；
- 每次永久写入都覆盖成功、失败、取消、stale、篡改和重启；
- 复用现有 typed source、对象库、版本和 renderer；
- 大改拆成可独立验收的最小补丁，但不把局部测试通过写成任务完成；
- 发现任务卡无法判定成败时，先补退出条件，不继续扩功能。

### 10.4 验证

结果统一分类：

- `PASS`：本轮在当前工作区真实运行并通过；
- `FAIL`：任务范围内失败，必须修复或保持任务未完成；
- `KNOWN FAIL`：已验证属于既有/并行/外部阻断，说明证据和影响；
- `NOT RUN`：未运行，不能用历史 PASS 或推断补齐。

真实 Provider、视觉 Provider、packaged app、真人评分和付费伙伴实验必须单独列出；任何本地 fixture 都不能替代。

### 10.5 交接

任务结束同步：任务索引、handoff、能力—Gate 矩阵，以及受影响的 API/Schema/用户/操作文档。handoff 必须包含：

```text
Task ID:
Goal relation:
Branch / commit / dirty state:
Dependencies checked:
Files changed:
Contracts and migrations:
Commands run:
PASS:
FAIL:
KNOWN FAIL:
NOT RUN:
Current user-visible truth:
Remaining blockers:
Next single task:
```

只有任务卡全部退出、相关自动测试通过、失败/重启/幂等边界有证据、文档同步且没有开放必需项时，才能标为 `done`。

## 11. 阻断与停止规则

下列情况必须停止当前推进并报告，而不是扩大范围：

- 同一阻断连续出现且没有新的安全检查或替代路径；
- 需要用户授权真实计费、上传图片、外部服务、签名身份或独立真人；
- 脏工作区在同一文件存在无法安全合并的并行改动；
- 当前方案要求新增第二产品真值、任意代码执行或绕过确认；
- 为通过 Gate 必须删除测试、fixture、迁移、发布目标或降低阈值；
- U002 无法删除对象类别回退，或不能在缺少表示时返回 typed limitation；
- U003 无法把视觉 claim、材质/投影与最终 readback 建立可验证 lineage；
- U004 的新增表示不能进入同一真值链，或成本、许可证、体积和恢复边界不成立；
- U005 冻结未见任务的质量、成本或留存没有达到 ADR-0022 的 Go 门。

需要外部输入时可以把原子任务保持 `blocked/external`，但不能把整个 Goal 写成完成。仍有安全的本地工作时，继续处理当前任务的可独立部分。

## 12. Luna 的首轮 Goal 提示词

可将下面内容作为 Luna 的首轮 Goal 输入；开始前仍要让它读取本文件和仓库权威文档：

```text
目标：在小团队预算内，把 ForgeCAD 收敛成轻量、高自由度、类别开放、外观优先、可编辑、可审计的通用参考条件 3D Agent。严格遵守 AGENTS.md、DOCUMENTATION_MAP、DOCUMENTATION_STATUS、CODEX_HANDOFF、CODEX_EXECUTION_PLAN、CODEX_TASK_INDEX、AUTHORITATIVE_STATE、USER_GUIDE、DESIGN 和 ADR-0022；ADR-0020/0021 只提供仍有效的成本、typed program 与 1+1 约束。

当前只领取并推进任务索引中的 FGC-U002。先运行 git status -sb、git diff --check、git rev-parse --short HEAD，保留全部用户/并行修改。完整读取 U002 任务卡、当前 concept-spec、Rust app-server/core、domain inference、multimodal request、restricted worker/readback 和现有 Gate；先记录 focused baseline。

实现目标：新增并贯通 `SubjectProfile@1`、`VisualFeatureContract@1`、`RepresentationPlan@1` 和统一 sealed multimodal author request；纯文本、真实单图、多视图和已有资产走同一入口。删除未知对象到 C111、机械臂或未来武器的静默回退；Domain Pack 只作知识提示。当前没有可执行表示时返回 typed limitation，不创建版本、不伪造候选。

不要提前实现 U004/U005，不要新增第二 renderer、任意 Python/JavaScript/shell/path/URL 执行或绕过预算/确认；不要把类别开放入口或 U003 source lineage 写成角色、生物、真实纹理投影或任意图片质量已实现；不要把 Luna 写成产品 Provider；不要覆盖并行工作区修改。未获得精确 Provider/model/pricing/disclosure 授权时保持 0 付费网络调用。

一次只推进一个原子任务。每次阶段输出必须列出当前证据、下一小步、PASS/FAIL/KNOWN FAIL/NOT RUN。未满足当前任务全部退出条件时不要标记完成；U004–U005、跨类别正式质量和真人评分继续写 NOT RUN。
```

## 13. 自动续跑时的最小状态摘要

上下文即将压缩或一次运行结束时，Luna 应留下不超过一页的状态摘要：

```text
Goal:
Active Task:
Task status:
Last verified commit:
Dirty files owned by user/parallel work:
Files changed by this task:
Last passing focused Gate:
Current failing Gate and exact error:
Evidence artifact/hash:
External authorization still needed:
Next safe command or patch:
Forbidden scope reminders:
```

摘要只帮助续跑，不替代 `CODEX_HANDOFF.md`、任务索引、Git diff 或测试证据。

## 14. 成功定义

Goal 的成功不是“文档很多”“工具调用很多”或“生成了一个好看的截图”，而是：

1. 冻结硬表面分布中的未见任务达到质量、时间和成本门；
2. 用户能从提示词/授权图片得到唯一真实 GLB/PBR，继续语言修改并可靠导出；
3. 资产跨确认、版本、导出和重启保持 exact-lineage；
4. 新设计主要通过可组合设计语言产生，而不是增加整机 fixture；
5. 至少 5 家付费伙伴重复使用核心闭环；
6. 每个尚未达到的能力继续如实标记 `目标设计` 或 `blocked`。

在这些条件满足前，Goal 可以持续推进，但不得用“终极形态已完成”描述当前 Alpha。
