# ForgeCAD Runtime 打包合同

> 2026-08-26 `04AF`：当前新回执属于 source Runtime/MCP 真实 D1 与隔离 AuthoringMesh restart，不是 packaged Desktop、Unreal/Unity import 或 commercial delivery PASS。最终打包必须携带同 export hash 的 canonical GLB、KTX2/texture set、LOD/collision/socket、FPS presentation sidecars、engine validation 与 human approval；当前均未闭合。

> 2026-08-26 现行 source：**525 schemas / 112 read + 84 write = 196 tools**。新增商业资产内核仍仅通过本地 source compile/link；packaged same-cohort 资产闭环与 Hero Weapon 交付仍 `NOT_RUN/NOT_PROVEN`。

> 商业交付包必须携带 canonical GLB、KTX2/fallback、LOD/collision/socket/animation/event sidecars 与固定 Unreal/Unity profiles；外部库需固定 revision、LICENSE/NOTICE/SBOM/provenance、resource/determinism receipts。详见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 商业资产包：保留 canonical source `visual.glb`，派生 `EngineDeliveryPackage@1` 同时携带纹理/KTX2、独立 HeroLodSet、CollisionSet、SocketSet、AnimationClipSet、GameplayBeatSet、VfxCueSet、AudioCueSet 和 engine profile manifest。glTF Transform 仅允许固定 operation allowlist；每次 prune/dedup/meshopt/KTX2 前后必须保存 semantic diff、source hash、decoded texture/mip hash 和压缩 bytes hash，Khronos Validator 不能替代引擎或人审。

> 2026-08-25 商业质量打包边界：评估中的 retopo/UV/ray/material/image/LOD 第三方候选均不得因文档列名进入 App、CLI、Worker、lockfile 或 installer。未来 accepted Worker 必须固定 binary/source cohort、无网络、无任意脚本、资源封顶、SBOM/NOTICE/signature/provenance，并保留无该依赖时的 unavailable/fallback 行为；商业资产仍需 packaged same-hash engine/human/restart Gate。

## 商业 Hero Weapon 的 11 组打包验收

打包只接受同一 `candidate_hash → export_hash` 已闭合全部 11 组质量门的资产：

`Art Direction/ReferenceViewSet → AuthoringMesh → High → Low → UV → Cage/Bake → Material → LOD → Viewer/animation/VFX/audio validation → Engine → independent Hero Art Review`

该 11 组是商业验收分组，不是第二套 Stage 状态机。实际 Runtime 写入只认 `ProductionStage@3` 的 19 状态顺序；其中 `hero-art-review-approved` 在 `engine-validated` 之前，随后才是 `export-confirmed`。打包器必须验证两门均在同一 candidate/export hash 通过，不得按上面研究分组的展示顺序越级写 Stage。

打包清单必须分别记录每门的 `PASS`、`NOT_RUN`、`NOT_PROVEN`、`BLOCKED` 或 `NOT_PASSED`，不能将 source/transport/Three.js smoke 合并为商业通过：

- Art Direction/ReferenceViewSet、AuthoringMesh、High、Low、UV：当前最多有 reviewed/reference 或 structural/source receipt；Hero UV durable prepare/replay/drop/reopen/get **1/1 PASS**、4 CAS roots linked/GC 仍不代表 artist review、package 或 engine PASS。
- Cage/Bake：Formal High internal materializer、fixed Worker、8-map/dilation、七记录 Store/MCP seam 与独立 Formal High public `get/prepare` 只有 source/compile/focused；完整 positive replay/cleanup/restart/raw transport 与 current-D1 receipt 缺失，new prepare 零写失败。旧 coverage/miss/fallback/cross-part/padding=0 只能作为失败诊断；正式 Cage/Bake 未通过。
- Material：当前只有 **4 MaterialZones / 6 formula textures** 的 fixed-formula preview，尚未形成 `MaterialLayerGraph@1` 商业通过。
- LOD 与 Viewer/animation/VFX/audio validation：Three.js 仅结构消费；动画、VFX、audio、无障碍和性能证据仍 `NOT_RUN`。
- Engine 与 Independent Hero Art Review：**Unreal/Unity 均未运行**，human=`NOT_RUN`；没有 `PASS_HUMAN_ART_REVIEW` 就不得生成商业发布包。

当前源面为 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。即使 package verify 通过，也不能绕过上述 11 阶段或激活 `HERO_ASSET_APPROVED`。

| 模块/依赖 | 发布包状态 | 精确边界 |
|---|---|---|
| Manifold | accepted bounded slice | 仅固定 revision、同一 Part union/difference/intersection Worker |
| `mikktspace@0.3.0` | accepted restricted slice | 仅 source-focused tangent Worker；不等于 Hero UV/PBR PASS |
| OpenSubdiv/xatlas/MaterialX/OCIO/glTF Validator | `NOT_IN_RELEASE` | research-authorized，不是依赖；外部 Validator `NOT_RUN` |
| QuadriFlow/Embree/OIIO/meshoptimizer | `NOT_IN_RELEASE` | approved-for-evaluation，snapshot-blocked |
| Blender/Substance/Maya/DCC sidecar | `EXCLUDED` | reference-only；无 binary/script/project fallback |

