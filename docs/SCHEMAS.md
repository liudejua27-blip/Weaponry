# ForgeCAD Runtime Schema 规范

版本：2026-08-09
状态：MCP005–MCP009 functional core 已落地；MCP006 历史 receipt 为 44 个 JSON Schema；当前 MCP010B 源码为 52 个 Schema，并已通过 source-focused Gate、历史 `bfa56ac…de9` package Gate 和当前 `d9c23b…ac0bd` Dev.app structural package/live Desktop activation Gate

## 1. 唯一来源

新合同源位于 `packages/forgecad-contracts/schemas/**`。MCP003 已验证首批 15 个 JSON Schema；MCP004 增加审批、候选、restore 和 diagnostic export records；MCP005 增加 reference admission/get records；MCP006–009 已落地 `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1`、`RecipePlan@1`、`ArtifactReadback@1`、`RenderSet@1`、`QualityReport@1`、`ChangePrepareResult@1`、GLB export profiles 和 Skill manifest/list/get/receipt/eval records，共 44 个历史 JSON Schema。MCP010B 当前源码另增加 8 个受限 V2/evidence Schema，故 manifest 当前为 52；这不改写 44-contract 历史 receipt。全部 Schema 均须可解析、带 draft/id、contract manifest 为 `forgecad-runtime-contracts@1` 且声明 `model_calls=false`，manifest 与目录无漂移。Rust records 由 `forgecad-contracts` 维护；完整生成器、TypeScript 生成和额外 transport/未来宿主 conformance 仍未完成。旧 Concept/Weapon/Provider/Agent Schema 已删除。

## 2. 首批 Schema

### Runtime/MCP

`RuntimeCapabilities@1`、`RuntimeTool@1`、`RuntimeProject@1`、`RuntimeSnapshot@1`、`RuntimeJob@1`、`RuntimeError@1`、`RuntimeJobEvent@1`、`RuntimeResourceContents@1`、`Selection@1` 已落地；MCP annotations/resources Schema snapshot 在 `docs/evidence/mcp003/`。

### Project/Version

`Project@1`、`ActiveDesignSnapshot@1`、`Candidate@1`、`DesignAssetVersion@1`、`CasObject@1`、`AuditEvent@1`、`SemanticChangeSet@1`、`ApprovalReceipt@1`、`ExportManifest@1`。

### Reference/Design/Geometry/Appearance（分阶段落地）

MCP005 已落地 `ReferenceEvidence@1` 和四个 reference import/get request/result 合同；MCP006 已落地 `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1` 和 `RecipePlan@1`；`VisualProgram@1`、`MaterialGraph@1`、`TextureSet@1`、`UvLayout@1`、`BakeRecipe@1`、`ExplodedViewPlan@1` 仍由 MCP007–010 分阶段落地。

### Evidence/Skill（MCP006–009 已落地合同，执行证据仍分层）

MCP006 已加入 `ArtifactReadback@1`、`RenderSet@1`、`QualityReport@1`、`SkillBundleManifest@1`、`SkillListResult@1`、`SkillGetResult@1`、`SkillExecutionReceipt@1`、`SkillEvalReport@1`，MCP009 加入 `ChangePrepareResult@1`、GLB export profile 和 limited quality projection；`RecipePlan@1` 的单位/坐标/确定性顺序/max_edges 是显式合同。像素级 `VisualReviewReport@1`、landmark/region metric 和完整生产 export 仍是后续质量/发布工作，不得用空 Schema 代替。

## 2.1 MVP 落地顺序

| Task | 新增合同 |
|---|---|
| MCP005 | `ReferenceEvidence@1`、`ReferenceImportRequest/Result@1`、`ReferenceGetResult@1`（已完成） |
| MCP006 | `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1`、`RecipePlan@1`、MVP Skill manifest/receipt |
| MCP007 | `GeometryProgram@1`、`GeometryPrepareResult@1`、`ArtifactReadback@1`、Part/source map、worker compile request/result |
| MCP008 | `AppearancePrepareResult@1`、`RenderSet@1`、GLB UV/tangent/PBR readback |
| MCP009 | `QualityReport@1`、`ChangePrepareResult@1`、`VersionDiff@1` projection、`ExportManifest@1` `mvp-glb` profile |
| MCP010B | `GeometryProgram@2`、`OperatorCatalog@1`、`GeometryProgramHashRequest@1`、`GeometryProgramHashResult@1`、`ArtifactReadback@2`、`GeometryPrepareResult@2`、`GeometryQualityReport@2`、`GeometryCandidateEvidence@1` |

不能一次加入全部空 Schema 再宣称能力存在；每项必须与 validator、negative tests 和实际 producer/consumer 同任务落地。

## 2.2 MCP010 合同（当前 source 52；历史 44 receipt 不改写）

| Task | 目标合同 | 激活条件 |
|---|---|---|
| MCP010B | `GeometryProgram@2`、`OperatorCatalog@1`、`GeometryProgramHashRequest@1`、`GeometryProgramHashResult@1`、`ArtifactReadback@2`、`GeometryPrepareResult@2`、`GeometryQualityReport@2`、`GeometryCandidateEvidence@1` | 当前源码的 producer/consumer、真实 GLB JSON/BIN/accessor readback、closed GLB profile、V2 restore hardening 和损坏输入负向 Gate 已通过；当前 `d9c23b…ac0bd` Dev.app 已通过 ad-hoc/package、隔离/raw、真实 Codex CLI structural 和完整重启后的 live Desktop structural Gate |
| MCP010C | `ReferenceViewSpec@1`、`CameraCalibration@1`、`RenderSet@2`、`ReferenceComparisonReport@1`、`VisualReviewReport@1`、`HumanVisualReviewReceipt@1`、`QualityReport@2` | perspective/z-buffer renderer、九 AOV、metric/review persistence、tool E2E |
| MCP010E | `MaterialPackManifest@1`、`MaterialDefinition@1`、`TextureSet@1`、`TextureBuildReceipt@1`、`AppearanceProgram@2` | AssetPack/UV/tangent/PBR producer、逐资产 provenance、GLB readback |

工具 request/result Schema 随各自 producer 同任务增加；实际 manifest 数量只能从目录和 contract manifest 计算，不能把上表简单相加后提前写成当前总数。`@1` 历史版本继续只读，破坏性变化不得回填旧对象。

`GeometryPrepareResult@2` 是闭合的短生命周期 MCP 返回，只包含 candidate、job、operator catalog 与 `ArtifactReadback@2`；它不应额外暴露持久 evidence。`GeometryQualityReport@2` 只表示 strict hard gate 已通过的 quality CAS receipt，失败走 typed rejection 而不是伪造 `hard_gate_passed=false` 的该 Schema。`GeometryCandidateEvidence@1` 是 Runtime/Store 的 candidate-bound durable provenance：它绑定 program、artifact、readback、quality、catalog/readback-config 和可选 reference hash，并由 confirm/restore reread 使用。当前 source-focused PASS 不等于新的安装包、Desktop live、PBR、reference similarity、human review 或 360°证据。

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
