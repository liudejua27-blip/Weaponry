# ForgeCAD 视觉材质系统

版本：2026-07-15
状态：G6 预览、Agent asset ChangeSet 绑定和 Agent GLB 回读切片已实现；M101–M107 已完成视觉合同、目录、受控纹理对象摘要、Material Zone 检视/选择/绑定、领域筛选与 Snapshot/CAS 持久化；G826 已补齐真实 GLB 的 UV0/tangent 与稳定 face→part/zone readback。M108 已完成源码侧的同源内置五通道 PBR、真实 zone→material 绑定、固定工作室环境 readback，并以 Khronos Validator 对四领域原始 GLB 做零 error/zero warning 自动检查；macOS arm64 packaged sidecar/Tauri PBR Gate 已通过，但优化/压缩平台采用、其他平台 packaged sidecar、正式安装发布与独立人工视觉基准仍未完成。P0 是视觉 PBR 材质，不是工程材料数据库。

## 1. 用户体验

零基础用户可以直接说：

- “车身换成亮面汽车漆，轮胎用橡胶。”
- “机械臂主体用拉丝铝，关节护罩用黑色塑料。”
- “飞机座舱使用深色玻璃，机身用哑光复合材料。”
- “这个未来道具使用磨砂钛金属外观。”

Agent 把自然语言映射到真实存在的材质预设，并先在主视图预览。当前实现提供 13 个、覆盖六类的本地参数材质预设、`GET /api/v1/agent/materials`、blockout 视觉覆盖和 Agent asset ChangeSet 绑定；Material Zone 抽屉可按名称/标签搜索、按六类筛选，并显示真实对象存在性、来源/许可证和参数回退；用户确认后会创建新的 Agent asset 子版本。Agent GLB 可回读基础材质索引；M101 已支持可选纹理资产 ID、法线、自发光、透射、IOR、清漆、纹理缩放和来源元数据；M103 允许显式登记受控 PNG/JPEG/WebP 纹理对象，但不自动下载、不接受文件路径，也不自动把第三方纹理标成原创。

## 2. MaterialPreset@1

当前 `MaterialPreset@1` 字段：

```text
schema_version
material_id / display_name / category
base_color / metallic / roughness / opacity
base_color_texture_asset_id? / metallic_roughness_texture_asset_id?
normal_texture_asset_id? / occlusion_texture_asset_id? / emissive_texture_asset_id? / normal_strength
emissive_color / emissive_strength / transmission / ior
clearcoat / clearcoat_roughness / texture_scale[2]
visual_tags[] / source? / license? / version?
thumbnail_asset_id? / thumbnail_fallback / texture_summary[]
```

所有数值必须有范围，纹理必须来自内容寻址资产。UI 不允许仅凭显示名称伪造“真实材料”状态；没有可用纹理时使用 `thumbnail_fallback=parameter`，对象缺失时不显示为“已加载”。

## 2.1 MaterialTextureObject@1

纹理对象是视觉资源元数据，不是工程材料或来源证明。当前 API 只接受显式提交的 PNG、JPEG、WebP 原始 base64，服务端读取媒体头得到尺寸，限制 4 MB、4096×4096 和 1600 万像素，并写入 SHA-256 内容寻址对象库。对象 ID 形如 `asset_tex_<sha256 前 24 位>`。

允许用途：`base_color`、`metallic_roughness`、`normal`、`occlusion`、`emissive`、`thumbnail`。来源/许可证必须真实填写：内置纹理只能是 `forgecad_builtin/not_applicable`；本人创作只能是 `user_created/self_declared_original` 或未能确认时的 `unknown`；第三方必须是 `imported_reference/third_party` 或 `unknown`，并由作者提供 `license_ref`。系统不会访问 URL、读取任意本地路径或推断许可证。

`GET /api/v1/agent/materials` 会对材质引用返回 `texture_summary`（对象 ID、用途、`exists`、来源和许可证摘要），不返回绝对路径。对象文件缺失或哈希不匹配时，材质仍可使用参数 PBR 回退，且不会自动修改 Agent 资产版本。

