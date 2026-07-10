# Weapon Concept Module Pack 资产制作规范

状态：`ModulePackManifest@1`、校验 CLI 和批量注册已实现；正式 8–12 个高质量 GLB 尚需按本文制作。

仓库已提供 `assets/module-packs/weapon-concept-v1-reference`：10 个确定性程序化参考 GLB，用于工作台、Connector、性能和导出闭环。它包含 UV0、normal、三材质、缩略图与许可证，但不是人工 Blender 最终美术。

本文只描述未来武器概念、游戏资产、影视道具和非功能展示模型的视觉资产管线。模块可以有精细比例、硬表面细节和明确接口，但不包含真实工作机构、弹道、承压或制造就绪结论。

## 1. 从哪里开始

第一包固定为 `pack_weapon_concept_v1`，第一项目固定为“寒地巡逻 S1”。不要一开始制作完整模型；先完成下列 9 个 LOD0 模块并通过机器校验：

| 顺序 | category | 建议 module_id | 目的 |
| --- | --- | --- | --- |
| 1 | `core_shell` | `module_core_shell_01` | 冻结整体比例与主要基准 |
| 2 | `front_shell` | `module_front_shell_01` | 跑通核心到前部替换 |
| 3 | `front_shell` | `module_front_shell_02` | 证明同 slot/type 可替换 |
| 4 | `rear_shell` | `module_rear_shell_01` | 完成后部轮廓 |
| 5 | `grip_shell` | `module_grip_shell_01` | 验证握持角和主轮廓 |
| 6 | `top_accessory` | `module_top_accessory_01` | 建立顶部视觉层次 |
| 7 | `side_accessory` | `module_side_accessory_01` | 验证侧向 Connector |
| 8 | `lower_structure` | `module_lower_structure_01` | 验证下部挂接 |
| 9 | `storage_visual` 或 `armor_panel` | `module_storage_visual_01` | 完成非功能性视觉模块 |

第二轮再补齐缺少的第九类和 grip/side/armor 变化件，使正式包保持 8–12 个模块。release 门要求九类都出现，因此实际首包建议 10–12 个。

## 2. 文件布局

每个可发布包必须使用下列布局，不允许自由改名：

```text
weapon-concept-v1/
├── pack.json
├── LICENSES/
│   └── PACK.txt
└── modules/
    └── module_core_shell_01/
        ├── module.json
        ├── model.glb
        ├── thumbnail.png
        └── LICENSE.txt
```

- `pack.json`：`ModulePackManifest@1`，声明坐标、用途、许可证和模块索引；
- `module.json`：现有 `ModuleAssetManifest@1`；
- `model.glb`：唯一权威 LOD0 视觉资产；
- `thumbnail.png`：512×512 PNG；
- `LICENSE.txt`：模块级来源和授权文本，不能只在聊天或文件名中说明；
- 所有路径必须是包根目录内的 POSIX 相对路径，不能含 `..`、绝对路径、URL、盘符或反斜线。

模板位于：

- `docs/examples/module-pack/pack.template.json`
- `docs/examples/module-pack/module.template.json`

## 3. Blender 场景约定

### 3.1 单位与坐标

Blender 源文件：

```text
Unit System: Metric
Unit Scale: 1.0
Length display: Millimeters
1 Blender Unit = 1 meter
230 mm = 0.230 Blender Unit
Source up: +Z
Source forward: -Y
```

GLB 导出后的合同：

```text
right-handed
up_axis: +Y
forward_axis: -Z
manifest bounds_mm = GLB POSITION bounds × 1000
```

工作台合同：

```text
ModuleGraph node.position: mm
Connector transform.position: mm
rotation: rad, Euler XYZ
scale: dimensionless
```

Blender glTF 导出器负责源坐标到 glTF 坐标的转换。不要再在导出后手工旋转 GLB。

### 3.2 原点与 Transform

- `core_shell` 原点是整套概念模型的全局设计基准；
- 其他模块原点放在其主要装配 Connector 的中心；
- 物体进入导出 Collection 前执行 `Ctrl+A → Rotation & Scale`，并确保 Mesh node 的 translation/rotation/scale 为 identity；
- Blender 内用于建模的镜像修改器必须在最终 LOD0 上应用；运行时左右镜像由 `ModuleGraphNode.mirror_axis` 与 `set_mirror` ChangeSet 表达，不能把负 scale 写入 Transform；
- 原点不是“真实机械接口”，只是视觉模块的稳定装配 datum。

## 4. Blender 命名

完整稳定 ID、DCC 对象、材质、UV 与 LOD 规则见 [Module Naming Standard](MODULE_NAMING_STANDARD.md)。以下是首包最小集合；`assets:module-pack-gate` 会自动拒绝偏离规则的发布包。

```text
Collection: MOD_module_core_shell_01
Mesh:       GEO_module_core_shell_01_LOD0
Material:   MAT_primary / MAT_secondary / MAT_accent
UV map:     UV0
Empty:      CON_connector_core_front
```

要求：

