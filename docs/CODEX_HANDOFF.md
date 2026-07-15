# ForgeCAD Codex 当前交接

快照日期：2026-07-15
用途：后续 Codex 开始任务前的第一份上下文

文档状态账本：[DOCUMENTATION_STATUS.md](DOCUMENTATION_STATUS.md)。当本文件与用户指南、能力矩阵或任务索引出现状态冲突时，先按文档地图修正归属，不要直接领取代码任务。

## 2026-07-15：FGC-G824D Windows packaged evidence（已完成；G825 ready）

- GitHub 登录已恢复，用户明确授权 commit/push 当前工作区；分支 `codex/repository-integrity` 已推送到 Draft PR #3。主工作区提交为 `f12aa381`，后续 CI 修复截至 `6a9edefa`。
- GitHub Actions run `29383382978` 的真实 `windows-2022` frozen sidecar job 已通过并上传 `g824d-windows-packaged-candidate`。报告保存为 `evaluations/csg-g824d/windows-report.json`，再次运行 `check_g824d_windows_packaged_candidate.py` 通过。
- Windows AMD64/Python 3.11.9：executable 35,788,283 bytes，健康冷启动 2,528.125 ms；五组有效 fixture 的 provenance/GLB readback 通过，near-degenerate 在写出前拒绝；三个中断窗口均回收进程、清理 staging、保持 SQLite/对象库不变，Version/head/Snapshot 原子回滚/提交通过，Provider 调用为零。
- ADR-0013 已选择 `manifold3d==3.5.2` 作为 G825 唯一生产候选。当前生产依赖和默认 handler 仍未改变；G825 是唯一 `ready`，完成前不得宣称稳健通用 CSG 已实现。
- 为使干净 runner 与 Windows 语义一致，修复了 PyInstaller `_MEIPASS` 资源定位、C104 完整 ShapeProgram fixture、F006 10px 文本、desktop/Agent CI 隔离、sidecar 空输入临时夹具、D003 历史迁移夹具及 Windows SQLite 清理。backend、desktop 与 G824D 已在同一 run 通过；完整 PR checks 仍应以 `gh pr checks 3` 的最终状态为准。

## 2026-07-15：FGC-G824D Windows packaged evidence runner（历史阻断记录；已由上节解除）

- 新增 Windows x64 PyInstaller runner：实际当前 sidecar 入口通过 runtime hook 在 frozen binary 内运行 Manifold Python 六组 provenance/readback、near-degenerate 拒绝，以及 busy cancel、busy timeout、valid GLB ready-before-promotion 三个窗口；父进程用真实临时 SQLite、对象库和 UnitOfWork 验证零部分提升与原子回滚/提交。
- `.github/workflows/forgecad-core.yml` 新增独立 `windows-2022` job，固定 Python 3.11、PyInstaller 6.16.0、Manifold 3.5.2、NumPy 2.4.6；成功或失败均尝试上传 `g824d-windows-packaged-candidate` JSON artifact。候选进程只接收 staging marker/result/GLB 路径，不接收权威路径或 Provider 配置。
- 本机 runtime hook 已用隔离候选目录验证六组 fixture：五组真实 GLB readback/provenance 通过，near-degenerate 以 `CSG_DEGENERATE_OUTPUT` 在写出前拒绝；Ruff、compile 和 diff check 通过。连续三个目标回合均确认本机没有 Wine/QEMU Windows 环境、GitHub CLI 凭证失效且未获 commit/push 授权，所以不存在真实 Windows artifact，任务标记 `blocked`。恢复认证并明确授权发布该工作区，或提供 Windows x64 环境后，先运行/校验 artifact；在此之前不得新增采用 ADR 或领取 G825。

## 2026-07-15：FGC-G824C macOS packaged candidate（已完成；建议 Python，仍未正式采用）

- 隔离临时目录使用当前 `sidecar_entry.py` 实际构建并启动含 `manifold3d==3.5.2`/NumPy 的 arm64 PyInstaller onefile；archive、runtime hook 强制 import 与真实健康检查通过。构建没有覆盖仓库 sidecar，也没有修改生产依赖、lock、manifest 或 handler。
- 当前基线/候选包体为 19,445,536/24,207,728 bytes，增量 4,762,192 bytes；同轮冷启动 18,250.329/19,243.281 ms，相对回归 992.951 ms；候选完整进程树峰值 RSS 87,376 KiB。均通过固定的 48 MiB 总包体、28 MiB 增量、5 秒相对冷启动和 300 MiB RSS 预算。
- Manifold Apache-2.0 与 NumPy BSD-3-Clause/捆绑许可证文件已记录版本和 SHA-256；PyInstaller 需要显式 hidden import `numpy._core._exceptions`。WASM 不适配当前 Python sidecar 执行宿主，因此建议唯一候选为 Python，但状态仍是 `recommended_pending_windows_runtime`。
- 新 Gate `agent:g824c-packaged-candidate-smoke` 校验提交报告及生产依赖隔离。Windows x64 packaged sidecar 同 fixture 与 superseding ADR 仍未完成，G825 继续 blocked。本轮未 commit、未 push。

## 2026-07-15：FGC-G824B CSG staging/权威状态提升补证（已完成；仍未采用候选）

- 全量迁移建立真实临时 SQLite 和 `ContentAddressedStore`，保存活动 Agent v1/head/Snapshot/proposed ChangeSet。Python/WASM 候选子进程不接收任何权威路径，只能写事务外 staging。
- 两个候选在 kernel cancel、kernel timeout、valid GLB ready-before-promotion 三个窗口终止；Version/head/ChangeSet/Snapshot/quality/import/idempotency 和对象库 fingerprint 均不变，staging GLB 清理。真实 `SQLiteUnitOfWork` 注入 Version/head/Snapshot 提升失败会整体回滚，成功会整体提交到 v2/revision 2。
- 新 Gate `agent:g824b-csg-promotion-boundary-smoke` 校验报告。该时点 Windows x64 packaged sidecar、唯一候选 packaged 预算/许可证与 superseding ADR 尚未完成；后续 G824C 已补齐 macOS packaged 证据并建议 Python，但 Windows 与正式采用仍阻断 G825。本轮未 commit、未 push。

## 2026-07-15：FGC-G824A CSG provenance/readback/取消补证（已完成；仍未采用候选）

- Manifold Python/WASM 的输入使用不同 source/material/zone property channel；四领域 union/subtract、coplanar 与 near-degenerate 在 `simplify` 后按 original ID、face ID 和 backside 建立逐三角 provenance。五组有效 fixture 生成相同 GLB hash 并通过 ForgeCAD triangle/material/surface/custom provenance readback；near-degenerate 以 `CSG_DEGENERATE_OUTPUT` 在部分 GLB 前拒绝。
- 两个候选分别用隔离进程验证 `CSG_CANCELLED`/`CSG_TIMEOUT`：marker 后终止、进程回收、无候选 GLB，隔离 Snapshot/Version/cache sentinel 不变。它不等于真实生产 Worker/数据库事务已经验证。
- 新 Gate `agent:g824a-csg-adoption-evidence-smoke` 校验报告及生产依赖继续隔离。Windows x64 packaged sidecar 仍未实机执行，且没有 superseding ADR 选择唯一候选；G825 继续 blocked。本轮未 commit、未 push。

## 2026-07-15：FGC-G824 CSG 内核隔离 benchmark（已完成；未采用候选）

- 临时目录固定比较当前 handler、`manifold3d==3.5.2`（commit `11235e6...`）和 `manifold-3d@3.5.1`（commit `cc8a7f66...`）；报告记录 macOS arm64 环境、安装/运行命令、Apache-2.0、包增量、冷/热时间、峰值内存、四领域 fixture、coplanar/near-degenerate 和重复 mesh hash。
- Python/WASM 在本机 fixture 中都成功且产生相同 hash；但 ForgeCAD material/surface/zone provenance、operation 取消/稳定错误码和 Windows 实机 packaged runtime 均未证明。ADR-0012 因此明确不采用候选，G825 保持 blocked，并列出解除条件。
- `agent:g824-csg-benchmark-smoke` 校验报告和生产依赖隔离；没有修改 production manifest、Worker handler、Python/npm 依赖或锁文件。临时目录删除即可移除候选。本轮未 commit、未 push。

## 2026-07-15：FGC-G823 受限路径 Sweep（已完成）

- `sweep` 已进入唯一 runtime manifest，只消费 canonical closed/hole-free `ProfileSketch@1` 与 2–32 点有界 3D path。Worker 使用确定性 parallel-transport frame，支持开放路径有限 twist、开/闭 path、固定 sample seam 和显式 cap。
- 运行边界拒绝零长度、短于截面视觉比例的段、接近 180° frame 翻转、明显路径自交、闭合 path cap/twist、点数/bounds/triangle 超限；GLB readback 验证 `sweep_side/seam/start_cap/end_cap`、UV0、normal、closed/boundary/non-manifold/degenerate 和连续 triangle ranges。
- 新 Gate `agent:g823-sweep-smoke` 覆盖直线、折线、多点平滑近似、有限 twist、开/闭 path、封盖、重复字节、拓扑和失败预算；G819/Q003/G820–G822、contracts 与 Agent 回归通过。下一项唯一 ready 为 G824 布尔 benchmark/ADR；Planner/UI 尚未自动采用 Sweep。本轮未 commit、未 push。

