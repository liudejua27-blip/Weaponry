# ForgeCAD 文档状态账本

版本：2026-07-22
状态：当前文档维护真值；不是产品运行时能力证明

本文件解决一个具体问题：ForgeCAD 同时有产品说明、目标设计、历史证据、兼容资料和任务计划。没有一个短的状态账本时，后续 Codex 容易把“目标设计”或“过去通过的 smoke”误读为当前已完成能力。

## 1. 当前一句话结论

ForgeCAD 是本机 Alpha 的轻量通用机械概念 3D Agent，当前已经有四领域的确定性后端 blockout、Agent 资产版本、受限编辑、Snapshot 真值、GLB 导出和工作台浏览器回归；它还不是生产级通用 3D 工作台。

2026-07-22 当前状态增量：C110A/B/C 的合同、Rust Core 物化和 ChangeSet 编译入口已完成；C110D 已新增 actuator cover、cable guide、wrist tool mount 三项受审 Recipe，`AssemblyDeltaProgram@1` Schema/allowlist 与 Rust Core/app-server 同资产两部件 preview→confirm→GLB readback 回归通过。C110E `ArmGeometryFamily@1` 已完成：serial-chain 的连杆/关节/基座/腕部/末端/线缆/材质意图会在 Rust 中同时修改真实 ShapeProgram 与 AssemblyGraph，并生成 intent/changed-count/ShapeProgram hash 绑定；不同 link language 已由 Core 与 app-server 测试证明会产生不同几何指纹。C110F 已完成 Provider 稳定性最小闭环：`plan_complete_concept` 向 DeepSeek 使用紧凑投影、Rust 仍以完整 schema 最终校验；Action Loop 默认采用 256K 累计上下文/4K 单次输出预留，结构化 JSON 错误最多一次固定 repair，针对首次 synthesis 的错误 AssemblyDelta 与无效 ArmDesignIntent 另有最多两次固定 Product Tool recovery，所有重试仍受硬上限约束。真实 `live_arm_intent_20260722g` acceptance 已通过，`live_turn.arm_intent_bound=true`，证明真实 DeepSeek 已将中文机械臂描述送入 Rust `ArmDesignIntent@1` lowering；取消、local fail-closed、零 Snapshot/资产写入和脱敏报告也通过。新增 Rust-owned 规则要求真实 DeepSeek robotic-arm plan 必须提供完整 intent；初次 synthesis 不能带 AssemblyDelta，只有存在活动 Agent asset 时才允许增量。进一步完成了 Rust-owned `ActiveDesignSnapshot` 只读上下文→Provider→严格 `AssemblyDeltaProgram@1`→当前 head 校验桥：生产 Turn 会把真实活动版本作为只读上下文，Rust Product Tool 会拒绝跨版本/legacy 基础，桌面修改模式会把 delta 转成同一个 ChangeSet 的 preview→真实 GLB→confirm 流程；确认前不改版本。当前仍不证明真实 DeepSeek 已完成真实 GLB 创意质量、`AssemblyDeltaProgram@1` live packaged binding、任意架构/拓扑/自由装配、图片级视觉或 M108B 4/5。性能风险仍是约 10 万三角形 showcase 在 build/readback/preview/export 中被多个受限 Python 子进程重复编译，下一步需做增量 artifact handle 或明确拆分交互预览与 production export。

`FGC-R002`–`FGC-R005`、`FGC-M101`–`FGC-M107`、`FGC-M108A`、`FGC-C101`–`FGC-C106`、`FGC-G808`–`FGC-G813`、`FGC-Q002` 与 `FGC-F007`–`FGC-F024` 已完成；R003 只在几何分组与稳定 Part 一一对应时生成透明爆炸概念图，否则明确不可用；R004 只将当前、指纹一致的 PNG 与机器可读清单打包下载，ZIP 不含模型源文件或工程资料；R005 让 Agent 下载抽屉只显示直接 GLB、单 PNG 与概念图包，浏览器 E2E 已通过，而原生 WebView 点击仍因当前会话缺少 macOS 辅助功能权限待复验。F008–F024 的纯展示边界见任务索引；F024 只说明已返回方向是离线规划、已连接模型服务生成还是来源待确认，绝不把确定性结果称为真实模型质量，也不显示 Provider 或模型内部标识。G812/G813 已让三张方向卡的 build/segment 使用同一个、按领域/轮廓/方向稳定解析的视觉变体，并让未保存候选以“换一版外观”轮换当前方向的三项预审外观；F022/F023 只保存并翻译其 project/request 屏障内的展示上下文，不公开 48 项技术目录或参数，且不写版本、Snapshot、质量或导出。候选与已确认资产仅通过持久化 ShapeProgram/AssemblyGraph 追溯外观来源。M107 将 Material Zone 选择纳入 Snapshot/CAS，并覆盖重启与 undo/redo 保留，M108A 验证双档 production 工件、512 v4 PBR、正式质量/导出、二进制 GLB 与 CAS，C101 将稳定内部 part role 显示为中文并对未知 role 安全回退，C102 为项目内组件提供来源质量、领域、role 与连接保留的可解释替换边界，C103 则只在现有装配/几何事实充分时提供拆分或合并候选并强制 preview→confirm，C104 则让锁定、隐藏与单独查看通过同一 Snapshot/CAS 保存，锁定由后端阻止相关 ChangeSet，C105 已用 8 项/四领域代码所有 Recipe 完成固定 optional child slot、non-root active edit、零写展开、preview→confirm、版本/undo/restart 与受限 Python production 编译的机制闭环；其四领域 4 个 GLB 合计 416 triangles，只是机制 fixture，不是生产级视觉基线。C106 已完成 Rust-owned 机械臂黄金路径目录：3 个 reviewed root 供内部选择但每 Turn 只合成 1 个，每个展开为 10 Parts/9 connections；当前服务展示 root 真实编译为 15,340 triangles/44 primitives、19 个 authored Material Zones、8 类 PBR 材质与 512 v4 贴图，9/9 Recipe 具有受限 A005 slot；这只证明机械臂 production-concept 工件与生命周期，不是图片级或四领域视觉基线。G808 冻结 Part 参数的路径/范围/步长/单位/显示名合同，G809 已将非空声明接入 ChangeSet 的路径/范围/步长验证并冻结旧资产六路径兼容，G810 让四领域新 blockout 的真实单一 size 输出生成有界比例声明，G811 将真实声明接入当前 AssetVersion 的零基础步进控件，Q002 冻结 bootstrap 的兼容语义并使质量写入按 Snapshot ETag 幂等；不支持自由参数、单位换算或工程尺寸；均不引入工程材料数据库、正式审阅冒充或工程结论。

