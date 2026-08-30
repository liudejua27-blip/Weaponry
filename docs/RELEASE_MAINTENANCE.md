# ForgeCAD 发布维护

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

> 2026-08-26 商业维护补充：每个 accepted 第三方组件、engine profile、texture encoder 和 optimization allowlist 都必须锁 revision/cohort、LICENSE/NOTICE/SBOM、可重放 recipe 和退出方案。升级必须对 canonical source GLB、decoded mip chain、engine imported projection 与固定 FPS shots 做 semantic/visual diff；压缩 bytes 因平台不同可另存，但逻辑材质与 decoded content 必须保持约定一致。

版本：2026-08-25
状态：MCP013 后的目标流程；不阻塞开发 MVP

商业资产路线补充：release cohort 必须同时固定 Authoring Mesh Kernel、High、Low/Retopo、UV、Cage/Bake、Surface/Texture、LOD/Collision、Render/AOV 和 GLB/Engine Validator 的 module/version/binary/schema/canonical hashes。缺少任一模块或出现 cohort 漂移时，相关写路径与 Stage transition 必须关闭；不得静默回退到 Blender、用户本机 DCC、脚本插件或联网服务。

第三方依赖状态必须写入 release manifest：`accepted` 必须绑定固定 revision、LICENSE/NOTICE、transitive SBOM、module/binary hash、签名、资源与 removal receipt；`research-authorized` 和 `snapshot-blocked` 必须明确 `NOT_IN_RELEASE`。当前只有 bounded Manifold Worker slice 与 `mikktspace@0.3.0` restricted slice 可按各自 scope 进入依赖账本；OpenSubdiv、QuadriFlow、xatlas、Embree、MaterialX、OIIO、OCIO、meshoptimizer 与 glTF Validator 都不是 active release dependency。外部 glTF Validator 只是未来条件性 adapter，当前 Runtime strict readback 仍为格式权威，商业 EngineValidation 仍需真实 Unreal/Unity receipt。

## 1. 版本集合

每次 release 固定 Viewer、Runtime、MCP、workers、contracts、DB schema、Skills、asset packs 和 accepted ForgeCAD modules 的版本/hash。release manifest 是分发真值；单独替换组件会关闭写路径。结构性模块就绪不等于商业质量通过；视觉、人评、引擎和分发状态必须分别维护。

## 2. 发布分支

从干净、已通过 MCP013 的 commit 构建。CI 对目标 commit 绿色不证明本机脏工作树。构建产物、SBOM、NOTICE、provenance、signatures、Codex E2E 和 quality/human evidence 绑定 commit。

## 3. 更新类型

- patch：合同兼容 bug/security 修复；
- minor：向后兼容能力/Skill；
- major：合同或 DB 破坏性变更，需 ADR、迁移和 rollback；
- Skill/asset revocation：独立签名 registry 更新，不改历史版本 receipt。

## 4. 维护 Gate

依赖更新先看 changelog/license/SBOM/security/API，再在隔离分支跑全 Gate。不得使用浮动 Git revision 或未经审查的自动下载。Renderer/codec/geometry 依赖变化必须重跑 golden、readback、跨平台和真人基线。

## 5. 回滚

升级前备份；失败原子恢复旧二进制和兼容数据库快照。已经用新 Schema 写入的数据不能直接由旧 Runtime打开时，进入 read-only recovery并提供导出，而不是强行 downgrade。

## 6. 支持策略

发布说明区分实现、已测宿主/平台、已知限制和未通过质量类别。安全/许可证/Skill 撤销有紧急通道；不静默改变用户资产或版本历史。
