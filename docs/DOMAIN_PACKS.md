# ForgeCAD 首批领域包设计

版本：2026-07-15
状态：G2 通用合同与四个最小本地 registry manifest 已落地；G3 轻量 ShapeProgram worker、G5 分件候选/基础 mount Connector/机械臂 revolute Joint 和 G6 AgentAssetVersion 已生成四领域 48 个确定性 blockout 变体/候选图并可进入基础编辑；AgentComponent 注册/同角色替换、声明式 Connector 对齐和受限 GLB 导出已接入；G6.5 可安全导入任意领域的自包含 GLB 作为参考模型；前端变体目录、碰撞/运动学、外部 GLB 自动重建/深度分件和正式资产目录仍待后续。

## 1. 定位

ForgeCAD Core 负责 Agent、ShapeProgram、分层装配、版本、材质、检查、渲染和导出。领域包只提供某类机械概念的语义，不复制工作台或创建独立 Mode。

首批领域包：

```text
future_weapon_prop
vehicle_concept
aircraft_concept
robotic_arm_concept
```

用户不需要选择领域包。Agent 根据 Brief 推断，无法判断时用一句普通问题确认。`DomainInferenceResult@1` 和四领域关键词/同义词 fixture 规定结果只能是 `recognized`、`ambiguous` 或 `unsupported`。D002 在新 Turn 路径接入该合同；D003 已将后两态变成一个等待澄清的 Turn/Item 和四个面向用户的选项。该分支不会回退到未来武器包，也不会写入 Plan、Blockout、版本或资产。

## 2. DomainPackManifest@1

当前机器可读字段：

```text
schema_version / pack_id / domain
display_name / description / non_functional_only
templates[] / connector_types[] / joint_types[]
material_preset_ids[] / quality_profile_id / export_profile_id
```

领域包不得包含可执行代码、绝对路径或 Provider 密钥。模板引用已注册的 ShapeProgram fixture、模块和材质预设；所有 ID 必须经过 registry 校验。

## 3. 通用装配角色

所有领域包从以下通用角色组合：

- `primary_body`：决定整体体量和主轮廓；
- `secondary_body`：舱体、护罩、外壳或结构段；
- `mobility`：车轮、履带、旋翼外观或可动底座；
- `control_surface`：方向面、舵面、操作面或外观控制件；
- `joint`：固定、旋转、铰链或滑动关系；
- `tool_or_payload`：任务载荷、末端工具或非功能性道具附件；
- `trim`：面板、灯组、通风、装甲和装饰；
- `transparent`：舱罩、灯罩和观察窗；
- `material_zone`：可独立换材质的表面区域。

## 4. 通用连接与关节

连接器：

```text
surface_mount / axial_mount / rail_mount / socket_mount
panel_mount / wheel_mount / wing_mount / tool_mount
```

概念级关节：

```text
fixed / revolute / hinge / slider / ball_preview
```

关节只支持姿态、限位和简单动画预览，不代表真实动力学、承载、控制系统或安全认证。

## 5. Future Weapon Prop Pack

用途：未来武器概念、游戏资产、影视道具和非功能展示模型。

主要角色：

```text
core_shell / front_shell / rear_shell / grip_shell
top_accessory / side_accessory / lower_structure
energy_or_storage_visual / armor_panel / surface_detail
```

初始模板：紧凑型、长轮廓型、重型支援外观、能量道具型。所有模板只表达外观、比例、模块和材质，不包含现实工作机构、弹药、承压、加工或制造能力。

当前 `weapon-concept-v1-reference` 是该包的兼容技术基线。四个领域均已有最小 manifest、确定性 blockout、分件候选和基础 AgentAssetVersion 路径；这证明了通用运行时闭环，不等于四个领域都已有正式美术 Pack 或最终质量资产。

当前 API 可读取四个最小 manifest（`GET /api/v1/agent/domain-packs`），并可通过 `POST /api/v1/agent/blockouts` 与 `POST /api/v1/agent/blockouts:segment` 生成每个领域的三个轻量 blockout 和分件候选。manifest 是运行时语义入口；正式缩略图、完整材质目录、人工审阅和发布级资产仍属于后续 Pack 晋级流程。

## 6. Vehicle Concept Pack

用途：未来汽车、地面载具、探索车、竞速车和科幻运输工具的完整外观概念。

主要角色：

```text
body_shell / cabin / chassis_visual
wheel_or_track / fender / lighting
aero_surface / bumper / intake_visual
interior_silhouette / trim_panel
```

初始模板：城市双座、越野探索、低矮竞速、重型运输。Agent 必须先生成完整车身比例和四角姿态，再分出车身、座舱、移动部件、灯组和外观件；不得只生成一个车头或车轮。

关键可编辑参数：轴距、轮距、离地高度、车身长宽高、座舱位置、轮径、前后悬、曲面饱满度和细节密度。

## 7. Aircraft Concept Pack

用途：未来飞机、无人飞行器、垂直起降器和科幻航空器的完整外观概念。

主要角色：

```text
fuselage / cockpit_canopy / main_wing
tail_surface / nacelle / intake_visual
landing_gear_visual / payload_pod / panel_detail
```

初始模板：高速单座、宽体运输、垂直起降、无人侦察。Agent 必须先建立完整机身、机翼和尾部关系，再分件；左右对称默认开启，用户可明确选择非对称概念。

