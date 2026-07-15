# ForgeCAD 文档状态账本

版本：2026-07-16
状态：当前文档维护真值；不是产品运行时能力证明

本文件解决一个具体问题：ForgeCAD 同时有产品说明、目标设计、历史证据、兼容资料和任务计划。没有一个短的状态账本时，后续 Codex 容易把“目标设计”或“过去通过的 smoke”误读为当前已完成能力。

## 1. 当前一句话结论

ForgeCAD 是本机 Alpha 的轻量通用机械概念 3D Agent，当前已经有四领域的确定性后端 blockout、Agent 资产版本、受限编辑、Snapshot 真值、GLB 导出和工作台浏览器回归；它还不是生产级通用 3D 工作台。

`FGC-R002`–`FGC-R005`、`FGC-M101`–`FGC-M107`、`FGC-C101`–`FGC-C104`、`FGC-G808`–`FGC-G813`、`FGC-Q002` 与 `FGC-F007`–`FGC-F024` 已完成；R003 只在几何分组与稳定 Part 一一对应时生成透明爆炸概念图，否则明确不可用；R004 只将当前、指纹一致的 PNG 与机器可读清单打包下载，ZIP 不含模型源文件或工程资料；R005 让 Agent 下载抽屉只显示直接 GLB、单 PNG 与概念图包，浏览器 E2E 已通过，而原生 WebView 点击仍因当前会话缺少 macOS 辅助功能权限待复验。F008–F024 的纯展示边界见任务索引；F024 只说明已返回方向是离线规划、已连接模型服务生成还是来源待确认，绝不把确定性结果称为真实模型质量，也不显示 Provider 或模型内部标识。G812/G813 已让三张方向卡的 build/segment 使用同一个、按领域/轮廓/方向稳定解析的视觉变体，并让未保存候选以“换一版外观”轮换当前方向的三项预审外观；F022/F023 只保存并翻译其 project/request 屏障内的展示上下文，不公开 48 项技术目录或参数，且不写版本、Snapshot、质量或导出。候选与已确认资产仅通过持久化 ShapeProgram/AssemblyGraph 追溯外观来源。M107 将 Material Zone 选择纳入 Snapshot/CAS，并覆盖重启与 undo/redo 保留，C101 将稳定内部 part role 显示为中文并对未知 role 安全回退，C102 为项目内组件提供来源质量、领域、role 与连接保留的可解释替换边界，C103 则只在现有装配/几何事实充分时提供拆分或合并候选并强制 preview→confirm，C104 则让锁定、隐藏与单独查看通过同一 Snapshot/CAS 保存，锁定由后端阻止相关 ChangeSet，G808 冻结 Part 参数的路径/范围/步长/单位/显示名合同，G809 已将非空声明接入 ChangeSet 的路径/范围/步长验证并冻结旧资产六路径兼容，G810 让四领域新 blockout 的真实单一 size 输出生成有界比例声明，G811 将真实声明接入当前 AssetVersion 的零基础步进控件，Q002 冻结 bootstrap 的兼容语义并使质量写入按 Snapshot ETag 幂等；不支持自由参数、单位换算或工程尺寸；均不引入工程材料数据库、正式审阅冒充或工程结论。

`FGC-E001` 已冻结 4×20 正常 Brief 与 20 条安全停止评测；`FGC-E002` 已提供默认拒绝联网的隔离执行器、80 次正常 Provider 请求上限、本地安全停止和脱敏 run report；`FGC-A002` 让该隔离器在 macOS 上显式复用 ForgeCAD 的 Keychain 配置，而不把密钥导出到环境或报告。`FGC-G814` 已把其中的有限概念范围边界接入普通 Turn：`ConceptScopeDecision@1` 在 DomainInference 后、Planner/Provider 前本地决定允许、类别澄清或范围停止；明确现实制造、工程安全/控制请求只保留可读 Turn/Item，不创建任何 Plan、资产或 Snapshot。`FGC-G815` 已让安全 Brief 的有限轮廓、细节、色彩和展示姿态分类稳定选择已有四领域视觉族，且每个选择仍经现有 ShapeProgram/GLB/分件/确认链；这不是自由风格生成、真实 Provider 创意质量或工程 CAD。它们只证明合同与执行边界可安全加载；真实 Provider baseline 仍为 `external`，绝不能因 E001/E002/A002/G814/G815 或离线 Gate 标记为通过。