`FGC-E001` 已冻结 4×20 正常 Brief 与 20 条安全停止评测；`FGC-E002` 已提供默认拒绝联网的隔离执行器、80 次正常 Provider 请求上限、本地安全停止和脱敏 run report；`FGC-A002` 让该隔离器在 macOS 上显式复用 ForgeCAD 的 Keychain 配置，而不把密钥导出到环境或报告。`FGC-G814` 已把其中的有限概念范围边界接入普通 Turn：`ConceptScopeDecision@1` 在 DomainInference 后、Planner/Provider 前本地决定允许、类别澄清或范围停止；明确现实制造、工程安全/控制请求只保留可读 Turn/Item，不创建任何 Plan、资产或 Snapshot。`FGC-G815` 已让安全 Brief 的有限轮廓、细节、色彩和展示姿态分类稳定选择已有四领域视觉族，且每个选择仍经现有 ShapeProgram/GLB/分件/确认链；这不是自由风格生成、真实 Provider 创意质量或工程 CAD。它们只证明合同与执行边界可安全加载；真实 Provider baseline 仍为 `external`，绝不能因 E001/E002/A002/G814/G815 或离线 Gate 标记为通过。

`FGC-R006` 已完成：三张未保存方向在选择前可各自显示同源的 320×240 软件概念 PNG。该调用不写入幂等、候选、资产、Snapshot、质量或导出；前端只在 project + plan + request 的临时上下文保留图片，开始新 Brief、选择方向、换一版或切换项目都会丢弃，迟到结果不会回写。它不是下载、真实渲染、工程图或制造资料。

`FGC-P008` 已完成：版本化 `ForgeCADPackagedSidecarInput@1` 只声明本机 packaged Alpha 所需目标二进制、架构、启动与健康检查边界，并用无密钥、离线、非执行预检区分 `blocked_missing_sidecar` 与 `ready_for_local_alpha`。当前 macOS arm64 输入已为非空 Mach-O 并报告 `ready_for_local_alpha`，P002 本机 Alpha 也已完成；Intel macOS、Windows、Linux sidecar 仍为空占位，因此安装、签名、公证和跨平台发布继续 blocked。

2026-07-14 用户明确取消“三方向让用户选择”的目标，并要求 Agent 内部选择最佳结果、Codex 式简洁工作台、DeepSeek/Codex/Claude 式运行模型、专属 Skill、高真实度纹理/多材质、参考引导重建和通用生活机械扩展。ADR-0010 已将 `FGC-V002` 标记为 `superseded`。

2026-07-15 用户进一步确认以“3D 机械设计系统”取代 HTML 六面拼接或单一 box 雕刻。G819、Q003、G820–G826 已完成，仍是概念 Mesh/GLB，不是 B-Rep/工程 CAD。A003 已完成 Provider preflight、SSE 生命周期、取消、用量、稳定错误与禁止静默 fallback；F025 已完成 Agent/legacy 控制隔离；D005 已提供四领域各 4 个非工程 Style Token/比例配方。A004 现以 13 个代码所有、Schema 验证的 ForgeCAD Product Tool 建立单 Turn Action Loop，离线 Planner 与 DeepSeek 都能执行候选 build、真实 GLB readback、四视图、硬门和未保存 preview；DeepSeek thinking Tool Call 会在同一短生命周期续传 `reasoning_content`，但不会持久化。M108 进行中：当前源码 GLB 会嵌入并回读 128×128、材质专属、确定性生成的五通道视觉 PBR、真实 zone→material 映射和固定工作室环境。primitive 的材质来自显式数值目录或有限 part-role 绑定；自动化检查实际使用的 material index/role，以及实际可见深色玻璃的 transmission+IOR、信号红涂层的 clearcoat，不把未使用扩展当证据。showcase 只为 box 增加受限 `bevel_approx`，并要求真实 readback 至少出现一个 `bevel_approximation`；这不是自由 fillet。G826 对 box/wedge/cylinder/capsule、六主轴 cylinder/capsule 和受限 bevel 增加了封闭网格外向绕序、无退化三角形及正有向体积 Gate；内置视觉 primitive 以 320 mm 只读展示基线生成 UV 重复元数据，M108 要求每个 fixture primitive 携带该值，readback 拒绝错值和超出有界范围的 UV。工作台仍只有一个 renderer，但 Agent blockout 在 GLB 可用时优先解析该同源 GLB 并检查实际 PBR map 绑定，参数 ShapeProgram 只能作为明确标识的无 GLB 回退。固定环境使用 `ShadowMaterial` 地面和前向 iso 视角；锁定的 Khronos Validator 已对四领域原始 GLB 建立零 error/zero warning 门禁；glTF Transform 写出仍因改变 ForgeCAD readback 而被拒绝，KTX2/BasisU 也未采用。真实 arm64 packaged sidecar 的既有 PBR/readback、ChangeSet、undo/redo、CSG 和重启链已有回归证据；四领域无评分审阅包和独立评审协议已可生成，但人工视觉基准仍未收集。本段的三方向/三项轮换是 F026 前的历史事实；当前 F026 只允许第一条文本方向的单结果兼容适配器，仍未完成 V003。

2026-07-16 用户进一步要求桌面端核心主要由 Rust 编写，并以 OpenAI Codex app-server 的生命周期与协议架构为参考。ADR-0014 已接受 Rust-first 目标：K001 建立 initialize、版本化 JSON-RPC、通知/取消/背压、cursor replay 和 Tauri bridge；K002 迁移 Thread/Turn/Item/Approval policy、DeepSeek Provider 与 Product Tool 所有权；K003 迁移 Project、AgentAssetVersion、ActiveDesignSnapshot、ChangeSet、Quality、Export、SQLite 和对象库所有权。K001–K003 现均已完成：Rust app-server/core 单一拥有 Agent 生命周期与权威产品状态、SQLite/WAL、CAS 和对象库；Python 只保留 capability-gated `RestrictedGeometryExecutor`，无数据库/对象库路径、Provider Key 或 Snapshot 写权限。C105 也已完成。

