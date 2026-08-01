# ForgeCAD 测试策略

版本：2026-07-29
状态：生产级测试目标与当前差距

## 0.1 2026-08-01 W4 集成回归记录

W1/W2/W3 已在 `7758a01` 基线上按顺序合并，W3 修复链随后 fast-forward 至 `8af0abd`。主环境后续 F025、T002 14/14、typecheck/build/F026/F006 与 U004 PBR smoke/rendered Playwright PASS；W4 原先三个 U004 bridge tests PASS。桥接重跑因 rustc 长时间无进展中止，必须标为 KNOWN FAIL/未形成新证据。真实 DeepSeek、真实千问、packaged app、未见输入和真人评分仍须单独登记，不能由 fixture、离线构建、截图或自动评分替代。

## 1. 测试原则

1. 测试证明当前能力，不证明文档中的未来目标；
2. 确定性 smoke、真实 Provider、浏览器 E2E、原生 Tauri 和安装测试分别记录；
3. 每个副作用都验证幂等、取消、失败、重试和重启；
4. 每个版本操作都验证源版本、目标版本和不可变父版本；
5. 安全失败必须发生在执行、联网或写盘之前；
6. 绿色 CI 只对其 commit 和实际执行的命令有效。
7. 工程正确性、视觉相似度、独立真人质量和商业单位经济是四类证据，不能互相替代；
8. 开发模型、VLM 或实现作者的评分不能代替独立真人。
9. 类别开放由未知对象不模板回退、表示能力路由和跨类别分布证明；四领域/E005 只能作为机械回归，不能替代 U005。

## 2. 测试层级

### L0：静态与合同

```bash
npm run agent:check
npm run contracts:types:check
npm run desktop:typecheck
npm run desktop:build
npm run desktop:tauri-check
```

已接入：Python/Rust lint、Python/Rust 依赖漏洞审计、生成文件漂移和 Markdown 文档门禁。依赖审计在 CI 的 `dependency-audits` job 生成 npm/Python/Rust JSON artifact，高危返回非零并阻断合并。

### L1：领域单元测试

需要从脚本式 smoke 中拆出可定位的单元测试：

- 当前 Domain Pack 分类、含糊/未知领域的零写入屏障和后续澄清回归；U002 后增加 SubjectProfile/RepresentationPlan 与 typed limitation；
- ShapeProgram validator；
- AssemblyGraph 环、孤儿、Joint 和 Connector；
- ChangeSet 状态机和 stale base；
- AgentAssetVersion head 原子更新；
- export source version；
- GLB parser 边界和预算；
- Provider 错误映射和超时。

当前已建立 `apps/agent/tests/` 常规 pytest 套件以及 desktop Rust workspace 测试，这已替代“当前缺少常规 `tests/` 单元测试套件”的旧记录。K003 的 canonical code gate 已覆盖 Rust app-server/protocol/core/desktop、Python Agent/ports/restricted geometry、迁移、CAS、对象库和 GLB 产品状态；源码绑定的五层 Gate 以 `output/k003-layered-gate-final-source-20260718/report.json` 和同目录 manifest 输出宿主、纯 Rust Core、Rust↔Python 合同、packaged 生命周期和工作台报告，并冻结 `host → rust_core → rust_python_contract → packaged → workbench` 的顺序及 Gate 前后 source fingerprint。尚未设全仓覆盖率阈值，因此不能把测试数量解释为所有未来 Recipe、视觉质量或发布平台均已完成。

`FGC-K001` 的 Rust 测试覆盖 initialize 前拒绝、版本/能力协商、稳定 JSON-RPC envelope/ID/error、malformed/oversize frame、重复/未知 ID、取消竞态、通知 ack、有界队列满/慢消费者、cursor replay、断线与进程重启，以及 compatibility adapter 的 method + segment-shape/header/body 白名单。`ForgeCADReadOnlyResourceCompatibility@1` 另验证 custom scheme 只接受 localhost GET、无状态、复用相同路径/header 策略并拒绝递归 app-server 或任何写请求。

`FGC-A001` 增加 Provider Conversation 单元覆盖：第二轮保留前一轮用户/助手消息、普通语言微调复用已绑定领域、Snapshot 只以摘要进入最新请求、缓存 token 映射、日预算预留/结算、无 usage 的停止边界，以及 Provider HTTP 不在 SQLite 事务中执行。fake Provider 只验证协议和用量解析，不能替代真实 DeepSeek 质量或费用证据。

### L2：服务集成

当前 G1–G7 smoke 验证最小纵向切片：

```bash
npm run agent:g1-kernel-smoke
npm run agent:g2-contracts-smoke
npm run agent:d1-domain-inference-contract-smoke
npm run agent:d2-domain-inference-service-smoke
npm run agent:g3-shape-program-smoke
npm run agent:g4-mechanical-planner-smoke
npm run agent:g5-geometry-worker-smoke
npm run agent:g6-segmentation-smoke
npm run agent:g6-material-catalog-smoke
npm run agent:g6-asset-editing-smoke
npm run agent:g6-component-registry-smoke
npm run agent:g7-external-glb-import-smoke
npm run agent:s1-active-design-snapshot-smoke
npm run agent:s2-active-design-snapshot-smoke
npm run agent:s3-active-design-api-smoke
npm run agent:r001-render-preset-smoke
npm run desktop:s5-active-design-machine-smoke
```

`FGC-G814` 另验证普通 Agent Turn 的本地范围屏障：`npm run agent:g814-concept-scope-smoke` 同时校验 `ConceptScopeDecision@1` JSON Schema/Pydantic、10 条明确越界输入的零 Planner 调用与零 Plan/blockout/资产/Snapshot/质量/导出写入、已选 Domain Pack 不能绕过屏障、D003 含糊澄清，以及四个非功能完整外观 Brief 仍可进入确定性 Planner。`npm run desktop:f002-agent-conversation-smoke`、`npm run desktop:f008-agent-conversation-state-smoke` 与 `npm run desktop:t002-workbench-e2e-scenarios` 验证工作台给出可读 scope-stop、没有方向卡或资产写入。该规则是有限且版本化的产品范围策略，不能被视为完整内容安全评测或真实 Provider 质量证据。

`FGC-G815` 使用 `npm run agent:g815-visual-intent-projection-smoke` 验证 `VisualIntentMapping@1` Schema/Pydantic、四领域各两条安全完整外观 Brief、同一 plan ID 下的不同受限视觉族、ShapeProgram/GLB 指纹重复性与损坏映射的旧轮廓安全回退。它还回归 G4 的 Provider 结果本机归一化、G812/G813 的 build/segment/candidate 同源，以及 F002 的方向卡不显示视觉族索引或字段名。该 Gate 只证明本机规则选择预审概念外观，不证明真实 Provider 的创意质量、自由风格迁移或工程 CAD。

`FGC-G816` 使用 `npm run desktop:g816-shape-program-preview-smoke` 验证主视口的 display-only ShapeProgram 适配器能显示 `box`、`cylinder`、`wedge`、`capsule` 与现有 `bevel_approx` 来源，且选择高亮、隐藏、阴影、PBR 展示材质和资源释放保持正确。它回归 `desktop:typecheck`、`desktop:build`、`desktop:r3-concept-workbench-smoke`、`desktop:t003-performance-smoke`、G801、G806 和 G807；T003 继续要求单一 canvas/WebGL context，并在重启后检查 renderer 资源。该 Gate 不证明工程倒角、真实材料、照片级渲染或更高精度 GLB 几何。

`FGC-G817` 使用 `npm run agent:g817-showcase-quality-smoke` 验证四领域的 `quick_sketch`/`showcase` profile：展示档稳定添加有限概念外观部件、三角数增加、GLB readback 一致、快速草图不携带外观层、展示部件不产生参数控件，并且 build、segment、候选 JSON 和 AssemblyGraph 共享同一 profile。它还验证未知 profile 被 Pydantic 拒绝；回归 G6、G801、G806、G807、G812、G813、G815、G816、F002、T002、r3、T003、typecheck/build。该 Gate 不证明真实材料、功能、工程细节或照片级模型质量。

`FGC-G818` 使用 `npm run agent:g818-visual-detail-grammar-smoke` 验证四领域的展示档稳定产生七类有上限的外观细节：面板、分缝视觉线、护板、灯带、线缆槽视觉线、孔洞点缀和紧固件点缀；快速草图没有这些部件。Gate 同时验证 ShapeProgram/GLB/AssemblyGraph/segmentation 同源、视觉部件没有参数或 Joint、七个有限 GLB PBR 材质槽可回读，以及机器人视觉部件不会成为 Joint 目标。它回归 G817、G6、G801、G806、G807、G812、G813、G815、G816、F002、T002、r3、T003、typecheck/build。该 Gate 只证明轻量概念展示层，绝不证明真实材料、孔槽、散热、电气、固定、工程细节或照片级质量。

