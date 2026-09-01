# ADR-0031: Weaponry 高质量 Three.js 刀类工作台

- Status: accepted for design and implementation
- Date: 2026-08-31
- Adds: a lightweight browser-asset route beside the Rust game-asset DCC route
- Upstream baseline: `img2threejs@9fbd0ca5bbcc3b13bebe712745d6784d33db0b85`
- Preserves: Runtime single writer, closed operations, Store/CAS lineage, explicit approval, immutable evidence

## Context

Weaponry 现有 Rust 路线已经具备刀类 Curve、AuthoringMesh、Modifier/Evaluation、High/Low、
UV/Bake 的多个结构纵切，但完整商业资产链成本高，且 Three.js 当前只在 Viewer 中读取 GLB，
不是一条可独立生成浏览器资产的生产路线。

`img2threejs` 已证明 reconstruction-by-code 的低门槛价值：把图像或文字目标拆成 component/spec，
分阶段生成 TypeScript `THREE.Group`，再用固定浏览器场景循环修正。它的 Apache-2.0 源码、
ObjectSculptSpec、生成器、阶段状态、CS2 刀类 route 和比较工具全部可作为新路线的上游基线。

但本机实测和源码审计也证明，上游默认 `blade/grip/guard/pommel/bolster` 组件树、固定截面
`ground-blade`、恒定二维截面 `curve-sweep` 和小参数坐标下降不足以稳定表达高质量 kukri：
它们可生成“像刀的对象”，不能稳定表达独立刀脊/刃口、沿长度变化的非对称截面、宽腹、
刀尖收束、磨削面和多部件连接。继续在这个表示空间中增加视觉循环，收益会快速平台化。

## First-principles corrections

### 数学不能消除可观测性

单张图到 3D 是欠定问题。公式可以在给定先验、相机和预算下找到误差更小的候选，不能证明
隐藏结构是真实结构，也不能单独证明商业审美。Weaponry 可以取消每轮无边界的主观 Agent
自评，但不能取消渲染观测。否则“视觉质量”没有可测对象。

新路线采用 deterministic-first：固定相机、silhouette、boundary、landmark、cross-section、
negative-space、Part/Material region、normal/curvature highlight 和 FPS occupancy 是主反馈；
Codex 负责提出结构假设与受限参数 delta，不自行授予商业 PASS。

### 动态目标必须可变但不可漂移

“让目标动态变化”若只依赖聊天上下文，会增加幻觉和偏移。每次目标变化必须创建不可变
`KnifeObjectiveLedger@1` successor，绑定父 ledger、program、候选、作用域、冻结部件、假设、
前后指标、预算、停止条件和证据。旧目标不覆盖，未被证据支持的目标变化不执行。

## Decision

### 1. 接受 img2threejs 为完整上游基线

Weaponry 接受 pinned upstream 的以下内容进入新路线的 adoption scope：

- ObjectSculptSpec 与 component hierarchy；
- detail inventory、pre-spec assessment 和 pass state；
- `blockout → structural → form → material → surface → lighting → interaction → optimization`；
- TypeScript `THREE.Group` factory、browser render harness 和 comparison artifacts；
- CS2 knife route、material analysis、camera matching 和 bounded correction mechanics；
- upstream scripts and fixtures as a compatibility baseline.

“完整接受”不表示把未经审计的 main 分支直接复制进 Runtime。首个实现原子必须固定上述 commit，
保留 Apache-2.0 LICENSE/NOTICE、生成 SBOM/provenance，并将 upstream 放在隔离 adapter/worker
边界。调用方不能注入任意路径、URL、shell 或 TypeScript；upstream 输出先作为派生候选进入
Runtime readback 和 CAS。

现有 Rust DCC 路线不依赖 upstream。新 Three.js 路线允许上游兼容输入和 TypeScript factory
成为一等派生资产，但不允许它们直接写 SQLite/CAS 或覆盖 `KnifeSceneProgram`。

### 2. 新建独立产品路线

新路线命名为 `Weaponry Three.js Knife Studio`，首阶段只针对非功能性游戏/展示刀类，输出：

- canonical `KnifeSceneProgram@1`；
- generated TypeScript `THREE.Group` factory；
- browser preview and fixed-view render set；
- optional preview GLB；
- metric receipt and objective ledger；
- provenance-aware package.

它不声称替代 Rust 路线的 editable Low、Hero UV、Cage Bake、UE5 导入或商业交付。

### 2.1 降低门槛后的三档完成定义

Three.js 路线不再把“穿越火线商业资产”当作第一阶段的隐含完成条件。每个任务必须先选定一档，
不得用较低档的证据冒充较高档：

