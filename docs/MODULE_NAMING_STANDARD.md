# ForgeCAD Module Naming Standard

版本：`ForgeCADModuleNaming@1`

适用范围：Weapon Concept Pack 的未来武器概念、游戏资产、影视道具和非功能展示模型。命名表达视觉装配与资产追踪，不表达真实武器机构、工艺或制造能力。

## 1. 不可变 ID

| 对象 | 规则 | 示例 |
| --- | --- | --- |
| Pack | `pack_<name>_v<N>` | `pack_weapon_concept_v1` |
| Module | `module_<category>_<NN>` | `module_front_shell_02` |
| Asset | `asset_<category>_<NN>` | `asset_front_shell_02` |
| Connector | `connector_<owner>_<interface>` | `connector_front_02_core` |
| Slot | `<owner>.<interface>` | `front.core` |
| Connector type | `<purpose>_mount` | `shell_mount` |

`NN` 固定为 `01–99`。Module 的 category 必须是九个 P0 category 之一；Asset 的 category 和序号必须与 Module 完全相同。ID 一旦注册永不因模型细节、显示名称或文件重导而改变；真正不兼容的接口或语义变化创建新序号。

允许的 category：

```text
core_shell / front_shell / rear_shell / grip_shell
top_accessory / side_accessory / lower_structure
storage_visual / armor_panel
```

## 2. Blender 与 GLB 名称

以 `module_front_shell_02` 为例：

```text
Collection  MOD_module_front_shell_02
Object      GEO_module_front_shell_02_LOD0
Mesh        MESH_module_front_shell_02_LOD0
UV map      UV0
Materials   MAT_primary / MAT_secondary / MAT_accent
Empty       CON_connector_front_02_core
```

一个模块有多个 mesh-bearing object 时，仅允许追加两位序号：

```text
GEO_module_front_shell_02_LOD0_01
MESH_module_front_shell_02_LOD0_01
```

导出 GLB 后，机器门检查 `GEO_...` 和 `MESH_...`。Blender Empty 不作为运行时权威数据；它必须与 `module.json` Connector 同步，导出审阅后以 Manifest 为准。

## 3. 材质、UV 与 LOD

P0 材质使用跨模块稳定的语义槽：

```text
MAT_primary
MAT_secondary
MAT_accent
MAT_emissive
MAT_transparent
```

需要细分时可追加小写 snake_case，例如 `MAT_primary_polymer`，不得使用 `Material.001`、作者名、颜色值或模块 ID。GLB 实际 primitive 使用的材质集合必须与 `material_slots` 完全一致。

P0 只发布 `LOD0`，所有 primitive 必须提供 `TEXCOORD_0`；Blender UV map 固定命名 `UV0`。`LOD1/LOD2` 只有在运行时切换、质量门和导出合同同时实现后才能进入发布包。

## 4. Connector 规则

- 使用全小写 snake_case，至少包含 owner 与 interface 两段；
- ID 表达“谁的哪个视觉接口”，不写尺寸、公差、口径或真实机构名；
- 同一 Module 内 slot 唯一，整个 Pack 内 connector_id 唯一；
- 可替换模块保持相同 `slot + connector_type`；
- 轻微位置修正只更新 transform，不重命名 Connector；
- 破坏兼容性的接口变化创建新 Module/Connector ID，不覆盖已发布版本。

## 5. 设计者第一天操作

只做三件资产：

1. `module_core_shell_01`：冻结 230 mm 概念轮廓、三材质语义和 `core.front` 基准；
2. `module_front_shell_01`：建立第一种前部轮廓和 `front.core + shell_mount`；
3. `module_front_shell_02`：做明显不同的替换件，但保持同一 slot/type。

每个模块目录必须同时放入 `module.json`、`model.glb`、`thumbnail.png`、`LICENSE.txt`。完成后先运行：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/concept_module_pack.py \
  /absolute/path/to/your-pack
```

三模块阶段不要加 `--release`；缺少 category 会是 warning。通过后再启动 Agent、显式 `--import`，进入 `#/cad` 完成 front 01 → front 02 替换、Undo/Redo、保存与重启恢复。

仓库标准回归：

```bash
npm run assets:module-pack-gate
```

该门拒绝非法 Module/Asset/Connector/Material 名、GLB node/mesh 名、路径、哈希、许可证、UV、材质集合、bounds、triangle 和 release category 覆盖问题。