`FGC-R002` 已验证同一 Snapshot/AgentAssetVersion 生成 iso（3/4）、front、side、top 四张 PNG：每张图片都有来源 `asset_version_id`、宽高、PNG signature/IHDR readback、字节数和 SHA-256；相同输入重复生成的 render-set fingerprint 一致。`FGC-R003` 在同一 Gate 上验证：部件 primitive 组和稳定 Part ID 一一对应时生成条件式 `exploded_iso`，其透明 alpha、`part_ids`、模式和 fingerprint 可重复；映射数量不足时明确不生成候选。`FGC-R004` 验证必须携带该 fingerprint 的 ZIP 仅包含固定 `manifest.json`/PNG member、manifest hash/readback/模式/Part ID、固定 ZIP 元数据与重复下载字节一致；缺 fingerprint 或旧 fingerprint 必须拒绝。命令为 `npm run agent:r002-render-views-smoke`、`npm run agent:r003-exploded-views-smoke`、`npm run agent:r004-render-package-smoke`，桌面接线运行 `npm run desktop:typecheck`、`npm run desktop:f004-workbench-drawers-smoke` 与 `npm run desktop:t002-workbench-e2e-scenarios`。它们是软件栅格化概念图，不是工程渲染、装配说明或照片级材质证明。

`FGC-R006` 的后端 `AgentBlockoutConceptPreview@1` 继续作为 legacy 回归：`npm run agent:r006-blockout-concept-preview-smoke` 覆盖四领域、固定 320×240 PNG、重复 hash、与同 plan/direction/variation 的 build `variant_id`/`topology_hash` 同源，以及预览调用不会写入幂等、候选、资产、Snapshot、质量或导出表。F026 已从产品前端移除三方向 UI；2026-08-01 又删除了无运行时引用的 `agentDirectionConceptPreviewState`、hook 和独立 desktop smoke。T002 中历史三方向场景只能作为 legacy fixture 读取，不能要求恢复产品 UI。后端 smoke 不是下载图、真实渲染、工程图或 Provider 质量评测。

`FGC-P008` 使用 `npm run release:packaged-sidecar-preflight-smoke` 验证 `ForgeCADPackagedSidecarInput@1`：空占位 sidecar 必须得到可读的 `blocked_missing_sidecar`，预检绝不读取 Provider secret、联网或执行 sidecar；临时正确的 Mach-O arm64 输入必须得到 `ready_for_local_alpha`，错误 CPU 架构和 secret-like 合同值必须拒绝。该状态只证明 P002 输入结构已准备好，P002 仍必须真实启动 sidecar、探测 `/api/health` 并验证首次初始化与重启恢复。

`FGC-P002` 以 `npm run desktop:packaged-sidecar-alpha-smoke` 执行真实 macOS arm64 frozen sidecar，并以 `npm run desktop:packaged-tauri-alpha-smoke` 通过 LaunchServices 启动实际 `.app`，再以 K002/K003 native smoke 验证原生双启动：临时空 Library、无 Provider 环境、`/api/health`、`mode=packaged-sidecar`、确定性机械臂概念到可编辑资产的 GLB 导出、Rust-owned K001/K002/K003 状态、readback/render package，以及重启后的同一资产读取/导出和语义 hash。后者还验证监听 sidecar 为桌面进程后代；真实界面复测验证正常窗口关闭回收 listener。所有 smoke 都不读取 Keychain、不发送 Provider 请求，也不证明安装器、签名、公证或外部分发。当前本机 arm64 runtime 证据已通过；未准备的平台目标、签名、公证、安装和生产视觉质量仍阻断发布。

K001 以 `npm run k001:code-gate` 和 `npm run k001:packaged-gate` 作为两个不可互相替代的 Gate。code gate 包含上述 56 个 Rust 测试、协议 oracle/TypeScript transport、A003/A004/S008、T002/T003/r3、contracts/typecheck/build/Tauri 与安全门；T002/r3 的 browser compatibility observer 必须解包 JSON-RPC `compat/http` response 并断言内部业务 status/body，不能把外层 `/frames` HTTP 200 当作业务成功。packaged gate 必须启动真实 `.app` WebView，经 Rust protocol + `forgecad-resource` 完成 Project、Thread、Turn、SSE、blockout/segment/commit、ChangeSet edit/undo/redo/export、JSON 与二进制 GLB；重启阶段从上一 cursor/事件 ID 续传、创建新 Turn，要求首个恢复事件等于上轮最后序号 + 1、后续连续，并验证同一 asset/GLB SHA。直接调用 Python helper、只跑 Rust 单测或只启动 sidecar 均不能替代该证据。

K002 以 `npm run k002:code-gate` 和 `npm run k002:packaged-gate` 作为不可互相替代的 canonical Gate。code gate 聚合 K001、上述 173 项 Rust/69 项 Python Agent/51 项 ports、13 项 Product Tool manifest、native ForgeApi、T002 14/14、T003、r3、contracts、typecheck/build、Tauri、安全和密钥检查；覆盖未配置/Provider 错误、thinking Tool Call 内存续传、预算、取消、迟到结果、terminal replay、approval policy、环境密钥隔离和脱敏 trace。packaged gate 必须保留 K001 WebView 业务/重启链，并额外执行 K002 原生双启动。

2026-07-17 的 K002 packaged 结果仍作为 Agent 生命周期迁移历史保留。当前 K003 五层聚合报告是 `output/k003-layered-gate-arm-final-20260719-v3/report.json`：它验证 `rust_core_state_owner=true`、Python product/lifecycle route=410、无 Python database/object/provider path、重启语义 hash 一致、T002 14/14 与工作台 renderer/quality/export，并记录 `status=passed`、`exit_code=0`、`source_changed=false`。本轮额外回归可选 SQLite WAL/SHM 在权限加固期间消失的瞬态竞态，以及 Provider session 建立失败仍须持久化脱敏 gateway Item 的离线重启合同。Host preflight 把 `kern.num_vnodes` 限定为 kernel cache proxy：它只触发 warning/corroboration；达到阈值后必须在 tmp 和隔离 library 上执行可终止、有界的真实容量 probe。只有 `ENFILE`/`EMFILE`、`ENOSPC`、`EIO`、`EROFS`、timeout、cleanup residue 或 worker residue 才构成 hard fail，且结构化报告不得泄露绝对路径。`npm run k003:layered-gate-self-tests` 已接入 Linux CI，完整 `npm run k003:layered-gate` 已接入 macOS packaged job并上传结构化报告。该证据仍不证明真实 DeepSeek 质量、M108B 视觉质量、安装器、签名或公证。

`FGC-M101` 必须同时验证旧 `MaterialPreset@1` payload 的安全默认迁移、完整 PBR 字段、内部纹理 asset ID 格式和越界拒绝；命令为 `npm run agent:m101-material-contract-smoke`，并保留 `agent:g2-contracts-smoke`、`agent:g6-material-catalog-smoke` 和 `contracts:types:check` 回归。

`FGC-M102` 必须验证 13 个唯一内置预设覆盖 metal、polymer、rubber、composite、glass、coating 六类；所有结果保持 `visual_only=true`，不包含工程字段。命令为 `npm run agent:m102-material-catalog-smoke`。

`FGC-M103` 必须验证 `MaterialTextureObject@1` 的 PNG/JPEG/WebP 受控内容登记、尺寸/字节/哈希、相对对象路径、来源/许可证组合、重复幂等、对象缺失回退和目录存在性摘要；禁止 URL、data URI、绝对路径和自动许可证推断。命令为 `npm run agent:m103-material-texture-smoke`，并保留 `agent:m102-material-catalog-smoke`、`contracts:types:check`、`desktop:typecheck` 和 `release:secrets-files` 回归。

`FGC-M104` 必须验证 Material Zone 抽屉显示选中部件/区域、六类中文筛选、名称/标签搜索、真实纹理存在性、来源/许可证摘要和对象缺失时的参数回退；组件不得创建第二 renderer 或直接写版本。命令为 `npm run desktop:m104-material-zone-smoke`，并保留 `desktop:f004-workbench-drawers-smoke`、`desktop:typecheck`、`desktop:build`、`desktop:t002-workbench-e2e-scenarios`、`desktop:r3-concept-workbench-smoke` 和 `contracts:types:check` 回归。