| 档位 | 必需证据 | 不要求 | 可用结论 |
| --- | --- | --- | --- |
| Procedural Draft | closed Spec、合法几何、完整 Parts/Materials、固定视图可见性、相对显式 baseline 的非零结构变化、GLB 字节严格回读 | 参考图相似、人审、UE5 | `THREEJS_DESIGN_READY` |
| Reference-Similar | Draft 全部证据 + 授权参考 + 冻结相机 + 轮廓/边界/landmark 改善 | 1:1 隐藏结构、商业签收 | `REFERENCE_SIMILARITY_MEASURED` |
| Commercial | Reference-Similar + Low/UV/Bake/材质/FPS/引擎/真人批准 | 无 | `COMMERCIAL_ACCEPTED` |

用户当前降低门槛只改变目标档位，不改变证据含义。没有请求 reference likeness 时，视觉相关指标
应记为 `NOT_REQUESTED`，而不是 `PASS`；没有真人门时仍不能出现 `COMMERCIAL_ACCEPTED`。

`THREEJS_DESIGN_READY` 只由 evaluator 从真实 `KnifeSceneProgram` 字节、显式 baseline 字节、固定
八视图重放和真实 GLB payload 内部计算。调用方不能提交 compiled scene、delta、metric 或 GLB hash
来替代重算。没有同语义编译 cohort 的 baseline 时，该门保持 `BLOCKED`；“文件存在”不等于就绪。

### 3. 真值分层

| 层 | 权威对象 | 含义 |
| --- | --- | --- |
| Design truth | Brief、authorized references、`KnifeKnowledge@1` | 观察、推断和原创设计先验分离 |
| Program truth | `KnifeSceneProgram@1` | 可编辑刀类建模语言和稳定语义 ID |
| Goal truth | `KnifeObjectiveLedger@1` | 动态目标的不可变 revision 与停止条件 |
| Derived asset | TypeScript factory、`THREE.Group`、preview GLB、renders | 可删除、可重建、必须绑定 program hash |
| Delivery truth | package manifest、license/provenance、批准 receipt | 交付状态，不反向改写 program |

Runtime 仍是唯一永久写者。Three.js Compiler、browser renderer、upstream adapter 和 Codex 都不能
直接写 Store/CAS。

### 4. 刀类建模语言

`KnifeSceneProgram@1` 不只是 ObjectSculptSpec 的改名。它在兼容导入后增加：

- 独立 `spine_curve`、`cutting_edge_curve` 和可选 profile curves；
- root、shoulder、belly、tip 至少四个沿长度校准截面；
- 截面宽度、厚度、偏心、不对称、扭转、taper 和局部 profile polygon；
- blade face、cutting edge、spine、ricasso、fuller 和 transition 的表面语义；
- guard、grip、pommel、fastener、gem、relief 和 attachment graph；
- MaterialZone、程序材质层、camera/lighting/AOV、socket 和 FPS presentation；
- stable node/Part/MaterialZone IDs、dependency and source lineage；
- bounded budget and unknown/inferred regions.

Action Space 位于 typed graph 内，不通过增加数百个 MCP 工具实现。默认仍使用现有 11 façade；
新操作映射为：program prepare/refine → `authoring_transaction`，compile/evaluate →
`surface_pipeline`，render/objective/metrics → `quality_review`，package/export → `delivery`。

### 5. 确定性评价与搜索

主指标分层而非压成一个总分：

1. geometry hard gates：finite、degenerate、normal、self-intersection、Part/Material IDs、预算；
2. camera/framing：handedness、view identity、bbox、centroid、occupancy；
3. silhouette/form：IoU、Boundary F1、symmetric Chamfer、P95 distance、landmark error；
4. section/surface：厚度连续性、截面偏差、curvature/highlight continuity；
5. assembly：attachment continuity、negative-space、Part region alignment；
6. material/presentation：Material-ID coverage、roughness/metalness separation、FPS framing。

候选搜索按层执行，保留 Pareto/beam 候选；关键硬门失败不能被平均分掩盖。每轮仅修改一个
`allowed_scope`，`frozen_parts` 的 semantic and derived hashes 必须保持不变。连续两轮低于
`minimum_improvement` 或达到预算时停止，不无限循环。

