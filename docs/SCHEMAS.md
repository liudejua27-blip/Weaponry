# ForgeCAD Runtime Schema 规范

版本：2026-08-08
状态：部分实现；MCP003 已落地首批 Runtime/Project/Candidate/Version/Snapshot/Job/Event/Audit/CAS、resource/selection Schema 和 Rust records

## 1. 唯一来源

新合同源位于 `packages/forgecad-contracts/schemas/**`。MCP003 已验证 15 个 JSON Schema 可解析、带 draft/id、contract manifest 为 `forgecad-runtime-contracts@1` 且声明 `model_calls=false`，manifest 与目录无漂移。Rust records 由 `forgecad-contracts` 维护；完整生成器、TypeScript 生成和跨宿主 conformance 仍未完成。旧 Concept/Weapon/Provider/Agent Schema 已删除。

## 2. 首批 Schema

### Runtime/MCP

`RuntimeCapabilities@1`、`RuntimeTool@1`、`RuntimeProject@1`、`RuntimeSnapshot@1`、`RuntimeJob@1`、`RuntimeError@1`、`RuntimeJobEvent@1`、`RuntimeResourceContents@1`、`Selection@1` 已落地；MCP annotations/resources Schema snapshot 在 `docs/evidence/mcp003/`。

### Project/Version

`Project@1`、`ActiveDesignSnapshot@1`、`Candidate@1`、`DesignAssetVersion@1`、`CasObject@1`、`AuditEvent@1`、`SemanticChangeSet@1`、`ApprovalReceipt@1`、`ExportManifest@1`。

### Design/Geometry/Appearance

`ReferenceEvidence@1`、`SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`VisualProgram@1`、`AppearanceProgram@1`、`MaterialGraph@1`、`TextureSet@1`、`UvLayout@1`、`BakeRecipe@1`、`ExplodedViewPlan@1`。

### Evidence/Skill

`ArtifactReadback@1`、`RuntimeJobEvent@1`、`RuntimeError@1`、`RenderRecipe@1`、`RenderSet@1`、`QualityReport@1`、`VisualReviewReport@1`、`SkillBundleManifest@1`、`SkillExecutionPlan@1`、`SkillExecutionReceipt@1`、`SkillEvalReport@1`。

## 3. 通用字段

每个持久/跨进程对象必须有：

```text
schema_version
id
project_id (适用时)
created_at
canonical_sha256
parent_refs / lineage
```

永久写请求增加 `base_version_id`、`prepared_object_id`、`prepared_object_sha256`、`approval_receipt_id`、`idempotency_key`。所有 ID opaque 且不能含用户名/路径。

## 4. 规范化与 hash

- UTF-8、明确 key 排序、数值/单位 canonicalization、禁止 NaN/Infinity/负零歧义；
- 时间使用 UTC RFC3339，hash 使用 SHA-256；
- 二进制只保存 CAS hash/MIME/size，不内联；
- 缺失与 `null` 语义不同，Schema 必须明确；
- unknown property 默认拒绝；
- enum 扩展按版本处理，不能静默映射；
- 任何 renderer/worker/platform 影响结果的配置进入输入 hash。

## 5. 几何和单位

默认米、右手坐标系、Y-up（最终 GLB lowering 明确转换）。每个长度/角度/颜色/纹理字段声明单位、范围和精度。Geometry Operator 只允许命名 typed 参数，不接受 JSON pointer、代码、URL 或路径。

## 6. 版本策略

同一 `@1` 只允许向后兼容的 optional 增加，且 validator/consumer 已知默认；破坏性变化创建 `@2` 和显式迁移。Runtime/MCP/Viewer/Worker 协商 contract set digest；不兼容时写路径关闭。

## 7. 负向 Gate

每个 Schema 至少测试 unknown fields、超长字符串/数组、深嵌套、非有限数、错误单位/ID/hash、路径/URL/secret-like 字段、循环 DAG、预算溢出、stale base、重复 key 和版本不兼容。