2026-07-18 用户明确重排实现优先级为 `F026 → A005 → R007 → V003`，并指定 V003 必须采用“单次完整合成 → 真实硬门 → 最多两次同意图原位修复”，不得生成多个完整模型后评分比较。V003 与 C106 均已完成：3 个 reviewed roots 只作内部 exact discriminator 单选，每 Turn 仅一次 synthesis；production 真实经 `RestrictedGeometryExecutor`，provider call 从 deny-on-call 与 `FakeDeepSeekClient.records` 实测为 0，A005 immutable v2 与旧 v1 隔离。R007B 于 2026-07-19 完成工程退出门：单图、多视图 contact sheet、严格 GLB readback 三类证据均在真实 packaged 工作台唯一 renderer 内形成独立、可复现的只读参考→新结果谱系。该证据仍固定 `visual_fidelity_validated=false`、`formal_eligible=false`。`FGC-M108B` 因四领域正式 kit 和三位独立真人逐领域 `4/5` 外部退出门尚未完成而保持 `blocked`。

2026-07-20 C107 完成机械臂黄金路径视觉深化：service-display production GLB 真实回读为 56,244 triangles/109 primitives/8 PBR materials，保持 10 Parts/9 connections/48 outputs 与 0 Provider 调用；连杆、关节、基座、线缆夹和双段夹爪均改为分层 Recipe 几何。`SurfaceLayerProgram@1` 只接受受限向量/Decal/normal/roughness/emissive token，经 Rust 密封 lowering 与 Python RestrictedGeometryExecutor 绑定到一个真实 Material Zone，五通道 PNG 和 provenance hash 进入 GLB/readback；缺失 zone 或篡改 seal 拒绝。工作台 SVG 只作 editor preview，并在唯一 renderer 中支持选择、测量和既有剖切。聚合 C107 Gate 已通过；实际浏览器截图仍明显低于目标图，512 贴图与 56k 网格未达到 M109 的 1K/2K、80–150k/LOD 展示档，M108B 继续 blocked。

2026-07-20 M109A 完成机械臂同源双档：`interactive_preview` 为 18,324 triangles、128×128 五通道 PBR、3.65 MB；按需 `production_concept` 为 99,092 triangles、109 primitives、8 materials、1024×1024 五通道 PBR、约 26 MB。两档来自相同 48-output ShapeProgram，保持 10 Parts/9 connections/Material Zone/Surface Layer lineage，生产工件通过 Q003、M108A、G826、Rust 产品状态/restart 绑定和 0 Provider 调用。真实截图同时证明该工件仍显著低于目标图：缺少可信的关节盒/轴承层级、内骨架、装甲嵌合、紧固件与微表面组织；因此 M109A 只完成双档工件工程退出，M108B 仍 blocked，不能称图片级或独立真人 `4/5`。

2026-07-20 C108 已完成：在不增加 48-output 上限的前提下，service-display 机械臂升级为 19,776-triangle preview 与 101,248-triangle/120-primitive/1K 五通道 PBR production；基座分段、回转环、装甲嵌合、surface panel 和夹爪接触垫进入真实 GLB。packaged 路径完成唯一结果、V1→A005 V2、Snapshot revision 4、28,195,464-byte GLB 导出和第二进程恢复，0 外网/0 凭据。它仍未达到目标图或 M108B 真人 `4/5`；冻结 renderer 冷启动与 A005 第二次全量编译是下一项性能债，HTML/CSS 仍不能成为几何真值。

同日 packaged WebView 首次复验发现 Rust `artifact_readback` 的 production PBR 尺寸仍固定为 512，会以 `FORGECAD_TEXTURE_CONTRACT_STALE` 拒绝新的 1K GLB；已同步为 1024。重建 sidecar 与 `.app` 后，原生首次初始化、编辑/导出、二进制/资源传输、native Item replay 和重启恢复 Gate 全部通过且 Provider 调用为 0。新版已安装至 `/Applications/CAD 工作台.app`；旧工作台观感的另一个根因是 F026 恢复旧 localStorage 项目选择，现以一次性 `agent-first-v1` 偏好迁移清除旧选择但保留数据库与项目。

机械臂 packaged 黄金闭环的当前重建证据（2026-07-20）：`output/arm-mvp-golden-path/packaged-protocol-proof.json` 与 resume 证明一条中文 Brief 经 Rust-owned Turn 产生唯一 C106 `production_concept` preview，确认 V1 后直接用 A005 生成 V2，保存 Snapshot，导出 15,340-triangle、5,746,336-byte GLB，再由第二个新 packaged 进程从同库恢复相同 V2 与一致 hash/bytes/triangles。该路径使用离线内部 Provider，0 外网与 0 凭据读取；本轮原生 packaged WebView 截图复验因 macOS 控制台锁定而 fail-closed，旧 WebView 证据不能替代当前视觉工件。它不是 DeepSeek 质量或目标图视觉验收。因此 `M108B` 仍为 `blocked`、`formal_eligible=false`，不得称照片级或生产发布已完成。R007B 新增的 `reference_class` 数据库漂移已通过前向 0041 migration 修复，未改写历史 0039。

K003 当前五层冻结检查点（2026-07-19）：`output/k003-layered-gate-arm-final-20260719-v3/report.json` 由 `ForgeCADK003LayeredGateReport@1` 记录 `status=passed`、`exit_code=0`、`source_changed=false`，并按 `host → rust_core → rust_python_contract → packaged → workbench` 全部通过。它覆盖当前 C106/R007B/V003/A005 source、packaged 首次/重启、Rust-only 产品状态、Python product/lifecycle HTTP 410、无 Python 数据库/对象库/Provider 路径和工作台 E2E。Host 只有 `HOST_VNODES_PRESSURE_WARNING`，tmp/library 有界容量探针实际通过，因此不是发布阻断。本轮修复的两个真实根因为：可选 SQLite WAL/SHM 在权限加固检查期间消失时被误判为文件系统 503；以及 Provider session 建立失败时未持久化脱敏 `provider_gateway` Item，导致重启 replay 合同不完整。两项均有确定性回归并通过最终聚合。

C105 最终检查点（2026-07-18，`done`）：`EditableComponentRecipe@1`、Recipe ref/request/candidate/instance provenance 与代码所有 registry 已把 Recipe 收敛为 Rust-owned、first-party、visual-only、不可再分发资源；registry 共 8 项并覆盖四领域。实例化与 optional-slot 候选保持零写；固定 child slot、connector `up`、pivot、recipe ref/registry hash 与来源/审阅/许可证随确认后的 AssemblyGraph 保存，active edit 可锚定既有 non-root Part，并在 head/Snapshot/lock/stale 检查后进入密封 ChangeSet 的 preview→confirm。版本升级、旧 candidate hash、undo/redo、重启和重复替换均有 lifecycle 证据。真实 restricted Python executor 只接收 Rust 展开的受限几何，已编译四领域 4 个 `production_concept` GLB，合计 416 triangles，`provider_calls=0`；它仍不能接触 registry、项目数据库/对象库、Provider Key 或 Snapshot 写权。最终独立审计为 `P0=0/P1=0`，Rust focused tests 为 `8+1+7`，根级 lifecycle、contracts/docs/integrity/safety/secrets/agent、typecheck/build/Tauri/R3/diff Gate 全绿。416-triangle fixture 只证明机制和跨语言线路，不是 M108B 生产级概念资产、照片级外观或真人 `4/5` 证据。

