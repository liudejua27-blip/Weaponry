# ForgeCAD Skill Package 标准

版本：2026-08-11
状态：历史十个 first-party Bundle 保持 `0.1.0`；MCP010D 已新增并激活受限 `hard-surface-detail@0.2.0`，`primitive-blockout@0.2.0` 继续 active；MCP010E 的 `uv-pbr@0.2.0`、`render-evidence@0.2.0`、`reference-compare@0.2.0` 与 `forgecad-hard-surface-robot@1.0.0` 已通过 source-focused 离线 Gate；xatlas/Validator、分发签名和第三方安装仍属后续 MCP012/013

## 1. 定义

ForgeCAD Skill 不是 prompt、说明文档或任意脚本。它是一份可签名、可静态审查、可 benchmark、可撤销的确定性能力包：

> 知识 + typed Schema + Recipe DAG + 受限 Operator + Validator + 材质/资产 + Benchmark + 许可证/SBOM + provenance + 签名

Codex 选择和组合 Skill；Rust Runtime 验证、展开和执行。Skill 不能调用模型、网络、shell、Python、JavaScript、文件路径、环境变量、密钥或数据库。

## 2. 目录结构

```text
packages/forgecad-skills/bundles/<skill-id>/<semver>/
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
  trust/
    manifest.sha256
  signature.bundle  # MCP012/013 distribution profile required
```

`skill.yaml` 至少包含：ID、semver、contract range、publisher、description、input/output Schema hashes、Recipe hashes、Operator IDs/versions、Validator IDs、asset manifests、capability/permission/budget、supported representations/categories、known limitations、Benchmark threshold、license/provenance/SBOM hashes。MVP 另含 development trust-root ID；distribution profile 才强制 signature hash 和撤销信息。

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

MCP010E 的 `forgecad-hard-surface-robot@1.0.0` 是独立 first-party AssetPack，不是 Skill，也不携带 Operator。Codex 只可在实施期把计划点名的 CC0 文件一次性下载到 adoption cache；逐资产 hash/license/SBOM/provenance 和派生 Recipe 通过后，包随开发 App Resources 离线提供并由 Runtime 校验后写 CAS。Runtime、Viewer 和安装器不得联网或调用素材 API。通用 pack publisher、安装、禁用、升级和撤销仍属于 MCP012。

## 6. MVP trust profile 与正式签名

MVP 不建设第三方插件市场，也不让 distribution signing 阻塞 3D vertical slice。仓库内 first-party Bundle 使用：canonical manifest hash、所有文件 hash、operator allowlist、LICENSE/NOTICE、SPDX SBOM、provenance 和仓库开发 trust root。任何 hash 漂移都 fail closed。

以下延后到 MCP012/013：第三方 publisher、keyless transparency、在线撤销、用户安装和自动更新。但历史版本仍从第一天记录 Bundle hash，迁移到分发签名时不能改写 lineage。

正式发布要求：

- SPDX SBOM 覆盖代码、工具、模型权重（若未来存在）、字体、HDRI、纹理、示例和生成物依赖；
- provenance 使用 in-toto/SLSA 风格记录构建者、source revision、构建参数和 artifact hash；
- 签名使用产品接受的 keyless/离线根和 transparency evidence；
- Runtime 先验证签名和撤销，再验证合同/预算/许可证/Benchmark；
- 签名失效、publisher 撤销、SBOM 漂移或 Benchmark 不兼容时 fail closed；
- 已确认版本继续保留使用过的 Bundle hash 和收据，不因卸载丢失历史可读性。

## 7. MVP first-party Skill 清单

| 顺序 | Skill ID | 责任 |
|---:|---|---|
| 1 | `reference-intake` | 引用已 admission 的图片、视图/授权/可见性约束 |
| 2 | `subject-profile` | typed 类别、比例、材质线索和不确定项 |
| 3 | `semantic-assembly` | Assembly/Part/MaterialZone 稳定层级 |
| 4 | `silhouette-blockout` | 当前用 bounded box/cylinder/sphere/transform 做轮廓 blockout；profile/sweep 仅为声明式后续目标 |
| 5 | `hard-surface-detail@0.2.0` | MCP010D 已实现 profile/loft/revolve/sweep/transform/mirror/array/panel/vent/joint/part-output；boolean/Manifold 保持 unavailable，Bundle 不携带 executable |
| 6 | `mesh-integrity` | 几何硬门、source map 和 GLB readback |
| 7 | `uv-pbr` | UV、tangent、metallic-roughness、normal/AO/emissive |
| 8 | `render-evidence` | 固定相机 beauty/silhouette/normal/part-ID |
| 9 | `reference-compare` | 占框、轮廓、关键比例、区域差异、typed review |
| 10 | `local-edit-and-export` | stable-ID change、GLB validator、version/export manifest |

这 10 个是组合能力，不要求 10 个可执行插件。执行代码仍是产品预注册 Rust Operator；Skill 只声明知识、Schema、Recipe、validator 和预算。爆炸图、纹理压缩、第三方安装和完整签名在 post-MVP 拆分扩展。

### 7.1 MCP010 目标版本（当前 unavailable）

| Task | 目标 Bundle | active 前置条件 |
|---|---|---|
| MCP010D | `hard-surface-detail@0.2.0`（primitive-blockout@0.2.0 active） | V2 Schema、真实 operator consumer、validator、benchmark、receipt 和同 cohort packaged raw structural probe 已通过；Manifold/视觉子门 NOT_RUN |
| MCP010E | `uv-pbr@0.2.0`、`render-evidence@0.2.0`、`reference-compare@0.2.0` | AssetPack、UV/tangent/PBR/render/metric producer 和逐资产 provenance |

其他 Bundle 保持 `0.1.0`。010A 文档重排不能修改 registry 版本或 active 状态；任何缺失 Operator/Asset/Benchmark 必须返回 partial/unavailable 和 `missing_operator_ids`。

MVP 的参考图工作流借鉴 `img2threejs` 的 staged-pass/detail-inventory/per-region-confidence 纪律，并把它重写为上述 first-party typed metadata；`img2css` 只作为离线颜色/区域预览的设计参考。两者的脚本、Three.js 工厂、CSS/base64 和网页运行时均不进入 Bundle、Worker 或 Runtime 真值。

## 8. 安装生命周期

MVP：`discover → hash/development trust → SBOM/license → Schema/DAG/operator/budget → adversarial tests → benchmark → staged enable → audit → disable`。

正式分发：`discover → quarantine → hash → signature/revocation → SBOM/license → Schema/DAG/operator/budget → adversarial tests → benchmark → staged enable → audit → disable/revoke`。

第三方 GitHub 仓库不能直接变成 Skill。必须按 [外部项目采用清单](EXTERNAL_PROJECT_ADOPTION.md) 分离算法、代码、资产、工作流思想和许可证，再封装为本标准的受限 Bundle。
