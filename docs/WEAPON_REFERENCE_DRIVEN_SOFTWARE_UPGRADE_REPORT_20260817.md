# 参考图驱动科幻武器：建模记录与 ForgeCAD 升级报告

日期：2026-08-17
任务：`FGC-MCP010F-reference-contour-weapon-graybox-20260817`
范围：游戏/影视虚构视觉资产；不包含现实可制造尺寸、结构、材料配方或性能建议。

## 1. 执行结论

本轮已经证明“先分解参考板，再以全局轮廓和连续结构组生成灰盒”比自由拼装基础体更适合作为建模方向，但随后审计发现旧轮廓目标存在决定性缺陷：1000×220 的侧视裁切被 `resize_exact` 拉伸为 512×512。旧候选 `candidate-520f61e38f07435e886f4cb69318abd3` 的 IoU `0.900887573964` 与 Boundary F1 `0.865083929789` 因此只能保留为历史传输证据，不能再作为视觉质量结论。严格 GLB 回读仍通过，但它只证明结构有效。

Runtime 源码现已改为等比缩放到固定画布，并以边缘像素延展填充留白；定向单元测试和 focused reference-mask 测试通过，修复版 Dev.app 已安装。当前 Codex MCP 会话仍连接旧 cohort，必须重新加载后才能为同一参考生成新的不可变目标并重新比较。状态保持 `QUALITY_TARGET_NOT_MET`，没有确认、没有创建版本、没有导出，也没有提前解锁 PBR。

## 2. 为什么旧模型粗糙

1. 缺少正式的参考板视图裁切合同，左右、顶底、前后和细节图没有形成一个同源、定向、可审查的 ViewSet。
2. 旧流程把视觉区域过早命名成枪托、弹匣等功能零件；对于无传统枪械结构的科幻外形，这会制造错误边界。
3. `profile-loft@1` 实际是沿局部深度堆叠同一侧视轮廓，不能沿武器纵向定义多站截面，所以容易得到“轮廓对、体积薄”的板状模型。
4. `panel@1` 和 `vent-array@1` 主要增加凸出几何，缺少真正的 inset/recess/bevel/cut 语义。
5. 当前 UV 是逐三角图表式分配，虽然结构有效，但会产生大量接缝，无法支持连续金边、拉丝方向和可控磨损。
6. 外观程序和 Worker 写死旧机器人材质包，缺少青蓝 emissive、完整 clearcoat 纹理和派生贴图回执。
7. 自动轮廓曾把任意宽高比参考强制变形成 512×512；这会让轮廓优化目标与真实模型投影使用不同的纵横比例，是本轮低质量与错误高分并存的首要根因。

## 3. 本轮实际建模过程

### 3.1 参考输入

- 授权原图：1491×1055，SHA-256 `1964704a62ed7a841b4d49c370b8d46f4626e201daad29092a9c39a40b4c4109`。
- 从同一参考板裁出 right、top、front、back 视图并进入 ForgeCAD CAS。
- right 作为当前主轮廓权威；top/front/back 的自动轮廓只作为待审输入，不能作为已确认真值。

### 3.2 中性视觉结构，而非功能分件

采用下列 continuity groups：

- `outer-spine`
- `lower-frame`
- `receiver-core`
- `energy-channel`
- `muzzle`
- `rear-shell`
- `top-spine`
- `side-armor`
- `luminous-core`
- `material-inlay`

其中后部大开孔是参考图明确观察到的 `subtract` 区域，不是另一个实体部件。允许结构组重叠、共享边界和跨越传统功能区，最终外轮廓仍拥有最高权威。

### 3.3 灰盒生成

1. 将右侧全局轮廓转换到统一的模型坐标。
2. 在 Operator 的 64 点预算内保形简化轮廓。
3. 以三层 profile-loft 建立主壳体，并保留轻微深度收边。
4. 用 profile-extrude + boolean difference 切出后部贯通空腔。
5. 将能量核心做成独立圆环体，避免用贴图假装体积。
6. 编译为 2 Part、480 triangles 的 GLB，执行严格 accessor/BIN/Part/source-map 回读。
7. 生成固定 AOV 并进行候选比较；最佳候选的比较证据哈希为 `e0cf0a572831fe0785c0d0122e152f9250d3fc223572bd32e14acffcc3291c56`。

