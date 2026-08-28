# ForgeCAD 开发指南

> 2026-08-26 现行 source 口径：**527 schemas / 28 operators / 115 read + 87 write = 202 tools**。真实 D1 `MoveVertices` 纵切已编译、物化、回读和六视图 replay；proposal 仍因 fresh owner/void/Part-ID FormArt 缺失而 blocked。后续开发必须沿同一资产完成 AuthoringMesh→High→editable Low→UV→Bake→Material→FPS→Engine，不能用 source compile 代替 `PASS_ASSET`。

> 2026-08-26 商业路线开发规则：先做能编译的最小纵向切片，再立即在同一 Hero candidate 上取真实 receipt；只为高风险合同、hash/lineage、崩溃/资源/确定性和真实关键链写测试。禁止为增加覆盖数量重复堆 fixture，也禁止用 source green 替代资产、引擎或人审。

版本：2026-08-25
状态：MCP001–009 functional core 已通过；FGC-MCP010F 是唯一 `in_progress`，当前开发主线已重排为商业 FPS Hero Asset 原生生产链。AuthoringMesh 已有只读 Runtime/MCP projection、7 个 durable contracts、Runtime/Store/MCP 三对象 prepare/get、Viewer source card和公共三对象 restart 1/1。IdentityLineage V2 的 4 合同、Store 4/4、Runtime restart 1/1、MCP 3/3 与联合编译已通过；basic preserving/topology edit 的稳定 authored ID、单调 tombstone 和 `preserved/created/retired` correspondence已有真实同 lineage多 candidate证据。split/merge、完整编辑历史以及 High/Low/UV/Cage/Bake/Material/LOD/engine/human 完整链仍未完成。

开发 workstream 固定为 `FormQuality → secondary-form-approved → AuthoringMesh → Native High → editable Low/Retopo → Hero UV → Cage/Bake → Material Layer Graph → FPS Presentation → LOD/Collision/Socket → commercial engine + independent human review → export/restart`。模块可提前做 source 工程，但 ProductionStage 只能按 19 状态顺序晋级，且 `hero-art-review-approved → engine-validated → export-confirmed`。第三方算法只能经固定 revision、许可证/SBOM、determinism/resource/security/package/removal Gate 后，以 ForgeCAD 自带 typed Worker 进入；不得直接安装 GitHub 插件或依赖外部 DCC。

## 1. 目标布局

```text
crates/
  forgecad-contracts/
  forgecad-core/
  forgecad-store/
  forgecad-runtime/
  forgecad-mcp/
  forgecad-worker-protocol/
apps/
  desktop/
  geometry-worker/
  render-worker/
  bake-worker/           target: typed high-to-low/cage executor
packages/
  forgecad-contracts/
  forgecad-skills/       MCP006 historical + MCP010B first-party bundles（10 个旧 Bundle + primitive-blockout@0.2.0）
migrations-runtime-v1/
docs/
```

旧工作台、App Server/Protocol 和 `apps/agent` 已删除；不得重新创建这些入口。

## 2. 分层

- JSON Schema 是跨进程合同源；生成 Rust/TypeScript 类型不得手改；
- Core 是无 UI/DB/网络纯逻辑；
- Store 只向 Runtime 暴露事务 repository、CAS metadata 和 backup/restore；
- Runtime 进行单写者文件锁、权限、Job、审批、版本和 orchestration；
- MCP/Tauri 是 adapter；MCP 配置 Runtime socket/token 时不能直接依赖 SQLite；
- Worker 只接受 bounded internal message；
- 产品不安装、调用或捆绑 Blender；不存在 Blender Worker 或 Blender fallback。Blender 仅允许按 ADR-0027 作为 reference-only 研究对象；
- Viewer 只读投影和 ephemeral UI state。

跨层快捷调用、第二 writer、compat fallback 和绝对路径协议会被 integrity Gate 拒绝。

## 3. 开发流程

1. 读取 AGENTS/文档链/当前任务；
2. 检查 dirty tree 和 owned paths；
3. 先写/改 Schema、negative tests 和 canonical fixtures；
4. 实现 Core/Store/Runtime；
5. 再接 MCP/Worker/Viewer；
6. 加幂等、stale、cancel、restart、budget、permission 测试；
7. 跑 focused → aggregate → 当前任务要求的真实 Codex/visual Gate；packaged 只在 MCP013；
8. 更新状态、能力矩阵和 handoff。

## 4. 安全开发

- CSP 和 Tauri capability 只允许 Viewer 所需本地资源；
- MCP/Workers 无网络和无 broad filesystem；
- 不读取 Provider Key、Codex Key 或用户 shell 环境 secret；
- attachment/file import 必须 canonicalize、root/symlink/MIME/size/hash 检查；
- 测试 fixture 不含真实用户图片、用户名和绝对路径；
- fuzz/adversarial 输入覆盖 GLB、图片、Schema、DAG、archive 和 IPC；
- 外部依赖/资产先走采用 receipt、SBOM 和 license Gate。

## 5. Git 与脏树