用户登记对象现支持五个 PBR 通道和缩略图，但尚未自动接入正式 Agent asset GLB；它们仍只在受控目录/ChangeSet 边界可见。与此同时，当前 ShapeProgram 编译会把内置、程序化、视觉专用的五通道 PNG 集直接写入同一 GLB，并将每个实际 zone 的 material id、纹理 hash/色彩空间/尺寸/来源/许可证/回退与环境 hash 从该 GLB 回读。它不代表已接入任意第三方纹理、也不代表通用照片级外观。

## 3. P0 材质目录

当前内置目录使用稳定 `material_id`，只表达视觉效果：`mat_graphite`、`mat_aluminum`、`mat_automotive_paint`、`mat_rubber`、`mat_composite`、`mat_dark_glass`、`mat_signal_red`、`mat_painted_steel`、`mat_abs_matte`、`mat_rubber_tire`、`mat_carbon_composite`、`mat_clear_glass`、`mat_powder_coat`。目录 smoke：

```bash
npm run agent:g6-material-catalog-smoke
```

这些预设在没有纹理文件时也能安全降级为参数材质，不代表真实工程材料牌号、强度或制造适用性。M102 目录 Gate 要求 13 个唯一 ID 覆盖 metal/polymer/rubber/composite/glass/coating 六类。

G818 的展示模型还会在生成时使用受限的 GLB/视口内部映射：石墨、复合外观、金属外观，以及仅用于灯带点缀的 `mat_emissive_blue`。其中后者不是第 13 个用户可选 `MaterialPreset`，不进入材质目录，也不代表发光部件或电气设计；它只是受控外观层的 PBR 颜色/自发光参数。

以下目录是目标扩展，不是当前已注册 ID。

### 金属外观

```text
painted_steel / brushed_steel / darkened_steel
brushed_aluminum / anodized_aluminum
titanium_matte / copper / brass
```

### 聚合物与橡胶

```text
abs_matte / nylon_technical / polycarbonate
rubber_soft / rubber_tire / polymer_grip
```

### 复合材料

```text
carbon_fiber_woven / carbon_composite_matte
fiberglass_painted / ceramic_composite_visual
```

### 透明与发光

```text
clear_glass / smoked_glass / colored_lens
emissive_strip / emissive_panel
```

### 涂层与自然材料

```text
automotive_paint_gloss / automotive_paint_matte
powder_coat / primer / worn_paint
wood_oiled / leather / technical_fabric
```

名称表达视觉预设，不证明对象真的由对应材料制造。

## 4. Material Zone

每个部件可以有多个稳定材质区：

```text
primary / secondary / accent
transparent / emissive
rubber / interior / trim
```

Material Zone 使用稳定 ID，不绑定具体颜色。更换材质不改变几何和部件 ID；需要改变纹理比例时只创建新的 material binding。M105 要求预览操作同时携带 `part_id` 和 `material_zone_id`；服务端拒绝不属于当前部件的 zone。当前确定性 blockout 多数提供一个 zone，多 zone 资产必须由真实资产元数据提供，不能由 UI 猜测。

## 5. Agent 规则

- 优先匹配现有预设，不凭空创建不存在的许可证或纹理；
- 一次只修改用户指定的部件/区域；
- 无法确认区域时先在视口高亮并询问；
- 推荐材质时说明视觉理由，不提供强度或制造承诺；
- 透明、发光和高反射材质必须运行渲染兼容检查；
- 所有确认型材质变更必须进入 ChangeSet，可预览、撤销和追溯；未保存为 Agent asset 时的 blockout 材质覆盖仍是临时预览，不写入 Version。

## 6. 质量与性能

- P0 使用 metallic-roughness PBR；
- 材质和纹理尽量跨部件复用；
- 默认纹理分辨率按实际屏幕用途限制；
- GLB 导出前执行未引用材质清理和纹理引用检查；
- 运行时支持的压缩策略必须在打包平台验证后启用；
- 没有纹理时使用参数材质安全降级，不显示为“缺失材质”。

### 6.1 高真实度 PBR 合同（M108 源码检查点）