`FGC-M105` 必须验证每个真实 `material_zone_id` 可被选择，MaterialDrawer 的显式预览动作将稳定 `part_id`/`material_zone_id` 传回父层，服务端拒绝不属于当前部件的 zone，且永久确认仍走 Agent asset ChangeSet 和 ActiveDesignSnapshot。命令为 `npm run desktop:m105-material-zone-binding-smoke` 与 `npm run agent:g6-asset-editing-smoke`，并保留 `desktop:m104-material-zone-smoke`、`desktop:f004-workbench-drawers-smoke`、`desktop:typecheck` 和 `contracts:types:check` 回归。

`FGC-M106` 必须验证 MaterialDrawer 只依据真实 `allowed_domains` 进行四领域正/负筛选；当前选中的不适配材质仍可被看见并明确提示，全部材质可显式切换，未知领域不猜测。命令为 `npm run desktop:m106-material-domain-filter-smoke`，并保留 M105/M104、F006、T003、T002、r3、`desktop:build` 和 `contracts:types:check` 回归。

`FGC-M107` 必须验证 `selected_material_zone_id` 的 Snapshot 合同、0030 迁移、API/CAS 所有权、重启读取以及版本切换/undo/redo 的部件与 zone 保留；非法 zone、legacy zone 和 stale revision 必须拒绝或保持 null。命令为 `npm run agent:m107-active-zone-smoke`，并保留 S8、M106、M105、S5、T002、r3、typecheck、build 和 contracts 回归。

`FGC-C101` 必须验证四领域稳定内部 role 的中文显示、关节角色判别和未知 role 的“未命名部件”安全回退；映射只能位于桌面显示边界，不得改变 `part_id`、AssemblyGraph、Snapshot 或候选匹配 role。命令为 `npm run desktop:c101-part-role-labels-smoke`；`desktop:f003-agent-selection-card-smoke` 额外验证候选卡不暴露 `joint_elbow` 等内部标识，并保留 F004、F006、T002、r3、typecheck 和 build 回归。

`FGC-C102` 必须验证项目内 Agent 组件的候选结论只依据启用状态、相同领域、相同稳定 role、来源资产最新质量和目标连接保留；来源 `unavailable`/`failed`、停用、跨领域或 role 不同必须不可替换，后端 ChangeSet 也必须拒绝质量不可用的手工请求。命令为 `npm run agent:c102-component-compatibility-smoke`，其中包含 HTTP 端点、服务、负例和 preview-first 边界；`desktop:f003-agent-selection-card-smoke` 验证零基础用户可读理由。该 Gate 不把正式 Module Asset 的审阅状态伪装进 AgentComponent。

`FGC-C103` 必须验证 `AgentStructureSuggestion@1` 只从当前 AssemblyGraph、稳定 role、受限 ShapeProgram 输出与已有连接事实得出；外部 GLB、锁定/关节或事实不足不得猜测。手工伪造 suggestion ID 必须被 ChangeSet 拒绝；真实建议必须先 preview（不改版本）、再 confirm（创建子版本），并在确认后保持质量、GLB 导出与 ActiveDesignSnapshot 同一版本。命令为 `npm run agent:c103-structure-suggestions-smoke`；`desktop:f003-agent-selection-card-smoke` 验证候选语言，`desktop:r3-concept-workbench-smoke` 验证事实不足时工作台明确不建议。该 Gate 不证明自由网格切割、工程连接或结构结论。

`FGC-C104` 必须验证 `ActiveDesignPartDisplay@1` 通过 Snapshot 的 revision/ETag/Idempotency-Key CAS 持久化，旧 Snapshot 缺少该字段可安全加载；锁定必须让服务端拒绝相关 Agent ChangeSet，隐藏/隔离必须拒绝不可见 part 的选择并在必要时清空当前选择；资产版本推进只保留仍存在的稳定 part ID。命令为 `npm run agent:c104-part-display-smoke`；`desktop:f003-agent-selection-card-smoke` 验证零基础控件与锁定禁用状态；`desktop:r3-concept-workbench-smoke` 验证锁定重启恢复、隐藏/单独查看/显示全部，并由 `desktop:t003-performance-smoke` 验证依然只有一个 canvas/WebGL context。该 Gate 不证明工程装配锁定、制造约束或额外 3D 预览器。

`FGC-G808` 必须验证 `EditableParameterBinding@1` 的 JSON Schema、Pydantic、OpenAPI/TypeScript 生成物一致；旧 `BlockoutPartCandidate` 缺少 bindings 时仍可加载；新声明只允许六个现有 position/scale 路径，强制有限默认值/范围/步长、单位匹配、缩放和位置边界及 Part 内 ID/路径唯一。命令为 `npm run agent:g808-editable-parameter-bindings-smoke`、`npm run contracts:types:check` 和 `npm run agent:g3-shape-program-smoke`。该 Gate 不证明参数已在 UI 中显示、会执行新的 ShapeProgram 或扩展现有 ChangeSet 白名单。

`FGC-G809` 必须验证 ChangeSet 在非空 `EditableParameterBinding@1` 存在时仅接受该 Part 的已声明路径，并拒绝未声明、越界和非步进数值；C104 锁定必须先于参数声明拒绝。旧资产的空绑定只兼容原六条 position/scale 路径及原全局概念边界，不能变成任意路径。smoke 还必须证明 preview 不创建版本、confirm 创建不可变子版本且 Agent head、Snapshot/export 与绑定声明保持一致。命令为 `npm run agent:g809-parameter-binding-changesets-smoke`，并回归 G6/C104/G808 与 `contracts:types:check`。该 Gate 不证明参数面板、自由尺寸、单位转换或新的 ShapeProgram 执行。

`FGC-D005` 必须在四领域真实生成/编译结果上验证 Style Token/Recipe、语义部件槽、G808 ratio binding、G819 operation manifest 与 G826 GLB surface provenance 一致，并覆盖锁定、越界、步长、无绑定回退、preview 取消/确认、Q003、重启和 undo/redo。命令为 `npm run agent:d005-semantic-proportions-smoke` 与 `npm run desktop:d005-semantic-proportions-smoke`；UI Gate 还检查中文、相对倍数和非工程提示。它不证明 Agent 已自动选择配方。

`FGC-G810` 必须验证四领域默认确定性分件均有至少一个非空 `EditableParameterBinding@1`，且每个声明只对应唯一 role 的单一 `box`/`wedge` ShapeProgram 输出和真实 `args.size`。三条比例声明固定为 `scale.x/y/z`、ratio、默认 1、范围 `0.6..1.4`、步长 `0.1`；重复 role 和当前 cylinder/capsule 输出不得伪造独立控制。命令为 `npm run agent:g810-generated-parameter-bindings-smoke`，并回归 G6/G809/G807/C104。该 Gate 不证明参数 UI、operation-level 编辑、自由尺寸或新几何执行。

`FGC-G811` 必须验证零基础参数控件只读取当前选中 Part 的 `editable_parameter_bindings` 和当前 AgentAssetVersion 的 AssemblyGraph 变换：每项显示声明名称、当前值、范围、步长与中文“比例（ratio）”单位；减少/增加仅以一个声明步长创建 `set_part_parameter` preview。空声明显示明确不可编辑说明而不生成猜测控件，锁定或已有 preview 时控件禁用；取消、确认、刷新与版本切换仍从 Snapshot/活动资产读取，且工作台保持单一 WebGL canvas。命令为 `npm run desktop:f003-agent-selection-card-smoke`、`npm run desktop:t002-workbench-e2e-scenarios`、`npm run desktop:r3-concept-workbench-smoke`、G809/G810、typecheck、build 和性能回归。该 Gate 不支持自由输入、单位换算、工程尺寸或新的几何执行。

`FGC-G812` 必须验证四个 Domain Pack 的每张方向卡都能稳定解析为同包的预审视觉 `variant_id`；build、segment、GLB、ShapeProgram、AssemblyGraph、候选 JSON 与确认资产中已保存的 ShapeProgram/AssemblyGraph 必须同源。显式跨包 ID、同幂等键重选 ID 和缺失的目录 ID 必须拒绝；普通工作台请求不输入变体文本，仍保持预览不写版本、确认才进入既有 commit → ActiveDesignSnapshot。命令为 `npm run agent:g812-direction-variants-smoke`，并回归 G807、G6、G809/G810、F009、T002、T003、r3、typecheck、build 和 contracts。该 Gate 不证明自由变体选择、自由几何、制造/功能参数、真实 Provider 质量或第二 renderer。