同日真实 production 工作台截图暴露了旧任务图的依赖环：双档 GLB、512×512 PBR、平滑法线和内容寻址缓存可以由代码完成，但固定 showcase 仍是 primitive/Loft/Sweep 的早期外观；真正的 child slot、connector、局部变换、语义比例和可复用完整部件属于 C105，而旧 M108 又阻止 C105 开始。ADR-0015 因而将旧 M108 标为 `superseded`，拆为 `FGC-M108A` 与后续 `FGC-M108B`。M108A 已验证 `interactive_preview`（24 段、128×128 v3）和 `production_concept`（48 段、512×512 v4、平滑 Loft/Sweep 法线）的同 ShapeProgram 派生、`GeometryCompileReadback@2`、production 质量/导出、二进制 GLB、延迟替换和 CAS；它只证明工件管线。K003/C105 依赖现已满足；M108B 仍须另建每领域至少 3 份 recipe-backed production fixture，并由至少三位未参与实现的独立真人逐领域对 `proportion`、`material_readability`、`surface_detail` 评分且各项中位数达到 `4/5`。该门在 2026-07-18 重排后为 `blocked`，视觉门未降低。

同日用户进一步明确 F026 目标布局：左侧是项目/对话记录与组件库，中央是 Agent 会话、步骤和单一结果，右侧是持续可见的 3D 区域，底部固定输入框并由“+”打开 Style Token、视觉材质和参考入口。该要求已由 F026 实现，并取代 ADR-0010 早期“左上 mini 3D”的位置描述；同一个 canvas 在 `docked | focus` 间切换，renderer/context 始终为 1。F026 专属 Gate、F001/F006/F025、T002 14/14、T003、r3、typecheck/build 与 1536×960/1180×760 浏览器证据均通过；这只证明工作台 shell，不证明 V003 或 M108B。

以下 2026-07-15/16 的 M108 增量段落是拆分前开发历史，其中“旧 M108 in_progress / C105 blocked”只描述当时状态；当前真值是 M108A、K003、C105 已完成，M108B 因独立真人门未完成而 `blocked`。历史代理审核、截图和 C105 的 416-triangle fixture 仍不能变成 M108B 真人退出证据。

2026-07-15 的本机运行态检查点中，`script/build_and_run.sh --verify` 曾报告 Provider `unconfigured`、`deterministic_mechanical_planner`、`capability_status=offline`、`network_call_made=false`；该段只保留历史。2026-07-20 当前 metadata 已配置为官方 Base URL 与当前 V4 model，0600 文件和 credential generation 均存在：v7 在用户授权后真实发起 1 次网络请求，但 Turn 以 `provider_execution` 失败、token 为 0、资产/Snapshot 写入为 0；当时探针尚未安全投影更细 Provider code。随后已修复启动器“正式报告为 1 次但 CLI 错报 0 次”的赋值边界，并为探针增加显式白名单的认证/余额/限流/服务/超时/传输/结构化输出错误码。DeepSeek adapter 现按官方 V4 合同显式使用 thinking/max、完整续传同 Turn Tool Call reasoning、读取 cache hit/miss，并移除请求侧 no-store；focused Rust 20 项和 production build 通过。`live_v4_thinking_20260720_a1` 仍因当前 ad-hoc binary 未获 Keychain secret read 授权而 `LIVE_REPORT_TIMEOUT`、0 次网络。因此当前准确结论仍是“metadata 配置存在、真实 Provider 未通过验收”，不能写成已连接、已计费完成或 DeepSeek 已生成。

M108 视口边界现明确区分当前显示 GLB 的来源与渲染能力：`compiled_agent_pbr` 缺少完整嵌入 maps 时必须失败；合法只读外部 GLB 可以在同一 renderer 中保留原始材质，但缺五通道时标为 `external_reference`，不得冒充 M108 同源 PBR。通过只读导入进入工作台、但实际具备完整 maps 的四领域评测 GLB 仍报告 `glb_pbr`；只有这类视口事实可进入独立评分。

`npm run agent:m108-visual-benchmark-workbench-capture` 只在同一真实工作台、同一 renderer/canvas 内依次捕获四领域 iso + `cad_neutral` 视口 PNG；`npm run desktop:m108-workbench-renderer-smoke` 则从当前源码重建临时 kit 并作为 workbench E2E CI Gate。最新真实捕获已验证四领域均是 `ready/glb_pbr`、`preview_mode=committed`、`xray=disabled`，并核对保留 GLB metre→millimetre 后的 520 mm 展示对角线、实时环境 recipe hash、PBR 颜色空间、固定 GPU 预算和单 WebGL context；`committed` 只表示当前非 ghost 视口，不是 Git 提交。捕获仍固定标记 `development_visual_audit_only`、`not_scored` 和 `human_benchmark_evidence=false`，只用于开发者发现问题；自动 GPU/环境 Gate 和截图都不是独立人工评分，不能把 M108 改为完成或解除 C105 阻塞。

M108 当前限定视觉修正把通用 showcase 贴片拆成四套互斥的领域/primary-role 白名单；未知或多锚点 fail closed，不引入 C105 Recipe。车辆代表 fixture 已降低座舱、让轮胎接地并增加四个铝轮毂，且显式使用独立 index 7、五通道 coated、`clearcoatFactor=0.86` 的汽车漆；飞机代表 fixture 使用胶囊机身、薄翼/薄旋翼和四个轮毂；机械臂使用胶囊连杆与盒式夹爪；虚构道具移除夸张三角片。它们仍是受限概念 Mesh，不是自由曲面、工程 CAD 或照片级外观；只有独立人工基准可判定是否达到逐领域 4/5 门槛。