详细机器证据见 `docs/evidence/mcp010f/reference-contour-weapon-graybox-20260817.json`。

## 4. 材质与纹理方案

当前阶段仅完成材质研究和合同升级，不把材质应用到未通过轮廓门的候选。

建议的武器专用层：

| 视觉层 | 起始参数 | 几何/纹理策略 |
|---|---|---|
| 白色陶瓷涂层 | metallic 0.02, roughness 0.22, clearcoat 0.65 | 独立外壳 Part；低频微表面，不用大划痕破坏轮廓 |
| 金色阳极氧化边饰 | metallic 0.95, roughness 0.24 | 独立 inlay/trim 几何或受控 mask；保持连续流线 |
| 黑色结构金属 | metallic 0.82, roughness 0.28 | 内骨架与通风槽底部；拉丝方向必须由连续 UV 控制 |
| 黑色工程材料 | metallic 0, roughness 0.42 | 握持与柔性视觉区；避免与金属共用粗糙度逻辑 |
| 青蓝能量核心 | emissive strength 6–10，中心可 10–14 | 独立 core/ring/channel Part；发光不参与轮廓补偿 |

候选 CC0 研究源：ambientCG `Metal010`、`Metal034`、`Porcelain001`、`Plastic006`；Poly Haven 可用于补充 HDRI/表面研究。任何采用必须先进入隔离 adoption cache，记录下载 URL、原始 archive hash、许可证、派生 recipe、输出 hash、SBOM 和 provenance，然后离线编入包。当前没有下载或激活第三方资产。

## 5. 本轮软件升级

本轮新增三个视觉结构合同，并纳入 contracts manifest 与自动 Gate：

1. `ReferenceViewSet@2`：同源参考板、归一化 crop、视角角色、方向状态、轮廓审查状态和 CAS hash。
2. `NeutralStructureGraph@1`：以 continuity group 表达视觉结构，显式支持 `add/subtract/material-only/guide`，并禁止把它冒充功能零件分解。
3. `TextureSet@2`：新增 emissive、clearcoat、clearcoat roughness、clearcoat normal 槽位，并要求派生纹理回执 hash。

同时完成了原 P0 计划中的第一项几何执行能力：

4. `forgecad.geometry.longitudinal-section-loft@1`：沿主体 +X 轴使用 2–16 个严格递增的 Y/Z 截面站生成封闭体积；每站 3–64 点且点数必须一致，拒绝乱序站点、退化截面、未知字段和非有限值。旧 `profile-loft@1` 保持兼容，不改变其沿 Z 堆叠语义。

该算子已进入 GeometryProgram@2、Worker 协议目录、Runtime 可执行白名单、Agentic action/Repair/Critic 合同及 `hard-surface-detail@0.2.0` 开发 Bundle。确定性编译、严格 GLB 回读、语义 lineage、负向输入、Skill 完整性和同队列 Runtime→Worker 动作测试均通过；新安装的 Dev.app 独立 raw stdio 探针显示 `17 entries / 17 active`，并生成 10 Part、620 triangles 的结构性测试资产。安装队列哈希为 `a5d38dd4dbb42548c1627e624d75575e4879cbcb631ab2c134579c21b3a88784`。

`npm run contracts:check` 已通过，当前为 142 个 schema；`npm run worker:check`、focused Runtime/Worker tests 与 `git diff --check` 通过。重新打开 Codex 后，MCP 与 Runtime 已共同加载新队列和 17/17 active Operator。真实项目随后生成了三个未确认纵向灰盒候选；全部通过严格 GLB 回读，但可见侧视门仍失败。当前最好新候选为 `candidate-bd242b5852554dd5b92d905cce73dca3`：396 triangles、IoU `0.63324188766`、Boundary F1 `0.039951997973`，状态为 `QUALITY_TARGET_NOT_MET`。完整回执见 `docs/evidence/mcp010f/weapon-longitudinal-live-graybox-20260817.json`。