2026-08-15 live/package cohort refresh：当前 `abae43f3` 已重建并安装用户级 `ForgeCAD Runtime Dev.app`，MCP/Runtime/Geometry Worker/Render Worker 四资源 cohort `5a1f108a…e2dd2f` exact-match；ad-hoc deep-strict、resource allowlist、隔离 Runtime/project/preflight probe 均 PASS，旧包以 timestamped backup 保留。新包 manifest 为 37 read + 24 opt-in write；安装与 probe receipt 位于 `docs/evidence/mcp010f/dev-app-install-live-cohort-20260815.json` 和 `docs/evidence/mcp010f/dev-app-live-cohort-probe-20260815.json`。当前 Codex MCP 会话仍缓存旧 `7f9e4c…ee518`/旧 manifest `05fca3…d4d0a`，必须重新建立会话才能验证 live tool surface；本次不升级真实 likeness、PBR、人评、export/restart 或 360。

版本：2026-08-26
状态：MCP013 正式发布合同；不阻塞 MCP005–009 开发 MVP，当前不可外部分发

## 1. MVP 与发布分界

MCP005–009 使用本地开发构建验证真实 3D，不要求 Developer ID/notarization。任何对外安装、自动配置 Codex、普通用户可用或正式版本声明仍必须满足本文全部要求。

## 2. 发布组件

同一 release manifest 包含：

- ForgeCAD Runtime Viewer；
- `forgecad-runtime`；
- `forgecad-mcp`（拥有 MCP stdio，并按需启动同包 `forgecad-runtime`）；
- geometry/render workers；
- ForgeCAD-owned fixed geometry/high/retopo/UV/cage-bake/surface/LOD/render workers；Blender worker 明确不进入 P0/P1 发布包；
- first-party Skill/asset packs；
- Runtime V1 migration；
- contracts/tool manifest/license/NOTICE/SBOM/provenance/signatures；
- Codex Desktop/CLI P0 配置助手；IDE/VS Code/Cursor/Windsurf 配置基线只作为未来兼容资产保留。

组件合同版本和签名必须一致。旧 sidecar、App Server/Protocol、Python FastAPI、端口 8000、模型 Key 配置和 legacy packs 不进入安装包。

## 3. macOS 边界

安装包完成 code signing、hardened runtime、entitlements 最小化、notarization/stapling。Workers/MCP 也是签名可执行文件；Runtime 只开放 authenticated local IPC。Viewer CSP/Tauri capabilities 不允许 broad filesystem/network。

Codex MCP 配置只写本机签名二进制路径、timeout 和 write approval policy，不包含 secret 或项目绝对路径。卸载默认保留用户 Library，数据删除需独立选择。

当前 `forgecad-runtime serve` 用于独立诊断，正常入口是 `forgecad-mcp`：MCP 先完成 stdio initialize，再异步启动同包 Runtime，并通过受保护的 `ready.json`/status handoff 连接 authenticated local IPC。生命周期回归已通过；2026-08-15 同 cohort Dev.app 的四资源 Resource allowlist、ad-hoc deep-strict package verify 和 packaged Runtime → sibling Render Worker 九 AOV raw transport 已通过，证据见 `docs/evidence/mcp010f/dev-app-install-render-worker-20260815.json`、`dev-app-package-verify-render-worker-20260815.json`、`packaged-render-worker-raw-20260815.json`。该 raw probe 使用 synthetic reference，只证明 packaged resource/process/protocol；distribution signing Gate 仍保持 BLOCKED，正式 notarization、packaged UI E2E、真实 likeness/PBR/人评和 360 不由此升级。本机可见 1 个有效 codesigning certificate，但以名称和 SHA-1 选择身份的只读签名探针均返回 `errSecInternalComponent`，keychain settings 读取还返回 passphrase error，且没有修改 keychain；详见 `docs/evidence/mcp004/macos-signing-diagnostic.json`。`docs/evidence/mcp004/codex-cli-write-e2e.json` 只证明真实 Codex CLI 对开发诊断入口的事务交接。

本地打包命令要求调用方显式提供签名身份；没有身份时命令直接失败，不自动退回 unsigned：

```bash
APPLE_SIGNING_IDENTITY="<approved signing identity>" \
  npm run desktop:tauri-package:macos
```

该命令会先构建 release `forgecad-runtime` 与 `forgecad-mcp`，再运行 Tauri app bundle。仓库内 `npm run desktop:tauri-build` 通过 `script/with_rust_toolchain.sh` 固定 Cargo 查找；签名失败、notarization 未运行或 packaged Desktop 3D E2E 未验收时，状态必须分别记录为 BLOCKED/NOT_RUN。MCP010A 的开发 App 激活证据不替代 MCP013 正式发布门。

## 4. Blender/DCC 排除边界

Blender、Substance、Maya、BlenderMCP、`.blend`、`bpy`、Python addon 与任意 DCC sidecar 不进入 ForgeCAD P0/P1 发布包，也不是 fallback。对应 capability 必须保持 `unavailable/reference-only`。研究可用于 clean-room 问题定义和隔离 benchmark，但不能把 DCC 二进制、脚本、工程状态或输出升级为 Runtime 真值。High、Low、UV、Cage/Bake、Surface、LOD、Render 与 Validator 必须由 ForgeCAD-owned fixed typed Worker 随同一签名 cohort 分发。

## 5. 安装/升级

安装前验证磁盘和兼容性；升级前备份 Runtime V1 DB/CAS manifest；在副本跑 migration；原子替换整套组件；失败回滚二进制和数据库。禁止不同版本 MCP/Runtime/Viewer/Worker 混跑写路径。

## 6. 发布 Gate

clean-room 构建可复现、签名/notarization、SBOM/license、安全扫描、无绝对路径/secret、无 legacy/model/8000、Codex Desktop/CLI P0 packaged E2E、Viewer 关闭运行、升级/回滚、离线启动、灾难恢复、跨类别质量和真人门。IDE 兼容只有在未来升级支持范围时才加入发布 Gate。