`FGC-R006` 已完成：三张未保存方向在选择前可各自显示同源的 320×240 软件概念 PNG。该调用不写入幂等、候选、资产、Snapshot、质量或导出；前端只在 project + plan + request 的临时上下文保留图片，开始新 Brief、选择方向、换一版或切换项目都会丢弃，迟到结果不会回写。它不是下载、真实渲染、工程图或制造资料。

`FGC-P008` 已完成：版本化 `ForgeCADPackagedSidecarInput@1` 只声明本机 packaged Alpha 所需目标二进制、架构、启动与健康检查边界，并用无密钥、离线、非执行预检区分 `blocked_missing_sidecar` 与 `ready_for_local_alpha`。当前 macOS arm64 输入已为非空 Mach-O 并报告 `ready_for_local_alpha`，P002 本机 Alpha 也已完成；Intel macOS、Windows、Linux sidecar 仍为空占位，因此安装、签名、公证和跨平台发布继续 blocked。

2026-07-14 用户明确取消“三方向让用户选择”的目标，并要求 Agent 内部选择最佳结果、Codex 式简洁工作台、DeepSeek/Codex/Claude 式运行模型、专属 Skill、高真实度纹理/多材质、参考引导重建和通用生活机械扩展。ADR-0010 已将 `FGC-V002` 标记为 `superseded`。

2026-07-15 用户进一步确认以“3D 机械设计系统”取代 HTML 六面拼接或单一 box 雕刻。G819、Q003、G820–G826 已完成，仍是概念 Mesh/GLB，不是 B-Rep/工程 CAD。A003 已完成 Provider preflight、SSE 生命周期、取消、用量、稳定错误与禁止静默 fallback；F025 已完成 Agent/legacy 控制隔离；D005 已提供四领域各 4 个非工程 Style Token/比例配方。A004 现以 13 个代码所有、Schema 验证的 ForgeCAD Product Tool 建立单 Turn Action Loop，离线 Planner 与 DeepSeek 都能执行候选 build、真实 GLB readback、四视图、硬门和未保存 preview；DeepSeek thinking Tool Call 会在同一短生命周期续传 `reasoning_content`，但不会持久化。M108 进行中：当前源码 GLB 会嵌入并回读 128×128、材质专属、确定性生成的五通道视觉 PBR、真实 zone→material 映射和固定工作室环境。primitive 的材质来自显式数值目录或有限 part-role 绑定；自动化检查实际使用的 material index/role，以及实际可见深色玻璃的 transmission+IOR、信号红涂层的 clearcoat，不把未使用扩展当证据。showcase 只为 box 增加受限 `bevel_approx`，并要求真实 readback 至少出现一个 `bevel_approximation`；这不是自由 fillet。G826 对 box/wedge/cylinder/capsule、六主轴 cylinder/capsule 和受限 bevel 增加了封闭网格外向绕序、无退化三角形及正有向体积 Gate；内置视觉 primitive 以 320 mm 只读展示基线生成 UV 重复元数据，M108 要求每个 fixture primitive 携带该值，readback 拒绝错值和超出有界范围的 UV。工作台仍只有一个 renderer，但 Agent blockout 在 GLB 可用时优先解析该同源 GLB 并检查实际 PBR map 绑定，参数 ShapeProgram 只能作为明确标识的无 GLB 回退。固定环境使用 `ShadowMaterial` 地面和前向 iso 视角；锁定的 Khronos Validator 已对四领域原始 GLB 建立零 error/zero warning 门禁；glTF Transform 写出仍因改变 ForgeCAD readback 而被拒绝，KTX2/BasisU 也未采用。真实 arm64 packaged sidecar 的既有 PBR/readback、ChangeSet、undo/redo、CSG 和重启链已有回归证据；四领域无评分审阅包和独立评审协议已可生成，但人工视觉基准仍未收集。当前 Planner 尚未自动采用新几何语法/Recipe 或只显示唯一最佳结果，Alpha 仍显示三方向和受限三项外观轮换。