随后修复 `reference_mask_png` 的纵横比破坏问题：参考图现在等比适配 512×512 画布，留白由缩放后边缘像素确定性延展，避免人为黑/白边切断背景 flood fill。`automatic_reference_mask_preserves_wide_reference_aspect_ratio` 与 focused reference-mask tests 均通过。新 Dev.app cohort `4a9a01eb349096605534b0d0de36b62aa1c0c63ac14af51d9920adfc8ed3d1aa` 已完成本地构建、ad-hoc 深度签名和原子安装。安装后的首次检查曾发现 Codex MCP 已更新而 Runtime 仍为旧 cohort `a5d38dd4dbb42548c1627e624d75575e4879cbcb631ab2c134579c21b3a88784`；旧 Runtime 被定向停止后，由 supervisor 启动同 cohort 新 Runtime，写入门恢复。证据见 `docs/evidence/mcp010f/aspect-preserving-reference-mask-source-gate-20260817.json`。

重新加载后，MCP 与 Runtime 已共同切到 cohort `4a9a01eb349096605534b0d0de36b62aa1c0c63ac14af51d9920adfc8ed3d1aa`。同一 1000×220 right reference 生成等比目标，再以自动主轮廓重新栅格化为未人工确认的干净目标，消除了自动 mask 的孤立边缘噪点。固定 `CameraCalibration@2` 正交右视下，旧五块灰盒基线为 IoU `0.580433270652`、Boundary F1 `0.29581498568`；统一缩放虽然改善 bbox，但 IoU 退化到 `0.524241302739`，因此未保留。随后用 16 个轮廓拐点截面生成单一连续 `receiver-core`：候选 `candidate-3a8665878160400da75695e517864257` 的 IoU 为 `0.927965967174`、Boundary F1 为 `0.913080476426`、bbox error 为 `0.001953125`、centroid error 为 `0.003680825465`，四个主轮廓数值门均通过；252 triangles，严格 GLB 回读通过。由于目标仍为 `unreviewed` 且无 landmarks/regions 与其他视图，Runtime 总状态保持 `QUALITY_TARGET_NOT_MET`，PBR、confirm 和 export 仍锁定。证据见 `docs/evidence/mcp010f/receiver-profile-envelope-live-20260817.json`。

2026-08-18，用户明确确认该 right-view 主轮廓。Runtime 以相同 contour/mask 生成 `user_confirmed` 目标 `c42a353a34c4d4013bd3a26dc872cb6b2eaa61007c912b197e657d8e0b025d6f`；重新比较继续使用原 `CameraCalibration@2`，没有重新拟合镜头。随后按单层质量闭环依次准备三层二级结构：

1. `armor-layering`：上部装甲、下部龙骨壳与后部机械内衬；第一版因下部装甲越过轮廓，IoU 从 `0.927965967174` 降至 `0.918237215731`、Boundary F1 降至 `0.885525217757`，被自动拒绝。收紧到轮廓安全区后两项指标恢复到基线并通过。
2. `energy-core`：黑色外壳、金属环与独立发光核心，仍保持主轮廓数值不变。
3. `surface-linework`：纵向能量导槽、五槽通风阵列和背脊导轨，仍保持主轮廓数值不变。
4. `panel-relief`：前/后上装甲片、下部前护罩、核心前后护肩与前端收口，全部位于已确认轮廓内部，主轮廓数值仍不变。

当前审查候选为 `candidate-58dafc5e3ba243c29fd749ea56ddf978`，GeometryProgram hash `342b100f00e7164e8543c029c5566d6a5d892878086f7dc23d74c84c40dc6ea0`，GLB hash `8456dd72bc395e5c464627a1db4d6136888ae5587837ba48f0ffab5198dec63b`。它包含 16 个语义 Part、1212 triangles；严格回读的 boundary/non-manifold/degenerate/invalid-index/non-finite/winding/UV/tangent 错误均为 0，Part、MaterialZone 和 source coverage 均为 1.0。侧视 IoU `0.927965967174`、Boundary F1 `0.913080476426`、bbox error `0.001953125`、centroid error `0.003680825465`，与确认的主轮廓基线完全一致。机器回执见 `docs/evidence/mcp010f/weapon-secondary-structure-live-20260818.json`。