## 2026-07-15：FGC-G822 受限多截面 Loft（已完成）

- `loft` 已加入 `ShapeProgramRuntimeManifest@1` 的唯一 operation 真值，并由 Schema、Pydantic/semantic validator 和 Worker 共用；只接受 canonical `ProfileSectionSet@1`、2–12 个统一采样闭合无孔截面、严格 section 顺序、有界二维 scale/axis length、有限 twist、固定采样 seam、`linear` continuity 和首尾 cap。
- Worker 新增确定性多截面网格，支持 x/y/z 主轴和截面 scale/twist；GLB 保留 `loft_side/seam/start_cap/end_cap` 连续三角范围、UV0、normal 和 profile provenance。编译在 GLB 写出前先拒绝三角预算，readback 再验证 triangle/bounds、闭合、boundary/non-manifold/degenerate 与 accessor/range 一致性。
- 新 Gate `agent:g822-loft-smoke` 覆盖汽车、飞机、家电和机械臂四类壳体 fixture、曲线/矩形截面、尺寸/位置/扭转/封盖、重复字节与真实 readback，以及排序、混合采样数、翻转风险、自交、退化、bounds、损坏 hash 和预算失败。G819/Q003/G820/G821/G807/G818、G1–G7（含 Agent asset commit）、contracts、Agent unit/check 已通过。
- 下一项唯一 ready 为 `FGC-G823`，只实现受限 Sweep path/frame runtime。当前 Planner/UI 不自动生成 Loft，用户指南没有新增自由轮廓或放样入口；孔洞 Loft、Sweep、稳健 CSG、NURBS/B-Rep、PBR/Recipe 均仍未实现。本轮未 commit、未 push，并保留既有脏工作区。

## 2026-07-15：FGC-G821 增强 Profile/Extrude/Revolve（已完成）

- 现有 `profile` operation 新增 canonical `profile_input_id` + 二维 `profile_scale` 分支，直接消费 G820 重采样结果；旧 `args.points` 保持兼容且禁止混用新参数。Extrude 支持曲线、孔洞、独立首尾 cap 和明确开放 ribbon；Revolve 支持轴点、完整/部分角度、8–64 radial segments 与部分角 seam cap。
- 服务端三角化保留外轮廓/孔洞方向；轴点 Revolve 使用单三角扇，避免退化四边形。GLB primitive extras 保存 side/hole_wall/start_cap/end_cap/seam 的连续 triangle ranges；真实 readback 解析 POSITION/NORMAL/UV0/index，校验 accessor 对齐、UV 范围、range 覆盖、closed/boundary/non-manifold/degenerate topology。profile 结果出现退化面会失败，不写部分资产。
- 新 Gate `agent:g821-profile-solid-fidelity-smoke` 覆盖带孔/无孔/开放 Extrude、完整/部分 Revolve、轴点、封盖、seam、UV0、表面区间、拓扑、重复 GLB、损坏 hash、负半径、孔洞 Revolve 和 triangle budget。G1–G7、G802/G803、G819/Q003/G820、contracts、Agent unit（16 passed）/compile/ruff、desktop typecheck/build、文档 walkthrough、repository integrity、安全范围、密钥文件和 `git diff --check` 均已通过；Vite 仍只报告既有大 chunk/dynamic import 警告。
- 下一项唯一 ready 为 `FGC-G822`，只实现受限多截面 Loft。当前 Planner/UI 尚未自动采用新 Profile，用户指南不增加自由轮廓能力。本轮未 commit、未 push，继续保留既有脏工作区。

## 2026-07-15：FGC-G820 ProfileSketch 与截面合同（已完成）

- 新增 `ProfileSketch@1` 与 `ProfileSectionSet@1` JSON Schema、Pydantic 模型和生成 TypeScript/Python registry。合同限制 normalized `[-1,1]` 的 line/quadratic/cubic、闭合/开放和实际绕序、最多 8 个孔洞、统一重采样，以及 2–12 个严格排序截面的有限 position/scale/twist/cap；自由 SVG、URL/路径、非有限、退化、自交、孔洞越界/重叠和预算失败均在 Worker 前拒绝。
- `profile_contracts.py` 提供确定性曲线采样、规范化和 canonical SHA-256：外轮廓统一 counter-clockwise，孔洞统一 clockwise，等价顺逆输入产生同一 hash。ShapeProgram 新增可选 `profile_inputs` provenance，保存 canonical payload、合同版本和 input hash；不一致即拒绝，旧 ShapeProgram 不带该字段仍原样通过。
- 新 Gate `agent:g820-profile-sketch-contract-smoke` 已通过；任务前/后 G819、Q003、G802、G803、`contracts:types:check`、`agent:unit`（16 passed）、`agent:check`、`.venv/bin/ruff check apps/agent`、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files` 和 `git diff --check` 均已通过。本轮没有新增 Loft/Sweep/Worker operation 或用户 UI，当前用户指南不变。
- 下一项唯一 ready 为 `FGC-G821`：只让现有 Profile/Extrude/Revolve 消费 G820 合同并补曲线、孔洞、封盖、UV0 和 surface provenance。工作区继续保留用户既有大量未提交修改；本轮未 commit、未 push。

## 2026-07-15：FGC-Q003 真实编译/GLB readback 质量真值（已完成）

- 新增 `GeometryCompileReadback@1` JSON Schema/Pydantic 合同，同一次编译产生 program/GLB hash、字节数、triangle、bounds、mesh/primitive/material 数、operation 与 output role 事实。生成类型和 OpenAPI 已同步。
- 质量检查已删除 box/cylinder 常数估算，并将 readback 嵌入不可变报告；导出使用同一 compile/readback 结果。损坏回读生成 `compile_failure/unavailable` 质量或 `GEOMETRY_READBACK_FAILED` 导出拒绝，未知操作仍由 G819 无副作用拒绝。
- 旧 `legacy_estimate` 报告读取时隔离为 unavailable，且不再成为组件来源质量证据。Q002 的 Snapshot ETag/Idempotency-Key 重放保留，新 quality request hash 防止旧估算响应被当作 Q003 报告重放。
- 新 Gate `agent:q003-compile-readback-quality-smoke` 已通过，覆盖四领域、导出 hash/数字一致、损坏 readback、未知操作、旧报告隔离与重启幂等。G801–G818、G819、G5/G6/G7、Q002、C102、T002（14 场景）、T003、r3、desktop typecheck/build、`contracts:types:check`、`agent:check`、文档/安全/密钥 Gate 和 `git diff --check` 均已通过。
- 下一项唯一 ready 任务为 `FGC-G820`。本轮未 commit、未 push，保留用户现有脏工作区。

## 2026-07-15：FGC-G819 运行时操作 manifest 单一真值（已完成）

- 已新增 `packages/concept-spec/fixtures/shape-program-runtime-manifest.json`（`ShapeProgramRuntimeManifest@1`），唯一声明 14 个当前可执行操作与其 executor；`scripts/generate_schema_types.py` 从此文件生成 `shape-program.schema.json` 的 operation enum，`contracts:types:check` 会拒绝 schema/manifest 漂移。
- `ShapeProgramPayload` 让 Pydantic Agent response/version 输入复用同一 Schema/manifest 校验；`shape_program.py` 在 JSON Schema 前拒绝未知操作；Geometry Worker 在每次编译前核对 manifest executor coverage，所有原先的执行循环静默 `continue` 已改为明确拒绝。`build_blockout` 也改走同一个 GLB 编译/readback 入口。
- preview、confirm、质量入口和导出在写入/输出前统一编译并检查运行时兼容性，未知、非法或缺少执行器返回 `UNSUPPORTED_RUNTIME_OPERATION`；损坏的持久化 ShapeProgram 也会在 Pydantic 读取边界拒绝。质量入口在本任务只使用该 compile/readback 作为拒绝门，仍不消费其 readback 数字。`agent:g819-runtime-operation-manifest-smoke` 覆盖 manifest 中每项操作、未知 `pivot` 与故意移除 executor，分别验证 preview/confirm/quality/export 零副作用。
- 本轮已通过：G1–G7（含外部 GLB 只读参考回归）、G3、G801–G807、G812–G815、G817/G818、G819、Q002、`agent:unit`（16 passed）、`agent:check`、`contracts:types:check`、`desktop:typecheck`、`desktop:build`、T002、T003、r3、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files` 和 `git diff --check`。组件及 r3 smoke 现从服务器已声明的角色/参数/连接器读取 fixture，不再假定 `upper_link` 或“长度比例”。未 commit、未 push；继续保留用户既有脏工作区。
- 下一项唯一 ready 是 `FGC-Q003`：质量报告仍以 manifest 声明的旧估算模式计算 box/cylinder 数字；必须改为同一次真实 compile/readback 的只读事实，不能把 G819 的拒绝边界误写为 Q003 已完成。

## 2026-07-15：3D 机械设计系统、混合建模语法与目标操作手册（仅文档设计）