- 一个 `module_id` 对应一个导出 Collection；
- P0 只注册 `LOD0`；`LOD1/LOD2` 已在包合同预留，但当前导入门会拒绝，避免把未实现的多 LOD 行为伪装成可用；
- GLB 中实际被 primitive 使用的 material 名称集合必须与 `material_slots` 完全一致；
- 每个 primitive 必须有 `POSITION` 和 `TEXCOORD_0`；
- material、asset、module 和 connector ID 在整个包内唯一。

## 5. Connector 标注

Connector 是视觉装配合同，不是功能性机械参数。每个 Blender Empty 对应 `module.json` 中一个 Connector：

```json
{
  "connector_id": "connector_core_front",
  "slot": "core.front",
  "connector_type": "shell_mount",
  "transform": {
    "position": [92, 0, -4],
    "rotation": [0, 0, 0],
    "scale": [1, 1, 1]
  },
  "scale_range": [0.95, 1.05],
  "exclusive": true
}
```

- `position` 使用模块局部坐标，单位为毫米；工作台加载 GLB 时将标准米制几何换算为毫米；
- `rotation` 使用当前 `Transform@1` 的欧拉角约定；P0 首包优先保持零旋转，减少解释差异；
- 可替换模块必须提供相同 `slot + connector_type`；系统据此重映射 edge；
- 替换后服务端以 root 建立确定性父子树：非 root 替换固定父节点并重定位本节点及后代，root 替换固定 root 并重定位全部子树；
- 额外循环边必须在 0.1 mm / 0.1° 内同时满足，否则 preview 返回 Connector snap conflict；
- 可运行时镜像的模块应把主要装配 Connector 放在镜像平面上；服务端会镜像 Connector 局部位置但保留其 Euler 旋转 frame，离开镜像平面的 Connector 可能触发节点/后代重定位；
- 镜像或父模块替换不得迫使 locked 后代移动，否则 preview 失败；
- `connector_id` 永久稳定，模型微调不能顺手改 ID；
- 一个模块内 `slot` 唯一；包内 `connector_id` 唯一；
- 第一条替换链固定验证 `core.front ↔ front.core`，再扩展 top/side/grip。

## 6. 几何、UV、材质和缩略图门

正式 LOD0 的每次提交必须满足：

- GLB 2.0 binary envelope 完整，声明长度与文件长度一致；
- 至少一个 Mesh，primitive 使用 TRIANGLES；
- 每个 primitive 有 UV0；
- GLB accessor 的 triangle count 与 `triangle_count` 一致；
- POSITION accessor 提供 min/max，换算毫米后与 `bounds_mm` 在 1% 或 0.5 mm 容差内；
- Mesh node 没有未应用 translation、rotation、scale；
- 材质使用 PBR-safe 参数，不把贴图绝对路径带入 GLB；
- 缩略图 512×512、透明或统一深色背景、相同焦距和三分之四视角；
- GLB SHA-256 写入 Manifest，任何重新导出都必须更新哈希；
- `LICENSE.txt` 和包许可证存在且为非空 UTF-8 文本。

当前门不会声称已检查非流形、法线、穿插、贴图像素内容或 GPU 性能；这些属于 R5 Mesh/Assembly 检查和 R3 压力测试。

## 7. 校验与导入

重新生成或检查仓库参考包：

```bash
npm run assets:reference-pack:generate
npm run assets:reference-pack:check
```

默认命令只读，不写数据库：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/concept_module_pack.py \
  "$PWD/assets/module-packs/weapon-concept-v1-reference" --release
```

通过后会输出 `mode: dry-run`。修复全部错误后，启动本地 Agent，再显式导入：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/concept_module_pack.py \
  "$PWD/assets/module-packs/weapon-concept-v1-reference" \
  --release \
  --api-base-url http://127.0.0.1:8000 \
  --import
```

导入前会完成整包校验；注册请求使用内容派生的稳定幂等键。对同一包重复执行不会复制模块。模块资产进入内容寻址对象存储，原 GLB 和 Manifest 不覆盖。

专项回归：

```bash
npm run assets:module-pack-gate
```

它用 9 个带真实 triangle/UV/material 的最小 GLB 验证 dry-run、release 覆盖、批量导入、幂等重放、重启恢复，以及哈希、路径、许可证、Module/Asset/Connector/Material 命名和 pack_id 负向用例。这些 fixture 只证明工具链，不代表正式资产质量。

## 8. 设计者的第一个可验收任务

先只制作 `module_core_shell_01` 和两个兼容的 `front_shell`：

1. 冻结寒地巡逻 S1 的 230 mm 总体轮廓参考；
2. core 只做外壳、大倒角、主色/辅色/强调色三个材质槽；
3. 两个 front 共享 `front.core + shell_mount`，轮廓明显不同；
4. 分别导出 GLB、生成 512×512 缩略图、填写 Manifest 和许可证；
5. 不加 `--release` 先跑 3 模块 dry-run；缺少类别只会报告 warning；
6. 导入本地 Agent，在 `#/cad` 中完成 front 01 → front 02 替换并创建子版本；
7. 这条链稳定后再扩到正式 10–12 模块。

第一阶段的“细致、精密”体现在比例、命名、稳定 ID、材质/UV、Connector、哈希、版本和可回退，而不是把概念 GLB 宣称为可制造武器 CAD。
