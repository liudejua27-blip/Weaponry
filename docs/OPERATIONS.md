# ForgeCAD 操作文档总索引

版本：2026-07-15
状态：当前文档路由，不再承载完整 legacy 手册

ForgeCAD 当前是本机 Alpha。普通用户、开发者、资产作者和发布维护者需要不同的信息；本文件只负责把读者带到唯一正确的手册，避免把 Weapon、Unity、ComfyUI 和旧 Provider 命令重新混入主操作路径。

## 1. 按角色选择文档

| 角色 | 首选文档 | 解决的问题 |
| --- | --- | --- |
| 零基础测试用户 | [USER_GUIDE.md](USER_GUIDE.md) | 启动、配置 Provider、生成、编辑、检查和导出当前 Agent GLB |
| 产品设计与目标体验评审 | [MECHANICAL_DESIGN_OPERATIONS.md](MECHANICAL_DESIGN_OPERATIONS.md) | 单一最佳结果、轮廓/截面、Recipe、PBR、Provider 诊断和目标失败恢复；不是当前操作能力 |
| 本机开发者 | [DEVELOPMENT.md](DEVELOPMENT.md) | 环境、Tauri/Vite、测试、调试和证据采集 |
| 组件与美术资产作者 | [ASSET_AUTHORING.md](ASSET_AUTHORING.md) | 模块制作、元数据、原创声明、独立审阅和晋级 |
| 发布维护者 | [RELEASE_MAINTENANCE.md](RELEASE_MAINTENANCE.md) | CI、sidecar、SBOM、签名前检查和发布阻断 |
| 故障值班人员 | [DISASTER_RECOVERY.md](DISASTER_RECOVERY.md) | 备份、校验、恢复、回滚和事故记录 |
| 后续 Codex | [CODEX_HANDOFF.md](CODEX_HANDOFF.md)、[DOCUMENTATION_STATUS.md](DOCUMENTATION_STATUS.md) | 当前工作区、文档状态账本、任务顺序、已知失败和首轮命令 |

完整文档归属和废弃记录见 [DOCUMENTATION_MAP.md](DOCUMENTATION_MAP.md)。不要通过搜索到的旧文件直接开始操作。

## 2. 当前产品真相与 Gate 口径

代码路径/历史证据（本轮不等同于 PASS）：

- Tauri/React 桌面壳和本地 FastAPI；
- SQLite、内容寻址对象和不可变版本基础；
- 四领域最小 manifest 与 48 个确定性 blockout 变体（前端当前仍只展示三方向）；
- 受控 ShapeProgram Worker（box/cylinder 以及已通过门禁的 G801–G806 操作）；
- 分件候选、受限部件修改、13 个六类视觉材质；
- AgentAssetVersion、ChangeSet、项目内组件和 GLB 回读；
- macOS Keychain Provider 配置；
- 自包含 GLB 只读参考导入。

本轮或当前明确未完成/阻断：