这仍是二级结构审查候选，不是成品：现有 AssetPack 只有暖橙发光材质，因此橙色仅作结构占位，不能冒充参考图的青蓝发光；金色阳极边饰、连续 UV、武器专用 PBR、其他视图和真人视觉门尚未通过。候选未 confirm、未创建永久版本、未 export。

## 6. 必须继续升级的能力

### P0：参考到灰盒的生产链

1. 新增 Runtime producer：`reference_view_set_prepare`，自动裁切只能产生 `automatic-unreviewed`，用户确认后才可成为门禁真值。
2. 新增 `neutral_structure_graph_prepare`，将描边、空洞、遮挡关系和 continuity group 固化为 CAS 对象。
3. 使用已实现的 `longitudinal-section-loft@1` 对当前武器建立 7–12 个参考驱动截面站；下一步仍需增加截面重采样、扭转限制和更强的自交诊断。
4. 让几何程序从 NeutralStructureGraph 自动生成 `VisualStructureGeometryPlan@1`，不依赖功能零件名称。
5. 增加联合多视图门：right/left/top/front/back 必须在同一候选上非退化；未确认视图不计 PASS。

### P1：二级硬表面

1. `panel@2`：真实 inset、recess、border、bevel、support loop。
2. `vent-array@2`：切除式槽阵列、底层 Part、边缘倒角和间距预算。
3. `recessed-channel@1`：能量通道的连续路径、宽度/深度渐变和端部过渡。
4. `energy-core@1`：同心环、护圈、发光核心和非发光机械层的固定语义输出。
5. 局部修复目标必须绑定 Part、参考视图和非退化全局门，禁止“一处变好、其他视图变差”。

### P1：UV、PBR 与材质包

1. 保留 `forgecad-hard-surface-robot@1.0.0`，新增 `forgecad-fictional-energy-weapon@1.0.0`，不要就地篡改旧包。
2. MaterialPack manifest 升级为从清单解析纹理，移除 Geometry Worker 中的硬编码文件 key。
3. 新增 `cyan-emissive`、`gold-anodized-accent`、`white-ceramic-clearcoat`、`black-structural-metal`、`engineering-polymer`。
4. `uv-atlas@2` 需要按 continuity group 生成连续岛、方向约束、texel density 和 padding 证据；最终共享 2K 优先于用 4K 掩盖几何问题。
5. `AppearanceProgram@3` 应支持 TextureSet@2、分层磨损 mask、decals/inlay、clearcoat roughness/normal 与 emissive map。
6. 导出 GLB 必须嵌入纹理、使用 glTF 规定的色彩空间，并通过 Khronos Validator；九 AOV、重启 hash 和真人门仍需单独验证。

## 7. 验收顺序

1. 人工确认 right/top/front/back 裁切与轮廓。
2. 新 longitudinal loft 灰盒在可见视图同时通过 IoU、Boundary F1、bbox 和 centroid 门。
3. 添加 panel、vent、energy ring；每次只改一类视觉结构并做多视图非退化检查。
4. 完成连续 UV 和 tangent，再应用武器专用 AssetPack。
5. 执行 beauty/depth/normal/AO/part-ID/material-ID/wireframe/UV-stretch/silhouette 九 AOV。
6. Codex typed review 后由真人独立检查；用户明确批准准确候选后才可 confirm/export。

## 8. 当前真实状态