M108 进一步把 cylinder/capsule 的固定运行时采样从 16 段提高到 24 段，并由真实 GLB `surface_provenance` 锁定 96/432 triangles；没有新增 operation、自由参数或第二质量模式。评测 manifest 记录真实三轴 `bounds_mm`，工作台核对 GLTFLoader 加载后的毫米 bounds，并按实际 aspect/FOV 投影 8 个角点，要求模型完整落在 NDC `[-0.9, 0.9]` 内；相机距离、动态 fog 和安全区进入无评分捕获，1180×1024 resize 会重新求解，损坏 GLB 会恢复基础工作台并清除旧 blockout facts。本轮实际最大 6,080 renderer triangles；对应上限只因 24 段 pass 保守上界 6,776 从 5,000 调整为 7,000，其余 GPU 上限不变。该自动证据改善棱面和裁切，不证明比例、材质或细节已经达到人工 4/5，M108 仍为 `in_progress`。

M108 新生成 PBR 的 texture-set ID 以 `_builtin_v2` 结尾、map ID 含 `_v2_`、`version=2`：周期平滑微表面替代旧格噪与 composite 硬织纹，coated/brushed/glass 的 baseColor 调制低于 roughness/normal；旧 `builtin` v1 的原 ID/字节仅作为历史 GLB 的精确 readback 清单保留。自动门解码八种材质的全部五通道，对 8/12/16/18/28/32 px 的每个相位拒绝硬格线，只对 metallicRoughness/normal 要求微变化，不强迫 baseColor/AO/emissive 添加噪声。readback 逐 material index 核对 authored→规范 texture material 穷举映射、texture-set/map 元数据、PNG 字节、UV0 TextureInfo 和固定采样状态；同步篡改自报 SHA、自定义 sampler/texture transform、未知材质、布尔伪索引或单资产 v1/v2 混用均失败。正常 v2 首次编译只生成 8 个集合，读取 v1 后 cache 上限为 16 个集合、543,327 字节 PNG；旧 v1 报告相对当前 v2 过期时返回 `stale_compile_readback/unavailable`，组件候选与 confirm 写入前都以最新完整报告重验。四领域固定 fixture 另用既有 primitive 增加部件连接外罩；G818 从最终 GLB POSITION accessor 要求连接罩 AABB 与各目标正体积重叠且有体积位于目标 AABB 并集外，不把它表述为实体相交证明。最新真实工作台最大 6,176 triangles/87 draw calls，仍在预算内。对应 31,793,536-byte、SHA-256 `4b0e43b2d5251bd939bcaaa90b4f62f0476d26c9139a49919f2e38abccb62560` 的 tracked macOS arm64 sidecar 已通过本机 packaged 初始化、当前 PBR readback、CSG、undo/redo、导出和重启恢复，`.app` 构建与 packaged Tauri smoke 通过，`provider_calls=0`；本轮未生成 DMG，v1 历史兼容由源码 M108 Gate 的真实 GLB 改写回读单独证明。模型仍是 Alpha blockout，人工评分未收集，M108 状态不变。

同日本机诊断确认 Agent 服务健康，但 ForgeCAD Provider metadata 与 `ForgeCAD Agent Provider/default` Keychain 项均缺失，运行时因此使用确定性离线 Planner，现有日志没有 `provider:check` 或 DeepSeek 请求。A003 现会把该状态明确显示为未配置且 `network_call_made=false`；只有用户显式保存配置、四段 preflight 就绪并主动发起 Turn/连接测试时才可能联网。官方当前模型 `deepseek-v4-pro` 有效，不是此前“无响应”的根因。本结论只描述本机 2026-07-14 配置快照，不代表其他机器或后续配置状态；本轮也未执行真实 Provider 评测。

M108 审阅真值增量（2026-07-16）：工作台截图前必须证明 ModuleGraph root 隐藏、blockout root 可见、axes/grid/transform helper 全部隐藏且 renderer line 数为 0，并把相同事实写入捕获 manifest；当前源码重建的四领域画面均通过，旧过暗/带坐标轴工件不会成为通过输入。评分校验器从提交 GLB 真实 readback 要求至少五套当前 `_builtin_v2`、完整五通道 `_v2_` map 和 128×128 尺寸，拒绝 manifest 自报替代。航空器四个旋翼支柱还从最终 POSITION accessor 要求与对应机翼 Z 范围至少重叠 0.07 m。以上仍是自动化概念视觉证据；真实独立评审未完成，M108 保持 `in_progress`、C105 保持 blocked。

M108 最终 GLB 真值增量（2026-07-16）：12 份固定审阅 fixture 的最终 BIN POSITION 现在由严格 accessor/bufferView 解码并与声明 bounds 对照，负索引、越出 view、非法 stride/alignment、缺失显式 buffer、伪造图片 view 和 scene/node 变换或实例均 fail closed；当前 ShapeProgram GLB 只接受单 mesh、单 scene、单 identity node。A/B/C fixture 的视觉连续性门要求一个最终 AABB 分量，新增航空器 pod 与机械臂 wrist/rail/carriage 外罩还锁定由目标部件推导的中心、轴向和双边尺寸范围。该证据只覆盖 12 份 fixture，且 AABB 连续不等于实体焊接、工程 connector 或全部 catalog；视觉件仍是 root 级绝对展示分组，真实配方附着归 C105。独立人工视觉评分仍为空，M108/C105 状态不变。

M108 Loft 与代理审核增量（2026-07-16）：车辆/航空器 A 代表资产的主壳与座舱已切换为真实 canonical ProfileSectionSet 驱动的受限 Loft，固定截面、参数、材质区和来源仍经 Schema/G819/Worker/Q003 同一链。Loft/Sweep 不再把 0–1 UV 拉伸一次覆盖长壳，而按周长与路径物理距离以 320 mm 展示基线生成并从 GLB 回读。车辆已去除屏幕中明显突兀的后部三角板与前端亮白盖；航空器实心旋翼盘改为小轮毂+叶片，工作台最高为 6,196 triangles/96 draw calls，未越 GPU 预算。Codex 只以明确标识的代理审查为开发反馈，不写人工回复、不伪造真人身份；代理结论仍指出飞机翼面偏大平直、所有领域表面细节仍为 Alpha 概念级。因此 M108 仍为 `in_progress`，C105/V003/F026 未解锁。