上述方向、目标区间、最小改善和回归限制由 `KnifeObjectiveFunction@2` 闭合表达。它必须完整覆盖
active ledger 的 objective/regression 并区分两种 role：objective 才参与 Pareto 改善，regression
只作为硬回归门。Studio 只接受 evaluator-owned fixed-rig metric receipt；缺少参考或专用 continuity
evidence 的项目保持 `NOT_COMPUTABLE`，不得用零填充。若没有候选满足闭合目标，正确结果是
`PARENT_RETAINED`，不是强行选择“看起来最好”的候选。

2026-09-01 起，目标词表改为单一 append-only Metric Catalog。既有 12 个 metric ID 的含义、r5
ledger 和 Adapter@1 receipt 均保持不变；新的刀身/装配数学语言使用独立 ID。`KnifeIntrinsicMorphology@1`
从双曲线与截面计算 profile continuity、G1、tip taper、extrema headroom，
`KnifeAssemblyIntrinsicMetrics@1` 从 program 与 compiled scene 计算比例先验、attachment continuity、
MaterialZone readability proxy 与 complexity efficiency。只有 successor ledger 显式引用这些 ID 时，
Studio 才选择 `WeaponryThreeJsKnifeObjectiveMetricAdapter@2`；它可给出确定性的
`REVIEW_ONLY_SELECTION`，但视觉仍是 `NOT_REVIEWED/NOT_COMPUTABLE`，质量仍是 `NOT_RUN`。

## Comparative acceptance

“超越 img2threejs”必须由同一 brief、同一输入、同一相机、同一浏览器/renderer cohort 的
基线对比证明：

- upstream pinned baseline；
- Weaponry compatibility import；
- Weaponry native `KnifeSceneProgram` result。

只有结构/渲染指标优于 pinned baseline 时可标 `METRICALLY_SUPERIOR_TO_PINNED_BASELINE`。
若没有独立盲评，不得标 `ARTISTICALLY_SUPERIOR`；若没有引擎/商业验收，不得标
`COMMERCIAL_ACCEPTED`。

## Implementation atoms

1. `WPN-THREE-ADOPT-001` — pin/vendor upstream adapter, LICENSE/NOTICE/SBOM/provenance, one bounded import.
2. `WPN-THREE-SPEC-001` — `KnifeSceneProgram@1`、`KnifeObjectiveLedger@1`、canonical hash and Store record.
3. `WPN-THREE-COMPILER-001` — program → deterministic TypeScript factory / preview GLB.
4. `WPN-THREE-EVAL-001` — fixed browser views, AOVs, layered metrics and bounded refinement.
5. `WPN-THREE-DRAGONFANG-001` — full semantic knife assembly and same-input baseline comparison.
6. `WPN-THREE-QUALITY-CLOSURE-001` — close the same-input comparison, visible assembly metrics, and bounded candidate-intake gaps below.