`FGC-G813` 必须验证每个领域、每张方向在 `variation_index=0..2` 时只轮换该 silhouette family 的三项不同预审视觉外观；build 返回的实际 `variant_id`/index 必须原样进入 segment、候选 JSON、ShapeProgram 和 AssemblyGraph。旧响应缺失 index 时安全默认为第 1 版，越界 index 和同一幂等键改变 index 必须拒绝。`desktop:f003-agent-selection-card-smoke` 必须验证零基础候选卡只显示“换一版外观”“当前第 N / 3 版”和不影响已保存设计的说明，点击只请求一次新 preview。命令为 `npm run agent:g813-variant-regeneration-smoke`、`npm run desktop:f003-agent-selection-card-smoke`，并回归 G812、F009、T002、T003、r3、typecheck、build 和 contracts。该 Gate 不证明自由变体目录、自由几何、版本覆盖、真实 Provider、工程 CAD 或第二 renderer。

`FGC-F022` 必须验证方向 preview 的纯展示状态仅保存 project、方向、轮换位置、GLB/ShapeProgram/分件候选、加载与可恢复错误；新 preview、过期请求、分件失败、project switch 和 clear 都必须正确归一化。该层不得拥有 asset version、Snapshot 或 ChangeSet。命令为 `npm run desktop:f009-agent-blockout-display-state-smoke`，并回归 F003、T002、T003、r3、typecheck 和 build。该 Gate 不证明新的几何、版本、API 或 Agent 能力。

`FGC-F023` 必须验证纯 selector 只从 F022 的同一 project/request 展示状态翻译“正在生成完整外观预览”“完整外观预览已准备好”“完整外观已生成但暂不能整理部件”或“这次预览没有生成成功”。对话区与候选卡必须使用同一来源，提示中不得出现 `variation_index`、variant ID、API 错误码或几何技术术语；selector 不触发 Provider、写入或自动重试。命令为 `npm run desktop:f023-agent-blockout-preview-presentation-smoke`，并回归 F002、F003、F009、T002、T003、r3、typecheck 和 build。该 Gate 不证明新的模型、版本、API 或真实 Provider 质量。

`FGC-F024` 必须验证纯 selector 只从已返回的 `MechanicalConceptPlan.provider_id` 翻译“本机离线规划”“已连接模型服务生成”或安全的“规划来源待确认”。确定性 plan 不得伪装成真实模型结果；普通工作台状态、连接成功和失败提示均不得显示 Provider、模型、Base URL、Key、token、原始错误或费用信息。selector 不读取密钥、不联网、不写入版本、Snapshot、质量或导出，也不构成真实 Provider 质量评测。命令为 `npm run desktop:f024-agent-plan-source-presentation-smoke`、`npm run desktop:f002-agent-conversation-smoke`，并回归 T002、typecheck 和 build。

`FGC-Q002` 验证 Snapshot bootstrap 与质量检查合同：首次 `GET /active-design` 只从有效 Agent head 或 legacy current version创建一行，空项目不创建 Snapshot，两个读取端点均为 `Cache-Control: no-store`；navigation 明确是无独立 ETag 的派生读模型。质量检查必须带 `Idempotency-Key` 和当前 Snapshot `If-Match`，相同键/资产/revision 重放原报告，同键不同 revision 冲突，旧 revision 返回 stale 且不写新报告。该 smoke 同时验证本机浏览器开发壳的 CORS 预检允许 `If-Match`，并可读取暴露的 `ETag`。命令为 `npm run agent:q002-active-design-contract-smoke`、`npm run agent:s8-active-design-navigation-smoke` 和 `contracts:types:check`。该 Gate 不等于广泛多客户端压力测试或生产缓存策略。

必须增加负例矩阵：重复确认、过期 ChangeSet、非法 part、跨项目组件、错误材质、超预算 GLB、崩溃恢复和数据库迁移回滚。

`FGC-G819` 的操作清单 Gate 已使 Schema、Pydantic、运行时、质量入口与生成类型只接受同一操作集合；未知或缺执行器在副作用前返回 `UNSUPPORTED_RUNTIME_OPERATION`。`FGC-Q003` 的 `agent:q003-compile-readback-quality-smoke` 已覆盖四领域真实 GLB readback、质量/导出一致、编译或 readback 失败、未知操作、旧报告隔离和重启幂等；真实 pre-v2 报告在当前 v2 导出下必须由 GET 与幂等重放返回 `stale_compile_readback/unavailable`，质量不再以 box/cylinder 常数平行估算，也不会复用过期视觉合同。

ADR-0011 的几何子链必须逐项建立独立 Gate。G820 已覆盖合同。`FGC-G821` 已由 `agent:g821-profile-solid-fidelity-smoke` 覆盖曲线重采样、带孔/无孔 Extrude、开放 ribbon、完整/部分 Revolve、轴点、封盖、seam、bounds/triangle、UV0/normal accessor、逐三角 surface ranges、closed/boundary/non-manifold/degenerate readback、重复字节和预算/非法输入拒绝，并回归旧 G802/G803。`FGC-G822` 已由 `agent:g822-loft-smoke` 覆盖四领域壳体、不同主轴/截面/尺寸/位置/有限 twist、固定 seam、封盖、重复字节、triangle/bounds/normal/UV0/surface/topology readback，以及排序、采样点数、翻转、自交、退化、bounds 和写出前三角预算拒绝。`FGC-G823` 已由 `agent:g823-sweep-smoke` 覆盖直线、折线、多点平滑近似、有限 twist、开/闭 path、cap、frame 连续、重复字节、UV0/normal/surface/topology readback，以及零长度/过短段、180° 翻转、自交、点数/bounds/triangle 超限。`FGC-G824`–`FGC-G824D` 已用隔离 benchmark、双平台 frozen artifact、provenance/readback、取消/超时、真实临时 SQLite/对象库/UnitOfWork 原子提升和许可证/预算证据解除 ADR-0012，并由 ADR-0013 只选择 Python。`FGC-G825` 已由 `agent:g825-feature-csg-smoke` 覆盖唯一 Manifold handler、壳体 union、窗洞/轮拱/凹槽 subtract、coplanar、退化/非封闭/深度/输入/三角预算拒绝、取消/超时、重复 GLB 与 feature result hash、surface/material/zone/backside provenance、旧 G805 fixture、preview 零版本副作用、confirm 不可变子版本和质量/导出 GLB 同源 readback；任何失败不得输出部分 GLB。`FGC-G826` 已由 `agent:g826-surface-readback-smoke` 覆盖 primitive、Extrude/Revolve/Loft/Sweep、受限 edge finish/trim、mirror/array、Manifold CSG 的 split/weighted normal、UV0、tangent、稳定 face/source-face ID 与 face→part/zone 映射；并覆盖重复 hash、缺失/损坏 tangent、UV 退化、空/重叠 zone、损坏 face ID、半径/细分/三角预算拒绝。G819/Q003/G821–G825 与 M101–M107 继续作为兼容 Gate。HTML/SVG/GSAP 都不能作为这些 Gate 的几何证据。

`FGC-A003` 已由 `npm run agent:a003-provider-gateway-smoke`、`npm run desktop:a003-provider-connection-smoke` 和 Rust `cargo test` 覆盖 Provider metadata/私密凭据/capability preflight、实际网络调用标记、SSE stream、普通 Turn 与 `provider:check` 取消、usage/cache、DeepSeek 400/401/402/422/429/500/503、网络/超时、空内容、非法 JSON、Schema 错误、脱敏、重启读取以及“真实 Provider 失败不得进入 legacy Planner”。本机 Alpha 使用 Rust-owned、generation-bound 的私密文件：目录 0700、key 0600、原子替换且拒绝 symlink；普通启动只读 metadata，显式连接检查与真实 Turn 各读取一次短生命周期快照，terminal 后 zeroize。旧 `macos-keychain` metadata 不自动读取，固定返回 `PROVIDER_CREDENTIAL_MIGRATION_REQUIRED` 并要求用户显式重存一次；当前 Alpha 因而不依赖 ad-hoc 签名 identity，也不触发系统钥匙串密码弹窗。UI smoke 固定 `secret_status=not_checked` 的未授权展示。该 Gate 使用本机 fake Provider，不构成真实四领域模型质量或账单证据。`FGC-F025` 已由 `npm run desktop:f025-legacy-isolation-smoke` 覆盖 Agent-active/legacy-read-only 两条路径、显式延迟读取、迟到响应失效、只读旧信息、Agent-only 质量/导出与单视口/父层行数预算，并聚合 F001/F006/T002；T003/r3/typecheck/build 继续独立回归。`FGC-D005` 必须覆盖四领域语义比例/Style Token 配方、范围/步长、预览取消/确认与越界拒绝；不得出现自由工程尺寸。

