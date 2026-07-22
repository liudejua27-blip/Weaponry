# ForgeCAD 当前 Agent API

版本：2026-07-18
状态：`/api/v1/agent` compatibility 产品合同 + K001–K003 Rust 协议、Agent/Provider/Tool 与产品状态合同

本文只描述通用机械概念 Agent 的 `/api/v1/agent` 集成边界。旧 Weapon、Patch、ComfyUI、神经 3D 和 Unity 路由仍可能由兼容启动链挂载，完整生成 OpenAPI 快照包含两代路由；它们的说明位于 [legacy API 说明](legacy/API_WEAPON_COMPATIBILITY.md)，不得作为新工作台集成入口。

完整机器合同由生成的 OpenAPI 和 Schema 文件负责：

- `packages/weapon-spec/generated/openapi.json`：当前 compatibility OpenAPI 快照；基础来自测试隔离的 FastAPI schema，`scripts/export_openapi.py` 再以显式、代码所有的 Rust-native overlay 加入不由 Python 挂载的产品路由。overlay 检测到 FastAPI 同路径或同组件时会失败，防止重新形成双 owner；
- `packages/concept-spec/schemas/`：Agent、装配、ShapeProgram、材质与资产合同；
- `packages/concept-spec/fixtures/forgecad-app-server-protocol-manifest.json`：K001 Rust app-server 线协议、能力、限制与 compatibility/resource 边界；其中 Python owner 字段是历史迁移快照，不代表 K003 当前 runtime ownership；
- `packages/concept-spec/fixtures/k002-native-agent-protocol.json` 与 `k002-product-tool-registry.json`：K002 native lifecycle/Provider/Product Tool 方法、DTO、通知和 13 项工具清单；
- `apps/desktop/src/shared/generated/api-types.ts`：前端生成类型。

## 1. 服务边界

Python compatibility service/packaged sidecar 的内部默认本机地址：

```text
http://127.0.0.1:8000
```

健康检查：

```text
GET /api/health
```

packaged 桌面前端只连接 Rust app-server bridge；浏览器开发壳只连接同一协议的本机 loopback compatibility adapter。K003 完成后，Rust app-server/core 是 Thread/Turn/Item/Approval/Provider/Product Tool 生命周期和 Project/Version/Snapshot/ChangeSet/Quality/Export/SQLite/CAS/对象库的公开且唯一所有者；Python 仅保留 capability-gated `RestrictedGeometryExecutor`。前端不直接向外部大模型发送 API Key，也不执行任意 Python、JavaScript 或 shell。武器题材结果仅限虚构游戏美术资产和非功能展示模型；API 不输出制造图、制造尺寸、材料配方、加工流程或现实功能机构。

### 1.1 K001 app-server 协议与传输

K001 冻结 `forgecad.app-server/1` 和 JSON-RPC 2.0 线协议。连接必须先完成 `initialize`/`initialized`；当前 request methods 为 `compat/http`、`compat/subscribe`、`compat/unsubscribe`、`thread/events/replay`，client notifications 为 `initialized`、`notification/ack`、`request/cancel`，server notifications 为 `compat/sse`、`stream/resyncRequired`。稳定 request/notification/cursor ID、有界 frame/in-flight/event queue、取消传播、通知确认和重连 replay 都属于 Rust 协议/transport 所有权。

`compat/http` 只接受 `ForgeCADHttpCompatibilityRequest@1`，响应为 `ForgeCADHttpCompatibilityResponse@1`，body 编码固定为 `empty | utf8 | base64`。它不是任意 HTTP 代理：Rust 和 Python oracle 共用代码所有的“HTTP method + 精确 segment shape”白名单，当前只允许明确列出的 `GET | POST | PUT | PATCH` 产品路由；任意 origin/URL、递归 `/api/v1/app-server`、路径穿越、未列出的 method/segment、Authorization/Cookie/Provider Key 等敏感 header 均在转发前拒绝。浏览器开发壳只可直接访问 `/api/v1/app-server/connections...` compatibility frames，不能绕过协议直连产品路由。

packaged WebView 的二进制/媒体子通道为 `ForgeCADReadOnlyResourceCompatibility@1`：`forgecad-resource://localhost/...` 固定 GET-only、无状态、无持久写权限，复用 `compat/http` 的 path/header 策略，只用于图片、GLB、媒体和用户触发的下载响应。它不能创建 Thread、版本、Snapshot、ChangeSet、质量或导出记录，也不是第二个状态源。

K001 只完成协议和传输；K002 迁移 Agent 生命周期；K003 已完成产品状态和持久化所有权切换。旧 Python `POST` Thread/Turn/Approval/Provider 生命周期及产品写路由默认稳定返回 410，不再是生产写入者；只有历史 packaged oracle 的显式 test-only 环境才能暂时重开对应旧路径。当前 runtime 的 initialize `migration_state.state_owner` 与数据库 ownership marker 必须为 Rust owner。

### 1.2 K002 Rust Agent 生命周期

K002 已冻结 native request methods：`thread/create|list|read|archive`、`turn/start|read|cancel`、`item/list|read`、`approval/create|read|resolve`、`provider/preflight|check|cancel` 与 `product-tools/list|execute`；对应通知为 `thread/created|updated|archived`、`turn/started|completed|failed|cancelled`、`item/started|completed` 和 `approval/created|resolved`。Rust app-server 单一拥有 Context Builder、DeepSeek SSE/thinking Tool Call、13 项代码所有 Product Tool、approval policy、12 次调用/时间/token/费用预算、取消树、usage/cache 与脱敏 trace。

Python 只接收带每进程 capability token 的 restricted geometry 请求；sidecar 环境不会继承任意 Provider credential，Python 请求不含 Provider Key、原始 `reasoning_content`、会话决策、数据库/对象库路径或 Snapshot 写令牌。SQLite 和产品状态由 Rust core 单写；native lifecycle cursor 仍不能当成 Snapshot revision。

## 2. 通用请求规则