- 旧右侧全局 IoU：STALE，不再作为质量门（目标纵横比失真）。
- 旧右侧 Boundary F1：STALE，不再作为质量门（目标纵横比失真）。
- 严格 GLB 回读：PASS。
- 顶/前/后轮廓：自动生成但未人工确认。
- 联合多视图：NOT_RUN。
- 面板/通风槽/能量环：PREPARED，单 right-view 主轮廓无回退；等待可见视图人工审查。
- UV/PBR/最终纹理：LOCKED。
- 真人视觉门：NOT_RUN。
- `HQ_360_PASS`：`BLOCKED_REFERENCE_COVERAGE`。
- 纵向截面放样源码与 Dev.app 安装包：PASS。
- 当前 Codex MCP 会话加载纵向算子：PASS（17/17 active，旧 cohort）。
- 等比参考轮廓源码测试与 Dev.app 安装：PASS。
- 等比参考轮廓 live MCP 加载：PASS，同 cohort `4a9a01eb...`。
- receiver-core 正交侧视主轮廓数值门：PASS（IoU `0.927965967174`，Boundary F1 `0.913080476426`）。
- receiver-core 轮廓人工确认：PASS（`user_confirmed` target `c42a353a...`）。
- 新纵向灰盒严格 GLB 回读：PASS。
- 新纵向灰盒最好当前队列 IoU：`0.63324188766`，FAIL。
- 新纵向灰盒最好当前队列 Boundary F1：`0.039951997973`，FAIL。
- 正确纵横比目标重建：PASS。
- 正交侧视取景/轮廓投影：PASS（单 right view、自动未确认目标）。
- 二级结构候选：PREPARED（16 Parts / 1212 triangles / strict readback PASS / right-view numeric no-regression PASS）。
- PBR：LOCKED_SECONDARY_STRUCTURE_VISUAL_REVIEW；青蓝 emissive 与金色阳极材质仍缺失。

因此，当前正确成果是“已确认单 right-view 主轮廓，并形成不破坏该轮廓的第一版模块化二级结构”，而不是“已经达到《生死狙击2》成品资产水平”。下一门是二级结构可见视图人工审查；通过后才能建立武器专用 MaterialPack、连续 UV 与青蓝/金色 PBR。其他视图、同候选多视图非退化、真人门和 `HQ_360_PASS` 仍独立阻断。

## 9. 2026-08-18 最终化尝试与阻断结论

用户确认继续后，先对准确候选 `candidate-58dafc5e3ba243c29fd749ea56ddf978` 发起永久确认；Runtime 连接在该写入请求期间返回 `RUNTIME_UNAVAILABLE`。按照审批与幂等边界，没有自动重放 `candidate_confirm`，所以候选仍为可审查状态，没有创建永久版本，也没有绕过 Runtime 复制 CAS 文件冒充正式导出。

随后尝试进入材质阶段。第一次调用错误地把历史兼容 `AppearanceProgram@1` 附着到 `GeometryProgram@2`，Runtime 以 `APPEARANCE_REJECTED` 正确拒绝。改为正式 `AppearanceProgram@2` 后，第二次调用暴露出 GeometryProgram 的 `part_outputs.material_zone_id` 必须与 AppearanceProgram 的 `zone_id` 精确一致；自定义视觉别名导致 `GEOMETRY_WORKER_REJECTED`。准备脚本现已修正为四个现有离线分区 ID：`white-dielectric-clearcoat`、`black-anodized-metal`、`brushed-steel`、`warm-orange-emissive`。由于同一准备动作已经失败两次，本轮停止自动重试，避免把未知 Runtime 状态反复写入项目。

当前可回读资产仍为 GLB `8456dd72bc395e5c464627a1db4d6136888ae5587837ba48f0ffab5198dec63b`，九 AOV RenderSet 为 `48b5c39f4e588dc2cd6bf19f379579609e3f4e4e2a5ff6d190362433acf2b288`，beauty 为 `0f3b7407826458e8a26a4c50979cbf5576887a819d630fd8761360785b90ec8f`。视觉审查结论为 `NEEDS_REVISION`：侧视轮廓门已通过，但 1212 triangles、16 Parts 的结构密度、曲面转折、面板层级、通风槽切深、核心同心环、青蓝发光和金色边饰均不足，不能标记为《生死狙击2》同级成品。

下一次继续执行时，应先重新读取该候选状态并由用户重新授权确认重试；确认成功后再运行已修正的 `scripts/prepare_weapon_final_appearance.py`。新的外观候选仍需单独的人类精确候选批准，才能 `candidate_confirm → export_prepare → export_confirm`。武器专用青蓝/金色 MaterialPack、至少一轮三级硬表面细节和已确认的 top/front/back 目标仍是正式成品的必要条件。