ADR-0010 已将 `FGC-V002` 标记为 superseded，不再为三方向解释/重混增加 Gate。`FGC-A004` 已由 `npm run agent:a004-action-loop-smoke` 覆盖 13 项代码所有 Tool Registry、正常 plan→build→GLB readback→四视图→evaluate→preview、输入 Schema/G819 拒绝、12 次上限、取消/timeout/Provider 断线、重复 ID、stale Snapshot 和审批前零永久资产副作用。F026/A005 专属 Gate 保持不变。ADR-0016 将 R007 拆为 R007A/R007B；`npm run agent:r007-gate` 已聚合 Python 只读合同、直接/已导入 GLB、Rust HTTP、原生 ChangeSet preview→confirm/reject、参考/plan/Snapshot 重启读回、F026、typecheck 和 contracts。R007B 的 packaged producer/validator 已覆盖 `single_image`、`multi_view_contact_sheet`、`strict_glb_readback` 三类不同 analysis/plan/effect/result、缺失视角 capability ceiling、源/结果 hash 分离、原对象只读和同一真实工作台唯一 renderer 的 reference/result 捕获；当前证据为 `output/r007b-packaged-workbench-evidence-current-20260719/manifest.json`。该 Gate 明确保持 `visual_fidelity_validated=false`、`formal_eligible=false`，图片路径不做联网反搜或隐藏几何推断。当前类别开放产品 Gate 必须覆盖一次完整 author、真实编译/readback/同工作台 PBR 八视图硬门、最多一次同意图 typed patch、无结果与零版本副作用，并断言方向卡数为 0、单一结果卡为 1；不得生成多个完整模型、评分比较或把兼容适配结果伪装为唯一结果。旧 V003 的两次修复测试只作为回归读取。

U002 在 A004 冻结兼容清单之外增加代码所有的第 17 项 runtime 工具 `author_universal_asset`。`agent:u002-universal-author-contract-gate` 与 `desktop:u002-universal-author-workbench-smoke` 必须覆盖八类开放 profile、跨合同 hash、伪 observed/未知 capability/Provider 自报 executable 拒绝、机械臂正路径、未支持表示零 worker/版本副作用、无四领域选择器和活动资产保留；R3 继续覆盖 Snapshot、导出与重启。

ADR-0015 已将旧 `FGC-M108` 拆为 `FGC-M108A` 与 `FGC-M108B`；`agent:m108-*` 保留历史命名兼容，但测试结论必须分开记录。

`FGC-M108A` 验证生产概念工件真值。`npm run agent:m108-production-concept-smoke` 要求同一不可变 ShapeProgram 可确定性派生 `interactive_preview`（24 段、128×128 v3）和 `production_concept`（48 段、10 段 capsule 半球、Loft/Sweep 平滑法线、512×512 v4）。两档的 ShapeProgram hash、bounds、operation/output role、Part、Material Zone、material 和 source-operation 身份必须一致，production triangle 必须高于 preview；GLB hash/face/triangle 可随 profile 不同。`GeometryCompileReadback@2` 与 GLB extras 必须携带精确 profile manifest/hash，篡改、缺失、128/512 或 v3/v4 混用均 fail closed。

M108A production 只为实际使用材质惰性生成 `_builtin_v4`/`_v4_` 五通道纹理，并用内容寻址对象缓存。Gate 覆盖首次写入、重复命中、重启读取、损坏对象拒绝，以及 SQLite、事件、日志和派生索引不含 GLB/base64。Q003 质量、`:model.glb`、正式下载和导出必须读取同一 production GLB/readback；旧 `GeometryCompileReadback@1`、preview 或旧材质报告隔离为 stale/unavailable。`agent:m108-gate` 继续聚合 PBR、Khronos Validator、glTF Transform 拒绝决策、G818 与 G826，外部报告不替代 ForgeCAD readback。

`npm run desktop:m108-workbench-renderer-smoke` 从当前源码生成 production kit，在同一真实工作台和唯一 renderer/context 中依次加载。它校验 metre→millimetre、520 mm fit、环境 recipe hash、PBR 色彩空间、真实 bounds、安全取景、调试辅助隐藏、损坏 GLB 恢复，以及 production 上限：geometries 72、textures 48、draw calls 96、triangles 24,000、实际 PBR texture 35、RGBA8 完整 mip-chain 估算纹理显存 64 MiB。当前检查点四领域为 7,308/68、9,148/78、8,116/96、13,704/53（triangles/draw calls）。preview 的 T003 预算必须独立保持；不得为了 production 全局放宽编辑档。R3/F009 还须用 display request token 拒绝迟到替换，并区分 `compiled_agent_pbr`、`production_concept` 与只读 `external_reference`。

2026-07-28 external-reference smoke 的 GLB fixture 通过显式参考导入入口，因此运行时事实应为 `ready/external_reference`，不是 confirmed Agent asset 的 `glb_pbr`；它仍只证明同一工作台的 PBR 采样、取景、环境和失败保留策略。C111B 的独立开发 capture 记录在 `output/c111b-workbench-capture/capture-manifest.json`，不得替代授权比较或真人门。`desktop:c111b-packaged-agent-webgl` 已用真实 packaged `.app` 在 run `c111b_c0cc…faf8` 证明同资产 `Agent→V1→A005 V2→Snapshot→visible export→8 views→new-process restart→same export/8 views`，并记录 V2 `755dae1d…a289d8`、138,248/157/14、12 complete PBR、single renderer/context/canvas、六阶段 Rust metrics、0 network/credential 和 billable 0。production A005/SurfaceLayer 烘焙的向量化回归必须冻结五通道 PNG SHA，并额外证明完整 C111 production GLB bytes/hash、triangle/primitive 不变；当前 exact-equivalence benchmark 为 `103.174s→22.504s`、`35,964,044` bytes、GLB `5a134d6d…a72a`。compiler-only benchmark 不能替代真实 packaged Agent Turn 时间；控制台锁定仍必须 preflight fail-closed，但当前控制台已解锁并完成新 run。V2 hash 必须作为 action-derived lineage 从 initial 传给 restart，不能误用冻结 V1 hash。120s 生成 Gate 只读取 Rust terminal 六阶段 Turn，不读取后续人工 A005、导出或 restart wall time；该 PASS 不替代 sealed `ReferenceEvidence` comparison 或真人评分。`desktop:task-d-packaged-visual-program-smoke` 仍是另一资产/software-raster supporting Gate。

最新 run `c111b_4ad12339eca842b2993cf2d320498416` 已替代此前 run：V2 `af9f5605…54fa`、Snapshot r5、initial/restart 16 captures exact-match、工程链与显示像素可读性 PASS。冻结合同将 `120s` 绑定到六阶段 Agent Turn；实测 `94.402s <= 120s`，生成性能 Gate PASS。initial `220.240s`、restart `20.262s`、total `240.502s` 是完整 QA workflow wall time，必须作为独立响应速度指标保留，不能拿它覆盖合同定义。`smoke_c111b_packaged_webgl.py` 从 frozen fixture 读取目标并硬断言 Turn 总时长。Turn lower `93.989s` 仍是主要优化基线。固定视图捕获现使用显示域 sRGB 回执，Rust 与 Python 都要求每张 96×96 样本前景覆盖 ≥100 bps、中位亮度 ≥24、可读前景 ≥5000 bps；初次八视图分别达到中位亮度 `34–52`、可读前景 `7251–8868 bps`，重启逐张 hash/指标完全一致。该门只证明画面可读，不证明与授权参考相似或达到真人 `4/5`。

C111B reference policy 必须由 Rust 从冻结 `C111BVisualAcceptanceContract@2` fixture 原始字节解析，不允许在 Provider prompt、WebView 或测试中复制可变阈值。v2 将离线生成链的 0 网络/0 可变成本与视觉比较预算分离：视觉比较必须有显式用户授权和调用前预算预留，每个候选最多 3 次调用，总可变成本硬上限 `100000 microusd`；没有授权时仍严格 0 网络。`VisualReferenceComparisonInput@2` 必须携带 `VisualReferenceAcceptancePolicy@1` 和 source contract SHA；Gate 覆盖 7600/6500/5000 边界、关键 `not_visible`、policy tamper 导致 report lineage 失败，以及非 C111 program 保持通用政策。`agent:pv006c-evidence-context-gate` 继续覆盖 sealed evidence 像素读取、exact candidate eight views、Provider assessment-only、Rust 派生 pass/fail、最多两次 repair、confirm/Snapshot/export 和 adapter。离线 fake Provider PASS 只证明合同；没有当前 Project sealed evidence 与真实 comparison report 时仍写 `NOT RUN`。