除文档明确列出的 `GET /active-design` 兼容引导初始化外，产生持久化副作用的请求必须提供：

```http
Idempotency-Key: <稳定且唯一的请求键>
Content-Type: application/json
```

重复使用同一 Idempotency-Key 和相同请求应返回同一结果；同一键对应不同请求时返回 `409 IDEMPOTENCY_CONFLICT`。K003 的 `GET /active-design` 是纯读取：已持久化 Snapshot 直接返回，没有 Snapshot 的有效 legacy current version 则派生稳定的 `legacy_concept_read_only` 响应，不插入数据库行。其响应固定带 `Cache-Control: no-store`；空项目仍返回 `404 ACTIVE_DESIGN_NOT_FOUND`，不会创建空 Snapshot。

桌面浏览器开发壳跨源调用时，CORS 只允许已配置的本机/Tauri origin，并显式允许 `If-Match`、暴露 `ETag`；前端必须读取服务端实际返回的 ETag，不能自行猜测 revision。

统一错误形状：

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "可读错误",
    "recoverable": false,
    "details": {}
  }
}
```

## 3. Agent Kernel

K003 后，生产桌面统一使用 Rust native `thread/list`、`thread/read`、`item/list`、`item/read` 和 `thread/events/replay` 读取生命周期真值。下表的旧 HTTP Thread/Turn/Approval 路径只记录兼容形状，生产默认全部返回上述 `410`；只有同时显式启用 `FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE=1` 与 K001 packaged probe 的历史 oracle 才可临时恢复旧 Python 读写，用于协议回归，不能作为产品集成入口。

| 方法与路径 | 当前用途 |
| --- | --- |
| `POST /api/v1/agent/threads` | Rust 拥有；Python 生产路径返回 `410` |
| `GET /api/v1/agent/threads` | Rust native `thread/list` 拥有；Python 生产路径返回 `410` |
| `GET /api/v1/agent/threads/{thread_id}` | Rust native `thread/read`/`item/list`/`approval/read` 拥有；Python 生产路径返回 `410` |
| `POST /api/v1/agent/threads/{thread_id}/turns` | Rust 拥有；Python 生产路径返回 `410` |
| `POST /api/v1/agent/turns/{turn_id}/cancel` | Rust 拥有；Python 生产路径返回 `410` |
| `POST /api/v1/agent/threads/{thread_id}/approvals` | Rust 拥有；Python 生产路径返回 `410` |
| `POST /api/v1/agent/approvals/{approval_id}/resolve` | Rust 拥有；Python 生产路径返回 `410` |
| `GET /api/v1/agent/threads/{thread_id}/events` | Rust native notification 与 `thread/events/replay` 拥有；Python 生产路径返回 `410` |

历史 test-only SSE oracle 支持查询参数 `after` 和请求头 `Last-Event-ID`；生产桌面不调用该 HTTP SSE 路径，而通过 Rust app-server cursor、notification ack 和 `thread/events/replay` 恢复结构化 Item。当前 Rust 实现回放 `ProviderExecutionTrace@1` 对应的 preflight/request_started/streaming/validating/completed/failed/cancelled Item；Provider token delta 只在服务端组装，不作为用户可见原始内容转发。

### 3.1 多轮 Provider 上下文

Native `turn/start` 使用版本化请求体和稳定 business idempotency key。Rust app-server 在本地范围预检通过后，按固定顺序编译 Provider 上下文：版本化安全/输出合同、已绑定领域包、可丢弃的 ThreadMemorySummary、最近四组完整用户/助手消息，以及当前 `ActiveDesignSnapshot` 的只读摘要和新请求。Project、Version、Selection、Quality、Export 的真值不会转移到 ThreadMemory。

同一 Thread 已生成设计方向后，后续未重复写出“汽车/飞机/机械臂”等类别的普通语言微调会使用该已持久化领域绑定；明确要求切换到另一领域返回 `THREAD_DOMAIN_CHANGE_REQUIRES_NEW_THREAD`。每个 Thread 同时只允许一个 `queued/running` Provider Turn，返回 `THREAD_TURN_IN_PROGRESS` 时应等待或读取现有 Turn，不应创建并发请求。

Turn 的 `usage` 可返回 `latency_ms`、`prompt_tokens`、`completion_tokens`、`total_tokens`、`prompt_cache_hit_tokens`、`prompt_cache_miss_tokens`、`estimated_cost_cny`、`budget_reservation_cny`、`routing_mode`、`context_hash`、`prompt_contract_version`、`network_call_made`、`provider_phase`、`provider_attempt` 和 `fallback_used=false`。它不返回 Key、Base URL、请求头、完整 Prompt、完整 Response 或 `reasoning_content`。K002 只接受 13 项代码所有的 ForgeCAD Product Tool Calls；动态工具、重复 call ID、Schema/G819 非法参数、越权审批、shell/URL/路径与超预算请求都稳定拒绝。

## 4. 领域、材质和 Provider

| 方法与路径 | 当前用途 |
| --- | --- |
| `GET /api/v1/agent/domain-packs` | 返回四个最小领域包 manifest |
| `GET /api/v1/agent/materials` | 返回 13 个六类视觉材质预设及 `MaterialPreset@1` PBR/来源元数据；纹理字段只返回内部 asset ID，并附对象存在性/来源摘要 |
| `POST /api/v1/agent/material-textures` | 在显式请求下登记 PNG/JPEG/WebP 视觉纹理对象；仅接受原始 base64，不接受 URL、data URI 或文件路径；必须带 `Idempotency-Key` |
| `GET /api/v1/agent/material-textures` | 按用途、来源或关键词检索已登记的纹理对象；返回相对内容寻址路径和对象存在性，不返回绝对路径 |
| `GET /api/v1/agent/material-textures/{texture_asset_id}` | 读取单个 `MaterialTextureObject@1` 元数据和对象哈希/存在性 |
| `GET /api/v1/agent/provider` | Python compatibility HTTP 生产默认返回 `410`；桌面使用 native `provider/preflight` |
| `GET /api/v1/agent/product-tools` | Python 只读兼容目录；桌面使用 native `product-tools/list`，执行只允许 `product-tools/execute` |
| `POST /api/v1/agent/provider:check` | Rust 拥有；Python 生产路径返回 `410` |
| `POST /api/v1/agent/provider-checks/{check_id}/cancel` | Rust 拥有；Python 生产路径返回 `410` |

Provider 配置由 Tauri supervisor 注入本地 Agent。API Key 不属于 Project、Thread、Item、SQLite、导出或错误响应。

DeepSeek 适配器使用 SSE，并在请求中启用最终 usage 块。当前 `deepseek-v4-flash` / `deepseek-v4-pro` Agent 请求显式发送 `thinking.type=enabled` 与 `reasoning_effort=max`，不发送 thinking 模式下无效的 temperature/top-p/penalty 参数，也不使用 `/beta` strict Tool Call。A004 的 thinking Tool Call 会把同一子轮 assistant 的 `content + reasoning_content + tool_calls` 原样放回下一请求；缺少必需 `reasoning_content` 会在下一次网络请求前 fail closed。跨 Turn 的持久化上下文只含用户消息和最终 assistant 内容，不保存历史 Tool Call transcript，因此隐藏推理仍不进入 Agent Item、SQLite、资产、日志或用户可见摘要。Action Loop 最多 12 次调用、单次并发，只能使用 13 个已注册 ForgeCAD 产品工具；重复 call ID、stale Snapshot、Schema/G819 拒绝、超时或取消都会停止候选流程。固定 Provider 错误为 `DEEPSEEK_INVALID_REQUEST`（400）、`DEEPSEEK_AUTH_FAILED`（401/403）、`DEEPSEEK_BALANCE_EXHAUSTED`（402）、`DEEPSEEK_INVALID_PARAMETERS`（422）、`DEEPSEEK_RATE_LIMITED`（429）、`DEEPSEEK_SERVER_ERROR`（500）、`DEEPSEEK_SERVER_BUSY`（503），另有 `PROVIDER_NETWORK_ERROR`、`PROVIDER_TIMEOUT`、`PROVIDER_EMPTY_CONTENT`、`PROVIDER_INVALID_JSON`、`PROVIDER_SCHEMA_MISMATCH` 和 `PROVIDER_CANCELLED`。这些失败不自动重试，也不回退为 deterministic success。

对于 `api.deepseek.com`，本机默认日预算为 20 元。普通设计 Turn 按最多 12 个 Action Loop 子请求做保守额度预留，Loop 同时执行总 token 上限；成功时用所有子请求的 DeepSeek usage 汇总缓存命中、缓存未命中与输出 token 结算。Provider 的上下文硬盘缓存默认开启，Rust HTTP 请求不再发送 `Cache-Control: no-store`；ForgeCAD 只采用 Provider 明确返回的 `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`，缺失时记为 0 而不是由总输入 token 推算。显式 `provider:check` 只执行一次结构化探测，不进入 Action Loop，但仍可能产生一次 Provider 费用。usage 缺失会标为 `unavailable` 并阻止当日后续联网请求。该预算不阻止本地离线规划、查看、编辑、检查或导出。

## 5. 方向、blockout 和分件

| 方法与路径 | 当前用途 |
| --- | --- |
| `POST /api/v1/agent/threads/{thread_id}/turns` | 在一个 Turn 内生成结构化 legacy 计划，并通过受限工具完成一个未保存候选的 build、真实 GLB readback、四视图、硬门和 preview；F026 兼容层只能读取第一条文本方向并展示一个临时 3D 结果，直到 V003 替换该合同 |
| `POST /api/v1/agent/blockouts` | 根据方向生成 ShapeProgram、AssemblyGraph 和 GLB |
| `POST /api/v1/agent/blockouts:concept-preview` | 为未保存方向生成同源、低分辨率的软件概念 PNG，不写入任何记录 |
| `POST /api/v1/agent/blockouts:segment` | 生成按领域角色组织的分件候选 |
| `POST /api/v1/agent/blockouts:commit` | 把候选保存为 `AgentAssetVersion` |

`blockouts:commit` 请求至少带 `artifact_id`，并可带当前已打开的 `project_id`。服务端会校验候选项目与该 Project 一致；对于旧会话留下的未绑定候选，只有显式传入且有效的当前 Project 才允许补绑定，跨项目或无效项目一律拒绝。

当前 Geometry Worker 在受控 JSON 路径执行 `box`、`cylinder`、`capsule`、`wedge`、`profile`、`extrude`、`revolve`、`loft`、`sweep`、`mirror`、`array`、`radial_array`、受限 `union`/`subtract`，以及受控 `bevel_approx`/`surface_panel`；方向生成、几何构建和分件是三个显式步骤；确认 blockout 前不得写入 Agent 资产版本。`profile` 只能作为 `extrude` 或 `revolve` 的前置操作，`loft`/`sweep` 直接引用 canonical profile input；复制、布尔、倒角和面板操作只能引用前置几何操作。

G825 的 `union`/`subtract` 只由 `manifold3d==3.5.2` 生产 handler 执行，不再静默回退旧 box 算法。输入必须封闭并满足深度、输入数与三角预算；内核运行于可取消/超时的隔离进程，不接收数据库、对象库、Snapshot 或文件路径。成功 GLB 的 `GeometryCompileReadback@2.feature_history` 回读每个有序节点的输入/结果 hash、内核版本和 surface/material provenance；失败返回稳定 CSG 错误及 node ID，不产生部分 GLB、版本或 Snapshot 提升。

G826 在同一 GLB 中为每个 primitive 写出 `TANGENT`、`_FORGECAD_FACE_ID`、`_FORGECAD_SOURCE_FACE_ID` 和稳定的 part-instance/Material Zone extras。readback 必须验证 normal/tangent 单位长度与正交性、tangent handedness、UV0 非退化、face ID 完整唯一、zone 非空不重叠，以及 CSG/mirror/array 后的来源；任何损坏都拒绝当前编译/导出。`material_zone_faces` 是 M108A 绑定纹理的唯一几何面事实，不允许客户端按颜色或朝向重建。edge finish 仍是有界 `bevel_approximation`，不是精确 CAD 圆角。

G820 已冻结 `ProfileSketch@1` 与 `ProfileSectionSet@1` 的机器合同，并允许 ShapeProgram 通过可选 `profile_inputs` 保存规范 payload、合同版本和 SHA-256 provenance。该合同在进入 Worker 前拒绝自由 SVG、非规范坐标、错误闭合/绕序、孔洞、自交、退化、超预算和无序截面；当前 HTTP 请求没有新增轮廓编辑入口，旧 `profile.args.points` 执行路径保持不变。

G821 已让 Worker 的现有 `profile → extrude/revolve` 分支消费 `profile_inputs`。Extrude 可生成带孔封闭壳或显式无 cap 的开放 ribbon；Revolve 可处理轴点、完整/部分角度和部分角 seam cap。结果 GLB 的 UV0、normal、闭合/边界、退化面和 surface triangle ranges 会被重新回读并进入 `GeometryCompileReadback@1`。该能力目前只在 ShapeProgram/Agent 后端合同开放，工作台没有自由 SVG 编辑器，普通 Planner 也尚未自动生成这些新节点。

G822 已让 Worker 的 `loft` 分支直接消费 canonical `ProfileSectionSet@1`，按声明主轴、严格 section position、统一 sample seam、section scale/twist 和首尾 cap 生成受限线性放样。运行边界会拒绝孔洞 Loft、混合采样数、相邻大角度翻转、明显中间截面自交、退化/越界和预算超限；GLB readback 返回 `loft_side/seam/start_cap/end_cap`、UV0、normal 与拓扑事实。该接口仍是后端 ShapeProgram 能力，Planner/工作台不会自动产生 Loft，Sweep 仍拒绝。

G823 新增的 `sweep` 直接消费 canonical closed/hole-free ProfileSketch 与 2–32 点有界 3D path。Worker 使用确定性 parallel-transport frame，支持开放路径有限 twist、开/闭路径和显式 cap；拒绝零长度/过短段、接近 180° 翻转、明显自交、闭合 cap/twist 与点数/bounds/triangle 超限。GLB readback 返回 `sweep_side/seam/start_cap/end_cap`、UV0、normal 与拓扑事实。Planner/工作台尚不自动产生 Sweep，该能力不提供真实管径、流体、电气、承压或结构结论。

`POST /api/v1/agent/blockouts:concept-preview` 是 R006 的纯计算路径。它接收当前内存中的 `MechanicalConceptPlan@1`、`direction_id` 和受限的 `variation_index`，在本机重新构建同一 ShapeProgram/GLB，并只返回固定 `320×240` 的透明背景 iso PNG、实际 `variant_id`、`topology_hash` 与 `render_context_sha256`。它不接受客户端 GLB、文件路径、下载选项或工程参数；不要求或创建幂等记录，也绝不写入候选、`AgentAssetVersion`、`ActiveDesignSnapshot`、质量、导出或 Thread/Turn。桌面端必须以 project + plan + direction 的临时 request context 拒绝迟到响应；选择方向、换一版外观、取消或切换项目后应丢弃图片。该 PNG 只是软件概念图，不是照片级渲染、工程图、装配图或制造资料。

G807 已在 worker 内部注册 `BLOCKOUT_VARIANT_IDS`：四个领域各 12 个、共 48 个结构不同的确定性变体，并由 `agent:g807-blockout-diversity-smoke` 校验 GLB/readback、三角预算、装配连通性和重复生成一致。G812 让 `POST /blockouts` 与 `POST /blockouts:segment` 接受可选 `variant_id` 并返回实际使用的 ID；G813 再为两个请求和响应增加受限 `variation_index`（仅 `0..2`，缺失或旧候选安全默认为 `0`）。G815 在普通方向生成时将本机识别的有限轮廓、细节、色彩和展示姿态写为 `VisualIntentMapping@1`，再映射到当前 Domain Pack 的既有 0–3 视觉族；没有匹配或映射损坏时安全回退到 G812 的轮廓选择。它只选择预审族，不能产生尺寸、ShapeProgram 操作、任意网格、工程/制造参数或新的 API 端点。G817 为 build/segment 请求和响应固定 `presentation_profile` 为 `quick_sketch|showcase`（旧请求安全默认 `quick_sketch`）；G818 将展示档的固定外观层扩展为 `visual_panel_*`、`visual_groove_*`、`visual_guard_*`、`visual_light_strip_*`、`visual_cable_slot_*`、`visual_vent_*` 与 `visual_fastener_*`。这些部件和有限 PBR 材质索引必须贯穿 GLB、ShapeProgram、AssemblyGraph、候选 JSON 和确认链；它们不表达真实的槽、开孔、冷却、电气、固定或工程材料。未提供 exact ID 时，服务端只根据当前 Pack、受限意图/方向和 index 在相近的三项预审视觉变体中稳定选择一个；提供 exact ID 时该 ID 控制几何，index 只作为预览来源回传。工作台只保留三张普通语言方向卡、“换一版外观”以及两档外观质量选择，不公开视觉族索引、48 项技术目录、尺寸、ID 或自由参数；它把 build 的实际 ID、index 和 profile 原样传入 segment。变体与档位只控制非功能概念外观；轮换或切换档位只替换未保存 preview，不写版本、Snapshot、ChangeSet、质量或导出，confirm 仍经既有 commit → ActiveDesignSnapshot 路径完成；跨包 ID、越界 index、未知 profile 或同幂等键改变视觉选择必须拒绝。

`POST /api/v1/agent/threads/{thread_id}/turns` 先执行 `DomainInferenceResult@1`，再执行本地 `ConceptScopeDecision@1`；两者都在 Planner 或 Provider 之前。唯一且范围允许的 Pack 才调用 Planner。普通含糊/未知领域会返回一个 `waiting_for_clarification` Turn，其中包含一个 `clarification` Item、一个普通语言问题和候选选项；用户选择后以保留原始创意的新 Turn 继续。

范围明确不支持时，Turn 直接以 `completed` 返回，唯一的 `clarification` Item 带 `kind=scope`、`status=unsupported`、空选项和用户可读的改写提示。该分支不会调用 Provider/Planner，也不会创建 Plan、blockout、AgentAssetVersion、Snapshot、质量或导出；持久化范围只限 Thread、Turn、Item 与幂等记录。工作台因此不会显示方向卡。当前没有单独的领域推断或范围预检 HTTP 端点。

用户选择候选后，后续 Turn 可携带 `clarification_domain_pack_id`（只能是四个已注册领域包之一）。这是用户明确选择的领域绑定，不是默认武器回退，也不能绕过范围预检；原始 Brief 仍作为 `message` 保留在新 Turn 中。`ConceptScopeDecision@1` 是一组可审查的产品边界规则，不是完整内容安全系统；运行时仍依赖受限 ShapeProgram、工具权限和确认边界。

## 6. Agent 资产版本

| 方法与路径 | 当前用途 |
| --- | --- |
| `GET /api/v1/agent/asset-versions/{asset_version_id}` | 读取不可变 Agent 资产版本 |
| `GET /api/v1/agent/asset-versions/{asset_version_id}:preview.glb` | 返回同一 ShapeProgram 的 `interactive_preview` 二进制 GLB，用于即时工作台反馈 |
| `GET /api/v1/agent/asset-versions/{asset_version_id}:model.glb` | 返回按需生成/缓存的 `production_concept` 二进制 GLB，用于质量审阅和正式下载 |
| `GET /api/v1/agent/asset-versions/{asset_version_id}/parts/{part_id}/semantic-proportions` | 读取当前活动资产/部件可用的四领域外观比例配方；只读，不创建版本 |
| `GET /api/v1/agent/asset-versions/{asset_version_id}/structure-suggestions` | 读取当前资产的拆分/合并候选（只读） |
| `POST /api/v1/agent/skills/surface-adornment:enable` | 显式评测并启用内置 visual-only 表面细节 Skill；必须带确认，不会自动启用 |
| `POST /api/v1/agent/asset-versions/{asset_version_id}/surface-adornments:preview` | 为当前 Part/Material Zone 提出受限 `SurfaceAdornmentProgram@1` ChangeSet；不直接创建版本 |
| `POST /api/v1/agent/reference-evidence:create` | 将用户授权的 PNG/JPEG/WebP、直接自包含 GLB 或同项目已导入 GLB 记录为 Rust-owned 只读 `ReferenceEvidence@1`；不创建/推进资产版本或 Snapshot |
| `POST /api/v1/agent/projects/{project_id}/reference-guided-rebuild:preview` | 以当前可编辑 ForgeCAD 资产为必需 base，从同项目/同领域参考证据提出一个 C105 Recipe-backed 标准 ChangeSet；初始空项目合成归 V003 |
| `POST /api/v1/agent/asset-versions/{asset_version_id}/change-sets` | 提出部件修改 |
| `POST /api/v1/agent/change-sets/{change_set_id}:preview` | 标记预览 |
| `GET /api/v1/agent/change-sets/{change_set_id}:preview.glb` | 临时编译当前预览的真实 PBR GLB；二进制不落库 |
| `POST /api/v1/agent/change-sets/{change_set_id}:confirm` | 确认并创建子版本 |
| `POST /api/v1/agent/change-sets/{change_set_id}:reject` | 放弃修改 |

当前 ChangeSet 支持受限比例、位置、关节姿态、视觉材质、组件替换、声明式 Connector 对齐，以及 `split_part`/`merge_parts`。`set_part_parameter` 对带有非空 `editable_parameter_bindings` 的 Part 只接受该 Part 声明的既有 position/scale 路径，并按声明的范围和步长校验；G810 使四领域新生成 blockout 的单一 box/wedge 输出默认携带三条有界比例声明。重复角色及当前圆柱/胶囊适配器不产生误导性的单部件声明。历史 AgentAssetVersion 缺少该字段时，明确保留原有六条 position/scale 路径及全局概念边界，绝不代表任意参数开放。锁定检查先于参数声明检查。后两者只能引用当前 `structure-suggestions` 返回的稳定建议 ID：服务端在提出和预览时都会根据 AssemblyGraph、稳定 role、受限 ShapeProgram 输出和连接事实重新验证；没有足够事实、外部 GLB、锁定/关节或过期建议都会拒绝。视觉材质操作必须指向属于目标 Part 的稳定 `material_zone_id`，材质本身还必须允许当前资产的真实 Domain Pack；确认才创建不可变子版本。

A005 的 `surface-adornment:enable` 只处理 ForgeCAD 内置、封闭工具交集的 Skill manifest，并要求显式 `confirm=true`；dry-run/eval/activation 以及重启恢复由 Rust Core 持有。`surface-adornments:preview` 只接受当前项目活动资产中的 Part、属于该 Part 的稳定 Material Zone、受限 kind/motif/intensity/coverage 和当前基础材质。成功响应返回可继续走标准 `ChangeSet :preview → preview.glb → :confirm|:reject` 的提案；真正的纹理程序及其 canonical SHA-256、Skill ID/version/hash、Part/Zone 和基础材质会进入不可变 AssemblyGraph、动态材质和 GLB/readback provenance。Python 只执行 capability-gated 的确定性五通道 PBR 烘焙，不接收 Provider Key、数据库、CAS 路径或 Snapshot 写权限。该路径不表示实体雕刻、加工深度、工程表面或任意外部纹理执行。

R007A 的证据入口必须带 `Idempotency-Key`、完整来源和权利/许可声明、缺失视角与有界用户备注。图片检查声明媒体的 magic；GLB 必须通过现有严格自包含 glTF 2.0 inspection。证据字节进入 CAS 或引用同项目已密封导入，不会作为可执行 ShapeProgram。`reference-guided-rebuild:preview` 必须显式绑定当前 head/Snapshot 中的可编辑 `base_asset_version_id`；无 base 时以 `REFERENCE_REBUILD_BASE_REQUIRED` 失败并保持证据只读。它只提出 ChangeSet；后续必须继续使用同一原生 `:preview → preview.glb → :confirm|:reject` 生命周期。当前 R007A 证明可追溯、受限可编辑闭环，不声称已从像素恢复精确轮廓或达到参考视觉保真度；后者属于 C106/R007B/M108B。

`:preview.glb` 只接受状态为 `previewed`、基础版本仍是当前 head、且 `ActiveDesignSnapshot.active_design/preview` 与该 ChangeSet 完全一致的请求。服务端在编译前后重复验证，并返回 `model/gltf-binary`、`Cache-Control: no-store`、`X-ForgeCAD-Preview-GLB-SHA256`、`X-ForgeCAD-Base-Asset-Version-ID` 和 `X-ForgeCAD-Preview-Triangle-Count`。GLB 不写入 ChangeSet、事件、幂等记录或 SQLite；客户端必须用同一显示 request token 拒绝迟到结果。它不支持任意几何脚本、自由 split/merge、精确碰撞或动力学。

资产级 `:preview.glb` 与 `:model.glb` 都来自同一不可变 ShapeProgram，不创建第二条版本链。响应以 `ETag`、`X-ForgeCAD-Artifact-Profile`、`X-ForgeCAD-Artifact-Profile-SHA256`、`X-ForgeCAD-Shape-Program-SHA256`、`X-ForgeCAD-GLB-SHA256`、`X-ForgeCAD-GLB-Byte-Size` 和 `X-ForgeCAD-Triangle-Count` 绑定真实工件；production 还可带下载文件名。`production_concept` 使用内容寻址派生缓存，索引不含 GLB/base64，损坏对象或身份不一致返回明确错误，不静默重编译并掩盖损坏。

D005 的语义比例端点先将四领域 Recipe 的稳定语义部件槽解析到当前 AssemblyGraph Part，再要求该 Part 具有 `ratio` scale binding，并要求当次 `GeometryCompileReadback@1.surface_provenance` 回读到匹配 role、zone、`texture_ready` 与非空 `source_operation_ids`。返回值包含 ShapeProgram/GLB hash、当前/目标值、上下界和步长。锁定、外部 GLB、无绑定或无回读事实不产生可执行选项；客户端只能把返回的 path/value 交给同一 preview→ChangeSet→confirm 路径。历史空声明兼容不能扩大 D005。

## 7. 组件、质量和导出

| 方法与路径 | 当前用途 |
| --- | --- |
| `POST /api/v1/agent/asset-versions/{asset_version_id}/components` | 保存项目内可复用组件 |
| `GET /api/v1/agent/components` | 按项目、领域、角色和关键词查询组件 |
| `GET /api/v1/agent/asset-versions/{asset_version_id}/components:compatible?part_id=...` | 返回当前目标部件的项目内候选及可解释替换结论 |
| `POST /api/v1/agent/asset-versions/{asset_version_id}/parts/{part_id}/component-recipes:expand` | Rust core 将代码所有、已审阅 Recipe 展开为只读 `ComponentRecipeCandidate@1`；零产品写入，不调用 Python Product API 或 Provider |
| `POST /api/v1/agent/asset-versions/{asset_version_id}:quality` | 以 Snapshot CAS 运行 Agent 资产轻量质量检查；必须带 `Idempotency-Key` 与当前 Snapshot `If-Match` |
| `GET /api/v1/agent/quality-reports/{quality_report_id}` | 读取已持久化的 Agent 质量报告 |
| `POST /api/v1/agent/asset-versions/{asset_version_id}:export` | 兼容导出 `AgentAssetExport@2`；内部 Agent 资产固定使用 `production_concept` GLB/readback |
| `GET /api/v1/agent/asset-versions/{asset_version_id}:render` | 生成当前 Agent 资产的四视图 PNG 与条件式爆炸概念 PNG 派生结果 |
| `GET /api/v1/agent/asset-versions/{asset_version_id}:render-package` | 下载与当前预览 fingerprint 相同的概念 PNG/manifest ZIP |

质量检查只接受当前活动 Agent asset；结果会写入不可变质量报告并更新 Snapshot 的 `quality` 引用。请求必须携带当前 `If-Match: W/"active-design-{revision}"`；相同 Idempotency-Key、资产和 revision 会重放原报告，同一键配不同 revision 返回 `409 IDEMPOTENCY_CONFLICT`，旧 revision 返回 `409 ACTIVE_DESIGN_STALE`，因此不会因重试创建重复报告。内部资产的 Q003 报告固定读取 `production_concept` 的 `GeometryCompileReadback@2`，其 GLB SHA 必须等于 `:model.glb` 与兼容 `:export` 的实际字节；旧 `GeometryCompileReadback@1`、interactive preview 或旧视觉清单报告在 GET/重放时返回 `stale_compile_readback/unavailable`，必须重新检查，不能成为当前正式导出真值。`components:compatible` 只评估项目内 `AgentComponent` 的启用状态、相同 `domain_pack_id`、相同稳定 role、来源资产的最新质量状态，以及“替换保留目标 AssemblyGraph 连接”的事实；`passed`/`warning` 可进入预览，`failed`/`unavailable`、停用、跨领域或不同 role 均不可替换。它不读取或伪造正式 Module Asset 的审阅状态，也不是工程、结构、安全或制造适配评分。Agent 资产支持 GLB 导出、四视图 PNG 和条件式爆炸概念 PNG 派生预览。旧 Concept API 的 OBJ、MP4 和 ZIP 不自动等价于 Agent 资产导出。

### 7.1 Rust-native C105 Recipe 候选展开

```text
POST /api/v1/agent/asset-versions/{asset_version_id}/parts/{part_id}/component-recipes:expand
```

该端点只由 Rust `forgecad-core`/desktop compatibility bridge 处理。它不在 Python FastAPI 注册，Python 也不是失败 fallback。生成 OpenAPI 时，`scripts/export_openapi.py` 用代码所有 overlay 描述该路径并标记 `x-forgecad-owner=rust-core`、`x-forgecad-zero-write=true`；如果 FastAPI 日后意外声明同一路径，合同生成会直接失败。

请求体为 `ComponentRecipeActiveCandidateRequest@1`：

```json
{
  "schema_version": "ComponentRecipeActiveCandidateRequest@1",
  "recipe_request_id": "recipereq_example",
  "component_recipe_ref": {
    "schema_version": "ComponentRecipeRef@1",
    "recipe_id": "recipe_example",
    "version": 1,
    "recipe_sha256": "<64 lowercase hex>"
  },
  "slot_bindings": [],
  "parameter_values": [],
  "material_zone_overrides": []
}
```

Rust 从 URL 中的活动 `asset_version_id`/`part_id` 重新取得 Project、head、Snapshot revision、Domain Pack 和 C104 lock，不接受客户端伪造这些字段。`slot_bindings` 只能启用 parent Recipe 已固定的 reviewed child ref；不是任意组件选择器。C105 v1 的 `parameter_values` 与 `material_zone_overrides` 必须为空，比例和材质继续使用现有 ChangeSet preview→confirm。

成功响应是 `ComponentRecipeCandidate@1`，包含 active-edit context、精确 Recipe/registry/candidate hash、expanded ShapeProgram/AssemblyGraph 和 instance provenance。`status=expanded` 只表示 Rust 展开成功，不等于 GLB、preview、quality、export 或新版本。调用前后 Version/head/Snapshot、ChangeSet、quality/export、SQLite、CAS、对象库和临时 GLB 都必须不变；该端点不读取 Keychain、Provider 配置或 API Key，也不发起 Provider 调用。

请求结构或非空自由 override 返回 `400 COMPONENT_RECIPE_REQUEST_INVALID`；不存在的 asset/Part/Recipe 返回 `404`；活动 head/Snapshot/registry/ref 已变化返回 `409 COMPONENT_RECIPE_CONTEXT_STALE` 或 `COMPONENT_RECIPE_CANDIDATE_STALE`，已有 preview、锁定、跨领域、child slot/hash、质量或预算冲突也以稳定错误拒绝并保持零写。客户端必须重新读取当前 Snapshot/Recipe ref 后再生成候选，不能把旧候选强行提交。

后续 `replace_part` 必须把候选密封事实原样放入同一 `AgentPartEditOperation`：`recipe_request_id`、`component_recipe_ref`、`recipe_slot_bindings`、`recipe_candidate_id`、`recipe_candidate_sha256` 与 `recipe_snapshot_revision`。`recipe_snapshot_revision + recipe_slot_bindings` 是 replay/stale 校验的一部分，不能由前端省略、重新排序或替换；该 Recipe variant 与旧 `replacement_component_id` variant 必须二选一。proposal/preview/confirm 会在 Rust 重新展开并比对，只有 confirm 才创建不可变子版本并更新 Snapshot。

`GET .../{asset_version_id}:render?width=512&height=512` 只接受当前活动 Agent 资产版本，尺寸范围为 64–2048，默认 640×640。返回 `AgentAssetRenderSet@1`，固定顺序为 `iso`、`front`、`side`、`top`，以及在 GLB primitive 几何组与 AssemblyGraph/稳定 Part ID 一一对应时出现的 `exploded_iso`。每个视图携带 `png_base64`、透明背景 alpha readback、字节数和 SHA-256；爆炸候选额外携带 `presentation_mode=exploded`、`background_mode=transparent` 与使用的 `part_ids`。不满足映射时返回四视图和不可用原因，绝不猜测分件。它是只读派生 artifact，不写入版本、Snapshot、质量或导出记录；不会调用 legacy Concept renderer，也不提供转台视频或工程照明结论。

`GET .../{asset_version_id}:render-package?width=512&height=512&render_set_sha256={fingerprint}` 只接受当前活动 Agent 资产和已由 `:render` 返回的同一 fingerprint。服务端重新验证 PNG readback/哈希后返回 `application/zip`，并以 `Cache-Control: no-store`、`Content-Disposition` 与 `X-ForgeCAD-Render-Set-SHA256` 标识响应。ZIP 固定只包含 `manifest.json`、四张 PNG 和可用时的 `exploded_iso.png`；`AgentAssetRenderPackage@1` manifest 记录来源版本、视图哈希/尺寸/展示模式/背景模式和爆炸图 `part_ids`。指纹不匹配返回 `409 RENDER_SET_STALE`，活动资产已变化仍返回 `409 ACTIVE_DESIGN_STALE`。该包不包含 GLB、OBJ、MP4、源文件、工程图、装配/维修说明或制造信息，也不创建任何 Version、Snapshot、Quality、Export 或对象库记录。

ShapeProgram 资产的 triangle、bounds、artifact profile 和 operation/output/material 证据来自当次 `GeometryCompileReadback@2`；readback 损坏时报告为 `unavailable`、导出拒绝，不回退到 primitive 常数或旧估算报告。这些是概念 GLB 事实，不是生产级视觉评分、工程、结构或安全结论。

## 8. 外部 GLB 参考

```text
POST /api/v1/agent/imports:glb
```

当前只接受自包含 glTF 2.0 GLB，并检查文件大小、三角预算、访问器范围、外部资源、压缩扩展和 SHA-256。成功导入后创建只读参考版本；该版本不能冒充可编辑 ShapeProgram。

## 9. ActiveDesignSnapshot API

以下接口是服务端读取、选择和安全版本导航边界。S004 已生成 desktop 类型并提供 client/error mapping，S005 已提供独立 reducer；S006 已让 Agent 资产的恢复、部件选择、检查和 GLB 导出读取同一 Snapshot；S007 已将 legacy Concept 变为只读且要求显式重建授权。S008 已持久化 preview/quality 引用并实现不可变回退/前进，且覆盖核心 CAS 竞争；广泛多客户端压力矩阵仍待完成。

| 方法与路径 | 当前用途 |
| --- | --- |
| `GET /api/v1/projects/{project_id}/active-design` | 读取 Project 唯一活动 Snapshot；legacy Project 没有 Snapshot 行时只派生稳定的只读响应，GET 不持久化；响应为 `Cache-Control: no-store` |
| `POST /api/v1/projects/{project_id}/active-design:select` | 选择或清空当前 Agent AssemblyGraph 内的一个 Part，并可用 `selected_material_zone_id` 选择该 Part 的真实材质区；服务端通过 revision/ETag 校验所有权 |
| `POST /api/v1/projects/{project_id}/active-design:convert-legacy` | 显式取得 legacy 只读设计的 Agent 重建输入；不把 legacy ModuleGraph 伪装成可编辑 AgentAssetVersion |
| `GET /api/v1/projects/{project_id}/active-design:navigation` | 读取当前 Agent head 是否可撤销/重做；不从 localStorage 推断版本历史；派生读模型为 `Cache-Control: no-store`，不提供独立 ETag |
| `POST /api/v1/projects/{project_id}/active-design:undo` | 从上一个逻辑版本创建新的不可变 AgentAssetVersion，并原子切换 Snapshot/head |
| `POST /api/v1/projects/{project_id}/active-design:redo` | 从服务端 redo 目标创建新的不可变 AgentAssetVersion，并原子切换 Snapshot/head |
| `POST /api/v1/projects/{project_id}/active-design:render-preset` | 以 CAS 更新 Agent Snapshot 的相机视图和灯光预设，不改变资产版本 |
| `POST /api/v1/projects/{project_id}/active-design:part-display` | 以 CAS 更新当前 Agent Snapshot 的部件锁定、隐藏或单独查看状态；不改变资产版本 |

`GET /active-design` 与成功的 `POST select/convert-legacy/undo/redo/render-preset/part-display` 返回 `ETag: W/"active-design-{revision}"`；两个 GET 都返回 `Cache-Control: no-store`。`GET /active-design:navigation` 是派生读取，不提供独立 ETag；客户端必须刷新 Snapshot 后才可发起 CAS 写入。写入 `POST` 至少提供 `snapshot_revision` 或 `If-Match`；质量检查使用强制 `If-Match`。缺少 revision 返回 `400 ACTIVE_DESIGN_REVISION_REQUIRED`，旧 revision 或不一致 ETag 返回 `409 ACTIVE_DESIGN_STALE`。legacy 派生 Snapshot 只能通过显式 `convert-legacy` 写入授权边界，不能由 GET 隐式落库。

render-preset 请求体为 `client_request_id`、`camera_view`（`iso|front|top|right`）和 `light_preset`（`cad_neutral|soft_studio|concept_contrast`），可选 `snapshot_revision`。它只允许 `source=agent_asset`；legacy 返回 `409 ACTIVE_DESIGN_LEGACY_READ_ONLY`，跨资产引用由 Snapshot 合同拒绝。该状态只控制现有单一 WebGL 主视口，不提供 PNG/ZIP、多视图或工程照明结论。

part-display 请求体为 `client_request_id`、`action`（`lock|unlock|hide|show|isolate|clear_isolation|show_all`）和按动作需要提供的 `part_id`，可选 `snapshot_revision`。锁定会成为服务端 ChangeSet 验证的一部分；隐藏和单独查看只影响现有主视口，绝不创建资产版本。请求会拒绝不属于当前 `AssemblyGraph` 的 part、legacy Snapshot、存在 preview 的 Snapshot 以及 stale revision；隐藏或隔离导致当前选择不可见时会返回已清空 selection 的新 Snapshot。它不是工程装配锁定或制造控制。

选择、转换、撤销、重做、render-preset、part-display 和质量检查都要求 `Idempotency-Key`。legacy Snapshot 拒绝 part selection、撤销、重做、render-preset 与 part-display；转换创建一个可审计的 `ready_for_agent_rebuild` 授权，保留旧 Project、ConceptVersion 和 ModuleGraph 不变。撤销/重做遇到未确认 preview 时返回可恢复冲突；成功时会清空 selection、preview 和 quality，且不会复活或覆盖历史版本。只有这个授权存在时，下一次确认的 Agent 资产才会原子成为活动设计；未授权提交继续返回 legacy read-only 冲突。转换不会复制或篡改旧几何。

## 10. 当前非权威 Concept 只读适配器

K003 仅保留旧工作台显式详情所需的 Rust 只读面：`GET /api/v1/projects/{id}`、`GET /api/v1/versions/{id}`、`GET /api/v1/module-graphs/{id}`、带明确 `pack_id` 的有界游标分页 `GET /api/v1/module-assets`，以及 `GET /api/v1/module-assets/{id}/file`。文档与 Manifest 的语义 SHA、身份关系、审阅元数据和 GLB 的 CAS/SHA/字节数都会在 Rust 重新验证；列表响应不返回 `logical_path`、`object_path` 或文件系统路径。这些 GET 不回退 Python，也不写数据库。

未使用的 Concept variants、ChangeSet、audit、quality、export 与 module thumbnail 读取稳定返回 `410 LEGACY_CONCEPT_ROUTE_RETIRED`；旧写入与旧导出不属于这个适配器。任何客户端都不得把 Concept Version 和 AgentAssetVersion 合并成一个版本号；权威选择规则见 [AUTHORITATIVE_STATE.md](AUTHORITATIVE_STATE.md)。

## 11. API 变更规则

1. 先修改 JSON Schema/Pydantic 合同；
2. 生成 OpenAPI、Python registry 和 TypeScript 类型；
3. 运行 `npm run contracts:types:check`；
4. 增加服务层 smoke 和失败用例；
5. 更新本文的当前能力与限制；
6. 未实现能力只能进入设计或计划文档，不能进入用户指南。