每个可发布视觉材质使用 `VisualTextureSet@1` 绑定：

```text
base_color
metallic_roughness
normal
occlusion
emissive?
clearcoat / clearcoat_roughness / clearcoat_normal?
transmission / thickness?（仅透明外观兼容集）
```

每个通道记录内容 hash、用途、色彩空间（sRGB 或 linear）、分辨率、来源、许可证、版本和回退。base color/emissive 使用 sRGB；metallic-roughness、normal、occlusion 等数据纹理使用 linear。网格使用稳定 UV0；normal/clearcoat normal 生效前必须有可验证 tangent space。没有 UV 或 tangent 时明确拒绝当前正式 PBR GLB，不能把纹理字段写入 GLB 后仍声称已生效。

glTF 2.0 metallic-roughness 是默认互操作基线。当前内置 GLB 已为涂层使用 `KHR_materials_clearcoat`，并只为受限透明外观同时写出 `KHR_materials_transmission`/`KHR_materials_ior` 和 `alphaMode=BLEND`；readback 缺少任一兼容字段即拒绝。`gltf-validator@2.0.0-dev.3.10` 作为开发/CI 门禁，对四领域各一份同一编译路径的原始 GLB 要求零 error、零 warning，并验证畸形 GLB 被拒绝；它不替代 `GeometryCompileReadback@1` 的资产真值。为了让这些标准 GLB 合规，内部稳定面追踪属性使用精确整数值的 FLOAT custom attribute，而不是 glTF 禁止用于顶点属性的 `UNSIGNED_INT`。纹理压缩 `KHR_texture_basisu`/KTX2 以及 glTF Transform inspect/validate/dedup/prune/压缩仍是**未采用**：尚无目标平台基线或 provenance 保留基准，不能以减小文件为由引入第二资产真值。只有 Three.js、GLB readback、映射保留和目标平台 smoke 全部通过后，才可另立 ADR 采用。

### 6.2 多材质区目标

`FGC-M108` 只能从真实部件/表面语义生成稳定区域，例如 `primary_shell`、`secondary_shell`、`trim`、`transparent`、`rubber`、`emissive`。每个区必须有稳定 ID、面集合/readback、默认材质、可选兼容材质和空区拒绝；不得由前端颜色猜测区域，也不得因换材质改变 Part ID 或几何 hash。

候选生成阶段先分区再选材质；CandidateEvaluation 必须检查：区域完整、透明件排序/双面边界、法线/tangent、纹理可读、许可证、draw calls、GPU 纹理预算和 GLB 回读一致。用户只看到普通语言区域名和材质效果，不看到纹理通道或 glTF 扩展。

Material Zone 的面集合必须由几何 feature provenance 产生：Profile/Loft/Sweep/CSG/edge finish 节点在编译时保留 surface role，G826 将其固化为稳定 face/zone readback；M108 只在这些真实区域上绑定纹理。布尔重拓扑、倒角或优化后若无法保留映射，编译必须将该候选标为不可用于正式多区材质，不能按颜色或法线方向重新猜区。

`EditableComponentRecipe@1` 可以声明需要的区域角色和默认材质，但实例化后仍以 GLB readback 的真实 zone 为准；Recipe 声明与输出不一致时质量失败，不由 UI 补齐。

### 6.3 视觉环境

真实感还依赖一致展示：当前源代码使用版本化的 `env_forgecad_room_studio_v1` 程序化 RoomEnvironment/PMREM（不是伪称的第三方 HDRI），线性色彩工作流、sRGB 输出、ACES Filmic、1.18 exposure、接触阴影与环境 hash 同时写入 GLB 并在唯一 renderer 中复用。经许可 HDRI 仍是后续候选；Poly Haven 等 CC0 来源必须经过显式导入、hash、许可证和离线打包审查，应用不得自动抓取。

## 7. 工程边界

P0 不保存或推断屈服强度、密度、疲劳、耐热、成本、供应商牌号、加工方法或结构适用性。以后 Engineering Pack 可以增加独立 `EngineeringMaterialProfile`，但不能用视觉 `MaterialPreset` 自动生成工程结论。