运行时预算 Gate 还必须覆盖 0044 迁移、授权幂等冲突、过期、request/graph/policy/Project 漂移、第一次实际 Turn 绑定、跨 Turn 拒绝、唯一活动预留、预网络释放、网络尝试保守记账、三次累计恰好 `100000 microusd`、第四次预网络拒绝，以及 Provider future timeout/drop 的保守 settlement。production 网络结果若没有 `VisualReferenceComparisonBudgetEvidence@1`，`VisualReferenceComparisonReport@2` 必须拒绝；离线 fake Provider 可省略 budget evidence，但不得把它冒充真实网络报告。

M108A 自动 Gate 通过只允许结论“生产概念工件管线已验证”。它不证明固定 showcase 的比例、完整部件语言或表面细节达到生产级视觉基线。

`FGC-M108B` 保留为 PV007 逐领域晋级后的跨领域正式基准；M108A/C111B/PV007 任一依赖未完成时保持 `blocked`。正式评审必须使用 `EditableComponentRecipe@1` 实例化的四领域 production fixture，每领域至少 3 份，记录 Recipe/child slot/connector/pivot/局部变换/语义比例/Material Zone/provenance，并先通过 M108A/Q003/G826。当前 `prepare_m108_visual_benchmark.py` 仍生成固定 showcase，因此只属于 M108B preflight，不能作为正式退出资产。ADR-0020 只把该门后移，不降低四领域或真人阈值；第一机械硬表面商业切片先由 C111B/E004/PV006 证明。

保留的 M108B 回归协议要求：至少三位未参与实现的真人，在同一 ForgeCAD `production_concept`、固定环境、非 ghost/xray 视口中评分；每个领域的 `proportion`、`material_readability`、`surface_detail` 中位数分别不少于 `4/5`。`validate_m108_visual_benchmark_scores.py` 不得生成分数，正式版本必须拒绝缺少 Recipe-backed manifest/provenance 的 kit。该旧四领域协议只作为 U005 的机械回归子集；Codex/代理审查、自动截图、跨领域总分或选择性隐藏截图都不能补齐真人证据。

M108 评分合同中的“至少五套”按至少五个不同 material index、texture-set ID 和规范 texture material 计算；重复 authored alias 不计数。production map 必须为 512×512 v4 完整五通道。工作台 renderer line 属性必须存在、为非负安全整数且等于 0，属性删除或改名不能通过默认值伪装。

### L3：工作台 E2E

当前入口：

```bash
npm run desktop:f002-agent-conversation-smoke
npm run desktop:f003-agent-selection-card-smoke
npm run desktop:f004-workbench-drawers-smoke
npm run desktop:f006-accessibility-smoke
npm run desktop:f007-workbench-lifecycle-smoke
npm run desktop:f008-agent-conversation-state-smoke
npm run desktop:f009-agent-blockout-display-state-smoke
npm run desktop:f010-agent-asset-workspace-state-smoke
npm run desktop:f011-legacy-compatibility-display-smoke
npm run desktop:f013-viewport-display-preferences-smoke
npm run desktop:f014-legacy-module-graph-workspace-smoke
npm run desktop:f023-agent-blockout-preview-presentation-smoke
npm run desktop:f024-agent-plan-source-presentation-smoke
npm run desktop:f025-legacy-isolation-smoke
npm run desktop:d005-semantic-proportions-smoke
npm run desktop:t002-workbench-e2e-scenarios
npm run desktop:f001-workbench-characterization
npm run desktop:r3-concept-workbench-smoke
```

`desktop:f002-agent-conversation-smoke` 是无浏览器副作用的组件树 smoke，检查 AgentConversation 的澄清、Kernel 步骤、方向卡和输入可访问标签；它不替代工作台 E2E。

`desktop:f003-agent-selection-card-smoke` 是无浏览器副作用的组件树 smoke，检查分件候选、角色选择、受限部件动作、兼容组件替换、质量入口和 ChangeSet 预览确认；它不证明后端写入或真实 Provider 质量。

`desktop:f004-workbench-drawers-smoke` 是无浏览器副作用的组件树 smoke，检查组件目录、视觉材质、质量检查和下载四类抽屉；其中 Agent 分支只允许直接 GLB、概念单图和概念图包，旧用途/OBJ/源包不会泄漏到 Agent UI；它不证明后端写入或真实 Provider 质量。

`desktop:f025-legacy-isolation-smoke` 先做静态责任边界检查，再聚合 F001、F006 和 T002：Agent-active 不挂载 Graph Inspector、旧参数、旧组件抽屉或旧格式导出，也不调用 legacy Planner/质量/导出；只有显式入口可读取只读 legacy 详情，关闭、项目切换和迟到响应不能污染 Agent Snapshot。T003 与 r3 继续分别证明单 renderer 资源预算和 Agent-first 版本链。

`desktop:t002-workbench-e2e-scenarios` 是可定位的浏览器场景套件，输出 `output/playwright/fgt002-scenarios/report.json` 和每个场景独立 JSON。当前 14 个场景全部通过：启动/单 canvas、legacy hand-off、未知领域澄清、范围停止、三张未保存方向的软件概念图、汽车、飞机、机械臂、未来武器概念道具、预览不写版本、可编辑资产提交、分件/材质、ChangeSet 取消、确认/质量/直接 GLB 与概念图包下载/重启恢复。范围停止场景断言本机提示、不出现方向卡、且不创建 Agent asset；它使用 deterministic Planner，不替代真实 Provider truth set。

`desktop:f001-workbench-characterization` 是前端拆分的行为基线，覆盖首次项目、legacy 显式转换、澄清写入屏障、方向预览、Agent 资产提交、Snapshot/导出对齐和重启后的单 canvas。2026-07-13 已在本机 Chrome 通过并登记到 CI；F005 在不改变这些断言和 F002/F003/F004 组件 smoke 的前提下完成抽屉组合层收敛；F006 已新增可访问性断言且保持版本/选择语义不变。

F005 没有独立的后端 smoke：`WorkbenchDrawerStack` 只负责四类抽屉的组合和 props/callback 转发，因此以 `desktop:f004-workbench-drawers-smoke`、`desktop:typecheck`、`desktop:build`、`desktop:f001-workbench-characterization` 和 `desktop:r3-concept-workbench-smoke` 作为组合层回归证据。F006 增加 `desktop:f006-accessibility-smoke`，检查最小尺寸、focus-visible、aria-live/label、dialog 初始焦点和 Escape/触发控件焦点返回；r3 负责浏览器级焦点行为。F007–F019 的仍在用切片分别验证生命周期、会话、blockout display、已提交资产工作区投影、legacy 只读兼容显示、本机视口显示偏好、legacy ModuleGraph 工作区会话、legacy 图临时叠层、Agent 概念图展示、编辑辅助读取、视觉材质目录与筛选读取。旧 F012 组件库偏好 state/hook 从未进入运行时，已在 2026-08-01 连同孤立 smoke 清理；组件目录真实展示继续由 `componentCatalogPresentationState` 与现有抽屉 Gate 覆盖。F019 覆盖筛选组合、context 切换清空和状态不持有 selected material/Material Zone/Snapshot/版本/质量/ChangeSet/导出/renderer；R002–R004/T002/r3 继续验证图像来源、PNG/manifest 包、版本链与单 canvas。若后续把状态或副作用移出父层，必须先增加针对 Snapshot/ETag/ChangeSet 的行为断言。

当前核心 Agent-first 门已通过，不能替代以下独立场景的完整回归：

1. 新建项目和首次初始化；
2. 四领域各一个明确 Brief；
3. 未知领域进入澄清，不创建版本；
4. 方向预览不写版本；
5. 保存分件候选创建 v1；
6. 部件修改预览、拒绝和确认；
7. 材质只改变目标 Zone；
8. 组件保存和同角色替换；
9. GLB 参考导入保持只读；
10. 质量和导出引用同一活动版本；
11. 重启恢复；
12. 始终只有一个 WebGL canvas/context。

当前 Agent-first smoke 已验证 legacy 只读转换授权、Snapshot 选择、preview→confirm 清理、活动资产检查、不可变 Agent undo→redo、GLB 导出不回退 legacy Concept、浏览器重启恢复和单 WebGL renderer。`agent:s8-active-design-navigation-smoke` 还验证 HTTP undo、redo、ETag/CAS、幂等 replay、head/Snapshot 切换和逻辑历史 frame；D002/D003 服务与 focused 浏览器 smoke 已验证未知/含糊领域单问题、幂等重放、无资产写入和明确选择后继续规划。完整工作台 r3、多客户端并发、真实 Provider truth set 与原生安装仍待覆盖。

### L4：原生 Tauri

验证：

- Keychain 保存和读取；
- supervisor 注入但不记录密钥；
- Agent 启动、错误服务拒绝和退出清理；
- Provider 配置后重启；
- 真实文件选择和下载；
- 休眠/唤醒、网络断开和应用重启。

