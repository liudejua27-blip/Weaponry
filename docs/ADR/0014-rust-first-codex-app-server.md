# ADR-0014：Rust-first ForgeCAD app-server 与受限几何执行器

- 状态：Accepted（K001–K003 已实现；Rust-first 核心所有权迁移完成）
- 日期：2026-07-16
- 决策者：项目维护者
- 取代：`AGENT_GITHUB_REFERENCE_ARCHITECTURE.md` 中“长期保留 FastAPI 作为 Agent API/状态所有者”和“不采用完整 Rust 运行时”的目标结论
- 补充：ADR-0009 的单一状态真值、ADR-0010 的 Codex 式工作台、ADR-0011 的 3D 机械设计系统

## 背景

ForgeCAD 的桌面壳、Keychain 和 supervisor 使用 Rust/Tauri。K001 已建立 Rust app-server protocol/bridge，K002 已把 Agent 决策生命周期、Thread/Turn/Item/Approval policy、Provider Gateway、Product Tool、预算和取消迁入 Rust；K003 又把项目/版本/Snapshot/ChangeSet/质量/导出、SQLite/WAL、CAS 与对象库迁入 Rust core。Python 仍提供受限几何编译，但不再拥有产品状态或持久化权限。

OpenAI Codex 的可复用部分不是 shell 或 coding-agent 权限，而是 Rust app-server 对初始化握手、Thread/Turn/Item 生命周期、JSON-RPC 请求/通知、流式事件、取消和有界队列的清晰所有权。ForgeCAD 仍必须保留自己的 Project、ActiveDesignSnapshot、ShapeProgram、Material Zone、ChangeSet 和单 renderer 真值，不能复制 Codex 的通用文件/命令权限。[OpenAI Codex app-server](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md)

## 决策

1. ForgeCAD 的目标运行时改为 Rust-first：
   - `forgecad-app-server-protocol` 定义版本化 JSON-RPC 2.0 请求、响应、通知、初始化能力和稳定错误；
   - `forgecad-app-server` 拥有 Thread、Turn、Item、Approval、Provider、取消、预算、队列和事件流；
   - `forgecad-core` 拥有 Project、AgentAssetVersion、ActiveDesignSnapshot、Selection、ChangeSet、Quality 和 Export 协调；
   - Tauri 只负责桌面窗口、Keychain、生命周期和本机桥接，不成为第二套状态真值。
2. React/Three.js 客户端最终只连接 Rust app-server。开发模式可以由 Tauri bridge 或受限 loopback bridge 承载相同协议；前端不得继续直接调用 Python FastAPI 作为长期架构。
3. Python 在迁移期间降为 `RestrictedGeometryExecutor`：
   - 只接收 Rust 已解析高层 Token/Recipe 后生成并通过 Schema/G819 校验的 ShapeProgram、Profile/SectionSet 或等价几何 IR 编译请求；
   - 不获得 Project SQLite、对象库、Provider Key、用户会话或 Snapshot 写权限；
   - 只返回候选 GLB、readback、错误和确定性 hash；
   - 通过版本化本机协议、超时、取消、预算和内容寻址 staging 隔离。
4. 迁移不得建立 Rust/Python 双写真值。每个阶段只能有一个持久化所有者；切换必须经过离线迁移、兼容读取、hash 对照、失败回滚和重启 Gate。
5. 现有 Manifold Python CSG 在没有新的体积、确定性、材质区、取消、打包和跨平台 benchmark 前继续作为唯一受限几何实现。Rust-first 不授权一次性重写几何内核或同时保留第二默认 handler。
6. Codex 风格只采用生命周期和协议模式：
   - `initialize` 后才接受请求；
   - Thread 包含多个 Turn，Turn 包含有序 Item；
   - `turn/started`、`item/*`、`turn/completed|failed|cancelled` 以通知流增量发布；
   - 所有 request/turn/item 有稳定 ID；
   - 有界队列、背压、取消传播和断线恢复必须测试；
   - 不开放 shell、任意文件系统、任意 MCP、原始隐藏推理或通用代码执行。
7. 迁移按 `FGC-K001 → FGC-K002 → FGC-K003` 三个原子任务完成；三项现均已退出，只有满足 K003 所有权和 Gate 证据时才可把核心描述为 Rust-first。