M108 Airfoil 与第二轮代理审核增量（2026-07-16）：航空器 A 左右主翼现以代码所有、四段 tangent quadratic 的非对称 `ProfileSketch@1` 经 Z 主轴 `ProfileSectionSet@1 → loft` 真实生成，固定 16 点重采样、600 mm 轴长和 420×24 mm 截面尺度由 G818 锁定；未开放自由曲线、细分参数或新 operation。四个轮毂为 52×48 mm，并各有两片交叉叶片；道具和机械臂突兀三角 guard 已改为紧凑 bevel box。`codex-iteration-9` 真实工作台 readback 为道具 4,688/33、车辆 6,748/72、航空器 6,508/96、机械臂 4,960/45（triangles/draw calls），全部单 WebGL context、GPU passed。Codex 第二轮代理评分仍只有 3–4 分，四领域均未同时达到比例、材质、细节 4/5；报告不写入人工响应，不能解除 M108/C105。tracked arm64 sidecar 已从当前源码重建为 31,809,232 bytes、SHA-256 `e6ca477d0b98b34ba0d20c0e53c4b61d69781124a0fe955685b6892e423133ff`，packaged sidecar 和新 `.app` 的原生 Tauri smoke 均覆盖 PBR/CSG/undo/redo/导出/重启并通过，`provider_calls=0`。

M108 四领域轮廓与连接细化增量（2026-07-16）：虚构道具 A 主壳由 capsule 改为六截面受限 Loft，并加入复合传感器壳和深色玻璃面；车辆 A 显式绑定橡胶轮胎、缩薄侧桥并增加四个受限楔形轮眉；航空器 A 的四个旋翼支架缩至约 40.32 mm 厚、120 mm 深，最终 GLB 与对应翼面 Z 范围仍至少重叠 0.03 m；机械臂 A 增加肩/肘/腕铝端盖。`codex-iteration-11` 真实工作台 readback 为道具 6,836/51、车辆 6,844/84、航空器 6,508/96、机械臂 5,536/51（triangles/draw calls），均保持单 WebGL context、`glb_pbr`、固定环境和 GPU passed。31,809,920-byte、SHA-256 `50bc173dd452d6e29e789f371bf437d2b6b9e252d949da1eb0ae35035ff74c4c` 的 tracked arm64 sidecar 已通过 require-ready、packaged sidecar、Tauri `.app`/DMG build 与 packaged Tauri smoke，`provider_calls=0`。Codex 代理审核仍认为连续轮拱、翼根/推进外罩、线缆/执行器和端部过渡不足，四领域没有同时达到三维度 4/5；M108 继续 `in_progress`，C105 不解锁。

M108 Sweep 连接与线缆增量（2026-07-16）：虚构道具 A 握把由 capsule 改为五截面 Y 主轴 Loft，安装环从真实显示外包围恢复半径；车辆 A 的四个楔形轮眉改为四点路径、八点截面的封闭 G823 Sweep，并收敛重复座舱框、顶置排气和侧围紧固件以保持固定 GPU 预算；航空器 A 的四块平板旋翼支架改为封闭 Sweep 曲线外罩，尾部圆柱视觉排气口改为楔形；机械臂 A 增加封闭橡胶服务线缆 Sweep。`codex-iteration-14` 真实工作台 readback 为道具 6,248/51、车辆 6,892/78、航空器 6,868/96、机械臂 5,720/53（triangles/draw calls），均为单 WebGL context、`glb_pbr`、固定环境和 GPU passed；车辆 7,180 与航空器 7,132 的中间结果按真实 renderer 预算失败后才减面，没有放宽 7,000/96 上限。glTF Transform 评估改为临时文件 readback，消除大 GLB 同步 stdin 的偶发等待，但 writer 仍按原合同被拒绝。31,813,296-byte、SHA-256 `202dca17abcbb2c6210c1b753cdebc5607747dcb34482ca8dce7e0975b5c4383` 的 tracked arm64 sidecar 已通过 require-ready、packaged sidecar Alpha 和 Tauri check，`.app`/DMG 已重建；当前打开的 CAD 工作台占用固定端口，因此本轮未重复运行 packaged Tauri smoke。完整 packaging readiness 仍因 Intel macOS、Windows 和 Linux 空 sidecar 占位按设计阻断。Codex 代理审核仍认为道具偏筒形、车辆轮眉有模块拼接感；独立人工视觉评分仍为空，M108/C105 状态不变。

M108 硬表面截面与领域轮廓增量（2026-07-16）：固定 showcase 新增八段 line/quadratic 的代码所有 `hard_surface` ProfileSketch，道具 A 主壳和车辆 A 底盘通过既有 G822 Loft 获得平顶/平底/直侧带与圆角肩线；它不是自由轮廓或工程截面。车辆轮眉提升为五点 Sweep、24×18 mm 视觉截面，两个圆形顶置视觉口改为低面数楔形槽；航空器主翼改为 700 mm Z 主轴、360×32 mm airfoil 比例并收紧翼尖；机械臂上下夹爪改为三截面渐缩 hard-surface Loft。第一次车辆 renderer 以 7,084 triangles 超过原 7,000 门而失败，最终 `proxy-review-20260716-iteration15b` 为道具 6,248/51、车辆 6,556/78、航空器 6,868/96、机械臂 5,832/53（triangles/draw calls），四项均保持同源 `glb_pbr`、固定环境、单 context 和 GPU passed。`agent:m108-gate` 与真实工作台 renderer 已通过；tracked arm64 sidecar 为 31,815,424 bytes、SHA-256 `bd582746e0daa3646a1de1b3ea881ddcc66ccdf003e9f03377279ee32038793b`。代理审核认为轮廓和连接可读性提升，但不写人工评分，M108 仍为 `in_progress`，C105/V003/F026 不解锁。

M108 v3 微表面纹理增量（2026-07-16）：新生成资产从 v2 升级为 `_builtin_v3`/`_v3_`/`version=3`，用材质专属、高频低振幅、多尺度、周期连续的 roughness/normal 细节表达拉丝、机加工、复合材料细纹、橡胶颗粒和涂层橘皮；baseColor 只保留弱色差。第一次 v3 真实 renderer 虽通过自动预算，但代理视觉审核拒绝机械臂铝件波纹和明显复合棋盘，最终迭代已压回细微材质响应。`proxy-review-20260716-iteration17-v3` 四领域仍为 6,248/51、6,556/78、6,868/96、5,832/53（triangles/draw calls），没有通过堆几何改善纹理。历史 v2/v1 继续精确 readback，聚合 SHA-256 分别为 `045f788cce7bdb8a83cfa8bbdfec0e554a2914e4637b63ef526ecb136aaab661` 与 `0b4701fe31946dfc9572990daa5e1e9260d05ddcfcfdef640c9eac776e10b62f`；跨版本混用拒绝，三版本 cache 上限为 24 个集合、702,750 字节。完整 M108 Gate 与真实 renderer 通过。tracked arm64 sidecar 已重建为 31,817,584 bytes、SHA-256 `39b8a0cf9e4038a5ea36f03307e67371b962d11f338886cc66dc9af1e7ca92c9`，require-ready、packaged sidecar Alpha 和 Tauri check 通过，覆盖 v3 PBR/CSG/undo/redo/导出/重启且 `provider_calls=0`；用户当前运行的 ForgeCAD Agent 占用 8000 端口，packaged Tauri smoke 未重复运行。截图仍不是独立人工评分，M108/C105/V003/F026 状态不变。