- 用户确认不以 HTML/CSS 六面拼接或单一 box 连续裁剪作为最终路线，而是借鉴 UI 组件库思想建立 3D 机械设计系统：HTML/React 负责工作台，SVG 只编辑规范化 Profile，GSAP 只做状态过渡；主形体使用 Profile/Extrude/Loft/Revolve/Sweep，CSG 负责局部开孔/组合，Recipe 负责复用，PBR/GLB readback 负责真实外观与质量证据。
- `DESIGN.md` 已升级到 v6，新增 `MechanicalStyleToken@1`、`ProfileSketch@1`、`ProfileSectionSet@1`、`EditableComponentRecipe@1`、建模语法路由、不可变 feature node、CSG benchmark/单一内核、edge/normal/UV/tangent/zone provenance、GSAP 与可丢弃 SDF 边界。ADR-0011 接受该路线；`MECHANICAL_DESIGN_OPERATIONS.md` 是新的目标操作手册，不替代当前 USER_GUIDE。
- 原 G819/Q003 仍不可跳过，且当前唯一 `ready` 仍是 G819。新增原子几何子链 G820–G826；完整目标顺序为 `G819 → Q003 → G820 → G821 → G822 → G823 → G824 → G825 → G826 → A003 → F025 → D005 → A004 → M108 → C105 → V003 → F026 → A005 → R007 → D006`。M108 消费 G826 的真实表面事实，C105 组合 Profile/feature/连接/材质 Recipe，V003 最后自动选择建模语法、Recipe 和唯一最佳候选。
- 当前运行能力没有因本轮文档变化而扩大：Alpha 仍显示三方向，仍以低多边形 blockout、有限组合操作和多数单材质区为主；Loft、Sweep、稳健 CSG、轮廓编辑、真实多区 PBR、Recipe 和单一最佳结果均是目标设计。USER_GUIDE 与能力—Gate 矩阵未被改写为已实现。
- 使用 `documents:documents` 的结构/可读性规则整理 Markdown，使用 `game-studio:web-3d-asset-pipeline` 固化 GLB/readback/纹理和单 renderer 边界，使用 `gsap-core` 固化动画只反映状态、不成为几何或版本真值；没有生成 DOCX、没有新增依赖或代码。
- 本轮 Gate：`release:docs-walkthrough` PASS（任务索引 111 项、无 issue）、`repository:integrity` PASS、`release:safety-scope` PASS、`release:secrets-files` PASS（557 文件、0 匹配）、`agent:check` PASS、`git diff --check` PASS。工作区继续保留用户已有大量未提交修改；本轮未 commit、未 push。

## 2026-07-14：视觉真实度、单一最佳结果、Codex 式工作台与 DeepSeek 诊断（历史目标；已由 ADR-0011 扩展）

- 用户明确取消“三方向供选择”的目标：Agent 应在内部生成/编译/readback/渲染/评审候选，只展示一个最佳结果；3D 默认缩到左上 mini viewport，点击后把同一个 canvas 移到中央 focus。ADR-0010 已接受该决策，`FGC-V002` 已标为 `superseded`，USER_GUIDE 仍保留当前三方向事实直到 V003 真正完成。
- 本机实时检查：`CAD 工作台.app` 与本地 Uvicorn 正在运行，`GET /api/health` 返回 `status=ok, mode=sqlite_mock`；`~/Library/Application Support/ForgeCAD/provider.json` 缺失，Keychain service/account `ForgeCAD Agent Provider/default` 也缺失。Rust supervisor 因而没有注入 `FORGECAD_AGENT_PROVIDER=openai_compatible`，`mechanical_planner_from_env()` 选择确定性离线 Planner。`.wushen-agent.log` 有普通 Agent Turn，但没有 `provider:check`/DeepSeek 请求；一次 409 是同 Thread Turn in progress，不是 DeepSeek 错误。
- 已核对 DeepSeek 官方文档：`https://api.deepseek.com` 和 `deepseek-v4-pro` 当前有效；模型名不是此次根因。官方 JSON Output 仍可能返回空 content，thinking Tool Calls 的后续子请求必须续传 `reasoning_content`；400/401/402/422/429/500/503 应分别处理。当前 adapter 拒绝 Tool Calls并泛化部分错误，前端再把失败压成“暂时无法连接/测试未完成”，这是独立的可观察性缺陷。
- 已用 GitHub connector/官方文档核验 OpenAI Codex app-server 的 Thread/Turn/Item 事件生命周期和 `SKILL.md` loader、Claude Code 的专用 subagent/Skill/hook/tool restriction、Zoo Design Studio 的 code-as-model/XState、glTF PBR/clearcoat/KTX2 与 glTF Transform inspect/validate/优化。只采用模式，不复制通用 shell Agent、云几何引擎或完整上游运行时。
- 新主链：`G819 → Q003 → A003 → F025 → D005 → A004 → V003 → F026 → A005 → M108 → C105 → R007 → D006`。G819 仍是唯一 ready；其余均 blocked。A003 处理 DeepSeek Provider Gateway，A004 处理受限产品 Action Loop，V003 处理内部最佳候选，F026 处理简洁布局，A005 处理专属 Skill，M108/C105/R007/D006 依次处理高真实度 PBR、组件配方、参考引导重建与新机械领域晋级。
- 本条只更新目标设计、计划、任务和审计文档，不实现 Provider、UI、几何、材质、Skill 或新领域，不修改 USER_GUIDE/能力矩阵为已实现。`release:docs-walkthrough`（任务索引 104 项、无 issue）、`repository:integrity`、`release:safety-scope`、`release:secrets-files`、`agent:check` 与 `git diff --check` 均通过。工作区继续保留原有大量未提交修改；未 commit、未 push。

## 2026-07-14：原用户优先 CAD 设计能力任务链（历史；已由 ADR-0010 更新）

- 当时用户指定顺序：`FGC-G819 → FGC-Q003 → FGC-F025 → FGC-D005 → FGC-V002`。后续同日“不要三方向选择”的新指示已将 V002 标为 superseded，并按本文件上一节扩展主链；当前仍只可领取 G819。P009 保持独立发布回归任务，不与该链混合实施。
- G819 的核心退出条件是 Schema、Pydantic、Worker、GLB 编译/readback 与质量检查共同消费一个运行时操作白名单；未实现操作必须在任何持久副作用前明确拒绝，不能跳过后继续成功。Q003 随后才将质量事实改为读取该次真实编译/readback。
- F025、D005、V002 分别限定为 Agent-first/legacy 只读隔离、四领域非工程语义比例配方、三方向的解释/单维临时重混/Brief 覆盖反馈。多材质区、可编辑组件配方、参考模型引导重建必须等 V002 后另拆原子任务。
- 本条目不实现运行时、合同、迁移或 UI；所有新能力仍是目标设计，未写入用户指南或能力矩阵。本轮已通过 `release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files`、`agent:check` 与 `git diff --check`；首次文档门曾因 G819 表格依赖引用了未登记的 G818 而失败，已将索引依赖改为已登记的完成基线后通过。

## 2026-07-14：FGC-A001 DeepSeek 多轮上下文与缓存账本

- 已实现 `ForgeCADProviderConversation@1`：固定 Provider 前缀、四组近期历史、当前 Snapshot 摘要、已绑定领域复用和确定性 `ThreadMemorySummary@1`；它不拥有任何资产或 Snapshot 真值。
- OpenAI-compatible Planner 现解析 DeepSeek `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`，只使用 Schema JSON，收到 Tool Calls 就拒绝，未保存 `reasoning_content`。
- Provider HTTP 已移出 SQLite 事务；每个 Thread 限制一个运行中 Turn。DeepSeek 20 元日预算预留/结算与缺失 usage 停止已实现。真实 Provider 测试仍未执行。
- 本轮已通过：`npm run agent:unit`（16 passed）、`npm run agent:g1-kernel-smoke`、`npm run agent:g4-mechanical-planner-smoke`、`npm run agent:check`、`npm run contracts:types:check`。仍须执行完整文档/安全 Gate 与桌面回归。

## 1. 先刷新，不盲信快照

本文件记录 2026-07-13 的已验证状态。开始新任务时先运行：

```bash
git status -sb
git diff --check
git log --oneline --decorate -8
```

当前工作区已有大量未提交产品和文档修改。它们属于用户正在推进的工作，禁止 reset、checkout、清理或覆盖无关文件。

## 2. 产品现状

产品已经在文档和最小运行时层面从 Weapon Concept Agent 升级为通用机械概念 3D Agent。四个首批领域包是未来武器概念道具、汽车、飞机和机械臂。

当前不是生产软件。准确定位：本机 Alpha + 轻量纵向切片。

文档已按当前权威、历史 ADR、历史 evidence 和 legacy 兼容资料分层。开始前先读 [DOCUMENTATION_MAP.md](DOCUMENTATION_MAP.md)；已删除的本地神经 3D、Unity、Blender Starter 和旧 Weapon 工作台文档不得从 Git 历史恢复到主路径。

## 3. 当前已验证通过

最近一次文档阶段验证：