浏览器 smoke 不能替代该层。

2026-07-13 的 R005 本机尝试已通过 `FORGECAD_LOCAL_VISUAL_PACK=0 ./script/build_and_run.sh --verify`：macOS `.app` 已构建并启动，`local-dev-python` Agent 健康检查通过。随后使用 `osascript -l JavaScript` 检索原生窗口并尝试点击下载时，系统返回“osascript 不允许辅助访问”；因此原生 WebView 的点击/下载尚未自动化验证。这是可复现的 macOS 辅助功能授权阻断，不得写成原生下载已通过。授予运行测试的 Codex/Terminal 辅助功能权限后，必须重复直接 GLB、单 PNG 和概念图包下载，并断言 Snapshot/版本/质量/选择和单 WebGL context 不变。

### L5：安装与升级

在全新用户账户和无开发环境机器上验证：

- 安装、启动和首次初始化；
- packaged sidecar；
- 数据目录权限；
- 升级迁移和回滚；
- 卸载不误删用户资产；
- macOS/Windows 签名提示；
- 离线打开已有项目。

当前尚未进入该层。

### L6：冻结视觉分布与商业验证

L6 不进入普通 CI；它使用冻结未见任务、真实授权输入、当前发布候选和独立参与者验证产品，而不是验证某个函数。

ADR-0022 的当前正式目标是 U005 八类跨类别分布；每类单独报告，不以总体平均掩盖失败。以下 30 条机械硬表面 E005/PV006 规则保留为 procedural regression 子集，覆盖纯文字、单图、多视图和已有资产局部修改；既有 PV006 的 20 条门保持原阈值。每条先冻结 `VisualAcceptanceContract` 或等价 manifest，再记录：

- macro 轮廓/比例/负空间；
- meso 部件层级/连接/壳体过渡；
- micro 倒角/紧固/线束/Decal/磨损；
- PBR zone、UV/tangent、五通道消费和材质逻辑；
- 同一 GLB 的固定八视图和跨视图一致性；
- Part/选择/修改/版本/导出/重启的资产可用性；
- 调用、token/图片用量、缓存、修复次数、端到端时间、估算成本和失败类别；
- 三位未参与实现的独立真人评分。

ADR-0021 的目标阈值均为 `target`：首次真人 `≥4/5` 至少 70%，一次 typed patch 后至少 85%；无 patch P50 ≤32 秒/P90 ≤70 秒，一次 patch 后 P90 ≤105 秒；严重回归 `<10%`；可变成本不高于实收收入 25%。商业层另验证至少 5 家付费设计伙伴和第 4 周不少于 50% 的核心闭环周留存。未真实招募、收费和运行时必须标记 `NOT RUN`，不得用问卷意向或内部演示补齐。

`FGC-E005` 使用 `npm run agent:e005-unseen-distribution-gate`。合同层必须冻结 30 条 task-set SHA，并验证 source manifest 恰好逐条覆盖、无重复/乱序/跨集合/hash 漂移。没有显式 `E005ProviderRunAuthorization@1` 或 authored v2 source 时，每条只能生成 `not_run/E005_PROVIDER_UNAUTHORIZED|E005_SOURCE_UNAVAILABLE` receipt，author/patch/network/cost 均为 0，且不得出现 source、GLB、固定视图或时间字段。授权合同必须绑定精确 Provider/model/source policy/pricing/disclosure、30+30/60 调用上限和 token/成本/时间预算；schema、账本和测试 PASS 均不等于用户授权真实调用。

同一 Gate 运行 Core 的 E005 原子预算测试。正式调用必须先在 0045 账本原子 reserve，再由唯一 worker 原子取得一次 dispatch，最后 settle；公开 API 不接受外部 now/deadline。只有 reserved 状态可以按 pre-dispatch release，dispatching 状态的成功、超时、取消、传输失败和重启恢复必须保守 accounted；启动恢复还必须释放遗留 reserved。首次 settlement evidence/after-counters/output source/gate hash 必须持久化，其他任务完成后的重放仍返回完全相同内容；Patch 资格判断必须先重验该证据，不能直接信任可变 SQL 输出列。测试必须证明第 31 次 author、第 31 次 patch/第 61 次 total、并发重复 dispatch、非冻结 task/hash、跨 Provider/model、SQL/canonical JSON/output hash 漂移、过期、单次/批次 deadline、token/cost 超限和非 repairable patch 全部 fail closed；首轮通过不可 patch，同任务每种 call kind 最多一次。账本测试不得联网或读取凭据。正式 runner 还必须从实际请求内部计算 request SHA/token/成本，不能信任上层自报。

正路径测试由 Python 启动真实 restricted uvicorn sidecar，再由被忽略的桌面 Rust test 使用生产 `NativeProductToolExecutor`/`LoopbackHttpPort` 执行 E005，输出唯一 `E005RunReceipt@1`，最后回到 Python JSON Schema 校验；假 HTTP backend 只做端口回归，不可替代这条 live Gate。focused adapter/port tests还必须覆盖预取消零几何调用、timeout 稳定码、无 source patch 拒绝、精确 replay 只重渲染/full-cache hit，以及一个 repairable hard Gate 后至多一次 typed patch 与 operation fragment hit/miss。

正式 30 题前还必须通过三个质量前置门。E005-R1 已通过。E005-R2 Gate 必须证明：唯一 visual call 同时接收 exact sealed reference bytes 与同一 GLB/renderer 的 TurntableEight，返回 assessments + ephemeral proposal；Rust 派生 report 后才能密封 patch。accept 总计 1 geometry build；typed patch 总计 2 builds、1 visual call、0 final visual recheck，并明确标记待视觉确认；参考/候选任一字节或 hash 交换必须在 Provider 前失败。SurfacePlan 已接入受限 A005/PBR 后，Gate 必须证明每个展开 zone 恰有一套五通道纹理、provenance 通过，`set_surface_tuning` 改变 PBR input identity；只检查 GLB/manifold/bounds 不算视觉 patch。formal-call 子门还必须证明 exact body prepare-once、同一 0045 `Patch` reservation、无 0044 双重预算、同 authorization/provider/model/pricing、1 Author + 1 visual、真实 usage 上界、reservation ID 只进入 idempotency header，以及 identity drift 预网络失败。formal recovery 子门要求 Author 已 accounted 后的 AwaitingVisualReview checkpoint、重启不重发 Author、visual dispatching/未知状态进入 reconciliation、真实 `E005VisualSession@1`/final receipt 与篡改拒绝；这些现已由 0047 和 R2 focused fixture 通过。

`npm run agent:e005-r3-production-review-contract-gate` 是 R3 的合同子门：同一 R2 final source 必须只增加一次 `production_concept`、640×640、TurntableEight 编译；`E005ProductionReview@1` 绑定 source、SurfacePlan、全部 adornments、完整 RestrictedGeometryInput、production GLB/normalized geometry/readback、11 sets/55 maps 五通道 PBR、八视图和 lower/compile/render/total 时间；production receipt upgrade 保持 1 Author + 1 visual，不增加第三次 Provider/VLM。当前 Gate 覆盖 8 Core R2、9 app-server R2、3 app-server R3 与 generated contracts。它仍不是 R3 done：真实 repository-backed 单图/多视图/当前资产输入、completed-visual→production 跨重启、Author/visual Provider 等完整 wall-clock、main/startup 和正式 sidecar/renderer 四模态证据尚未完成；图片文字描述只能作预检。

0046 batch checkpoint 测试必须证明：按 ordinal 最多一个 task 为 running；未产生 network-attempted reservation 的任务可在 Provider recovery 后回 pending；任何 dispatching/accounted/网络不确定任务都进入 `reconciliation_required` 且不可自动 retry；receipt canonical JSON/hash 与 authorization/task 原子封存，exact replay 幂等、冲突 replay 拒绝。该 substrate PASS 只证明恢复安全，不授权也不触发正式 Provider run。

结构自由度不得由 GLB hash 或 report 布尔值自报。成功 receipt 必须从 patch 后最终 source 生成顺序/ID 无关的 canonical semantic graph，并从最终 GLB 生成忽略材质、metadata、primitive 顺序、平移、轴交换与统一缩放的三角几何 hash；435 对中任一同 semantic structure 或同 normalized geometry 都失败。对抗测试至少覆盖材质/metadata、平移/轴交换/统一缩放、source node/output 重排不变，以及真实依赖边和非等比形状变化可见。