当前实现已超过纯设计态：pinned upstream 的许可证、NOTICE、SBOM、provenance 与可恢复源码
快照已进入隔离 adoption 包；一个不执行上游代码的 bounded ObjectSculptSpec adapter 已把
`ground-blade` 数据归一化为 `KnifeSceneProgram@1`。当前确定性编译器已用独立刀脊/刃口曲线、
八组校准截面和沿长度插值生成刀身，并把 `KnifeAttachmentLoft@1`、`ReliefCurveGraph@1` 与
`LayeredSurfaceField@2` 接入同一 13-Part Dragonfang scene/GLB；分层表面通过标准 `COLOR_0` 与
glTF PBR 属性输出，禁止材质底色与顶点颜色二次相乘。
真实浏览器固定八视图已运行，并由一个 preserveDrawingBuffer 的独立捕获器产生 beauty、silhouette、
depth、normal、Part-ID、Material-ID、wireframe 共 56 张 PNG；相机矩阵、rig/program/scene fingerprint、
PNG hash 和尺寸进入闭合 manifest/receipt，状态仅为 `RENDERED_NOT_APPROVED`。pinned upstream 的
validator/generator 也已在临时隔离目录离线执行固定结构 fixture，生成 7 meshes/1,049 tris 且重复
receipt hash 一致，但 strict quality 明确 `BYPASSED_FOR_FIXTURE`。真实 FRONT Part-ID 已与授权
参考完成一次性 aspect-preserving fit，并绑定固定 browser camera/rig；candidate replay 禁止 refit。
8 控制点/8 截面的 blade-only successor 在该冻结输入下把 IoU/F1 从 `0.404717/0.094143`
提升到 `0.565626/0.181272`，五项跟踪指标均改善，但质量阈值仍 FAIL。当前 R2 GLB 为 13 Parts、
5 MaterialZones、4,598 triangles，固定 8×7 捕获完整且状态为 `RENDERED_NOT_APPROVED`。数学搜索器
已覆盖 blade 与当前 guard/pommel/relief 作用域，最多 32 个确定性候选，只能提出 `REVIEW_ONLY`
Pareto successors。严格同输入 benchmark 现已让 pinned upstream 与 Weaponry compatibility
import 共享同一 ObjectSculptSpec、normalization contract、`256×256` 八视图、七 AOV 和同一
WebGLRenderer 实例。随后闭合的 compatibility compiler 以 pinned revision/tree/generator/validator
hash 为身份，保留源 component/material 顺序、几何描述、变换与 tessellation；未知字段、primitive
漂移、非 root parent 和 source identity 漂移均 fail closed。重复回执为
`PASS_SAME_INPUT_BROWSER_AOV_CAPTURE / STRUCTURAL_PARITY`：上下游归一化 bounds 和 1,049 triangles
一致，7/7 source components 与 4/4 source materials 均被映射且 ignored/unsupported 为空。Weaponry
将源 blade 拆成 `blade-body` 与 `cutting-edge` 两个语义 target parts，因此 target 共 8 Parts；这不等于
视觉或艺术质量提升。fixture 仍没有授权参考像素目标和校准排名阈值，故 metric/visual superiority
保持 `NOT_PROVEN`。工作台的 `candidates_generate` 已不再用 segment-count 假装候选差异：它现在要求
canonical `KnifeObjectiveLedger@1`，将 program、allowed scope、frozen Parts、evidence、seed 与 2–4
candidate budget 绑定到 `KnifeKnowledgeCandidatePlan@1`。每个 native candidate 只修改一个 bounded
semantic scope，并保留 old/new、rationale、budget 与 program fingerprint；零权重补位、冻结 Part 漂移、
重复 singleton role 和没有固定视图结构变化的候选都会 fail closed。
compatibility/source-envelope 输入保持 immutable baseline，只返回 `SOURCE_REVIEW_ONLY`，不被直接改写。
每个 native candidate 都在同一固定八视图 rig 下生成 depth-resolved `KnifePartVisibilityMetrics@1`；Studio
同时记录 `KnifePartBoundaryMetrics@1`、`KnifeGuardFpsMetrics@1` 和
`KnifeCandidateStructuralDelta@1`，并把 evaluator-owned metric receipts 送入
`KnifeObjectiveFunction@2`。当前 r5 Dragonfang 三候选实际覆盖其 ledger 允许的 guard/pommel/relief
范围；13/13 Parts 可见，选中 guard-horn-sweep 的固定视图结构 delta 为 4 views / 67 silhouette pixels，
旧结构 proxy 仍可生成 review candidate，但 ObjectiveFunction@2 因必须改善的 coverage 已达 1、其余关键
目标缺专用 evidence，真实决策为 `PARENT_RETAINED`。这不是失败，而是防止把“可测”偷换成“已改善”。
R2 本身继续作为不可变的同程序 artifact 证据；不能把 R1/R2 两个由同一 program 派生的 GLB 伪装成
结构前后对比。当前已把 R2 的 exact `KnifeSceneProgram@1` 冻结为 baseline，再由 r6 ledger、固定 seed
和单范围 generator 产生 3 个候选，按“固定八视图非零结构变化最大、ordinal 最小优先”确定
`grip-taper` successor r7。r7 program/GLB 二次生成字节完全一致，readiness 的 closed program、compile、
13 Parts/5 Materials、八视图可见性、非零 delta、budget 与严格 GLB readback 全部 PASS，因此状态为
`THREEJS_DESIGN_READY`。该状态仍只表示可重放 Procedural Draft；选择是
`NON_VISUAL_STRUCTURAL_RANKING / SUCCESSOR_MATERIALIZED_REVIEW_ONLY / NOT_REVIEWED / NOT_RUN`，不表示外形
更像参考图。`WPN-THREE-STUDIO-PERSIST-004` 已完成 prepare/get/execute 的
Contract → Runtime → Store/CAS → MCP 结构纵切：closed program 成为 immutable CAS root，
真实 r7 已经 prepare→exact replay→Runtime reopen→get；固定 Worker build/export 又生成
同一 hash-bound GLB。Worker 不接受 caller 脚本、模块、URL 或路径。当前 preview 只返回
8-view manifest，`renderer_invoked=false`。Runtime 已独立验证 GLB v2 header/chunks、r185 generator、
scene/mesh/primitive 与 Part-to-mesh lineage；打包级 Worker 固定、
真实 PNG/AOV、reference comparison 与独立批准仍未完成。
因此当前状态为 `SOURCE_BASELINE_ACCEPTED / THREEJS_LANGUAGE_AND_COMPILER_PASS /
BROWSER_AOV_RENDERED_NOT_APPROVED / REFERENCE_METRICS_FAIL`，不得声称高质量、超过上游或商业完成。