M108 材质显示真值增量（2026-07-16）：后端材质 ChangeSet 原本已经更新 zone binding、ShapeProgram `material_id` 并重编译 GLB，但桌面 preview/confirm 只更新 ShapeProgram，且活动内部资产恢复不会请求 GLB，导致视口可能一直优先显示旧材质。现在新 ShapeProgram 预览先清除旧 GLB/来源，当前 Agent asset 的 hydrate 会按活动 `asset_version_id` 重新导出并加载 `compiled_agent_pbr`；失败只保留明确的参数外观回退。完整 13 项材质目录、搜索/分类/领域筛选和 Material Zone 从隐藏 rail 移到左栏按需展开，内置项标明为同源五通道 PBR。G6 真实比较换材质前后 GLB hash，并回读目标 zone、authored material 和当前 `_builtin_v3`；F009 锁定旧 GLB 不得跨 ChangeSet 预览存活。该修复不构成照片级视觉达标，M108 继续 `in_progress`。

M108 材质预览闭环增量（2026-07-16）：服务端新增不落库的 ChangeSet 二进制预览 GLB 读取，只有 ChangeSet、head、ActiveDesignSnapshot.active_design 和 Snapshot.preview 完全一致才会编译并返回；编译前后重复验证，响应提供 SHA/base asset/triangle headers。材质操作新增真实 Domain Pack→`allowed_domains` 拒绝。桌面读取活动版本 `material_bindings[part:zone]` 作为已提交显示真值，跨 part/zone 的本地 preselection 不再污染；快捷材质按领域过滤并携带稳定 `material_zone_id`，旧部件卡硬编码材质旁路已删除。确认入口只在同一 request token 的 `compiled_agent_pbr` 预览进入视口后出现，失败会 reject 并恢复已提交 GLB。T002 14/14 已覆盖请求体、binary headers、`glb_pbr` 和 preview 清理。13 个目录项仍只对应 8 套规范内置 PBR 外观，登记纹理未进入编译；真实截图仍显示 Alpha blockout，M108/C105/V003/F026 状态不变。

M108 虚构道具与航空器细化增量（2026-07-16）：代表未来游戏道具 A 的鼓形主体已替换为 7 截面 hard-surface Loft，并以受限 Loft/Sweep 增加分层前罩、下罩、渐缩后罩、传感器罩、倾斜握持外观、侧面流线和错列视觉通风；端部是铝边框与深色玻璃面，小红 badge 只作识别色，不表达功能机构。航空器四个原先弯管感较强的旋翼支柱改为 18×42 mm、三点低拱、主壳同色的 Sweep 外罩。最终真实工作台为道具 5,772/68、车辆 6,556/78、航空器 6,676/96、机械臂 5,832/53（triangles/draw calls），均通过同源 `glb_pbr`、单 context、安全取景和 GPU 预算；没有新增操作、Recipe、制造语义或放宽 7,000/96 上限。画面仍是受限 Alpha 概念资产，独立人工评分为空，M108/C105/V003/F026 状态不变。

评分校验中的“至少五套”按至少五个不同 material index、texture-set ID 和规范 texture material 计算，重复 authored alias 不能累加；renderer line instrumentation 缺失、非法或非零都会 fail closed。

## 2. 事实的唯一归属

| 问题 | 唯一权威 | 允许引用 | 不得作为证据 |
| --- | --- | --- | --- |
| 当前用户能做什么 | `docs/USER_GUIDE.md` | 当前 Gate 矩阵、当前 smoke | DESIGN 中的目标工作流、旧截图 |
| 产品范围与安全边界 | `docs/PRODUCT_DEFINITION.md` | ADR-0008 | legacy Weapon 文档 |
| 目标架构与未实现设计 | `docs/DESIGN.md` | 执行计划 | 用户指南、历史 evidence |
| Project/Version/Selection/Quality/Export 真值 | `docs/AUTHORITATIVE_STATE.md` | API、Schema | localStorage、旧 Concept hook |
| 当前 HTTP 合同 | `docs/API.md` 和 JSON Schema | 生成 OpenAPI/TypeScript | legacy API |
| 任务顺序与领取资格 | `docs/CODEX_EXECUTION_PLAN.md`、`docs/CODEX_TASK_INDEX.md` | 本文件 | 聊天中的口头进度 |
| Gate 是否真的通过 | `docs/evidence/CAPABILITY_GATE_MATRIX.md` + 本轮命令输出 | evidence 历史记录 | “曾经通过”但未重跑的旧报告 |
| 事故恢复 | `docs/DISASTER_RECOVERY.md` | 备份/恢复 smoke | 手工复制 SQLite |
| 发布是否可交付 | `docs/PRODUCTION_RELEASE_CHECKLIST.md`、`docs/RELEASE_MAINTENANCE.md` | packaging gate | 本机 dev server 能启动 |

## 3. 当前状态标签规则

每个能力只能使用一个标签：

- `已实现`：代码存在，当前任务 Gate 通过，且用户指南可以描述；
- `部分实现`：有可运行子集，必须同时列出未支持子能力；
- `目标设计`：只存在合同、设计或计划，不能写入用户指南；
- `legacy`：只用于兼容、迁移或历史回归；
- `blocked`：任务有明确退出条件，但依赖或 Gate 失败；
- `external`：需要真实 Provider、独立 reviewer、签名账户或测试设备等仓库外输入。

“通过一次”不等于“生产就绪”。例如 Agent-first 工作台 smoke 通过，只能证明该确定性路径；它不覆盖真实 Provider 质量、全新机器安装、多客户端压力或签名发布。

## 4. 当前能力与阻断账本

