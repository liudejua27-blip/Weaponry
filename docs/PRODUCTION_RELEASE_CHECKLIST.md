# ForgeCAD MVP 与生产发布清单

版本：2026-08-09
MVP 结论：**功能核心 PASS；参考图真人验收/packaged release BLOCKED 或 NOT_RUN**
MCP010 结论：**FGC-MCP010A done；FGC-MCP010B structural source Gate PASS（Darwin OS memory hard cap deferred/NOT_RUN）；FGC-MCP010C in_progress / source-focused PASS_WITH_UNRUN_VISUAL_GATES；MCP010D–F BLOCKED**
生产发布结论：**BLOCKED，不可外部分发**

## 1. 已完成基座

- [x] MCP001 旧 UI/Provider/App Server/Agent/contracts 成组硬切
- [x] Runtime 是唯一 DB/CAS writer；OS 文件锁、第二实例 `RUNTIME_BUSY`
- [x] 产品代码无内置模型、Provider、模型 API Key、FastAPI/8000
- [x] Runtime V1 不自动打开旧 Library
- [x] MCP003 Codex Desktop/CLI read-only discovery/connection/mismatch scope
- [x] MCP004 candidate/Job/approval/idempotent confirm/reject/restore/diagnostic export
- [x] MCP initialize 不等待 Runtime；缺失/crash 时 stdio survival；一次有界 restart
- [x] 真实 Codex CLI diagnostic write 与 Viewer authenticated read model
- [x] `npm run release:mcp004`

MCP004 历史 manifest 仍保留 signing/reference/Geometry/GLB 的 BLOCKED/NOT_RUN；这些未被视为通过。

## 2. MVP Gate（MCP005–009）

### Reference / Codex

- [x] 真实 PNG/JPEG 原始字节通过 Codex CLI 进入 CAS，source/CAS SHA-256 相同（MCP005）
- [x] MIME/魔数、byte/pixel/dimension/frame/decode memory limits（MCP005）
- [x] symlink/path escape/目录/device/truncation/oversize/hash mismatch fail closed（MCP005）
- [x] DB/log/MCP/evidence 无用户名、绝对路径、原图副本（MCP005）
- [x] Desktop 附件不可验证时明确记录 `NOT_RUN / unavailable`（MCP005）；不伪造 Desktop PASS

### Typed Skills / Geometry

- [x] SubjectProfile/RepresentationPlan/AssemblyGraph/GeometryProgram/AppearanceProgram
- [x] 10 个 first-party declarative Skill 的 Schema/DAG/operator/validator/benchmark/LICENSE/NOTICE/SBOM/provenance
- [x] Bundle canonical hash/trust；unknown operator/cycle/budget/hash/license fail closed
- [x] 机器人真实多 Part mesh/GLB，不是图片平面/单盒/手工成品
- [x] finite/index/normal/degenerate/manifold/budget/source-map strict readback

### Appearance / Viewer / Quality

- [x] bounded UV/tangent/glTF metallic-roughness + typed emissive MaterialZone
- [x] 白外壳/黑机械/橙 emissive typed MaterialZone
- [ ] glTF Validator 0 errors（工具尚未接入；product-owned strict readback PASS）
- [x] Viewer 真实 GLB bytes/candidate hash 通过 authenticated IPC 读取
- [x] Viewer 关闭时 headless beauty/silhouette/normal/part-ID 仍成功（focused worker evidence）
- [ ] reference camera/轮廓/占框/关键比例/区域差异有实际值和 limitation
- [ ] Codex typed review 绑定 pass/region；用户评分有 evidence

### Transaction / Export

- [x] stable Part ID 一次局部修改（bounded recompile；通用 DAG reuse 未宣称）
- [x] reject 无版本；approve 只创建一个不可变子版本
- [x] restore 创建新版本，不改历史
- [x] CAS-backed `mvp-glb` export 绑定 confirmed version/artifact/quality/output hash
- [ ] restart 后真实 Codex Runtime/Viewer/export hash 一致（focused Runtime restart PASS；真实 Codex host restart/Viewer hash 仍 NOT_RUN）
- [x] 真实 Codex CLI 完成当前 MVP 主链：reference→compile→render→evaluate→confirm→version→CAS export（十二调用 receipt）
- [ ] 真实 Codex CLI 完成 reject→change→restore 的完整交互链（当前 focused/runtime evidence 有，真实 host 尚未运行）

全部勾选后只能声明“首个硬表面参考基准 MVP”，不能声明通用高质量或可公开安装。

## 3. 首个硬表面质量 Gate（MCP010A–F）

- [x] 010A：同 revision 用户级开发 App、raw stdio/CLI、用户重启后的真实 Codex capability/project/build hash
- [ ] 010B：in_progress；V2 Schema、真实 GLB/accessor/topology/source readback、损坏输入 fail closed、Worker structural 子门、历史 `bfa56ac…de9` receipt与当前 `d9c23b…ac0bd` Dev.app 的 ad-hoc/package/isolated/raw/real-Codex structural probes 和完整重启后的 live Desktop structural activation 已 PASS；其未确认 candidate 为 12 Parts/896 triangles/161104-byte GLB，Darwin OS 总内存硬门仍 NOT_RUN
- [ ] 010C：perspective/z-buffer 固定 renderer、九 AOV、silhouette/landmark/region metrics、typed review tools
- [ ] 010D：高细节 Operator/Skill 0.2；Manifold 仅在 adoption accepted 后启用
- [ ] 010E：first-party 离线 AssetPack、UV/tangent/PBR/texture、逐资产 license/SBOM/provenance、glTF Validator 0 errors
- [ ] 010F：Viewer compare/selection/isolate/explosion/a11y、真实 Codex change/confirm/restore/export/restart 同 hash、用户四项评分 ≥4/5
- [ ] 当前单图达到 `PARTIAL_VISIBLE_VIEW_PASS`
- [ ] front/back/left/right/rear-three-quarter 全身视图逐项通过后才记录 `HQ_360_PASS`；缺图时固定 `BLOCKED_REFERENCE_COVERAGE`

完成本节只能声明首个硬表面参考基准，不代表可外部分发或通用高质量。

## 4. 生产发布 Gate（MCP011–013）

### 功能与可靠性

- [ ] Job checkpoint/并发/cancel race/kill-9/disk full/GC/配额/性能
- [ ] 跨类别 Benchmark，展示最差类和失败样本
- [ ] 独立真人盲评

### 供应链与安全

- [ ] 所有 binary/Skill/asset dependency 固定 revision
- [ ] LICENSE/NOTICE、完整 transitive SBOM、provenance、最终分发签名
- [ ] Skill tamper/revocation/upgrade/rollback
- [ ] Worker sandbox、无任意脚本/网络/路径
- [ ] 无 secret/prompt/原图/绝对路径泄露
- [ ] 内容安全和资产授权

### macOS 与宿主

- [ ] Developer ID signing、hardened runtime、最小 entitlements
- [ ] notarization/stapling、`spctl`、clean install
- [ ] 安装器生成无 secret 的 Codex Desktop/CLI 配置
- [ ] signed package 上 Desktop + CLI 完整 write/attachment/Viewer E2E
- [ ] 离线启动、升级失败回滚、DB/CAS backup/restore
- [ ] Runtime/MCP/workers/Viewer/Skills 同合同/manifest/signature

IDE/其他 MCP Client 和 transport-specific official conformance 仍为 future optional，不是个人 MVP 或当前 P0 发布阻断。任何未勾选的生产必需项存在时，不可外部分发。