- Agent 路径的 `ActiveDesignSnapshot` 服务端与桌面接入已实现；当前 `desktop:r3-concept-workbench-smoke` 的 Agent-first 路径已通过，广泛多客户端压力矩阵与原生安装回归仍未完成；
- 未知/含糊领域的服务端澄清 Item 和 focused UI 已实现；完整工作台回归、真实 Provider truth set 和多语言/可访问性扩展仍未完成；
- 复杂轻量几何操作；
- 自动深度分件、精确碰撞和运动学；
- Agent 转台视频、OBJ/MP4 和工程渲染；R002–R005 已完成四视图及条件式透明爆炸 PNG 的软件栅格化、来源证明、alpha/readback、Agent 直接 GLB 下载、桌面预览/单图下载，以及当前 PNG/manifest 的受限概念图包，但它们仍是只读概念图；本机 `.app` 启动已通过，原生 WebView 点击下载仍因当前自动化会话缺少 macOS 辅助功能权限而待人工验收；
- 工作台核心 Snapshot E2E 本轮已通过（参考 GLB v1 → Agent 可编辑资产 v2–v5）；完整并发压力与原生安装回归仍待；
- F001 characterization 已在本机 Chrome 通过并登记到 CI；F002–F006 已完成 Agent 对话、选择卡、四类抽屉、组合层与可访问性收敛；FGC-T002 已通过 12 个独立工作台 E2E 场景，FGC-T003 已通过单 WebGL、抽屉/重载资源、内存和 bundle 预算；FGC-G801 已通过 wedge/capsule 确定性 GLB smoke，FGC-G802 已通过 profile/extrude 拓扑与 readback smoke，FGC-G803 已通过 revolve 拓扑与 readback smoke，FGC-G804 已通过 mirror/array/radial_array 引用与预算 smoke，FGC-G805 已通过受限 union/subtract 失败边界 smoke，FGC-G806 已通过 bevel/surface panel 视觉与 readback smoke，FGC-G807 已通过四领域 48 结构多样性与重复生成 smoke，FGC-R001 与 R002 已通过渲染 smoke；仍需保留 F001–F006、T002/T003、G801–G807、R001/R002 和 r3 作为回归门；
- 真实 Provider 四领域 truth set；
- 非空 packaged sidecar、签名和公证。

Q002 已关闭旧文档中的 bootstrap/质量写入缺口：首次 `GET /active-design` 只会从有效 Agent head 或 legacy current version初始化一行，空项目不写入；质量检查强制当前 `If-Match` 与 `Idempotency-Key`。开发或排查时不要使用前端缓存掩盖 `no-store` 读取和重复质量报告的 CAS 错误。

R001 已通过 `agent:r001-render-preset-smoke`：四个相机视图、三个灯光预设、CAS/幂等、legacy 写入阻断和跨资产合同校验均已验证。R002–R004 已通过 `agent:r002-render-views-smoke`、`agent:r003-exploded-views-smoke`、`agent:r004-render-package-smoke`：四张 PNG 的来源、alpha/readback、字节数、SHA-256 与重复生成 fingerprint 均稳定；几何组与稳定 Part ID 一一对应时才增加透明爆炸候选；当前预览指纹匹配时才可下载只含 PNG/manifest 的可复现 ZIP。桌面端 `desktop:typecheck` 和 T002 工作台流程通过。它不构成工程渲染、装配或制造能力。

能力状态以 [能力—Gate 矩阵](evidence/CAPABILITY_GATE_MATRIX.md) 为证据，以 [权威状态设计](AUTHORITATIVE_STATE.md) 为下一阶段数据合同。

## 3. 文档边界

当前权威文档：

- [产品定义](PRODUCT_DEFINITION.md)
- [系统设计](DESIGN.md)
- [3D 机械设计系统目标操作手册](MECHANICAL_DESIGN_OPERATIONS.md)：描述目标流程，不替代当前 USER_GUIDE
- [当前 Agent API](API.md)
- [实施计划](IMPLEMENTATION_PLAN.md)
- [测试策略](TEST_STRATEGY.md)
- [真实 Provider 四领域评测合同](AGENT_PROVIDER_EVALUATION.md)：仅供获授权的后续评测执行器使用；当前只有无网络 dry-run
- [生产发布清单](PRODUCTION_RELEASE_CHECKLIST.md)
- [Codex 执行总计划](CODEX_EXECUTION_PLAN.md)
- [Codex 原子任务索引](CODEX_TASK_INDEX.md)
- [Codex 完成定义](CODEX_DEFINITION_OF_DONE.md)

历史兼容资料位于 [legacy](legacy/README.md)。legacy 文档只服务回归和迁移，不是零基础用户操作说明，也不能作为通用机械 Agent 已完成的证据。

开发 Agent 或工作台前还应阅读：