```text
release:docs-walkthrough   PASS
repository:integrity       PASS
release:safety-scope       PASS
release:secrets-files      PASS
agent:check                PASS
agent:q002-active-design-contract-smoke PASS（bootstrap、CORS ETag/If-Match、质量重放/冲突/stale）
agent:s8-active-design-navigation-smoke PASS
contracts:types:check      PASS
git diff --check           PASS
desktop:f001-workbench-characterization PASS（本机 Chrome）
desktop:f004-workbench-drawers-smoke PASS
desktop:f006-accessibility-smoke PASS
desktop:c101-part-role-labels-smoke PASS（四领域 role、关节角色和未知回退）
desktop:f003-agent-selection-card-smoke PASS（中文角色显示边界）
desktop:t003-performance-smoke PASS（单 canvas/context 与资源预算）
agent:c102-component-compatibility-smoke PASS（HTTP/服务候选结论、质量/领域/role/停用负例和 ChangeSet 拦截）
agent:c104-part-display-smoke PASS（CAS/幂等、锁定 ChangeSet 拦截、隐藏/隔离选择保护、版本状态归一化）
agent:g808-editable-parameter-bindings-smoke PASS（JSON/Pydantic、旧资产兼容、路径/单位/范围/步长/唯一性）
desktop:typecheck PASS
desktop:build PASS（存在既有 bundle >500 kB warning；T003 预算门禁 PASS）
desktop:r3-concept-workbench-smoke PASS（Agent-first + 抽屉焦点/Escape + C104 锁定重启、隐藏/隔离恢复）
desktop:t002-workbench-e2e-scenarios PASS（12/12 场景）
agent:g801-shape-primitive-smoke PASS
agent:g802-profile-extrude-smoke PASS
agent:g803-revolve-smoke PASS
agent:g804-transform-arrays-smoke PASS
agent:g805-boolean-smoke PASS
agent:g806-bevel-surface-panel-smoke PASS
agent:g807-blockout-diversity-smoke PASS（四领域 48 个结构）
agent:r002-render-views-smoke PASS（四视图 PNG provenance/readback/fingerprint）
agent:r003-exploded-views-smoke PASS（条件式爆炸候选、透明 alpha、稳定 Part ID 与拒绝伪造分件）
agent:r004-render-package-smoke PASS（PNG/manifest ZIP、hash/readback、stale 拒绝和字节级重复性）
agent:m101-material-contract-smoke PASS（旧 payload 迁移、完整 PBR 字段与失败边界）
agent:m102-material-catalog-smoke PASS（13 个六类视觉材质预设）
agent:m103-material-texture-smoke PASS（内容寻址纹理对象、来源/许可证、路径边界和参数回退）
agent:unit PASS（13 passed；jsonschema RefResolver 仅有弃用警告）
desktop:typecheck PASS
desktop:build PASS（存在既有 bundle >500 kB warning；T003 Alpha 预算门禁仍通过）
```

本轮新增的 `desktop:f001-workbench-characterization` 已在本机 Chrome 通过并登记到 CI。它覆盖首次项目加载、legacy 显式重建 hand-off、含糊输入澄清、预览不写盘、Agent 资产提交、Snapshot/导出一致、重启恢复和单 WebGL canvas。F006 的 `desktop:f006-accessibility-smoke` 与 r3 浏览器断言增加了质量/组件抽屉初始焦点、Escape 关闭和导出关闭后的焦点返回。legacy starter 在未执行“让 Agent 重建可编辑资产”时保存仍会返回 `ACTIVE_DESIGN_INVALID`，这是必须保留的写入屏障；本次测试已验证显式 hand-off 后再提交。CI runner 的远程结果仍以对应 commit 为准。

上一轮技术审计中，G1–G7 独立 smoke、contracts、desktop typecheck 和 cargo check 通过。开始代码任务时仍需针对当前工作区重新运行，不能直接复用旧结果。

## 4. 当前已知限制与发布阻断

### 工作台状态正确性

```bash
npm run desktop:r3-concept-workbench-smoke
```

历史核心 smoke 覆盖 legacy 显式重建授权、Agent asset 提升、preview/确认、持久化质量 ID、不可变 undo→redo、preview/quality/selection 的 revision 竞争、重启恢复和 GLB 导出不回退 Concept；当前 `desktop:r3-concept-workbench-smoke` 的 Agent-first 路径已通过（参考 GLB v1、可编辑资产 v2–v5、质量、导出、C104 锁定重启恢复、单独查看、隐藏清选择和显示全部）。原生安装恢复、多客户端压力矩阵和 legacy UI 退出仍未完成；即使 Snapshot S008/C104 已退出，也不能据此宣布整个工作台已生产就绪。

### 打包

```bash
npm run release:packaging-readiness
```

预期失败：四个平台 `wushen-agent-*` 是 0 字节占位文件。当前 Tauri 使用 `local-dev-python`，不是独立安装包。

## 5. P0 正确性缺陷

- Agent 路径已由 Snapshot 统一恢复、选择、preview、质量、回退/前进和 GLB 导出；F002–F004 已将 Agent 对话、步骤、选择卡和四类抽屉拆出，F005 已将四类抽屉收敛到 `WorkbenchDrawerStack` 组合层，F006 已完成可访问性收敛（控件尺寸、焦点、aria-live、Escape/焦点返回）；legacy 兼容 UI、父层状态与副作用仍待后续状态机任务处理；
- Q002 已收紧兼容 bootstrap 和质量写入：`GET /active-design` 仅从有效 Agent head 或 legacy current version创建 Snapshot，空项目不写；active-design/navigation 均 `no-store`，navigation 无独立 ETag；公共 `POST :quality` 要求当前 Snapshot `If-Match` 与 `Idempotency-Key`，同键同请求重放、冲突键拒绝、旧 revision 不写报告。CORS 明确允许 `If-Match` 并暴露 `ETag`，避免桌面开发壳丢失 revision；广泛多客户端压力和生产缓存策略仍未验证；
- legacy Concept 仍是兼容只读 UI，不得被重新作为 Agent 写入真值；
- 非 GLB 的旧 Concept 导出只属 legacy，不得被宣传为 Agent 导出；
- 含糊/不支持领域已在服务端阻断并持久化为单个 clarification Item；D003 focused UI smoke、F001 characterization 与当前工作台 r3 Agent-first 路径已有通过证据；
- backup 已枚举并恢复 `agent_imported_glbs.object_path`；`agent:r3-library-backup-restore-smoke` 还通过 `/active-design` 验证了恢复后的 Agent head、Snapshot 和 export source/version 同链。

## 6. 当前代码热点

```text
apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx   约 2473 行（F002–F005 已提取 AgentConversation/AgentSelectionCard/四类抽屉/组合层）
apps/desktop/src/features/cad-workbench/AgentConversation.tsx   Agent 输入、Provider、澄清、步骤和方向
apps/desktop/src/features/cad-workbench/AgentStepItem.tsx       单个 Kernel Item 展示
apps/desktop/src/features/cad-workbench/AgentSelectionCard.tsx  分件选择和部件动作
apps/desktop/src/features/cad-workbench/agentAssetWorkspaceState.ts F010 已提交资产读取投影 reducer
apps/desktop/src/features/cad-workbench/useAgentAssetWorkspace.ts F010 已提交资产读取投影 hook
apps/desktop/src/features/cad-workbench/legacyCompatibilityDisplay.ts F011 legacy 只读显示模型
apps/desktop/src/features/cad-workbench/LegacyCompatibilityNotice.tsx F011 legacy 转换提示组件
apps/desktop/src/features/cad-workbench/componentLibraryPreferencesState.ts F012 组件库本机偏好 reducer/filter
apps/desktop/src/features/cad-workbench/useComponentLibraryPreferences.ts F012 组件库本机偏好 hook
apps/desktop/src/features/cad-workbench/viewportDisplayPreferencesState.ts F013 项目隔离的视口显示偏好 reducer
apps/desktop/src/features/cad-workbench/useViewportDisplayPreferences.ts F013 视口显示偏好 hook
apps/desktop/src/features/cad-workbench/legacyModuleGraphWorkspaceState.ts F014 legacy ModuleGraph 工作区会话 reducer
apps/desktop/src/features/cad-workbench/useLegacyModuleGraphWorkspace.ts F014 legacy ModuleGraph 工作区会话 hook
apps/desktop/src/features/cad-workbench/legacyModuleGraphOverlayState.ts F015 legacy ModuleGraph 临时叠层 reducer
apps/desktop/src/features/cad-workbench/useLegacyModuleGraphOverlay.ts F015 legacy ModuleGraph 临时叠层 hook
apps/desktop/src/features/cad-workbench/agentRenderPresentationState.ts F016 Agent 概念图展示 reducer
apps/desktop/src/features/cad-workbench/useAgentRenderPresentation.ts F016 Agent 概念图展示 hook
apps/desktop/src/features/cad-workbench/agentEditAssistPresentationState.ts F017 Agent 编辑辅助读取 reducer
apps/desktop/src/features/cad-workbench/useAgentEditAssistPresentation.ts F017 Agent 编辑辅助读取 hook
apps/desktop/src/features/cad-workbench/agentMaterialCatalogPresentationState.ts F018 视觉材质目录读取 reducer
apps/desktop/src/features/cad-workbench/useAgentMaterialCatalogPresentation.ts F018 视觉材质目录读取 hook
apps/desktop/src/features/cad-workbench/partRoleLabels.ts       内部 role 的中文显示与安全回退
scripts/smoke_c102_component_compatibility.py                   项目内组件候选与拦截 Gate
apps/desktop/src/features/cad-workbench/ComponentDrawer.tsx      组件目录和替换检视
apps/desktop/src/features/cad-workbench/MaterialDrawer.tsx       视觉材质与细节密度
apps/desktop/src/features/cad-workbench/QualityDrawer.tsx        Agent/legacy 质量检查摘要
apps/desktop/src/features/cad-workbench/ExportDrawer.tsx         按用途选择导出
apps/desktop/src/features/cad-workbench/WorkbenchDrawerStack.tsx 四类抽屉组合层；只转发 props/callback，不拥有状态真值
scripts/smoke_workbench_accessibility.mjs                         F006 可访问性静态/组件 Gate
scripts/smoke_workbench_e2e_scenarios.mjs                         T002 12 场景 E2E 报告
scripts/smoke_workbench_performance.mjs                           T003 单 WebGL/内存/bundle 门禁
scripts/smoke_g801_wedge_capsule.py                               G801 wedge/capsule GLB readback
scripts/smoke_g802_profile_extrude.py                             G802 profile/extrude GLB readback
scripts/smoke_g803_revolve.py                                     G803 revolve GLB readback
scripts/smoke_g804_transform_arrays.py                            G804 mirror/array/radial_array readback
scripts/smoke_g805_boolean.py                                     G805 restricted union/subtract readback
scripts/smoke_g806_bevel_surface_panel.py                         G806 bevel/surface panel readback
scripts/smoke_g807_blockout_diversity.py                          G807 48 blockout diversity/readback gate
apps/desktop/src/features/cad-workbench/ModuleGraphViewport.tsx 约 883 行
apps/desktop/src/features/cad-workbench/cad-workbench.css        约 1993 行
apps/agent/forgecad_agent/application/agent_asset_editing.py     约 1104 行
apps/agent/forgecad_agent/application/agent_kernel.py            约 659 行
```