## 实施状态（2026-07-18）

- K001 已完成：`forgecad.app-server/1`、initialize、版本化 JSON-RPC 2.0、稳定错误、通知顺序、取消、背压、cursor replay、Tauri invoke/event 与受限 loopback 传输均由 Rust-owned 合同承载；React 不另建第二状态源。
- K002 已完成：Rust app-server 单一拥有 Thread/Turn/Item/Approval policy、Context Builder、DeepSeek Provider、13 项 Product Tool Action Loop、预算、取消树、usage 和脱敏 trace；旧 Python lifecycle POST 生产默认返回 410。
- `npm run k002:code-gate` PASS：Rust 173 项（app-server 72、protocol 38、desktop 49、DeepSeek 14）、Python Agent 69 项、ports 51 项，以及 T002 14/14、T003、r3、contracts、typecheck/build、Tauri、安全和密钥门通过。
- `npm run k002:packaged-gate` PASS：K001 packaged 业务链保持通过；K002 原生 packaged 双启动验证未配置 Provider、`network_call_made=false`、`PROVIDER_NOT_CONFIGURED`、两个有序 Item、Python lifecycle POST 410、无持久化 `reasoning_content` 和 `provider_calls=0`。
- 当前 macOS arm64 sidecar 为 31,972,320 bytes，SHA-256 `5aeb68334f54bfee070319191ca055479c1290c9b368a1da569dd39a943620d3`。
- K003 已完成：Rust app-server/core 单一拥有 Project、AgentAssetVersion、ActiveDesignSnapshot、Selection、ChangeSet、Quality、Export、SQLite/WAL、CAS 和对象库；Python 默认/frozen runtime 只暴露 capability-gated restricted geometry，产品/lifecycle HTTP 默认 410。
- 当前源码绑定的五层聚合报告固定为 `output/k003-layered-gate-final-source-20260718/report.json`；其通过条件为 `status=passed`、`exit_code=0`、`source_changed=false`，并强制覆盖 Core 13 facets、Rust↔Python 5 contracts、packaged 首次/重启、T002 14/14、T003、r3、M108 与文档/安全/密钥门，且 `provider_calls=0`。

## 迁移目标结构

```text
React + Three.js Workbench
          │ Tauri invoke/event or bounded loopback JSON-RPC
          ▼
Rust forgecad-app-server
├── initialize / capability negotiation
├── Thread / Turn / Item / Approval
├── DeepSeek Provider Gateway / usage / cancel
├── Product Tool Registry / Skill policy
├── ActiveDesignSnapshot / Version / ChangeSet
├── SQLite / content-addressed object ownership
└── RestrictedGeometryExecutor port
          │ versioned request, no DB path or secret
          ▼
Python geometry executor (transitional)
├── expanded ShapeProgram/Profile geometry IR validation
├── Extrude/Revolve/Loft/Sweep/CSG
├── UV/tangent/Material Zone/PBR compile
└── GLB + readback + deterministic hashes
```

## 后果

- 当前 FastAPI sidecar 只作为受限几何执行器和历史兼容输入，不是业务实现或持久化写入者。
- 本 ADR 原定“完整 M108 退出后才执行 K001–K003”的依赖结论已由 ADR-0015 取代。M108A 与 K001–K003 已完成；接下来依次执行 C105 和 M108B。
- 当前 `.app` 是 Rust/Tauri + Rust app-server/core lifecycle/Provider/Tool/product-state + Python restricted geometry executor。该边界允许称核心 Rust-first，但不等于整个产品已发布、几何内核已改写成 Rust或视觉已达 M108B。
- Provider Key、Snapshot、版本、质量和导出边界保持不变；迁移不能削弱现有 Gate。

## 被否决方案

- 只把 Tauri 壳称为“主要 Rust”：实际 Agent/状态/几何仍由 Python 所有，描述不真实。
- 一次性重写全部 Python：无法保持现有 100+ Gate、packaged sidecar、Manifold 和历史 GLB readback。
- Rust 与 Python 同时写 SQLite：会破坏 ActiveDesignSnapshot 和不可变版本链。
- 直接 fork Codex：会引入通用 coding-agent 权限和与 ForgeCAD 无关的文件/命令模型。