M108 视口边界现明确区分当前显示 GLB 的来源与渲染能力：`compiled_agent_pbr` 缺少完整嵌入 maps 时必须失败；合法只读外部 GLB 可以在同一 renderer 中保留原始材质，但缺五通道时标为 `external_reference`，不得冒充 M108 同源 PBR。通过只读导入进入工作台、但实际具备完整 maps 的四领域评测 GLB 仍报告 `glb_pbr`；只有这类视口事实可进入独立评分。

`npm run agent:m108-visual-benchmark-workbench-capture` 只在同一真实工作台、同一 renderer/canvas 内依次捕获四领域 iso + `cad_neutral` 视口 PNG；`npm run desktop:m108-workbench-renderer-smoke` 则从当前源码重建临时 kit 并作为 workbench E2E CI Gate。最新真实捕获已验证四领域均是 `ready/glb_pbr`、`preview_mode=committed`、`xray=disabled`，并核对保留 GLB metre→millimetre 后的 520 mm 展示对角线、实时环境 recipe hash、PBR 颜色空间、固定 GPU 预算和单 WebGL context；`committed` 只表示当前非 ghost 视口，不是 Git 提交。捕获仍固定标记 `development_visual_audit_only`、`not_scored` 和 `human_benchmark_evidence=false`，只用于开发者发现问题；自动 GPU/环境 Gate 和截图都不是独立人工评分，不能把 M108 改为完成或解除 C105 阻塞。

M108 当前限定视觉修正把通用 showcase 贴片拆成四套互斥的领域/primary-role 白名单；未知或多锚点 fail closed，不引入 C105 Recipe。车辆代表 fixture 已降低座舱、让轮胎接地并增加四个铝轮毂，且显式使用独立 index 7、五通道 coated、`clearcoatFactor=0.86` 的汽车漆；飞机代表 fixture 使用胶囊机身、薄翼/薄旋翼和四个轮毂；机械臂使用胶囊连杆与盒式夹爪；虚构道具移除夸张三角片。它们仍是受限概念 Mesh，不是自由曲面、工程 CAD 或照片级外观；只有独立人工基准可判定是否达到逐领域 4/5 门槛。

M108 进一步把 cylinder/capsule 的固定运行时采样从 16 段提高到 24 段，并由真实 GLB `surface_provenance` 锁定 96/432 triangles；没有新增 operation、自由参数或第二质量模式。评测 manifest 记录真实三轴 `bounds_mm`，工作台核对 GLTFLoader 加载后的毫米 bounds，并按实际 aspect/FOV 投影 8 个角点，要求模型完整落在 NDC `[-0.9, 0.9]` 内；相机距离、动态 fog 和安全区进入无评分捕获，1180×1024 resize 会重新求解，损坏 GLB 会恢复基础工作台并清除旧 blockout facts。本轮实际最大 6,080 renderer triangles；对应上限只因 24 段 pass 保守上界 6,776 从 5,000 调整为 7,000，其余 GPU 上限不变。该自动证据改善棱面和裁切，不证明比例、材质或细节已经达到人工 4/5，M108 仍为 `in_progress`。