关键可编辑参数：机身长宽高、翼展、后掠视觉角、翼厚、座舱位置、尾翼布局、发动机舱数量和起落姿态。

本包不输出空气动力学、飞行安全、结构强度或适航结论。

## 8. Robotic Arm Concept Pack

用途：机械臂、工业机器人、服务机器人上肢和科幻操作机构的概念资产。

主要角色：

```text
base / shoulder_joint / upper_link
elbow_joint / forearm_link / wrist_joint
end_effector / cable_cover / guard_panel
```

初始模板：轻型精密、重型搬运、长臂维护、双工具服务。Agent 必须生成完整运动链并保持父子顺序；调整一段连杆时，子级随关节重新定位。

关键可编辑参数：自由度、各段长度、关节外壳尺寸、基座尺度、工具类型、收拢/展开姿态和线缆外观。

本包的关节只做概念姿态与简单动画，不输出载荷、扭矩、碰撞安全、控制代码或工业认证。

## 9. 完整外观与自动分件

每个领域包都必须经过三个阶段：

```text
blockout
→ segmented_concept
→ editable_asset
```

- `blockout`：完整轮廓、体量和姿态，不允许只生成局部零件；
- `segmented_concept`：按领域角色自动分成主要部件；
- `editable_asset`：稳定名称、层级、pivot、材质槽、可替换接口和质量报告。

Agent 可以建议分件，但正式分件必须通过 AssemblyGraph 校验。目标编辑集合包含选择、隔离、隐藏、锁定、移动、旋转、缩放、参数修改、换材质、替换和保存为组件；当前只完成其中的受限比例/位置/关节姿态、材质、组件保存与同角色替换。

D005 为四个 Pack 各提供 4 个非工程语义比例 Recipe，并共用 4 个版本化 builtin Style Token。Recipe 不依赖变体内部 role 名称，而是将 `primary_form`、`secondary_form`、`cabin_form`、`base_form`、`upper_link_form`、`end_effector_form` 等稳定语义槽确定性映射到当前 AssemblyGraph Part；只有该 Part 同时存在 G808 ratio binding 和 G826 GLB surface provenance 时才返回一步比例预览。它不增加领域包可执行代码、工程尺寸、功能机构或性能结论。

### 9.1 领域建模配方

每个正式 Domain Pack 的目标数据还必须声明：

- 允许的 `MechanicalStyleToken@1` 与相对比例档位；
- 关键 role 的 `EditableComponentRecipe@1`；
- 主壳体应使用的 Profile/Extrude/Loft/Revolve/Sweep 语法候选；
- 需要的稳定 Material Zone 和默认视觉材质；
- 语法不可用时的明确失败或保守 Recipe，不能回退到万能 box 模板；
- 每条 Recipe 的完整外观、readback、预算和视觉基准 fixture。

领域包只提供语义和已审阅引用，不能携带可执行操作实现。一个操作只有进入全局 `ShapeProgramRuntimeManifest@1` 后才可被 Recipe 使用。

## 10. 正式领域包最低目标

发布级每个领域包至少应包含：

- 4 个完整外观模板；
- 12 个 ShapeProgram fixture；
- 12–20 个可复用模块；
- 1 套通用材质集合和 1 套领域材质集合；
- 20 条 Brief、20 条局部修改和 10 条失败/取消评测；
- 1 个完整导出与重启恢复样例。

四个领域包全部通过相同 Core 合同和工作台，不为任何包增加独立页面。

当前有四个最小 manifest、每领域十二个确定性后端 blockout/分件候选和 Weapon reference 兼容资产；仍不满足带缩略图、完整材质、独立审阅和发布级质量的正式 Pack 规模。

## 11. 通用机械扩展路线

“不局限于武器”不通过删除领域语义或增加一个万能 fallback 实现。`FGC-D006` 目标是建立 `DomainPackAuthoring@1` 和晋级 Gate，使新领域可以复用同一 Core，同时保持明确角色、比例、组件和失败边界。

建议按相近结构分批扩展：

1. 家用与桌面机械：风扇、吸尘器、咖啡机、厨师机、缝纫机、打印/扫描设备等非功能外观；
2. 手持与工作间设备：电动工具外观、台式工具、泵/压缩机外壳、搬运设备；
3. 工程与农业机械：挖掘机、装载机、拖拉机、收割机等完整外观概念；
4. 服务与公共设备：售货/清洁/物流设备、服务机器人机身和其他机械产品外观。

医疗、交通、飞行、工业控制等类别仍只做概念数字资产，不提供治疗、安全、载荷、控制或认证结论。每个新包至少需要：20 条正常 Brief、10 条含糊/越界输入、4 个完整外观模板、12 个受限 fixture、语义比例配方、稳定 role/zone、可编辑组件配方、材质集合、视觉基准、失败回退和许可证清单。未达到 Gate 的包保持 `draft`，不进入自动领域推断或最佳候选选择。

专属 Skill 可以改变设计语言、研究步骤、参考来源和评审权重，但不能自行定义可执行几何操作。Domain Pack 提供“对象是什么”，Skill 提供“如何完成这类设计任务”，两者都只能引用全局 runtime manifest 和 Tool Registry。

3D 机械设计系统的目标操作顺序见 [MECHANICAL_DESIGN_OPERATIONS.md](MECHANICAL_DESIGN_OPERATIONS.md)。