不 reset、checkout 或覆盖用户修改。大删除前先取得用户授权，生成 binary-safe diff、untracked archive 和数据备份并证明恢复。除非明确要求，不 commit、push 或建 PR。

## 6. 现有基座 focused baseline

```bash
CARGO_TARGET_DIR=/tmp/forgecad-mcp002-cargo-target \
  script/with_rust_toolchain.sh cargo test \
  --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --workspace --offline
```

该 Gate 覆盖 canonical hash、迁移/旧库拒绝、双 Runtime 文件锁、崩溃后锁释放、事务回滚、CAS corruption/capacity、备份恢复和 authenticated IPC。临时 Cargo target 不使用旧 ignored `target`。

## 7. MCP005–MCP009 已完成与后续规则

MCP005 已以 `image-rs/image` 的受限 PNG/JPEG features、decoder limits、authorized-root 和真实 Codex CLI receipt 收口。后续不得把用户原图复制进 fixture/Git；测试 fixture 使用自建、可公开的最小图片。

MCP006 已建立 `SubjectProfile`、`RepresentationPlan`、`AssemblyGraph`、`GeometryProgram`、`AppearanceProgram`、`RecipePlan` Schema、十个历史独立 first-party Bundle、Recipe/DAG/单位/finite/预算/hash/license/SBOM/provenance Gate 和合成正/负 fixture。MCP010B 当前源码另提供 `primitive-blockout@0.2.0`，MCP010D 再提供 `hard-surface-detail@0.2.0`，绑定 GeometryProgram@2/真实 profile/loft/revolve/sweep/panel/vent/joint consumer 与 strict readback；MCP010E 提供 `uv-pbr@0.2.0`、CC0-derived 离线 AssetPack、512px UV atlas、固定 mikktspace 和 embedded PBR texture producer；MCP007 已完成 product-owned bounded box/cylinder/sphere compiler、deterministic GLB/readback、Runtime/MCP/Viewer focused Gate；MCP008 已在现有 readback 上完成 bounded UV/tangent/PBR/render；MCP009 已完成 limited quality、stable-Part change、immutable version/restore 和 CAS export functional core。`img2threejs` 只提供 staged/detail-inventory/compare 的设计参考，`img2css` 只提供颜色/区域预览参考；不得安装其脚本、运行任意 JS/CSS 或把生成代码当 Runtime 真值。

## 8. 基线命令

每个 MVP 任务：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
git diff --check
```

新基线覆盖 contracts、Rust workspace、Store crash tests、worker tests、MCP conformance、Codex E2E、Viewer typecheck/build、quality/GLB、packaging、安全和 SBOM；MCP008/009 已写入用户指南，但真实 Codex/visual/human/packaged 仍必须分开标注。

## 9. 代码评审红线

发现内置模型/Provider、任意脚本、8000/FastAPI、MCP 直写库、Viewer 版本头、未绑定质量的 confirm、未授权路径、absolute path 输出、未 pin GitHub 代码或旧合同恢复时，评审必须拒绝。

## 10. 商业模块的合同先行顺序（future / queued）

商业生产扩展必须逐模块建立 `Schema → Operator → budget → fixture → LICENSE/NOTICE → SBOM → provenance → signature/hash` 闭环，再接 Runtime/MCP/Viewer。预期顺序为：`AuthoringMesh@1`（当前 partial structural）→ Native High → Retopology/Low → Hero UV → Cage-Bake → Surface → LOD/Collision/Socket → `EngineValidationReceipt@1` → `HeroArtReviewReceipt@1`。Native High 的 source durable、Low 的 `DRAFT_UNREVIEWED` durable 与 Hero UV durable 1/1 replay 不能被写成商业模块已激活；Cage/Bake、Surface、LOD、Engine 和 Hero Art Review 当前 `NOT_RUN/NOT_PROVEN`。

每个模块必须由产品内建 `ForgeCadModule@1` 描述：`schema_refs`、`operator_refs`、有限 `budget`、deterministic 正/负 `fixture_refs`、LICENSE/NOTICE hashes、SPDX SBOM、source/build provenance、signature、module/contract/input/output hashes，以及 `network=false`、`dynamic_plugin=false`、`script=false`、`direct_db_write=false`、`direct_cas_write=false`。Worker 只接受 closed typed message；Runtime 是唯一 CAS/SQLite 写者；MCP/Viewer 只做 adapter/read projection。未有同 cohort benchmark、恶意输入/资源、重放和 package receipt 时保持 `queued`，不得新增 active tool 或跨阶段放行。

第三方 Manifold、OpenSubdiv、QuadriFlow、xatlas、Embree、MaterialX、OIIO、OCIO、meshoptimizer、glTF Validator 只可在 `EXTERNAL_PROJECT_ADOPTION.md` 的审计后，以签名确定性 ForgeCAD Worker 封装；Blender、Substance、Maya、任意 DCC、脚本、联网服务都不是产品依赖。所有当前口径仍为 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools**，不得用文档中的目标模块数量反推已实现能力。