不要在没有 characterization tests 的情况下整体重写这些文件。

## 7. 当前几何和材料边界

- Geometry Worker 当前执行受控 `box`/`cylinder`/`capsule`/`wedge`/`profile`/`extrude`/`revolve`/`mirror`/`array`/`radial_array`、受限 union/subtract，以及受控 `bevel_approx`/`surface_panel`；
- 四领域后端共 48 个确定性 blockout 变体（每个领域 12 个）；工作台仍只展示 3 个零基础方向，但 G812 已让每张方向卡稳定匹配其中一项，不展示完整技术目录或自由参数；
- ShapeProgram Schema 中的复杂操作多数尚未实现；
- 当前有 13 个、覆盖六类的完整字段视觉材质预设；M103 已完成受控纹理对象目录、来源/许可证边界和参数回退；M104 已完成 Material Zone 检视、中文分类筛选、关键词搜索、对象存在性和来源摘要；M105 已完成稳定 zone 选择、部件槽绑定、带 zone 的 ChangeSet 预览和非法 zone 后端拒绝；M106 已完成基于真实 `allowed_domains` 的四领域兼容筛选；M107 已将 zone 选择写入 Snapshot，并覆盖重启、版本切换和 undo/redo；C101 已将候选部件、材质上下文和组件保存名称中的稳定 role 映射为中文，未知值不显示内部标识而回退为“未命名部件”；
- 外部 GLB 是只读参考，不会自动变成 ShapeProgram；
- Agent 资产正式支持 GLB 导出，以及 R002/R003 的四视图和条件式透明爆炸概念 PNG 派生预览；R004 还支持下载当前、指纹一致的 PNG/manifest 概念图包。转台视频、OBJ/MP4 和源包仍不支持。

## 8. 推荐下一个任务

当前交接补充：`FGC-S001`–`FGC-S008` 的 ActiveDesignSnapshot 单一真值链保持不变；`FGC-G805` 已通过受限 disjoint union、贯穿槽 subtract、重叠/非贯穿失败和布尔输入数量校验；`FGC-G806` 已通过 1/3 段 bevel、±Y surface panel、面板适配和 GLB readback 失败边界；`FGC-G807` 已通过四领域各 12 个、跨领域共 48 个结构签名唯一的 blockout gate；`FGC-G816` 已让同一主视口的 display-only ShapeProgram 适配器完整显示 `box`/`cylinder`/`wedge`/`capsule`，并以柔化展示边缘、阴影和工作室环境改善概念观察，不写入几何、版本或 Snapshot；`FGC-G817/G818` 增加 `quick_sketch`/`showcase` 的有限外观质量档：展示档把面板、分缝视觉线、护板、孔洞/紧固件、灯带和线缆槽视觉线及有限 PBR 映射写入同源 ShapeProgram、GLB、AssemblyGraph、分件候选和候选 JSON，快速草图保持旧输出；工作台默认展示模型，切换只重建未保存预览。上述均为非功能概念外观，不是实际孔槽、散热、电气、工程材料或照片级渲染，且仍只有一个 WebGL canvas/context。`FGC-R001` 已通过 Snapshot 相机/灯光预设；`FGC-R002/R003` 已通过四视图、条件式透明爆炸候选、来源/alpha readback/fingerprint smoke 和桌面导出抽屉预览/单图下载接线；`FGC-R004` 已通过以当前 fingerprint 约束的 PNG/manifest ZIP 下载、stale 拒绝、固定 ZIP member/时间戳与浏览器下载断言。爆炸候选只在 GLB primitive 组与稳定 Part/AssemblyGraph 一一对应时出现；图包不得扩展成装配说明、源包、转台视频或工程渲染。

不要恢复 localStorage Agent 版本头或让 GLB 导出回退到 Concept。后续任务必须保持转换授权、Agent asset head、Snapshot、选择、质量、导出和 C104 part display 跨重启仍保持同一资产版本，并补齐广泛并发与原生安装验证。`FGC-M101`–`FGC-M107`、`FGC-C101`–`FGC-C104`、`FGC-G808`–`FGC-G812` 与 `FGC-Q002` 已完成；G812 让三方向的 build/segment/candidate/已确认资产保持同一受限视觉变体来源，仍不开放自由目录。Q002 的 `agent:q002-active-design-contract-smoke` 已覆盖空库、Agent/legacy bootstrap、no-store、质量重放、冲突键和 stale 拒绝。AgentComponent 没有正式 Module Asset 的审阅状态，不能在 UI 伪装为“已审”。当前没有可独立领取的 `ready` 任务；后续必须先定义新的原子任务。打包 sidecar、真实 Provider、广泛并发、正式审阅和签名仍是独立阻断项。

R005 更新（2026-07-13）：Agent 下载抽屉已收敛为直接 GLB、概念单图和指纹受限图包，旧用途/OBJ/源包不再出现在 Agent 路径；抽屉、12 场景浏览器 E2E 和 r3 回归通过。`FORGECAD_LOCAL_VISUAL_PACK=0 ./script/build_and_run.sh --verify` 已通过本机 `.app` 启动和 `local-dev-python` Agent 健康检查，但 `osascript -l JavaScript` 返回“osascript 不允许辅助访问”，因此原生 WebView 下载点击仍是已记录的辅助功能授权阻断，不能宣称已通过。该更新覆盖上文关于 R005 等待原生下载 E2E 的旧快照。

F007 更新（2026-07-13，脏工作区，未提交）：`useWorkbenchLifecycle` 已从 `CadWorkbenchPanel` 提取请求编号、取消/乱序响应屏障、既有错误映射和抽屉互斥/焦点返回状态；父层仍拥有 API、Snapshot hydration、ETag、ChangeSet、质量与下载副作用。新增 `desktop:f007-workbench-lifecycle-smoke`，并将其接入 desktop CI；同轮将一处 10px 辅助文字修正为 11px，F006 未被放宽。完整回归通过：typecheck/build、F001–F007、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check。该记录当时的下一项为 `FGC-F008`；其后续状态以本文件较新的 F008 更新和任务索引为准。

F008 更新（2026-07-14，脏工作区，未提交）：新增 `agentConversationState` 与 `useAgentConversationPresentation`，将输入、模式、提示、项目内 Agent thread、Kernel steps、澄清和方向卡从 `CadWorkbenchPanel` 提取为纯展示状态；项目切换会原子清空，project/request 双重检查拒绝旧项目或已取消 Turn 的迟到响应。父层仍是唯一 Agent API/SSE、legacy fallback、blockout/segmentation、提交、Snapshot、ETag、ChangeSet、质量与下载副作用入口。新增 `desktop:f008-agent-conversation-state-smoke` 并接入 CI。F008、F001、F002、F007、D003、T002（12/12）、T003、r3、typecheck、build 均通过；T003 确认单 canvas/context 与 bundle 预算保持通过。当前唯一 `ready` 为 `FGC-F009`：只抽取 blockout 候选展示协调，不得移动 AgentAssetVersion 或 Snapshot 真值。

F009 更新（2026-07-14，脏工作区，未提交）：新增 `agentBlockoutDisplayState` 与 `useAgentBlockoutDisplay`，将 GLB、ShapeProgram、分件候选和方向加载的显示缓冲从 `CadWorkbenchPanel` 提取；重选方向清空旧候选，分件失败保留仅供观察的未提交外观，项目切换/旧请求不能写回。该层不保存 AgentAssetVersion、Snapshot、ChangeSet、质量或导出 ID；父层仍是唯一 build/segment/commit、hydration 和持久写入入口。新增 `desktop:f009-agent-blockout-display-state-smoke` 并接入 CI。typecheck/build、F001、D003、T002、T003、r3 通过。当前唯一 `ready` 为 `FGC-F010`：只提取已提交资产工作区投影，不得让缓存成为版本 head。