同日本机诊断确认 Agent 服务健康，但 ForgeCAD Provider metadata 与 `ForgeCAD Agent Provider/default` Keychain 项均缺失，运行时因此使用确定性离线 Planner，现有日志没有 `provider:check` 或 DeepSeek 请求。A003 现会把该状态明确显示为未配置且 `network_call_made=false`；只有用户显式保存配置、四段 preflight 就绪并主动发起 Turn/连接测试时才可能联网。官方当前模型 `deepseek-v4-pro` 有效，不是此前“无响应”的根因。本结论只描述本机 2026-07-14 配置快照，不代表其他机器或后续配置状态；本轮也未执行真实 Provider 评测。

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
| Agent 方向与后端 blockout | 部分实现 | G4、G807、G812、G813、G815、G817、G818、R006、A004；三方向稳定匹配四领域受限视觉变体，未保存候选可在同方向三项族中轮换；`quick_sketch`/`showcase` 有受限外观层。A004 在同一 Turn 内为一个候选执行真实 GLB readback/四视图/硬门，但桌面不再自动并发请求三张方向概念图 | 唯一最佳结果等待 V003；真实 Provider 质量与自由外观生成仍待评测，M108 内置五通道 PBR 自动门已通过但独立人工视觉基准尚未收集；视觉层不等于真实材料、孔槽、散热或电气设计 |
| ActiveDesignSnapshot 单一状态 | 部分实现 | S001–S008、F025、Agent-first r3；legacy 细节只在显式只读表面加载 | 广泛多客户端压力、legacy 兼容数据最终迁移 |
| Snapshot bootstrap/质量检查幂等 | 已实现（受限） | Q002 API replay/stale/Agent+legacy bootstrap smoke | 广泛多客户端压力与生产缓存策略 |
| 受限 ShapeProgram | 部分实现 | G3、G5、G801–G806、G819–G826、Q003；canonical Profile 可驱动 Extrude/Revolve/Sweep，ordered section set 可驱动受限 Loft；union/subtract 由唯一 Manifold Python handler 执行并回读不可变 Feature History；G826 回读 edge finish/normal/UV0/tangent 与稳定 face→part/zone；M108 已把五通道内置 PBR 写入同源 GLB/readback | 自由曲面、精确 CAD、碰撞/运动学未实现；Planner 尚未自动使用新语法；M108 独立人工视觉基准仍待收集 |
| 可编辑参数声明与语义比例 | 已实现（受限） | G808–G811；D005 四领域 Style Token/语义槽/真实 binding+GLB provenance、preview/confirm/restart/undo/redo Gate | 自由参数与工程尺寸明确不在当前范围；Agent 自动选配方等待 C105/V003 |
| 可编辑 Agent 资产 | 部分实现 | G6、C103、C104、工作台 E2E | 深度自动分件、自由 split/merge、任意版本浏览 |
| 主视口相机/灯光预设 | 已实现（Alpha） | R001 smoke | 工程渲染 |
| Agent 多视图 PNG/概念图包 | 已实现（Alpha） | R002–R004 smoke、抽屉与工作台 E2E | 转台视频、工程渲染、真实 Provider 质量；爆炸图受真实几何分组约束，图包只含 PNG/manifest |
| Agent GLB 导出 | 部分实现 | G6/G7、r3、R005 浏览器下载 smoke | Agent 抽屉已直接提供 GLB；原生 WebView 点击、全新机安装与广泛并发仍待 |
| 组件/材质目录 | 部分实现 | F004、G6、M101–M107、C101–C104 | 正式资产许可证检索、更多正式资产槽位 |
| Provider 与桌面 sidecar | 部分实现 | 本机 `local-dev-python`、F024 来源展示、A001 多轮上下文/缓存预算、A003 metadata/Keychain/supervisor/capability preflight、SSE/cancel/usage/稳定错误/no-fallback；A004 受限 Product Tool Action Loop、thinking Tool Call 续传与零永久副作用；E001/E002 no-call 评测合同与合成执行器 smoke、P002/P008 packaged Alpha 证据 | 真实 DeepSeek 人工授权评测、新机器密钥发布策略及多平台正式安装；fake/离线 Gate 不代表真实模型质量或费用 |
| 生产发布 | blocked | `release:packaging-readiness` 当前以 `SIDECAR_BINARY_INVALID` 拒绝 Intel macOS、Windows、Linux 空 sidecar | 三个剩余目标的非空可执行 sidecar、安装/升级、公证/签名、全新机恢复 |
| CAD 设计能力闭环 | 部分实现 | G819/Q003、G820–G826、A003、F025、A004；D005 四领域语义比例已绑定真实参数/readback；M108 当前 128×128 材质专属纹理、role/material、实际使用扩展、受限 bevel 和 320 mm 视觉 UV 重复已进入源码 PBR/readback Gate；G826 锁定封闭 primitive 外向绕序与非退化三角。最新四领域真实工作台捕获已验证 `ready/glb_pbr`、`committed`、xray 关闭、实时环境 recipe hash、PBR 颜色空间、520 mm 展示尺度、GPU 预算和单 context，但它仍是无评分开发审计。本轮 tracked macOS arm64 sidecar 已重建，require-ready preflight、packaged sidecar/Tauri 回归及 `.app` build 均通过，并覆盖 PBR readback、CSG、undo/redo、重启和 `provider_calls=0`；仍等待独立人工视觉基准；通过后依次为 C105 → V003 → F026 → A005 → R007 → D006 | 独立视觉达标、内部最佳候选、简洁布局、Skill、组件 Recipe、参考重建和新领域仍无完整 Gate |

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