## 下一原子：打包固定 Worker 与真实固定视图

当前 R2→r7 已证明可生成、可编译、可导出、固定八视图观测和 exact baseline
结构 delta；Procedural Draft 已达到 `THREEJS_DESIGN_READY`。Contract/MCP 与真实 r7 持久化已闭合，
fixed-worker build/preview/export 也已接入，GLB 独立 inspector 已通过。下一步不再增加 Studio envelope，
而是将 Worker 做成打包资源，并让 preview 真正生成 PNG/AOV：

1. **无损全组件导入已闭合，质量提升未闭合。** `same-input-benchmark.receipt.json` 已经关闭同一
   input/normalization/rig/resolution/renderer 的比较基础，并证明 7/7 source components、4/4 source
   materials、1,049 triangles 和 normalized bounds 的结构等价；`ignored_component_ids` 与
   `unsupported_component_ids` 均为空。该结果只能标 `STRUCTURAL_PARITY`，不能自动升级为
   `METRICALLY_SUPERIOR`。下一步必须由 Weaponry 刀类知识生成真实不同的 native candidate，再在同一
   fixed rig 下比较，而不是把 compatibility parity 当作质量改进。
2. **结构指标与目标函数已闭合到非视觉层。** CPU fixed-rig evaluator 已产生 Part/Material coverage、
   Part-ID boundary、guard visible-opening proxy、FPS occupancy 和 candidate-to-baseline mask delta；
   `KnifeObjectiveFunction@2` 已接入 Studio，并从 evaluator-owned receipts 做 direction-aware Pareto。
   当前 r5 的 reference、landmark、thickness 和 normal continuity 项缺证据，因此保持
   `NOT_COMPUTABLE`；真实运行返回 `PARENT_RETAINED`，没有用旧 proxy 强行冒充目标接受。
3. **Procedural Draft readiness 已闭合。** readiness evaluator 从 program/GLB bytes 内部重算 canonical
   identity、compile、固定八视图、预算、GLB v2 header/JSON/mesh-node/material/triangle readback，并拒绝
   caller-supplied receipt。r7 successor 的 13 Parts/5 Materials、4598 triangles、901,372-byte GLB 和
   7-view / 323 changed-pixel structural delta 均由 exact baseline/candidate bytes 重放为 PASS；二次生成的
   program、GLB、lineage 与 readiness 文件 SHA-256 全部一致。
4. **Durable 产品接线仍未开始。** 当前 Studio 是 bounded in-process workbench；Runtime/Store/CAS/MCP
   尚未持久化 objective function、candidate receipts、readiness 和 parent-retained decision。该缺口不
   阻止本地 Procedural Draft 设计实验，但阻止把它称为 Weaponry 永久项目真值。

### 稳定高质量成功定义

`WPN-THREE-QUALITY-CLOSURE-001` 只有在一个可重放 receipt 同时绑定下列条件时才算完成：

- 若目标包含 comparative superiority，upstream 与 native 必须由同一 closed fixture/input hash 驱动，
  并共享固定 camera/rig、分辨率和 renderer cohort；compatibility 路径还必须保留全部输入
  component/material IDs，不得存在 ignored component；同输入逐视图指标可重算，才允许该标签；原创/非比较
  目标仍须绑定自己的固定观测与指标 receipt，但不强制虚构 upstream 对照；
- full-assembly 8×7 AOV 有逐视图/逐附件 occupancy、Part-ID boundary/continuity、negative-space 和
  FPS 指标；未提供参考时只报告可计算的结构/可见性结果，reference likeness 保持 `NOT_COMPUTABLE`；
- 一个 `REVIEW_ONLY` 候选可以沿 proposal、program、factory、AOV、metrics、parent ledger 和 frozen
  hashes 完整自动接入并重放，且不跨越 human/commercial approval 边界。

在此之前状态保持 `BROWSER_AOV_RENDERED_NOT_APPROVED` 或相应的 `NOT_COMPUTED`/`NOT_RUN`，不得标记
`METRICALLY_SUPERIOR_TO_PINNED_BASELINE`、`HUMAN_ACCEPTED` 或 `COMMERCIAL_ACCEPTED`。

## Architecture

高层运行图由 Archify 生成：

- `docs/architecture/weaponry-threejs-quality-workbench.archify.json`
- `docs/architecture/weaponry-threejs-quality-workbench.html`