F009 复验（2026-07-14，脏工作区，未提交）：修复首次加载时项目尚未绑定便可提交 Agent 的竞态；发送按钮现在等待项目就绪，E2E 也等待同一可交互状态。新回合会清空旧澄清/方向，避免已选类别继续遮挡新方向。`desktop:f002-agent-conversation-smoke`、F008、F009、typecheck、build、T002（12/12）与 T003 通过；r3 仍为已知基线失败，当前在 C104 重启后的 `active-design:part-display` 锁定请求等待超时，未删除或放宽该断言。当前唯一 `ready` 仍为 `FGC-F010`；开始前必须先处理或明确记录 r3 的独立基线阻断。

F010 更新（2026-07-14，脏工作区，未提交）：新增 `agentAssetWorkspaceState` 与 `useAgentAssetWorkspace`，从 `CadWorkbenchPanel` 提取当前 Snapshot 已选 Agent 资产的只读投影、选中部件、质量摘要与导航摘要。缓存只接受匹配当前 project、asset source 和 request 的读取响应；项目/source 切换清空旧投影，旧 selection/quality/navigation 无法写回。它明确不保存 asset head、Snapshot revision、ETag、ChangeSet、质量写入或导出身份；父层继续唯一负责 API、hydration、CAS、preview/confirm、undo/redo、质量写入和下载。新增 `desktop:f010-agent-asset-workspace-state-smoke` 并接入 CI；F003、F008、F009、F010、typecheck、build、T002（12/12）、T003、r3 已通过。r3 的先前 C104 重启动作超时被定位为 UI hydration/action-ready 竞态：现在等待 Snapshot 与已加载资产一致再允许动作，保留并通过锁定重启、隔离、隐藏/恢复与单 canvas 断言。当前唯一 `ready` 为 F011：只提取 legacy 只读兼容显示边界，不改变 Snapshot 或写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F011 更新（2026-07-14，脏工作区，未提交）：新增 `legacyCompatibilityDisplay` 与 `LegacyCompatibilityNotice`，将旧 Concept source 的只读说明和“让 Agent 重建可编辑资产”引导从 Agent 会话主体抽为纯显示边界。显示模型只由当前 Snapshot source 与 operation 派生；它不保存转换授权、asset head、Snapshot revision、ETag、ChangeSet、质量写入或导出身份，父层仍是唯一发起 legacy conversion authorization、CAS 和所有写入的入口。新增 `desktop:f011-legacy-compatibility-display-smoke` 并接入 CI；F002、F011、typecheck、build、F001、T002（12/12）、T003、r3 通过。当前唯一 `ready` 为 F012：仅提取组件库本机筛选/收藏/最近使用/抽屉高度偏好，禁止把偏好变成资产或版本真值。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F012 更新（2026-07-14，脏工作区，未提交）：新增 `componentLibraryPreferencesState` 与 `useComponentLibraryPreferences`，将组件库分类、关键词、审阅状态筛选、收藏、最近使用、抽屉模式与高度改为按 Project+Domain Pack 隔离的本机偏好。损坏或缺失的 localStorage 安全回退，收藏/最近使用有去重和长度边界；纯过滤 adapter 只消费真实 Module Asset 元数据，不制造审阅、许可证、质量或兼容结论。父层仍唯一读取资产目录、质量与缩略图，并唯一拥有组件替换 ChangeSet、Snapshot/CAS、API、版本和导出。新增 `desktop:f012-component-library-preferences-smoke` 并接入 CI；F004、F006、F012、typecheck、build、T002（12/12）、T003、r3 通过。当前唯一 `ready` 为 F013：只提取本机视口显示偏好，不能移动 Snapshot 相机/灯光、测量记录、renderer 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F013 更新（2026-07-14，脏工作区，未提交）：新增 `viewportDisplayPreferencesState` 与 `useViewportDisplayPreferences`，将工具、网格、线框、X 光、Connector、爆炸系数和截面偏移改为按 Project 隔离的本机显示偏好；缺失/损坏 localStorage 安全回退，工具白名单与数值边界由纯 reducer 固定。`CadWorkbenchPanel` 的 v6 通用 session 不再保存这些字段，也不再保存相机/灯光；相机/灯光继续仅由 `ActiveDesignSnapshot` 的 R001 CAS 路径读写。该层明确不持有 asset head、Snapshot revision、ETag、选择、质量、ChangeSet、导出或 renderer 身份。新增 `desktop:f013-viewport-display-preferences-smoke` 并接入 CI；R001、F006、F012、F013、typecheck、build、T002（12/12）、T003、r3、contracts、agent check 通过；T003 保持单 canvas/context，R3 重启恢复通过。当前唯一 `ready` 为 F014：仅提取 legacy ModuleGraph 本机工作区会话，不得移动 Agent Snapshot 选择、测量记录或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F014 更新（2026-07-14，脏工作区，未提交）：新增 `legacyModuleGraphWorkspaceState` 与 `useLegacyModuleGraphWorkspace`，将 legacy ModuleGraph 的 inspector tab、旧图节点/模块定位、变换坐标/吸附与测量模式改为按 Project 隔离的本机会话；损坏/缺失 localStorage 安全回退，返回图后只从现存节点恢复有效选择。旧全局 CAD session 读写已经删除；Agent source 打开空 context，不读取或写入 legacy session，当前 Agent part selection/quality/export 继续只读 Snapshot，测量标注仍使用原有项目/版本 key。新增 `desktop:f014-legacy-module-graph-workspace-smoke` 并接入 CI；F010、F011、F013、F014、F006、typecheck、build、T002（12/12）、T003、r3、contracts、agent check 通过；T003 仍保持单 canvas/context，r3 重启恢复通过。当前唯一 `ready` 为 F015：仅提取 legacy ModuleGraph 展示叠层，不得移动 Snapshot、Agent part display、质量、测量记录或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F015 更新（2026-07-14，脏工作区，未提交）：新增 `legacyModuleGraphOverlayState` 与 `useLegacyModuleGraphOverlay`，把 legacy ModuleGraph 的隐藏节点、聚焦节点、质量高亮/几何引用和组件缩略图失败记录移为纯瞬态显示层。该层以 Project+Graph context 绑定且不写 localStorage；切换 project、graph 或切到 Agent source 时会清空，图节点重载会过滤过期节点和几何引用。Agent source 的空 context 会拒绝旧图叠层动作，旧 `hiddenNodeIds` 从不与 Snapshot `part_display` 合并；Quality API、质量结果、Snapshot/CAS、版本、导出、ChangeSet、renderer props 和 Agent 部件显示仍由现有父层/服务端拥有。新增 `desktop:f015-legacy-module-graph-overlay-smoke` 并接入 CI；F010、F011、F013、F014、F015、F006、typecheck、build、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check 通过。当前唯一 `ready` 为 F016：只提取 Agent 概念图请求/展示状态，不得移动下载、Snapshot 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F016 更新（2026-07-14，脏工作区，未提交）：新增 `agentRenderPresentationState` 与 `useAgentRenderPresentation`，把当前 Agent 的四视图/概念图包 render-set、渲染/图包 loading 与请求屏障从 `CadWorkbenchPanel` 抽为纯内存展示状态。它只接受同一 project、当前 Agent asset version 与当前 request 的响应；切换 asset/source 会清空旧图，关闭抽屉会取消未完成请求并拒绝迟到响应，图包只允许使用当前 render-set fingerprint。父层仍唯一拥有 Render API、PNG/ZIP 浏览器下载、GLB 导出、Snapshot/CAS、质量、ChangeSet 和 renderer；该层没有 Snapshot、质量、ChangeSet、导出、图片 URL 或 asset head。新增 `desktop:f016-agent-render-presentation-smoke` 并接入 CI；R002–R004、F010、F015、F016、F006、typecheck、build、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check 通过。当前唯一 `ready` 为 F017：只提取 Agent 组件/结构建议读取状态，不得移动 preview→confirm 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F017 更新（2026-07-14，脏工作区，未提交）：新增 `agentEditAssistPresentationState` 与 `useAgentEditAssistPresentation`，把当前 Agent asset+selected Part 的组件替换候选、事实驱动结构建议、loading/不可用说明和请求屏障从 `CadWorkbenchPanel` 抽为纯内存展示状态。它只接受同一 project、当前 asset 与当前 Part 的候选/建议；source、project、asset 或 selection 切换即清空，迟到成功/失败均被拒绝，读取失败只显示“暂时无法读取”而不伪造结构建议。父层仍唯一拥有候选/建议 API、组件保存、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer；该层没有 Snapshot、质量、ChangeSet、导出、asset head 或 renderer。新增 `desktop:f017-agent-edit-assist-presentation-smoke` 并接入 CI；C102、C103、F010、F016、F006、F003、typecheck、build、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check 通过。当前唯一 `ready` 为 F018：只提取视觉材质目录只读加载状态，不得移动 Material Zone、preview→confirm 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F018 更新（2026-07-14，脏工作区，未提交）：新增 `agentMaterialCatalogPresentationState` 与 `useAgentMaterialCatalogPresentation`，把视觉材质目录、loading/真实回退说明和请求屏障从 `CadWorkbenchPanel` 抽为纯内存展示状态。它只接受同一 project、asset、domain pack 与 source 的目录响应；切换 context 即清空，迟到成功/失败均被拒绝。服务目录失败时只使用传入的本机内置视觉预设并明确说明；无回退预设才显示目录不可用。父层仍唯一拥有 Material Zone、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer；该层没有 Snapshot、选择、质量、ChangeSet、导出、asset head 或 renderer。新增 `desktop:f018-agent-material-catalog-presentation-smoke` 并接入 CI；M101–M107、F010、F017、F006、typecheck、build、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check 通过。当前唯一 `ready` 为 F019：只提取视觉材质筛选展示状态，不得移动选中材质、Material Zone、preview→confirm 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