- [GitHub 参考与采用边界](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md)：哪些项目只参考、哪些候选需要 benchmark、哪些路线明确拒绝；
- [插件与 Skill 操作设计](AGENT_PLUGINS_SKILLS_DESIGN.md)：后续 Codex 使用什么插件/Skill，以及产品内 Skill 的权限和评测；
- [文档地图](DOCUMENTATION_MAP.md)：唯一权威、历史证据、legacy 与已删除文档。

## 4. 按任务选择插件和 Skill

| 当前任务 | 使用 |
| --- | --- |
| 核验 GitHub 项目、PR 或 CI | `@github`，再选择 `github:github` / `github:gh-fix-ci` / `github:gh-address-comments` |
| 审查零基础用户流程 | `@product-design` + `product-design:audit`；做公开问题研究时用 `product-design:research` |
| 已有截图/Figma 后实现 | `product-design:image-to-code`，完成后做视觉 QA |
| 重构 React 工作台 | `build-web-apps:react-best-practices`，回归用 `build-web-apps:frontend-testing-debugging` 或 `playwright` |
| GLB、纹理和网格预算 | `game-studio:web-3d-asset-pipeline`，不得引入第二 renderer |
| 文档整理 | `documents:documents` + 当前文档门 |
| Tauri 本机与发布 | `build-macos-apps:build-run-debug`；外发时再用签名/公证 Skill |

插件/Skill 是开发工具，不进入用户安装包。使用前必须先读对应 `SKILL.md`，并遵守它的视觉来源、浏览器、权限或验证前置条件。

## 5. 发布前强制门禁

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
npm run release:packaging-readiness
```

`npm run release:packaged-sidecar-preflight` 会先输出不读取 Provider Key、不联网且不执行二进制的 P008 结构报告；当前 macOS arm64 input 的预期状态为 `ready_for_local_alpha`。`npm run desktop:packaged-sidecar-build` 冻结该 target，`npm run desktop:packaged-sidecar-alpha-smoke` 验证独立 frozen binary；`npm run desktop:packaged-tauri-alpha-smoke` 通过 LaunchServices 验证真实 `.app` 的 `packaged-sidecar`、首次初始化、确定性可编辑 GLB 导出和重启恢复。三者均不自动调用 Provider。`release:packaging-readiness` 仍预期失败：其他发布目标 sidecar、全新机器安装/升级/卸载、签名和公证尚未完成。不得通过删除检查、降低严重级别或改文案绕过。

## 6. 文档维护规则

1. 用户指南只写当前通过代码和测试验证的功能；
2. 目标能力写入 DESIGN、IMPLEMENTATION_PLAN 或 CODEX_EXECUTION_PLAN，并明确“未实现”；
3. legacy 命令只能写入 `docs/legacy/`；
4. 每个生产能力必须在能力—Gate 矩阵中有实现位置和自动证据；
5. 修改 API、状态真值、备份或发布流程时，同步更新对应专门文档；
6. 文档门禁必须拒绝断链、缺失命令和用户指南中的禁用承诺。
7. GitHub 参考只进入参考文档；实际加入依赖后才进入 lock、SBOM 和第三方许可证台账。

## 7. DeepSeek 本机运营边界

Agent 对 `api.deepseek.com` 使用本机日预算 20 元。请求先按 32k 输入、当前输出上限和缓存未命中价格预留额度；成功后按 Provider 返回的 `prompt_cache_hit_tokens`、`prompt_cache_miss_tokens` 与 `completion_tokens` 结算。当前价格表不是永久事实，发布或修改模型前必须在 DeepSeek 官方价格页复核，并更新实现注释与运营记录。

如 Turn 的 `usage_status=unavailable`，当天后续联网 Provider 请求应保持阻止状态；不要删除预算记录、伪造 usage 或通过自动重试绕过。超时 Turn 可能已有远端扣费，保留其预留额度并要求用户显式重新发起。Key 仅由 Tauri Keychain 注入，排查日志不得打印请求头、Base URL、完整 prompt、思维链或密钥。
