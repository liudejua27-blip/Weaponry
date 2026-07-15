# ForgeCAD 资产作者手册

版本：2026-07-13
适用对象：组件、美术资产、缩略图和领域 Pack 作者

## 1. 当前资产类型

ForgeCAD 同时存在两类资产：

1. `AgentComponent`：从 AgentAssetVersion 的部件保存，服务项目内轻量替换；
2. `ModuleAssetManifest@1`：带 GLB、缩略图、Connector、目录元数据和审阅状态的正式模块资产。

两者不能互相冒充。AgentComponent 是当前项目中的可复用快照；正式 Module Asset 需要独立文件、稳定 ID、几何事实、来源声明和审阅记录。

项目内 `AgentComponent` 的替换候选只读取启用状态、同领域、同 role、来源 Agent 资产的最新质量和目标连接保留；它没有正式资产的作者/独立审阅元数据，不能显示为已审资产。来源未检查或检查失败时不可用于替换。

## 2. 首批领域包

- `future_weapon_prop`：虚构未来武器概念道具；
- `vehicle_concept`：汽车与地面载具；
- `aircraft_concept`：飞机与航空器；
- `robotic_arm_concept`：机械臂与机器人机构。

新的汽车、飞机和机械臂资产不得套用 Weapon reference pack 的九类旧 category。角色、Connector、Joint 和材料要求见 [DOMAIN_PACKS.md](DOMAIN_PACKS.md) 与 [MATERIAL_SYSTEM.md](MATERIAL_SYSTEM.md)。

## 3. 最小交付物

每个正式模块至少包含：

- 自包含 glTF 2.0 GLB；
- PNG 缩略图；
- `ModuleAssetManifest@1` 几何事实；
- `ModuleAssetCatalogMetadata` 显示名、描述、标签和目录；
- `origin_claim` 和 `creator_name`；
- Connector/Joint/Material Zone 定义；
- 三角预算、bounds、法线和读取验证；
- 独立审阅记录。

详细制作规范见 [MODULE_ASSET_GUIDE.md](MODULE_ASSET_GUIDE.md) 和 [MODULE_NAMING_STANDARD.md](MODULE_NAMING_STANDARD.md)。

## 4. 原创声明与审阅

本人创作使用：

```text
origin_claim=self_declared_original
```

这表示作者声明原创，不等于独立审阅通过。正式状态机：

```text
draft → pending_review → approved
                    ↘ restricted
```

批准要求：

- reviewer 不是 creator；
- reviewer 实际检查 GLB、缩略图、源文件和元数据；
- 所有 checklist 为 true；
- 五项视觉评分均至少 4；
- 记录 `reviewer_name`、`reviewed_at` 和审阅说明；
- 验证命令通过。

当前独立审阅人“刘邦”已指派，但只有完成原始审阅文件和 attestation 后才能显示 `approved`。

## 5. 正式审阅流程

生成草稿：

```bash
npm run assets:formal-review-draft -- \
  --pack-root <pack-root> \
  --source-root <source-root> \
  --output <review.json> \
  --scope release_10_12
```

生成交接材料：

```bash
npm run assets:formal-review-handoff -- \
  --pack-root <pack-root> \
  --source-root <source-root> \
  --review <review.json> \
  --output <handoff.md>
```

审阅人完成原始 review 后验证：

```bash
npm run assets:formal-review-validate -- \
  --pack-root <pack-root> \
  --source-root <source-root> \
  --review <review.json>
```

不要为了通过验证而删除审阅字段、降低评分或把作者改写成审阅人。

## 6. 资产晋级条件

只有同时满足以下条件才进入正式目录：

- manifest、GLB、缩略图和源文件 hash 一致；
- 非占位几何和最小三角密度通过；
- Connector 稳定且组合验证通过；
- 原创/第三方来源声明完整；
- 独立 reviewer 批准；
- 质量和 DCC roundtrip 通过；
- 组件库 UI 读取真实元数据，不硬编码标签。

Blender re-export 或机器 smoke 通过，只能证明技术链可运行，不能替代人工最终美术审阅。

## 7. 安全边界

未来武器资产只用于虚构游戏美术、影视道具和非功能展示。不得在资产文档中加入现实制造图、内部功能机构、弹药、承压结构、材料配方或加工步骤。汽车、飞机和机械臂资产也不得宣称道路安全、适航、结构强度或控制认证。