G812/G813 更新（2026-07-14，脏工作区，未提交）：`resolve_blockout_variant()` 现将三方向按当前 Domain Pack、silhouette 与 direction ID 稳定映射到同一领域 12 项预审视觉 blockout 中的一项；G813 再以受限 `variation_index=0..2` 在同一三项族轮换。`BuildAgentBlockoutRequest`/`SegmentAgentBlockoutRequest` 与响应携带可选/实际 `variant_id` 和默认安全的 index；工作台只在未保存候选显示“换一版外观 / 当前第 N / 3 版”，不泄露 ID。Build 返回实际 ID/index 后，父层将其原样提交 Segment，确保 GLB、ShapeProgram、AssemblyGraph、分件候选与保存候选同源；轮换只替换临时 preview，不写 AgentAssetVersion、Snapshot、ChangeSet、质量或导出。跨包 ID、越界 index 和同幂等键改选视觉结果均在服务端拒绝，不引入自由几何、技术目录、制造/功能信息、第二 renderer 或新的 Snapshot 真值。新增并接入 backend CI 的 `agent:g813-variant-regeneration-smoke` 覆盖四领域三版轮换、候选保存、幂等和越界；F003 覆盖零基础按钮 callback。G812、G807、G6、G809/G810、F003、F009、T002（12/12）、T003、r3、contracts、typecheck/build、ruff、文档、安全、integrity、secrets 与 `git diff --check` 已通过。当前唯一 ready 是 F022，只抽取方向预览展示状态；不得把 G813 扩展为自由外观编辑、工程 CAD 或真实 Provider 质量评测。

F022 更新（2026-07-14，脏工作区，未提交）：`agentBlockoutDisplayState` / `useAgentBlockoutDisplay` 现在只保存 project/request 屏障下的 `directionId`、`variationIndex`、GLB/ShapeProgram/分件候选、加载和两种可恢复预览错误；开始轮换会原子清空旧候选，分件失败保留只供观察的 GLB，project switch/clear 会丢弃方向与轮换上下文。它不保存 AgentAssetVersion、Snapshot、ChangeSet、质量、导出或 renderer；`CadWorkbenchPanel` 仍唯一执行 build/segment API、提交和所有永久写入。扩展 F009 smoke 覆盖轮换、迟到响应、失败和 clear；F003、typecheck/build、T002（12/12）、T003（单 canvas/context、1.11 MB 主 JS 在 1.2 MB 预算内）、r3、contracts、文档/安全/integrity/secrets 与 diff check 已通过。当前唯一 ready 为 F023，只收敛普通语言预览提示，不能把展示状态扩展为任务中心、Mode 或新的 Agent 真值。

F023 更新（2026-07-14，脏工作区，未提交）：新增纯 `selectAgentBlockoutPreviewPresentation()`，只从 F022 已有预览展示状态翻译“正在生成完整外观预览”“完整外观预览已准备好”“完整外观已生成但暂不能整理部件”与“这次预览没有生成成功”。对话区和候选卡共用该来源，保留 r3 稳定的“预览状态”标识；用户看不到 variant ID、轮换 index、API 错误码或几何术语。selector 不调用 Provider、不自动重试、不创建版本或写 Snapshot；父层仍拥有 API/Turn/版本/质量/导出/renderer。新增并接入 desktop CI 的 `desktop:f023-agent-blockout-preview-presentation-smoke`，覆盖 idle、生成中、ready 与两类失败；F002/F003/F009、T002（12/12）、T003（单 canvas/context、1.11 MB 主 JS 在 1.2 MB 预算内）、r3、typecheck/build、contracts、文档/安全/integrity/secrets 与 diff check 已通过。当前唯一 ready 为 F024，只展示离线规划或真实 Provider 来源，不得触发调用、费用或泄露密钥。

F024 更新（2026-07-14，脏工作区，未提交）：新增纯 `selectAgentPlanSourcePresentation()`，只从已返回 `MechanicalConceptPlan.provider_id` 翻译“本机离线规划”“已连接模型服务生成”或“规划来源待确认”。确定性 plan 明确提示“尚未调用模型服务”，不会冒充真实模型结果；普通工作台的已配置、连接成功和失败提示也不再回显 Provider、模型、Base URL、Key、token、原始错误或费用信息。selector 不读取 Key、不联网、不创建版本、不写 Snapshot/质量/导出；父层仍拥有 Provider 配置、连接测试、Turn/API、版本与 renderer。新增并接入 desktop CI 的 `desktop:f024-agent-plan-source-presentation-smoke`，并扩展 F002 防止已配置状态泄露模型标识；F024/F002、typecheck、T002（12/12）、build、T003、r3、contracts、agent check、文档/安全/integrity/secrets 与 diff check 已通过。`FORGECAD_LOCAL_VISUAL_PACK=0 ./script/build_and_run.sh --verify` 另确认本机 `CAD 工作台.app` 可构建并启动，`local-dev-python` Agent 健康；它不等于真实 Provider 调用、外部安装、签名或公证验证。当前唯一 ready 为 `FGC-E001`：只冻结真实 Provider 四领域 truth-set 的显式、可计费评测合同，不得自动调用用户的 Provider。

E001 更新（2026-07-14，脏工作区，未提交）：新增 [AGENT_PROVIDER_EVALUATION.md](AGENT_PROVIDER_EVALUATION.md)、`evaluations/agent-provider-v1/contract.json` 与 `truth_set.json`，明确四领域各 20 条正常完整外观 Brief、20 条含糊/越界安全停止输入、固定 100 个测试条目、零默认费用、无自动重试、45 秒单请求上限、token 上限、脱敏证据和逐次人工授权。`agent:e001-provider-evaluation-dry-run` 与 contract smoke 均只读取静态 JSON：报告 `network_calls_made=0`、`asset_or_snapshot_writes=0`，并拒绝非零默认预算、CI 自动调用和截断 fixture；它们已加入 backend CI。真实 Provider baseline 仍为 external/NOT RUN，旧 Weapon R4 evaluator 不能作为通用四领域质量证据。

E002 更新（2026-07-14，脏工作区，未提交）：修正 E001 合同语义为“100 个测试条目 = 80 次正常 Provider 请求 + 20 条本地安全停止”，避免把越界输入发送给外部模型。新增隔离的 `provider_evaluation.py`、`run_agent_provider_evaluation.py` 与合成 Provider smoke：默认命令只 dry-run；真实路径同时要求三项固定 flag、正值且不超过 100 元的人工批准、操作者/时间/preflight 和有效 OpenAI-compatible 本机配置，缺配置在任何网络调用前拒绝。执行器不接触 Project、Thread/Turn、AgentAssetVersion、Snapshot、质量或导出；它仅输出内存中的脱敏 run report，固定映射 timeout/限流/鉴权/传输/结构化/策略/预算/取消，且不保存 Key、Base URL、模型 ID、原始 Brief/Response 或账单。`agent:e002-provider-evaluation-runner-smoke` 覆盖无凭据、缺确认、零/超额预算、超时、取消、无 usage、输出 token 上限、完整 telemetry 和脱敏；CI 只运行 no-call Gate。真实 Provider baseline 仍为 `EXTERNAL / NOT RUN`。当前唯一 ready 为 `FGC-G814`：把已隔离评测的概念范围预检提升为普通 Agent Turn 的 Planner 前屏障；`FGC-E003` 保持用户逐次授权且人工审阅的 external run，不是可自动领取的代码任务。