真人质量必须由 `E005HumanReviewBundle@1` 从同一 receipt/fixed-view hash 派生：3 个互异真人 commitment、每人 30 条、每条 3 人、共 90 份，七维中位数 `>=4/5` 才通过。blind packet 内容还需单独保证隐藏 Provider、实现来源和任务身份；只有 packet hash 而无生成/内容检查不能进入正式评审。聚合器从 authorization、receipts、matrix、human bundle 重算计数和 `formal_eligible`；30/30 lineage、435/435、三人逐任务、首轮 21/30、一次 patch 内 26/30 和 32s/70s/105s 任一缺失均为 false。离线 fixture、合成 self-test、自动/VLM 评价和硬门成功不得计入真人质量通过。

## 3. ADR-0022 通用能力 Gate

### U002：类别开放理解与路由

- 纯文本、sealed 单图、多视图和当前资产进入同一 author request；
- 至少覆盖机械、角色、生物、植物、家具、建筑/环境、载具和混合对象的 SubjectProfile fixture；
- 未知类别不产生 `unsupported domain`，真正含糊才澄清；缺 capability 返回 typed limitation；
- C111/机械臂/武器模板回退、跨 Project evidence、伪观察和未声明 capability 在 worker/Provider 前拒绝；
- 没有 executable representation 时零资产版本、零 Snapshot、零导出副作用。

### U003/U004：统一资产源、本地混合表示与 Provider 主权

- `UniversalAssetSource` 的各分支共享 source hash、CAS、版本头、readback 和导出；
- camera hypothesis、detail claim、appearance evidence、Material Zone appearance 与 projection compiler 有正负 fixture；
- deformable/local-hybrid 不得绕过格式、拓扑、预算、Part/Zone 和 GLB readback；
- DeepSeek/千问关闭、超时或失败后，既有项目仍能打开、检查和导出；第三个 Provider 必须预网络拒绝。

U004A 必须通过 `agent:u004-sovereign-provider-gate`：旧远程 Mesh adapter/credential/command/UI/live probe 文件保持删除；DeepSeek 凭据拒绝非官方 host 与非 `deepseek-*` 模型，千问凭据拒绝非官方域与非 `qwen*` 模型；desktop typecheck/build 与 F026 单视口继续通过。U004 后续每个本地表示还必须覆盖重复/悬空 part/feature、能力越权、非法参数、取消/超时、编译失败、迟到结果和版本/Snapshot/Quality/Export 零副作用。该 Gate 只证明 Provider 边界，不证明视觉质量或 capability 晋级。

### U005：跨类别正式质量

- 八类任务分别报告身份保持、轮廓、结构、材质、跨视图、可编辑性、首轮/一次 patch、时间、成本和真人评分；
- 不允许用总体平均掩盖任一类别失败；投影纹理不得在 clay/map-stripped、grazing-light 或 ID-buffer 视图中掩盖错误几何；
- 自动事实门、视觉 Provider、真人盲评和商业指标分别记录；未运行保持 `NOT RUN`。

## 4. 真实 Provider 评测

离线 deterministic 结果和 fake HTTP 只验证协议。`FGC-E001` 已冻结、`FGC-E002` 已实现 [Provider 与视觉组合评测合同](AGENT_PROVIDER_EVALUATION.md) 中的历史四领域 Planner 合同：每个领域 20 条正常 Brief（总计最多 80 次 Provider 请求），另有 20 条含糊/越界安全停止条目在本地预检；默认 dry-run 零网络、零费用、零资产/Snapshot 写入。`npm run agent:e001-provider-evaluation-dry-run`、`npm run agent:e001-provider-evaluation-contract-smoke`、`npm run agent:e002-provider-evaluation-runner-smoke` 和 `npm run desktop:deepseek-mvp-acceptance-smoke` 只证明合同、fixture、Rust 私密凭据所有权、授权/预算/脱敏边界，不证明当前单一结果的模型质量。Python 的旧 `macos-keychain` 评测路径已 fail-closed；真实 macOS 单 Turn/取消验收仅能通过显式 opt-in 的 Rust-native 命令执行。

真实 Provider run 必须由用户逐次授权，并由执行器验证正值成本上限、唯一 evaluation run ID、请求/图片/token 上限、wall time、最多一次 v2 typed patch 和停止条件后才可发起。浏览器开发仍使用默认 environment/0600 secret-file 配置；Python 不读取任何桌面私密凭据。原生 macOS 单 Turn/取消连通性只通过显式 opt-in Rust-native 命令执行。具体 Provider/model capability、请求字段、价格和 reasoning 续传合同必须在执行当天依据官方资料和 preflight 冻结到 run report，不能从本文件旧模型名推断。2026-07-20 的 Keychain acceptance 属于历史失败证据；2026-07-27 本机 Alpha 已迁移到私密文件，不再以 app identity 或钥匙串授权作为前置门。Provider 未返回完整用量、预算中断或任何失败时，报告仍不得作为合格模型证据。记录：

- 领域识别准确率；
- 结构化输出成功率；
- 完整外观率；
- 首次预览时间；
- token、费用和超时；
- 独立真人评分、修复前后变化和失败原因；
- 缓存命中、修复次数、端到端时间和估算可变成本。

真实评测必须显式授权费用，密钥、Base URL、模型内部 ID、原始 Prompt/Response 不进入工件。模型或 Prompt 变更后重新基线；旧 R4 Weapon 评测不得代替此基线。

## 5. 非功能测试

- 前端 bundle 预算和懒加载；T003 当前以 1.2 MB 最大 JS、1.4 MB 总 JS、150 kB CSS 为 Alpha 门禁，后续拆分任务应逐步收紧而非放宽；
- G801 wedge/capsule 几何必须验证 GLB header、bounds、triangle readback、重复生成字节一致，并保持 non-functional-only 边界；
- G802 profile/extrude 必须验证前置引用、非退化轮廓、三角数预算、GLB bounds/readback、重复生成一致和退化输入拒绝；
- G803 revolve 必须验证 profile 引用、非负半径、完整/部分角度、拓扑三角数、GLB bounds/readback、重复生成一致和非法输入拒绝；
- G804 mirror/array/radial_array 必须验证前置引用顺序、非零轴、数量/间距/半径预算、复制后的三角数、GLB readback、重复生成一致和非法输入拒绝；
- G805 union/subtract 必须验证不重叠 union、重叠拒绝、合法贯穿槽 subtract、非贯穿/越界失败、输入数量和 GLB readback；不得把复合 Mesh 当作 B-Rep manifold 证明；
- G806 bevel_approx/surface_panel 必须验证 1/3 段倒角、默认/显式面板、±Y 面约束、面板适配、半径/尺寸/偏移失败、GLB bounds/triangle readback 和重复生成一致；不得把低多边形近似宣称为精确 fillet、实体或制造能力；
- G807 必须验证四个 Domain Pack 各 12 个变体、跨领域结构签名唯一、完整外观部件数量、GLB bounds/triangle/readback、重复生成字节一致、AssemblyGraph 连通性和机械臂 joints；不得把后端目录 Gate 宣称为前端已开放目录或工程能力；
- 30 分钟连续编辑内存增长；
- 大 GLB 导入预算和拒绝耗时；
- SQLite WAL、并发读写和中断恢复；
- 1,000 项组件目录查询；
- 备份容量、校验、恢复时间；
- 键盘、焦点、屏幕阅读器状态通知和最小字号；
- API Key、路径越界和日志脱敏。

## 6. CI 必需检查

每个 PR 至少执行：

- repository integrity；
- contracts/types；
- Python lint/Agent checks；
- G1–G7 smoke；
- desktop typecheck/build；
- workbench E2E；
- Rust 三平台 cargo check；
- secrets/safety/docs；
- npm、Python、Rust 依赖审计。

审计命令与产物：

```bash
npm audit --audit-level=high --json
.venv/bin/pip-audit -r apps/agent/requirements-release.lock --format=json
cargo audit --file apps/desktop/src-tauri/Cargo.lock --json
```

三者在 CI `dependency-audit-reports` artifact 中保存原始 JSON，高危结果阻断合并。当前锁定 FastAPI 0.139.0、Starlette 1.3.1、Uvicorn 0.51.0，Python 与 Rust 审计均无漏洞；项目最低 Python 版本因此提升到 3.10。不得通过降低 audit level、忽略 CVE 或删除 lock 条目来放行。

发布分支额外执行 sidecar 构建、安装 E2E、许可证/SBOM、备份恢复和真实 Provider 基线（如本次版本改变 Provider 行为）。

## 7. 失败处理

- flaky 测试同样阻断，不能自动重跑后忽略首次失败；
- 修复产品错误时补回归测试；
- 修复过期断言时证明产品状态机仍正确；
- 截图只能辅助诊断，不能替代状态、网络和持久化断言；
- 任何跳过项必须写明负责人、原因和恢复期限。