| 能力 | 当前标签 | 当前证据/入口 | 仍缺什么 |
| --- | --- | --- | --- |
| 四领域推断、类别澄清与范围预检 | 已实现（受限） | D001–D003、G814、13 场景工作台 E2E | 真实 Provider truth set、多语言评测；范围策略不是完整内容安全系统 |
| Agent 单次生成与 legacy blockout | 已实现（受限） | V003 以一次完整 synthesis、13 项 v2 Gate、最多两次同意图修复和一个 formal preview 取代三方向用户选择；四领域固定 Brief、Playwright 单结果、确认与零写失败路径已通过。legacy Planner/三方向仅保留于明确隔离的 compatibility/test fixture，compatibility preview 没有 formal provenance | 真实 DeepSeek 四领域质量与自由外观生成仍待评测。M108A 工件管线不等于 M108B 生产级视觉基线；当前 Recipe/展示档也不等于照片级真实、真实材料或工程设计 |
| ActiveDesignSnapshot 单一状态 | 部分实现 | S001–S008、F025、Agent-first r3；legacy 细节只在显式只读表面加载 | 广泛多客户端压力、legacy 兼容数据最终迁移 |
| Snapshot bootstrap/质量检查幂等 | 已实现（受限） | Q002 API replay/stale/Agent+legacy bootstrap smoke | 广泛多客户端压力与生产缓存策略 |
| 受限 ShapeProgram | 部分实现 | G3、G5、G801–G806、G819–G826、Q003；canonical Profile 可驱动 Extrude/Revolve/Sweep，ordered section set 可驱动受限 Loft；union/subtract 由唯一 Manifold Python handler 执行并回读不可变 Feature History；G826 回读 edge finish/normal/UV0/tangent 与稳定 face→part/zone；M108A 已把 preview/production 五通道内置 PBR、profile identity 和 CAS 写入同源 GLB/readback | 自由曲面、精确 CAD、碰撞/运动学未实现；Planner 尚未自动使用新语法；M108B 独立生产级视觉基准仍未完成 |
| 可编辑参数声明与语义比例 | 已实现（受限） | G808–G811；D005 四领域 Style Token/语义槽/真实 binding+GLB provenance、preview/confirm/restart/undo/redo Gate；C105 已完成 Recipe 机制接入；C106 为机械臂关节护罩/连杆装甲/表面 trim 增加冻结 G808 ratio-only 外观 binding 并在 AssemblyGraph 回读；V003 把所选 Style Token 的 code-owned 比例调整真实应用到 Recipe ShapeProgram/AssemblyGraph 并以 hash 回读 | 自由参数、工程关节、扭矩/负载和工程尺寸明确不在当前范围 |
| 可编辑 Agent 资产 | 部分实现 | G6、C103、C104、工作台 E2E | 深度自动分件、自由 split/merge、任意版本浏览 |
| 主视口相机/灯光预设 | 已实现（Alpha） | R001 smoke | 工程渲染 |
| Agent 多视图 PNG/概念图包 | 已实现（Alpha） | R002–R004 smoke、抽屉与工作台 E2E | 转台视频、工程渲染、真实 Provider 质量；爆炸图受真实几何分组约束，图包只含 PNG/manifest |
| Agent GLB 导出 | 部分实现 | G6/G7、r3、R005 浏览器下载 smoke | Agent 抽屉已直接提供 GLB；原生 WebView 点击、全新机安装与广泛并发仍待 |
| 组件/材质目录 | 部分实现 | F004、G6、M101–M107、C101–C106 已完成；C106 的 3 roots/6 reusable components、9/9 A005 slots 与 production GLB/lifecycle Gate 已通过 | C106 仍只是机械臂黄金路径，不是四领域正式 kit；M108B 仍缺其他领域 production Recipe kit 与独立真人视觉门 |
| Provider 与桌面 sidecar | 部分实现 | K001–K003 已完成 Rust-owned `forgecad.app-server/1`、Thread/Turn/Item/Approval、Context/DeepSeek/Product Tool，以及 Project/Version/Snapshot/ChangeSet/Quality/Export、SQLite/WAL/CAS/对象库所有权；五层聚合、packaged 双启动、T002/T003/r3/M108 与最终后端门通过；Python 仅为无持久化权限的受限几何执行器；E001/E002 no-call 评测合同、P002/P008 packaged Alpha 证据 | 真实 DeepSeek 人工授权评测、新机器密钥发布策略及多平台正式安装仍待；fake/离线 Gate 不代表真实模型质量或费用 |
| 生产发布 | blocked | `release:packaging-readiness` 当前以 `SIDECAR_BINARY_INVALID` 拒绝 Intel macOS、Windows、Linux 空 sidecar | 三个剩余目标的非空可执行 sidecar、安装/升级、公证/签名、全新机恢复 |
| CAD 设计能力闭环 | 部分实现 | G819/Q003、G820–G826、A003–A005、F025/F026、D005、M108A、K001–K003、C105–C106、R007A 与 V003 已完成；R007B 自动工程 Gate 已通过 | R007B 仍需 exact-lineage 同工作台对比和视觉保真证据，其后是 M108B 四领域正式 kit/真人 `4/5` 与 M109；当前不保证图片级外观或通用生产级质量 |

## 5. 每次任务结束必须更新的文件

至少同步以下文件，避免状态漂移：

1. `docs/CODEX_TASK_INDEX.md`：任务状态、证据、下一项任务；
2. `docs/CODEX_HANDOFF.md`：当前工作区、命令结果、已知限制；
3. `docs/evidence/CAPABILITY_GATE_MATRIX.md`：能力标签与对应 Gate；
4. 受影响的 `API.md`、`SCHEMAS.md`、`AUTHORITATIVE_STATE.md`、`USER_GUIDE.md` 或 `OPERATIONS.md`；
5. 若只是目标设计，更新 `DESIGN.md`/`CODEX_EXECUTION_PLAN.md`，不要修改 `USER_GUIDE.md` 宣称已支持。

任务状态必须包含日期、工作区/commit 情况和命令结果。脏工作区可以交接，但必须明确“未提交”。

## 6. 文档审查顺序

后续 Codex 开始前按以下顺序读取：

```text
AGENTS.md
→ DOCUMENTATION_MAP.md
→ DOCUMENTATION_STATUS.md
→ CODEX_HANDOFF.md
→ CODEX_EXECUTION_PLAN.md
→ CODEX_TASK_INDEX.md
→ AUTHORITATIVE_STATE.md
→ USER_GUIDE.md
→ DESIGN.md
→ 与任务直接相关的 API / Schema / 测试 / 操作文档
```

如果这些文件对同一事实冲突，以 `DOCUMENTATION_MAP.md` 的唯一归属表为准；无法归属时先停止实现，修正文档合同，再领取代码任务。

## 7. 必跑文档门

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
git diff --check
```

这些命令只能证明文档结构、仓库完整性和安全边界，不会替代 Agent、工作台、安装或真实 Provider Gate。任何已知失败都必须保留并写入 handoff，不得删除测试或放宽断言来让文档门通过。