G814 更新（2026-07-14，脏工作区，未提交）：新增版本化 `ConceptScopeDecision@1` 与有限、可解释的本地策略，正常 Turn 固定经过 DomainInference → ScopeDecision → Planner。明确现实武器/制造、加工或材料配方、工程性能，以及车辆安全、适航/飞行、机器人控制/扭矩/认证请求得到 `unsupported`：Kernel 只写 completed Thread/Turn/一个 `kind=scope` clarification Item/幂等记录，绝不调用 Planner 或 Provider，也不写 Plan、blockout、AgentAssetVersion、Snapshot、质量或导出；已选领域不能绕过。普通含糊类别仍走 D003 单问题，四个非功能完整外观 Brief 仍可规划。工作台将 scope stop 显示为“请换一种外观创意描述”，不显示选择按钮或方向卡。`agent:g814-concept-scope-smoke`（10 条越界、选择绕过、D003、四领域正常）、G1/D2/D3、F002/F008、typecheck/contracts/agent check、T002（13/13，含 scope-stop 浏览器场景）、r3、desktop build、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files` 与 `git diff --check` 均通过。当前唯一 `ready` 为 `FGC-G815`：只将安全完整外观意图映射到既有视觉族，不得引入任意几何或工程参数。真实 Provider baseline 仍为 `EXTERNAL / NOT RUN`。

G815 更新（2026-07-14，脏工作区，未提交）：新增 `VisualIntentMapping@1` 与本机 `visual_intent.py`。确定性与 OpenAI-compatible Planner 输出均会用安全 Brief 的有限轮廓、细节、色彩和展示姿态分类归一化；该 mapping 只选择同一 Domain Pack 既有 0–3 视觉族，Geometry Worker 继续使用 G812/G813 catalog、现有 ShapeProgram、triangle budget、分件、preview→confirm 与 Snapshot 链。mapping 缺失或损坏时回退旧的 silhouette family，不会解释文本为尺寸、操作、脚本、自由网格或工程参数。新增 `agent:g815-visual-intent-projection-smoke`，覆盖四领域各两条 Brief、GLB/ShapeProgram 指纹分化和重复性、坏 mapping 回退；G2/G4/G5/G812/G813/G814、F002、typecheck/contracts/agent check、T002（13/13）、r3、desktop build、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files` 与 `git diff --check` 均通过。方向卡只显示普通语言，不显示视觉族 index 或字段名。当前唯一 `ready` 为 `FGC-R006`：只为未保存方向提供同源低分辨率概念图预览，不得持久化候选、增加 renderer 或调用真实 Provider。

R006 更新（2026-07-14，脏工作区，未提交）：新增 `AgentBlockoutConceptPreview@1`、`POST /api/v1/agent/blockouts:concept-preview`、纯内存的方向概念图展示状态与工作台方向卡图片。用户在保存前会看到三个同源、320×240 的软件概念图；它们只来自既有确定性 blockout 渲染，不创建候选、`AgentAssetVersion`、`ActiveDesignSnapshot`、质量报告或导出记录，也不调用真实 Provider 或增加 WebGL renderer。方向卡选择、重新生成或新 Agent 请求都会清空这组临时图片。新增 R006 后端/前端 smoke 并接入 CI；`agent:r006-blockout-concept-preview-smoke`、`desktop:r006-direction-concept-preview-state-smoke`、contracts、agent check、typecheck、F002、G815、R002、T002（14/14，含保存前无写入场景）、T003、desktop build、r3、文档/安全/integrity/secrets 与 diff check 通过。r3 首次曾因现有参数按钮等待时序超时，立即重跑通过，已如实保留为回归观察项。当前唯一 `ready` 为 `FGC-P008`：只实现本机 packaged sidecar 输入/预检合同，不下载或构建未知二进制，不接入真实 Provider、签名或发布；`FGC-P002` 仍受空 packaged sidecar 阻断。

P008 更新（2026-07-14，脏工作区，未提交）：新增 `apps/desktop/src-tauri/binaries/sidecar-inputs.json` 的 `ForgeCADPackagedSidecarInput@1`、无密钥 `packaged_sidecar_preflight.py` 与 smoke，并接入 backend CI、`release:packaging-readiness-smoke` 和 production packaging report。清单当前只声明 macOS arm64 目标、相对 sidecar 路径、`agent serve`、受限运行环境名称、health URL/响应与本机 Alpha 检查项；不含 Provider Key、Base URL、模型或用户数据。预检从不读取 secret、不联网或执行二进制：空占位稳定输出 `blocked_missing_sidecar`，临时正确的 Mach-O arm64 输入输出 `ready_for_local_alpha`，错误架构和凭据样式合同值被拒绝。`release:packaging-readiness` 仍按预期因四个空 sidecar 失败，这个失败没有被隐藏。`release:packaged-sidecar-preflight-smoke`、预检报告、agent check、desktop tauri check、docs walkthrough、integrity、安全、密钥和 diff check 通过。当前唯一 `ready` 为 `FGC-P002`：只构建当前 macOS arm64 的真实 frozen sidecar，并实际验证 packaged Alpha 启动、无 Key 初始化、工作台、GLB 导出和重启恢复；不得把 P008 结构性绿色称为安装、签名、公证或外部发布完成。

P002 完成（2026-07-14，脏工作区，未提交）：修复 packaged supervisor 在日志目录不存在时会在 spawn 前失败的问题；release 默认 `packaged-sidecar`，并将 PyInstaller onefile sidecar 放入独立进程组，正常窗口关闭会回收 wrapper 与实际 listener。为保证所有 macOS LaunchServices 路径都可靠，sidecar 在 Tauri setup 内同步完成健康检查后再交给 WebView 做幂等状态读取。`npm run desktop:packaged-sidecar-build`、`npm run desktop:packaged-sidecar-alpha-smoke`、`npm run desktop:packaged-tauri-alpha-smoke` 均通过：后者从真实 `.app` 验证 `mode=packaged-sidecar`、受管后代、临时空 Library 初始化、确定性机械臂可编辑 GLB 导出与重启恢复，输出 `provider_calls: 0`。真实界面复测还确认工作台加载以及正常关闭后端口 8000 不遗留 sidecar。没有调用 Provider、读取 Keychain、签名、公证、安装或外部发布结论；`release:packaging-readiness` 仍因其他平台 sidecar 未构建而阻断。下一项为 `FGC-P009`：仅把现有无密钥 macOS native smoke 接入专用 macOS CI/构建机，不能扩展 Provider、安装或发布范围。

可以独立并行但不得混入 S001 的任务：

- `FGC-T001`：把 G1–G7 纳入 CI；
- `FGC-B001`–`FGC-B002` 已完成：备份覆盖 imported GLB 对象，恢复后通过 API 回读 Agent head、ActiveDesignSnapshot 和 export source/version。P001/P007 已完成并解除依赖审计阻断；F006、T002、T003、G801、G802、G803、G804、G805、G806 与 G807 已完成，必须保持 F001/r3/T002/T003/G801/G802/G803/G804/G805/G806/G807 回归门以及 F002/F003/F004/F006 组件与可访问性 smoke。

若任务涉及 Agent 架构、开源依赖或开发工具，先读 [AGENT_GITHUB_REFERENCE_ARCHITECTURE.md](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md) 和 [AGENT_PLUGINS_SKILLS_DESIGN.md](AGENT_PLUGINS_SKILLS_DESIGN.md)。参考项目只提供模式；实际依赖必须经过 benchmark、许可证、体积、平台打包和退出方案审查。

## 9. 首轮基线命令

```bash
npm run agent:check
npm run contracts:types:check
npm run desktop:typecheck
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run agent:r004-render-package-smoke
npm run desktop:f004-workbench-drawers-smoke
npm run desktop:build
npm run desktop:r3-concept-workbench-smoke
```

2026-07-13 本轮结果：上述合同、Agent 检查、文档/完整性/安全/密钥门、R004 图包 smoke、抽屉 smoke、桌面 build、T002 浏览器下载断言和 r3 工作台 smoke 均通过；`desktop:build` 仍有 Vite 大 chunk warning。工作区保持用户已有的脏修改，未提交、未合并、未 push。

随后运行与任务直接相关的 smoke。不要一开始运行包含 legacy Unity/ComfyUI 环境的完整旧 release gate，除非任务就是迁移这些门。

## 10. 密钥和外部输入

- 不从聊天或历史输出复制 API Key；
- 原生运行使用 Keychain；浏览器开发使用 0600 secret file；
- 真实 Provider 评测会产生费用，必须获得明确授权；
- 刘邦已被指定为独立资产 reviewer，但“已指派”不等于已批准；
- 签名账户在外部发布阶段才需要。

2026-07-14 A002 更新（脏工作区，未提交）：`scripts/run_agent_provider_evaluation.py` 新增显式 `--provider-config-source macos-keychain`，只在获授权的隔离评测进程内读取 Tauri 使用的 `ForgeCAD Agent Provider/default` Keychain 项和非敏感 metadata；密钥不会进入环境、报告、ledger、日志或普通 Agent。`npm run agent:e001-provider-evaluation-dry-run`、`agent:e001-provider-evaluation-contract-smoke`、`agent:e002-provider-evaluation-runner-smoke`、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files`、`agent:check` 与 `git diff --check` 均通过；当前本机 Provider metadata/Keychain 为空，`--provider-config-source macos-keychain` 在任何网络调用前返回 `E002_PROVIDER_UNCONFIGURED`。下一项仍为 `FGC-E003` external：用户须在工作台保存已轮换的 Keychain 密钥、为一次具体 run 确认预算与操作者，然后由非执行者审阅脱敏报告和 Provider 控制台账单；本轮未执行真实 Provider 请求、未提交或 push。

## 11. 交接给下一位 Codex

结束任务时更新：

- `CODEX_TASK_INDEX.md` 任务状态；
- 本文件的已知失败或新阻断；
- `CAPABILITY_GATE_MATRIX.md` 的能力证据；
- 任务相关的 API、状态、测试或操作文档。

交接必须列出真实命令结果、工作区是否干净、是否提交/推送，以及下一项已解除阻断的任务 ID。
