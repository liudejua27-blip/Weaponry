# ForgeCAD Skill Package 标准

版本：2026-08-07
状态：目标设计；`FGC-MCP006` 起实施

## 1. 定义

ForgeCAD Skill 不是 prompt、说明文档或任意脚本。它是一份可签名、可静态审查、可 benchmark、可撤销的确定性能力包：

> 知识 + typed Schema + Recipe DAG + 受限 Operator + Validator + 材质/资产 + Benchmark + 许可证/SBOM + provenance + 签名

Codex 选择和组合 Skill；Rust Runtime 验证、展开和执行。Skill 不能调用模型、网络、shell、Python、JavaScript、文件路径、环境变量、密钥或数据库。

## 2. 目录结构

```text
packages/forgecad-skills/<skill-id>/<semver>/
  skill.yaml
  knowledge/
    overview.md
    constraints.md
    examples.md
  schemas/
    input.schema.json
    output.schema.json
    claims.schema.json
  recipes/
    default.recipe.json
  operators.lock
  validators/
    validator-set.json
  assets/
    index.json
    ... content-addressed payloads
  materials/
    index.json
  benchmarks/
    suite.yaml
    fixtures/
    expected/
  LICENSES/
  NOTICE
  sbom.spdx.json
  provenance.intoto.jsonl
  benchmark-receipt.json
  signature.bundle
```

`skill.yaml` 至少包含：ID、semver、contract range、publisher、description、input/output Schema hashes、Recipe hashes、Operator IDs/versions、Validator IDs、asset manifests、capability/permission/budget、supported representations/categories、known limitations、Benchmark threshold、license/provenance/SBOM/signature hashes 和撤销信息。

## 3. Recipe DAG

Recipe 是无环、有类型的声明式图。每个 edge 的 source/target 类型、单位、坐标系和 cardinality 必须兼容。Runtime 在执行前：

1. 验证 Bundle 和所有文件 hash；
2. 验证 DAG 无环、节点/边上限和确定性排序；
3. 将 Operator lock 解析为产品内置实现；
4. 计算最大 CPU、内存、三角、纹理、磁盘、Job 和递归预算；
5. 展开为 canonical execution plan；
6. 记录 plan hash、Skill hash、输入 hash和 Runtime 版本。

Operator 只有 manifest 中预注册的类型。P0 不允许 Bundle 携带可执行文件。未来第三方 WASI Operator 必须另立 ADR，增加能力沙箱、供应链和逃逸测试。

## 4. Validator 和质量声明

Validator 只能引用 Runtime 内置的检查器及参数范围，不能通过代码自定义“通过”。Benchmark receipt 绑定：Bundle hash、fixture licenses、Runtime/worker/renderer version、机器 profile、每项指标、失败类别和原始 evidence hashes。

Skill 安装成功不表示结果质量通过。每次候选仍需运行 Quality Compiler；Skill 的 Benchmark 只是“此版本在固定集合上的历史表现”。

## 5. 材质和资产

每个资产记录 source URL/ID、retrieved_at、SHA-256、作者、许可证 SPDX、license text hash、尺寸/单位、色彩空间、通道语义、允许用途、修改状态和 preview。没有明确许可证或 hash 的资产不得进入 first-party Bundle。

外部 CC0 资产也保留 provenance 和原始许可回执；“免费”不是许可证。资产不得在安装时从不受控 URL 下载，P0 由产品发布包或受签名离线 asset pack 提供。

## 6. 签名、SBOM 和撤销

- SPDX SBOM 覆盖代码、工具、模型权重（若未来存在）、字体、HDRI、纹理、示例和生成物依赖；
- provenance 使用 in-toto/SLSA 风格记录构建者、source revision、构建参数和 artifact hash；
- 签名使用产品接受的 keyless/离线根和 transparency evidence；
- Runtime 先验证签名和撤销，再验证合同/预算/许可证/Benchmark；
- 签名失效、publisher 撤销、SBOM 漂移或 Benchmark 不兼容时 fail closed；
- 已确认版本继续保留使用过的 Bundle hash 和收据，不因卸载丢失历史可读性。

## 7. 首批 first-party Skill 清单

| 顺序 | Skill ID | 责任 |
|---:|---|---|
| 1 | `reference-intake` | 图片 admission、视图/授权/质量诊断 |
| 2 | `subject-profile-author` | 类别开放语义与不确定项 |
| 3 | `silhouette-proportion-blockout` | 多视图轮廓和比例 blockout |
| 4 | `semantic-part-assembly` | Assembly/Part/MaterialZone 稳定层级 |
| 5 | `geometry-detail-author` | 中观/局部 typed detail |
| 6 | `mesh-integrity` | 几何硬门与 readback |
| 7 | `uv-layout` | unwrap、island、padding、density |
| 8 | `tangent-normal` | tangent space 和 normal validation |
| 9 | `pbr-material-author` | Principled PBR graph |
| 10 | `material-zone-binding` | 语义区域与材质绑定 |
| 11 | `texture-procedural` | 受限程序化纹理 |
| 12 | `texture-bake` | 多通道 bake |
| 13 | `texture-compress-delivery` | KTX/Basis 等交付优化 |
| 14 | `render-evidence` | 固定视图和 AOV |
| 15 | `reference-compare` | 轮廓、比例、区域差异 |
| 16 | `codex-visual-review` | typed 视觉评审 rubric |
| 17 | `typed-visual-repair` | evidence-bound 一次局部修复 |
| 18 | `glb-admission-export` | validator、readback、export manifest |
| 19 | `local-edit` | stable-ID SemanticChangeSet |
| 20 | `exploded-view` | ExplodedViewPlan 与可读性检查 |
| 21 | `asset-provenance` | license/lineage/receipt |
| 22 | `skill-benchmark-and-sign` | Bundle benchmark、SBOM、签名 |

前六个先建立可编辑主体和硬证据，随后建设 UV/PBR/渲染比较；不能先堆大量材质包掩盖几何和轮廓不合格。

## 8. 安装生命周期

`discover → quarantine → hash → signature/revocation → SBOM/license → Schema/DAG/operator/budget → adversarial tests → benchmark → staged enable → audit → disable/revoke`。

第三方 GitHub 仓库不能直接变成 Skill。必须按 [外部项目采用清单](EXTERNAL_PROJECT_ADOPTION.md) 分离算法、代码、资产、工作流思想和许可证，再封装为本标准的受限 Bundle。
