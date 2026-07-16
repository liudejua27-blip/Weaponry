# ForgeCAD Codex 当前交接

快照日期：2026-07-16
用途：后续 Codex 开始任务前的第一份上下文

文档状态账本：[DOCUMENTATION_STATUS.md](DOCUMENTATION_STATUS.md)。当本文件与用户指南、能力矩阵或任务索引出现状态冲突时，先按文档地图修正归属，不要直接领取代码任务。

## 2026-07-16：FGC-M108 Sweep 连接与线缆细化（进行中，未完成）

- `compact_prop_a` 的握把由等截面 capsule 改为五截面、Y 主轴的受限 Loft，并让安装环从真实显示外包围读取握把半径，避免几何语法改变后连接环塌缩。它仍是非功能虚构游戏/影视道具外观，不表达现实武器结构、制造尺寸或性能。
- `urban_scout_a` 的四个楔形轮眉改为四点路径、八点椭圆截面的真实 G823 Sweep；为守住实际 GPU 预算，删除重复座舱框、将顶置排气视觉件收敛为两个并只保留一个侧围紧固件。`vertical_takeoff_a` 的四块平板旋翼支架改为封闭 Sweep 曲线外罩，尾部 24 段圆柱排气口改为低多边形楔形出风口；`precision_light_a` 增加封闭的橡胶材质服务线缆 Sweep。所有新件都进入同一 ShapeProgram、GLB、PBR、zone、surface provenance 和 readback，不是前端贴图或静默估算。
- `codex-iteration-14` 的真实工作台 readback 为道具 6,248/51、车辆 6,892/78、航空器 6,868/96、机械臂 5,720/53（triangles/draw calls）；四项均为 `ready/glb_pbr`、单 WebGL context、固定环境和 GPU passed。车辆 7,180 与航空器 7,132 的真实超限中间结果被 Gate 拒绝，没有放宽 7,000 triangles 或 96 draw calls 上限。
- glTF Transform 评估原先会把约 0.7–0.9 MB GLB 通过同步 stdin 送入 Python，完整 npm 链中可偶发管道等待；现在改用临时文件输入且只返回本测试需要的 zone/texture 映射。连续两次评估和完整 M108 Gate 已通过；Transform writer 仍因改变固定采样状态而被拒绝，不能替代不可变编译 GLB。
- 已通过 Agent 18 项单测、G3–G7 相关 Gate、G817/G819/G822/G823、完整 M108 Gate、contracts、desktop typecheck/build、真实 M108 renderer、R3、T002 14/14、T003 和 Tauri check。tracked macOS arm64 sidecar 已从当前源码重建为 31,813,296 bytes、SHA-256 `202dca17abcbb2c6210c1b753cdebc5607747dcb34482ca8dce7e0975b5c4383`；require-ready 与 packaged sidecar Alpha 覆盖当前 PBR readback、Manifold CSG、undo/redo、导出和重启并通过，`provider_calls=0`，`.app`/DMG 也已重建。当前用户打开的既有 CAD 工作台占用固定 8000 端口，因此没有终止它去重复运行 packaged Tauri smoke。完整 `release:packaging-readiness` 仍按设计以 `SIDECAR_BINARY_INVALID` 拒绝 Intel macOS、Windows 和 Linux 空占位，没有放宽发布阻断。
- Codex 代理审核仍认为虚构道具主体偏筒形、车辆轮眉有模块拼接感，四领域仍是高质量概念资产而非照片级真实产品。未写人工评分或真人回复；M108 继续 `in_progress`，C105/V003/F026 不解锁。

## 2026-07-16：FGC-M108 四领域轮廓与连接细化（进行中，未完成）

- `compact_prop_a` 主体由等截面 capsule 改为六截面受限 Loft，保留非功能虚构道具边界，并加入复合材料传感器壳与深色玻璃面；删除重复发光小件后，真实工作台 renderer 从超限的 7,028 triangles 降到 6,836，未放宽 7,000 上限。
- `urban_scout_a` 明确将四轮绑定为 `mat_rubber`，侧桥缩薄并增加四个受限楔形轮眉；`vertical_takeoff_a` 四个旋翼支架从 64×217.5 mm 级厚板缩为约 40.32×120 mm 外罩，继续与翼面和轮毂正体积重叠；`precision_light_a` 增加肩、肘、腕三处铝端盖。
- `codex-iteration-11` 的真实工作台 readback 为道具 6,836/51、车辆 6,844/84、航空器 6,508/96、机械臂 5,536/51（triangles/draw calls），四项均为 `ready/glb_pbr`、单 WebGL context、GPU passed。Codex 代理审核仍判定四领域没有同时达到比例/材质/细节 4/5，报告未写人工响应，M108 保持 `in_progress`、C105 继续 blocked。
- 已通过完整 `agent:m108-gate`、Agent 18 项单测、G5/G6、contracts、desktop typecheck/build、M108 renderer、R3、T002 14/14、T003、文档/integrity/安全/密钥门。tracked macOS arm64 sidecar 已从当前源码重建为 31,809,920 bytes、SHA-256 `50bc173dd452d6e29e789f371bf437d2b6b9e252d949da1eb0ae35035ff74c4c`；require-ready preflight、packaged sidecar、Tauri check/`.app`/DMG build 和 packaged Tauri smoke 均通过，`provider_calls=0`。

## 2026-07-16：FGC-M108 Airfoil Loft 与第二轮 Codex 代理审核（进行中，未完成）

- 航空器 A 的左右主翼不再使用厚 wedge，而是分别通过受限 `ProfileSectionSet@1 → loft` 生成固定非对称 airfoil 截面。该内置截面使用四段 tangent quadratic、`symmetry=none`、固定 16 点重采样和 Z 主轴；轴长、截面尺度与四个代码所有截面由 G818 锁定，不开放用户曲线、自由细分或 Planner 路由。
- 四个升力单元改为 52 mm 半径、48 mm 高的小铝轮毂与两片交叉复合材料叶片；支撑罩改为连接翼面与轮毂的纵向桥。道具后部和机械臂基座原先突兀的三角 guard 改为紧凑 bevel box，航空器侧 chine、翼根和脊背贴片同步缩小。所有修改仍是非功能视觉件，不是推进、结构、制造或适航设计。
- `codex-iteration-9` 的真实工作台画面由 Codex 进行第二轮代理审核：四领域的比例/轮廓、材质可读性、表面细节均给出 3–4 分开发反馈；四个领域都没有同时达到三维度 4/5。报告明确标为非真人，不写入空的 `review-responses.json`，不能满足三位独立 reviewer 退出门。主要剩余问题是连接过渡、边缘语言、尺度化纹理、车辆轮拱/悬挂语义、机械臂关节/线缆语义和航空器旋翼支撑。
- 最新真实 capture 全部为 `ready/glb_pbr`、单 WebGL context 并通过 GPU 预算：虚构道具 4,688 triangles/33 draw calls，车辆 6,748/72，航空器 6,508/96，机械臂 4,960/45。航空器仍低于既有 7,000 triangle 上限，未放宽任何 renderer、纹理或 readback 门槛。
- 本轮已通过 `agent:m108-gate`、`desktop:m108-workbench-renderer-smoke`、G5/G6/G818、Agent 18 项单测、contracts、`agent:check`、desktop typecheck/build、R3、Tauri check、文档/integrity/安全/密钥门。tracked macOS arm64 sidecar 从当前源码重建为 31,809,232 bytes，SHA-256 `e6ca477d0b98b34ba0d20c0e53c4b61d69781124a0fe955685b6892e423133ff`；经仓库 Rust toolchain wrapper 重建的 `.app` 和 `desktop:packaged-tauri-alpha-smoke` 也通过。packaged 路径实际覆盖空库初始化、当前 PBR readback、Manifold CSG、undo/redo、导出和重启恢复，`provider_calls=0`。完整 `release:packaging-readiness` 仍按设计以 `SIDECAR_BINARY_INVALID` 拒绝 Intel macOS、Windows x64 和 Linux x64 空占位，没有放宽跨平台发布阻断。M108 继续 `in_progress`，C105/V003/F026 不解锁。

## 2026-07-16：FGC-M108 Loft 代表资产与 Codex 代理审核（进行中，未完成）

- 四领域 A 审阅资产中，车辆底盘/座舱和航空器机身/座舱现在通过受限 `ProfileSectionSet@1 → loft` 真实执行，不再由多个 box/wedge 冒充连续外壳。这些固定截面由代码所有、经 canonical profile 和 G819 白名单验证；未增加自由 Profile UI、Recipe 或 Planner 自动路由。
- Loft/Sweep 的侧面 UV 改为按截面周长与中心线累计距离以 320 mm 展示基线计算，cap 也使用物理平面坐标；实际 GLB primitive 回读 `forgecad_visual_uv_repeat_mm=320`。G822/G823 保留有界 UV 上限，M108 继续对每个实际 primitive 检查该值，没有为新曲面降低 PBR/readback 门槛。
- 车辆移除了会在截图中形成大型三角杂件的后甲板，前灯缩成嵌入式灯带，两块顶部饰面按当前 loft 截面高度重新贴合。航空器的四个实心旋翼盘改为小铝轮毂与可见复合材料叶片；最终工作台实拍为 6,196 triangles/96 draw calls，恰好达到但未超过现有预算，后续不能继续靠堆 primitive 提升该 fixture。
- Codex 已以代理审查员身份检查 `codex-iteration-4` 的四份真实 GLB/readback 与工作台 PNG，并保存明确标记的代理报告；未改写 `review-responses.json`、未伪造三位真人 ID，也未把代理评分送入人工退出门。审查仍认定飞机翼面偏大且平、四领域纹理/细节密度为 Alpha 概念资产级，不足以宣称真实产品外观。
- 本检查点已通过 `agent:m108-gate`、`desktop:m108-workbench-renderer-smoke`、Q003、G5/G6/G818/G822/G823、Agent 18 项单测、contracts、`agent:check`、desktop build/Rust check、文档/integrity/安全/密钥门。从当前源码重建的 tracked macOS arm64 sidecar 为 31,808,512 bytes，SHA-256 `51d2df030672901840de72fb9cf4adb1eff02288ce44ad6ad2ac4482ed59ef7e`；packaged sidecar Alpha 已通过 PBR/CSG/undo/redo/导出/重启且 `provider_calls=0`，`.app` 也已由该二进制重建。`desktop:packaged-tauri-alpha-smoke` 未进入产品断言：它在启动前按设计拒绝被 `/Users/liuchongjiang/Documents/大A交易系统/backend` 的 Python/uvicorn 占用的 127.0.0.1:8000；本轮未停止该跨工作区服务，因此 packaged Tauri 和依赖同一端口的 r3 记为未运行，不得写 PASS。arm64 `--require-ready` preflight 通过；完整 `release:packaging-readiness` 仍按设计以 `SIDECAR_BINARY_INVALID` 拒绝空的 Intel macOS/Windows/Linux sidecar，该发布阻断未放宽。M108 继续 `in_progress`，C105/V003/F026 仍不解锁。

## 2026-07-16：FGC-M108 最终 GLB 真值与十二份审阅资产嵌合增量（进行中，未完成）

- `read_shape_program_glb_facts()` 现在从 BIN 中逐个解码真实 POSITION，而不是信任 accessor `min/max`；声明 bounds 必须与有限实际坐标一致。accessor/bufferView 的引用、count、offset、显式 `byteStride`、component alignment 和读取末端都使用同一严格入口，负下标、越出自身 view、缺失显式 buffer、2/6/256 stride、错位 offset 都会拒绝。PBR 图片 view 同样必须显式引用单一内嵌 buffer，不能用字符串/浮点/bool offset 或 accessor stride。
- 当前编译 GLB 明确冻结为一个 mesh、一个 scene、一个无 TRS/children/instancing 的 identity node；第二 mesh/scene/node、平移实例以及 bool/float node 引用都拒绝。该限制是当前静态 ShapeProgram 导出合同，不代表 ForgeCAD 已实现 glTF 场景图或变换实例 readback。
- M108 的 12 份固定审阅 fixture（四领域各 A/B/C）现在都由最终 GLB POSITION AABB 形成一个视觉连通分量。航空器 B 的两侧 pod 外罩、机械臂 B 的 wrist 外罩、机械臂 C 的 rail/carriage 外罩由目标部件推导中心；G818 同时锁定轴向、尺寸/半径/高度上下界、与两个目标的正重叠和可见外露体积。车辆 A 的 paint/deck 面板、机械臂 A 的 upper-link 面板和道具/航空器胶囊顶部面板也已从隐藏或过宽状态收敛。`segment_blockout()` 与 AssemblyGraph 对视觉部件的 root grouping、无 Joint、无可调参数事实保持一致。
- 这里的“一个分量”只覆盖这 12 份 M108 fixture，且只是 AABB 视觉连续性代理，不是实体布尔焊接、工程连接或全部 48 项 catalog 的证明。视觉件仍使用 root 级绝对坐标且不可编辑；真正 child slot、局部变换传播和 connector 归 `FGC-C105`。胶囊贴片仍有约 4.84–7.31 mm 的边缘间隙，主体仍是 Alpha blockout，不是照片级产品。
- 最新源码 `agent:m108-gate`、真实工作台 renderer smoke、G818/G826 和严格负例通过；画面仍为 `not_scored/human_benchmark_evidence=false`。三位独立人工评审尚未收集，因此 M108 继续 `in_progress`、C105 继续 blocked。
- 从最终源码重建的 tracked macOS arm64 sidecar 为 31,801,392 bytes，SHA-256 `50d89fce6fb8b557ed57aed9aa8957e45cac9226582f642e718fd53899197bab`。该精确产物的 require-ready、packaged sidecar、Tauri check、经仓库 Rust wrapper 的 `.app`/DMG build 与 packaged Tauri smoke 均通过；空 Library、PBR readback、Manifold CSG、undo/redo、导出、重启恢复成功且 `provider_calls=0`。DMG 只完成本机 bundle 构建，未执行外部安装、签名或公证；跨平台 sidecar与正式发布阻断不变。
- `release:packaging-readiness` 已复跑：结构 smoke 与当前 arm64 preflight 通过，最终报告按设计以 `SIDECAR_BINARY_INVALID` 失败，因为 Intel macOS、Windows x64 和 Linux x64 仍是空占位。该已知发布阻断没有被删除或放宽。

## 2026-07-16：FGC-M108 审阅真值与航空器连接收紧（进行中，未完成）

- M108 工作台捕获现在只接受正常 blockout 展示状态：截图前必须读取并保存 `presentation_runtime_facts`，证明 legacy ModuleGraph root 隐藏、当前 blockout root 可见、axes/grid/transform helper 全部隐藏，并要求真实 renderer line 数为 0。当前源码重建的四领域捕获全部满足这些事实；旧目录中的过暗/带坐标轴截图不会被读取为通过证据。
- 人工评分校验器不再只相信 manifest 的材质计数。它会逐 GLB 重新 readback，要求至少五套当前 `_builtin_v2` texture-set、每套恰好包含 baseColor/metallicRoughness/normal/occlusion/emissive、map ID 含 `_v2_` 且尺寸为 128×128；旧 v1、少于五套、错误版本 ID、错误尺寸和通道缺失/重复均由 self-test 负例拒绝。
- “至少五套”同时要求五个不同 material index、texture-set ID 和规范 texture material；重复 authored alias 不能累加。renderer line instrumentation 缺失、非整数或非零都会 fail closed，不能用缺省 0 绕过正常展示状态。
- 航空器四个旋翼支柱的受限 showcase 偏移已调整，使其进入对应机翼；G818 不是读取生成参数，而是从最终 GLB POSITION accessor 要求每个支柱与机翼的 Z 范围至少重叠 0.07 m，并继续要求支柱与旋翼正体积交叠且有可见外露体积。这只是概念资产的视觉连续性代理，不是实体相交、结构连接或适航证明。
- 聚焦 `agent:m108-visual-benchmark-score-validator-smoke`、`agent:g818-visual-detail-grammar-smoke`、`agent:m108-visual-pbr-smoke` 和真实四领域工作台捕获已通过；最新捕获最大仍为 6,176 triangles/87 draw calls。工件明确为 `not_scored/human_benchmark_evidence=false`，三位独立人工评审尚未收集，因此 M108 继续 `in_progress`、C105 继续 blocked。

## 2026-07-16：FGC-M108 纹理连续性与部件嵌合增量（进行中，未完成）

- 真实工作台截图中的汽车漆马赛克不是 Three.js 色彩空间问题，而是旧内置 PNG 的离散格噪声被 320 mm UV 展示基线放大。新生成纹理的 texture-set ID 以 `_builtin_v2` 结尾、map ID 含 `_v2_`、`version=2`，周期平滑微表面替代旧格噪和 composite 硬织纹，coated/brushed/glass 的 baseColor 调制低于 roughness/normal。旧 `builtin` v1 的 40 张图、原 ID 和原字节以固定聚合 hash 保留为历史 GLB readback，不会被 v2 覆盖。
- M108 smoke 现解码八种材质的全部五通道，并对 8/12/16/18/28/32 px 的每个相位拒绝硬格线；只有 metallicRoughness/normal 必须有微变化，baseColor/AO/emissive 可合理纯色。正常 v2 首次编译只生成 8 个当前集合；真实读取 v1 后，全量 PNG cache 上限为 16 个材质×版本集合、共 543,327 字节压缩图，不建立逐像素缓存。
- GLB readback 不只相信图片 extras 自报 SHA：authored material 必须命中穷举目录→规范 `texture_material_id` 映射，texture-set/map metadata 与 PNG 字节必须匹配 current/legacy 清单，TextureInfo 固定 UV0 且不允许自定义 sampler/texture transform。同一资产实际使用的材质必须属于同一视觉纹理合同版本；未知材质、ID/index 不一致、v1/v2 混用、布尔伪索引、同步伪造 SHA、缺引用或损坏通道都会明确拒绝。精确 v1 报告仅迁移其缺失的规范材质字段；相对当前 v2 已过期的 GET/幂等重放返回 `stale_compile_readback/unavailable`，不能继续作为当前导出真值。glTF Transform writer 因改变固定采样状态而继续作为导出优化器被拒绝。
- 四个固定审阅 fixture 只复用现有 cylinder、box+bevel 和 wedge，新增非功能连接外罩：虚构道具 core↔grip 的安装环、车辆 chassis↔左右前后轮的侧围、飞机 wing↔四旋翼的支座、机械臂 joint↔偏置 link 的肩部桥。G818 从最终 GLB POSITION accessor 要求外罩 AABB 与每个目标正体积重叠，且有小体积位于目标 AABB 并集外；这不是实体相交证明。M108 同时要求这些输出封闭、无边界/非流形/退化三角，并具备真实 UV/tangent/zone/material。
- 最新源码重建的四领域真实工作台捕获仍是 `development_visual_audit_only/not_scored/human_benchmark_evidence=false`。旧离散格状伪影已由自动 Gate 排除，且未在本轮四张截图中再出现；连接层进入同源 GLB。实际最高为航空器 6,176 renderer triangles、87 draw calls，仍低于 7,000/96 上限。主体仍是受限 Alpha blockout，并未引入 C105 Recipe、Loft 自动路由、工程机构或照片级真实性；M108 和 C105 状态不变。
- 已从最终冻结源码重建 31,793,536-byte、SHA-256 `4b0e43b2d5251bd939bcaaa90b4f62f0476d26c9139a49919f2e38abccb62560` 的 tracked macOS arm64 sidecar。该精确产物的 require-ready preflight、packaged sidecar smoke、Tauri check、经仓库 Rust wrapper 的 `.app` build 与 packaged Tauri smoke 均通过；本机空 Library、当前 PBR readback、Manifold CSG、undo/redo、导出和重启恢复成功，`provider_calls=0`，退出后没有遗留 listener。本轮没有生成或验证 DMG、签名、公证或安装结论。
- 最终 `release:packaging-readiness` 仍按设计失败：Intel macOS、Windows x64 与 Linux x64 的 tracked sidecar 仍是空占位，分别缺少有效 Mach-O/PE/ELF 与可执行条件。不得删除或放宽该阻断；本轮通过结论只覆盖本机 macOS arm64 Alpha，不覆盖跨平台安装、签名、公证或正式发布。
- PR #3 的首轮 current-head CI 在 R002 只读渲染夹具中发现唯一残留的未注册 `mat_secondary`，严格运行时白名单按设计返回 409 并使后端 Gate 失败。该 ID 在仓库没有生产使用者，因此只把 R002 夹具改为已审阅的 `mat_aluminum`，没有扩宽生产材质映射；本地 R002 已恢复通过，断言也会在未来直接输出非 200 响应。

## 2026-07-16：FGC-M108 曲面与真实取景增量（进行中，未完成）

- 真实四领域截图确认 16 段 cylinder/capsule 的车轮、旋翼、关节和胶囊外壳存在明显棱面。Worker 现在使用固定 24 段基线，真实 GLB readback 分别要求 96/432 triangles；M108 Gate 逐 ShapeProgram role 对齐 `surface_provenance` 面数，未增加新的 operation、用户细分参数或 Recipe。
- 原 0.98×三维对角线相机距离没有消费 viewport aspect，车辆车轮与机械臂首尾会在截图中裁切。无评分 kit 现记录编译 `bounds_mm`；工作台先核对 GLTFLoader metre→millimetre 后的真实三轴 bounds，再按实际 FOV/aspect/OrbitControls 基向量投影 8 个角点求安全距离。捕获要求 NDC 全部位于 `[-0.9, 0.9]`，并记录 `cameraDistanceMm`；ResizeObserver 会在 1180×1024 窄视口重新计算，而不是保留加载时的陈旧距离。
- 安全距离超过旧固定 300–820 fog 深度时，首轮新截图虽不裁切却被压黑；blockout fog 现移动到完整对象之后。退出 blockout 或 GLB/PBR 解析失败会统一清除 blockout facts，恢复 300–820 fog、ModuleGraph/空工作台、相机、地面与 shadow camera；损坏 GLB 浏览器负例已锁定该路径，不能静默保留上一预览。当前源码重建的四领域真实工作台截图已再次通过 bounds、初始/窄视口安全区、520 mm 展示对角线、PBR/环境、单 renderer/context 和 GPU Gate；本轮实际捕获最大为 6,080 triangles。24 段后的保守 renderer pass 上界为 6,776，因此仅 renderer triangle 上限调整为 7,000；geometries、textures、draw calls 和纹理显存上限保持不变。
- 新截图仍暴露方块式主体、部件贴合、连续曲面和纹理尺度问题；它们是 `development_visual_audit_only/not_scored/human_benchmark_evidence=false`，不能宣称照片级真实、完成 M108 或解除 C105 阻塞。

## 2026-07-15：FGC-M108 同源视觉 PBR 自动化检查点（进行中，未完成）

- 当前增量已闭合后端与真实工作台自动门：`agent:m108-gate` 进入 backend CI，`desktop:m108-workbench-renderer-smoke` 从当前源码重建临时 kit 后进入 workbench E2E CI。后者锁定 GLB metre→millimetre 换算、520 mm 展示对角线、实时环境 recipe SHA-256、PBR 颜色空间、单 renderer/context 和 GPU 上限；不会读取旧截图冒充通过。四领域 showcase 还改为互斥的 role 白名单细节布局；汽车已有独立 index 7 的 coated/clearcoat 五通道 `mat_automotive_paint`，不再别名到 aluminum。代表 fixture 已改善车辆座舱/轮毂/接地、飞机胶囊机身/薄翼/旋翼轮毂、机械臂胶囊连杆/夹爪和虚构道具的大三角片问题。以上仍是无评分概念资产预审，不是照片级达标；至少三位独立 reviewer 的逐领域三维度中位数门尚未收集，所以 M108 保持 `in_progress`、C105 保持 blocked。
- 新增 `VisualTextureSet@1` 与五通道 `base_color`/`metallic_roughness`/`normal`/`occlusion`/`emissive` 合同；用户登记纹理对象也扩展为五通道。当前 ShapeProgram Worker 只生成本机确定性、材质专属的 128×128 PNG：machined、brushed、coated、composite、rubber、glass 与 emissive 各有固定微表面函数，逐张将 hash、sRGB/linear、尺寸、`forgecad_builtin/not_applicable` 与 `fallback=none` 写入同一 GLB；没有 URL、绝对路径、抓取或伪造许可证。
- `_build_glb` 将 images/textures、metallic-roughness、normal/occlusion/emissive、清漆和受限 transparent/IOR extension 写入同一产物；ShapeProgram primitive 现在总是携带数值目录或受限 part-role 解析后的真实 `material_id`，轮胎/履带/握把、座舱/玻璃、灯带、关节/旋翼等不会再因旧数值索引丢失而静默回退主材质。M108 Gate 按 primitive 实际使用的 material index/role 检查材质多样性；只有实际使用深色玻璃时才以 transmission+IOR 为证据，只有实际使用信号红或独立汽车漆涂层时才以 clearcoat 为证据，不能用 GLB 中未被 primitive 引用的扩展冒充可见效果。
- `read_shape_program_glb_facts` 对嵌入 bytes/hash/色彩空间、五个 channel、透明兼容、真实 primitive `material_id`、G826 zone face set 和固定 `env_forgecad_room_studio_v1` hash 一次回读。showcase 只对真实 box 输出增加受限 `bevel_approx`：半径为 X/Z 较小尺寸的 8%、3 段并继续受既有运行时上限约束；评测包必须至少回读一个 `bevel_approximation`，这不是自由 fillet、B-Rep 或工程圆角。缺 map、损坏 image、缺透明 IOR、无受限 edge finish 或 zone/material 偏离都会明确拒绝，质量/导出继续消费同一次编译 readback。
- G826 补充了封闭基础 primitive 的外向绕序 Gate：box/wedge/cylinder/capsule、六个主轴方向的 cylinder/capsule 及受限 bevel 都逐三角验证非退化、几何法线与声明法线同向，且有平移不变的正有向体积。内置视觉 primitive 还以 `forgecad_visual_uv_repeat_mm=320` 写入统一视觉 UV 重复基线；M108 要求每个 fixture primitive 真实携带该值，readback 拒绝伪改为 321 的元数据和超过 64 的有界重复坐标。320 mm 只是纹理展示密度，不是工程材料尺寸或制造参数。
- 桌面仍只使用一个 Three.js renderer/context；其 RoomEnvironment/PMREM、linear-sRGB、ACES Filmic、1.18 exposure、接触阴影、cad-neutral 灯光和地面参数与 GLB 环境合同固定一致。地面是黑色 0.16 透明度的 `THREE.ShadowMaterial` 阴影接收面，不是写入资产的几何；默认前向 iso 按合同使用 `[-0.9, 0.85, 1.55]`、38° FOV 和 0.98 距离比。视口保留 GLB metre→millimetre 后再乘确定性 fit scale，展示对角线固定为 520 mm；阴影接收面和 shadow camera 随当前 bounds 收敛。完整 PBR 不再叠加逐 mesh CAD 边线，参数外观回退才保留边线；这些都是显示状态，不创建资产版本、第二 renderer 或新的几何真值。
- 视口现以 `blockoutGlbBase64` 为优先级最高的 Agent 预览：同源 GLB 可用时由现有 `GLTFLoader` 解析实际 baseColor、metallicRoughness、normal、occlusion 和 emissive maps，并在同一 renderer 中检查至少一套完整嵌入 PBR 材质；GLB 缺失时才显示明确标记的 ShapeProgram 参数外观回退，解析失败不会静默降级。`desktop:r3-concept-workbench-smoke` 已在浏览器中断言该路径。当前不另建 renderer，也不让 display adapter 重新成为几何/纹理真值。
- 新增真实工作台开发视觉审计命令：先运行 `npm run agent:m108-visual-benchmark-kit`，再运行 `npm run agent:m108-visual-benchmark-workbench-capture`。它在同一个 ForgeCAD 工作台、同一个 renderer/canvas 中依次导入四领域 GLB，固定 iso + `cad_neutral` + `env_forgecad_room_studio_v1`，将四张视口 PNG 和带 GLB/screenshot hash、PBR load facts、`preview_mode`、`xray`、renderer generation/context 数的 `M108WorkbenchCapture@1` 写入 `output/m108-visual-benchmark/workbench-captures/`。最新真实捕获已验证四个领域均为 `ready/glb_pbr`、`preview_mode=committed`、`xray=disabled`且始终只有一个 WebGL context；这里 `committed` 只表示非 ghost 的视口状态，不是 Git 提交或新资产版本。该 manifest 仍固定为 `purpose=development_visual_audit_only`、`score_status=not_scored`、`human_benchmark_evidence=false`；截图只能帮助开发者发现比例、材质或细节问题，不能写入 `review-responses.json`、代替独立 reviewer 或完成 M108。
- PR `#3` run `29403266593` 暴露两项自动化合同回归。`packaged-macos-alpha` 在启动 `.app` 前仍导入已重命名的 packaged fixture helper，且没有解包其新增的 GLB hash 返回值；native smoke 现复用 `_create_editable_asset_with_navigation`，并在桌面重启后核对同一 PBR GLB SHA-256。`workbench-e2e` 则把 Agent 编译 GLB 的完整五通道 PBR 门错误施加到合法但不含纹理的只读外部 GLB；当前显示缓冲会随 GLB 保存 `compiled_agent_pbr` 或 `external_reference`，前者缺完整 maps 必须失败，后者可显示原始合法材质但不会冒充 M108 PBR。外部参考的异步 GLB 回读还绑定同一 display request token；开始新方向预览后，迟到的外部 GLB 会被 reducer 拒绝，不能与新候选 ShapeProgram/segmentation 混合。具备完整 maps 的评测 GLB 即使通过只读导入进入工作台，仍报告 `glb_pbr` 并可进入人工基准。
- R3 现分别断言普通外部参考为 `external_reference/ready`、生成候选为 `compiled_agent_pbr/glb_pbr/embedded_pbr_material_count>0`；失败时写出 stage、load state、GLB kind、render source、状态消息和全页截图到 `output/playwright`，避免 CI 只留下超时行。本轮已重建 31,773,408-byte、SHA-256 `13a0ccac41fd76f5f11664ffd524fdd0f6785b2f55947cfc3a19e84390200119` 的 tracked macOS arm64 frozen sidecar；当前精确产物的 `release:packaged-sidecar-preflight -- --require-ready`、`desktop:packaged-sidecar-alpha-smoke`、`desktop:tauri-check`、经仓库 Rust wrapper 的 Tauri `.app` build 和 `desktop:packaged-tauri-alpha-smoke` 均通过。packaged 路径已覆盖本轮 PBR readback、CSG、undo/redo、重启与 `provider_calls=0`；远端结果仍以本提交推送后的 PR checks 为准。
- PR `#3` 首次 CI 的 `backend-and-contracts` 在 Ruff `E731` 停止：`visual_texture_sets.py` 的确定性 PNG chunk writer 把局部 `chunk` 写成 lambda。已改成签名等价的局部函数；`ruff check apps/agent`、M108 PBR/readback、Khronos Validator、无评分评测包和 `agent:check` 均重新通过。该修复不改变纹理字节、GLB hash 合同或评分状态。
- 评测包原先只保留空的 `review-responses.json`，无法机械锁定未来评分使用的 PBR 视口事实和门槛。现新增 `validate_m108_visual_benchmark_scores.py`：它只读取已提交的人工记录，先核对每个 fixture 的领域 ID 对应、安全相对路径、互不重复的路径/内容哈希、字节数、SHA-256 与真实 GLB/PBR readback，再要求 manifest hash、至少三个不同 reviewer ID、每人一次独立性自我声明、每人四领域覆盖、`ready/glb_pbr/embedded_pbr_material_count>0`、无 PBR load failure、完整 1–5 分数，以及每个领域的三个维度中位数均至少为 4。工具只能校验工件、ID、声明、覆盖与分数结构；真实身份及与实现工作的独立性必须由评审流程人工核验。其 smoke 只用临时合成 fixture 检验规则，并包含重复 GLB、领域错配和 GLB 被替换后的拒绝负例，绝不产生人类评分或改变 M108 状态。
- `npm run agent:m108-visual-pbr-smoke` 覆盖四领域各 3 个 showcase 多 zone fixture（共 12 个），现对每份资产直接断言至少 3 个稳定 zone；确定性重复字节、Schema、五通道、128×128 材质专属纹理、实际 material index/role、实际使用扩展、受限 bevel readback、色彩空间、文件预算和环境 hash 仍由同一 Gate 验证。负例已参数化覆盖 baseColor、metallicRoughness、normal、occlusion、emissive 五通道的引用缺失和字节损坏，并明确拒绝删除 IOR、双重 alpha/transmission 与删除/篡改已使用 clearcoat。`npm run agent:m108-gate` 将 PBR、Khronos Validator、Transform 拒绝决策、无评分 kit、评分合同 self-test 和 G826 聚合进 backend CI；`desktop:m108-workbench-renderer-smoke` 另覆盖真实工作台 GPU/环境。两者都不产生人工评分。M108 仍为 `in_progress`，未通过独立人工视觉退出条件前不能宣称完成。`release:packaging-readiness` 按预期拒绝 Intel macOS、Windows、Linux 的空 sidecar；该生产发布阻断没有被隐藏或放宽。
- PR `#3` 对提交 `93eb574` 的 run `29417412478` 在旧 `agent:g805-boolean-smoke` 停止：G805 迁移到唯一 Manifold handler 后仍把具体三角化写死为 24，而本轮修正基础 primitive 外向绕序后，Manifold 会产生语义等价但不同的合法三角化。兼容 Gate 现改为从真实 GLB readback 核对非零 triangle、bounds、Material Zone、`manifold3d==3.5.2`、正确 operation、closed 与 Feature History/result triangle 一致，并继续要求 subtract 保留 `boolean_cut`、disjoint union 不伪造 cut；没有修改生产 CSG handler、预算或失败边界。远端结论以修复提交触发的新 checks 为准。
- 后续 run `29418322631` 已越过 G805，在旧 `agent:g806-bevel-surface-panel-smoke` 的固定 triangle 常量停止：修正后的封闭 bevel 上下盖补齐了原来缺失的四个扇形三角，因此 1/3 段 bevel+panel 的合法 readback 为 44/76，而非旧常量 40/72。G806 Gate 现按同一 GLB readback 核对精确受限细分公式、bounds、primitive/normal/UV0/tangent、Material Zone face 总数、`surface/trim`、封闭/零边界/零非流形/零退化与 Feature History，并要求三级细分实际增加 triangle；没有修改生产几何、运行时白名单或倒角上限。远端结论仍以本修复提交后的 PR checks 为准。
- 提交 `654ed3c` 的 run `29419173330` 已让 `backend-and-contracts`、desktop、`packaged-macos-alpha`、`g824d-windows-packaged-candidate`、Cargo、integrity 与 secrets jobs 通过，但 `workbench-e2e` 在 F001 选择飞机方向后等待“分件候选”的旧 20 秒单步上限停止。这里一次点击会串行执行 `POST /api/v1/agent/blockouts` 和 `POST /api/v1/agent/blockouts:segment`，而 segment 会再次生成用于 AssemblyGraph 的同一候选；候选卡只在第二个响应进入状态机后挂载。共享浏览器 helper 现于点击前精确监听两个端点，以测试专用、可配置的 `FORGECAD_AGENT_GEOMETRY_TIMEOUT_MS`（默认 90 秒）作为完整 build→segment 串行链路总预算，要求两个响应均为 201，并核对非空 `glb_base64`、`ShapeProgram@1`、正 triangle、同一 artifact/direction、`candidate` 和非空 parts；两个 API 成功后仍只给 UI 20 秒落稳，原有含糊输入零写入、preview 不写版本、commit/Snapshot/export 一致、重启、legacy 隔离与单 canvas 断言均未删除或放宽。R3/T002 的真实质量检查也先等待并检查对应 HTTP 响应，再要求 Q003 `geometry_compile_readback`、`GeometryCompileReadback@1/passed`、报告与 readback 的 triangle/bounds 一致、有效 GLB SHA-256、当前资产及 Snapshot 报告 ID 对齐，并保留原 UI 断言；这避免慢机器把合法 readback 误报成选择器失败，同时不会掩盖 4xx/5xx、旧报告或错误 payload。本地 F009、F001、R3、T002 14/14 与 desktop build 已通过；远端结论以新提交的 PR checks 为准。
- 新增锁定开发/CI 依赖 `gltf-validator@2.0.0-dev.3.10` 和 `npm run agent:m108-gltf-validator-smoke`：同一编译链的四领域原始 showcase GLB 必须由 Khronos Validator 得到零 error、零 warning，畸形 GLB 必须被拒绝。该检查发现并修复 `_FORGECAD_FACE_ID`/`_FORGECAD_SOURCE_FACE_ID` 原先写成无效 `UNSIGNED_INT` 顶点属性的问题；现以精确整数 FLOAT custom attribute 保留 readback，不把 Validator 报告当作资产真值。
- `npm run agent:m108-gltf-transform-evaluation` 锁定 `@gltf-transform/core/extensions@4.4.1` 为开发评估依赖。四份同源 showcase GLB 先由 ForgeCAD 真实 readback 验证，再在 glTF Transform 标准读取阶段对比 Part instance、zone、material 和 VisualTextureSet 映射；写回后的 GLB 仍通过 Khronos Validator，且标准重读仍保留该映射。但 writer 会删除 `baseColorFactor`、`metallicFactor`、`roughnessFactor` 和 `occlusionTexture.strength` 等 ForgeCAD 不可变 readback 必需的显式默认值；Gate 现要求这四份写出全部以该精确原因被 readback 拒绝，然后以 `decision=reject_core_writer_as_export_transform` 和成功退出固化“不采用”决策。`functions` 的 dedup/prune 与 KTX2/BasisU 没有进入生产或新的资产真值。
- 已重建 tracked arm64 frozen sidecar，并扩展 `npm run desktop:packaged-sidecar-alpha-smoke`：showcase PBR 的五通道、固定工作室环境、ChangeSet preview/confirm 后导出、undo 恢复初始 PBR GLB、redo 恢复编辑后 PBR GLB、CSG 后 GLB readback 及重启后同 asset GLB SHA-256 全部通过。`agent:m108-visual-benchmark-kit-smoke` 现可生成并核对一份无分数、四领域同源 GLB 审阅包；真实独立评审的人工核验流程与每领域各维度 4/5 中位数门槛固定在 `docs/evidence/M108_VISUAL_BENCHMARK_PROTOCOL.md`。M108 仍是 `in_progress`，直到真实独立评分被收集并复核；不能因此把当前 Alpha 描述为照片级真实产品或解除 C105 阻塞。

## 2026-07-15：FGC-A004 受限 Agent Action Loop（已完成；M108 in progress）

- 新增 `AgentActionLoop@1` 与不可动态扩展的 `ForgeCADProductToolRegistry@1`。13 个工具覆盖领域推断、受审本地参考查询、Style Token/比例配方、Profile/ShapeProgram author+validate、候选 build、真实 compile/readback、四视图 render、硬门 evaluate 和未保存 preview；没有 shell、Python/JavaScript、任意 URL/路径、通用 MCP、数据库或永久修改工具。
- 离线 Planner 与 DeepSeek 都通过同一工具循环执行 plan→build→GLB readback→render→evaluate→preview。DeepSeek Tool Call 的 `reasoning_content` 只在同一 Turn 的内存消息中续传；持久化 Item 只记录 stable tool/call ID、父 Turn、Schema 后事实、状态、耗时、幂等键、失败类别与审批策略。12 次调用、wall time、取消、Provider 断线、重复 call ID、stale Snapshot 和 G819 未知操作都 fail closed。
- `npm run agent:a004-action-loop-smoke` 覆盖正常链、DeepSeek 多轮续传且推理不落盘、Schema/G819 拒绝、上限、取消/timeout/断线、重复 Registry/Tool Call ID、stale Snapshot、审批前 `agent_asset_versions`/ChangeSet/Snapshot 为零，以及 completed/failed Turn 重启读取。新增只读 `GET /api/v1/agent/product-tools`；桌面 Turn 完成后不再自动并发三次方向 concept-preview API，当前三方向 UI 仍保留到 V003。
- A004 后续修正 packaged Alpha smoke：它只按 `plan_complete_concept` 提取计划，并显式兼容当前 `{tool_name, result: {plan}}` 与冻结 sidecar 的 legacy `{tool, result: plan}` 合同，不能再把同一 Turn 的 readback/render/evaluate/preview 误判为计划。本机冻结 arm64 sidecar 已越过该解析点，但其 GLB 早于 G826、缺少 UV/tangent/provenance，因而不能作为当前表面合同的本地证明；当前源码打包 CI 才是此 Gate 的有效证据。
- 当前唯一主链任务为 `FGC-M108`：只消费 G826 的真实 zone/UV/tangent，完成多区 PBR 纹理、色彩空间、环境与 GLB/视口/readback 一致性；不得提前实现 C105/V003/F026。

## 2026-07-15：FGC-D005 四领域语义比例配方（已完成；A004 ready）

- 新增 `MechanicalStyleToken@1`、`DomainSemanticProportionRecipe@1`、`ResolvedSemanticProportionOptions@1`，四领域各 4 个普通语言配方。实际候选使用大量变体 role，因此 D005 以稳定语义部件槽映射当前 AssemblyGraph Part，再要求真实 G808 ratio binding 与 G826 GLB `surface_provenance/source_operation_ids` 同时存在；不按猜测角色或 UI 估算提供按钮。
- 新只读 API 为当前活动 asset/part 返回 ShapeProgram/GLB hash、锁定状态、current/target/min/max/step/unit 与来源操作；无绑定、无表面来源、锁定和外部 GLB 明确回退。桌面 `AgentSemanticProportionControls` 只创建现有 `set_part_parameter` preview，确认仍由 ChangeSet/CAS 创建不可变子版本；无 localStorage/Snapshot 配方偏好或新几何 operation。
- 专属 Gate 已通过四领域目录、JSON/Pydantic/OpenAPI、真实编译/GLB readback、锁定/越界/步长拒绝、无绑定回退、preview 取消/确认、Q003 质量、重启和 undo/redo；UI Gate 覆盖中文、相对倍数、范围/步长和非工程提示。下一唯一可领取任务为 `FGC-A004`，用于受限 Product Tool Registry Action Loop；D005 本身没有让 Planner/DeepSeek 自动选择配方。
- 本轮最终通过 `contracts:types:check`、`agent:check`、Agent 18 项单测、G6/G808–G810/G819/Q003/G826/D005、desktop D005/typecheck/build/F006/F025/T002（14/14）/T003/r3，以及 docs walkthrough、repository integrity、safety scope、secret files 和 `git diff --check`。Vite 仍报告既有单 chunk 警告，但 T003 最终 bundle 1,076,132 bytes 低于 1,200,000 bytes 门槛；无 Gate 失败。工作分支为 `codex/repository-integrity`，提交与远端 PR 结果以 Git 历史/PR checks 为准。

## 2026-07-15：FGC-F025 Agent/legacy 工作台隔离（已完成；D005 ready）

- Agent-active 首次进入只读取 Project shell；只有点击“查看旧版只读信息”才读取旧版本、ChangeSet、审计和 ModuleGraph。关闭、项目切换及迟到响应均由 request guard 清理，不能重新挂载 legacy 表面。
- 新 `WorkbenchInspectorRail` 将 Graph Inspector、`WeaponParameters`、旧质量摘要和 SOURCE ZIP/OBJ/PNG/MP4 说明限制在显式只读边界；Agent 质量/导出抽屉不再接收 legacy props，Agent Turn 和修改意图不再回退调用旧 Planner。
- 首次推送的 Linux `workbench-e2e` 暴露 D003 路径漂移：真实 Kernel 的 ambiguous clarification 只返回两个推断候选，而 API 错误适配层返回四项。修复后两条路径都保持一个问题和四个安全领域选项，并由后端/UI D003 smoke 共同验证；没有放宽零资产写入或 legacy fallback 断言。
- F025 后 Linux R3 稳定执行真实 Agent 路径，不再用 legacy Planner 回退；无 GPU runner 超过旧 180 秒全局 watchdog，但没有单步断言失败。全局上限调整为 360 秒并增加九段阶段诊断，既有每步 20 秒超时、Snapshot/质量/导出/重启与单 renderer 断言均保留。
- F025 Gate 按文本边界记录 `CadWorkbenchPanel.tsx` 从 3,032 行降至 1,872 行；仍只装配一个 `ModuleGraphViewport`。新增 `desktop:f025-legacy-isolation-smoke`，并通过 F001、F006、T002（14/14）、T003、r3、typecheck/build。完整文档/安全/合同 Gate 与最终 commit/push/PR checks 记录以本轮结束结果为准。
- 下一唯一可领取任务是 `FGC-D005`：只建立四领域非工程语义比例/Style Token 配方与 G808/G811/G819/G826 允许的受限参数绑定，不增加自由尺寸、工程结论或新几何执行路径。

## 2026-07-15：FGC-A003 DeepSeek Provider Gateway 可观察性（已完成；F025 ready）

- 新增 `ProviderConnectionState@1` 与脱敏 `ProviderExecutionTrace@1`。Tauri Provider 保存/清除依次验证 metadata、Keychain、受管 supervisor restart 和本机 Agent capability；未配置固定报告 `unconfigured`、`network_call_made=false`，配置读取/重启/能力失败不再被 UI 吞掉。工作台明确区分“未调用 DeepSeek”“等待显式调用”“测试连接（会联网）”及稳定失败原因。
- OpenAI-compatible Planner 改用 SSE 并请求最终 usage；普通 Turn 与显式连接测试支持取消，生命周期记录 preflight/request_started/streaming/validating/completed/failed/cancelled、latency、usage/cache token、attempt、网络事实和 `fallback_used=false`。JSON mode prompt 明确要求 JSON 并包含版本化输出示例；`reasoning_content`、完整 prompt/response、Key、header、Base URL 不持久化。
- DeepSeek 400/401/402/422/429/500/503、网络、timeout、空 content、无效 JSON、Schema 不符和 Tool Calls 均映射为稳定错误。Provider 失败不自动重试、不进入 legacy plan/build，也不静默伪装成 deterministic success；失败、取消及迟到 completion 均不改变 AssetVersion、Snapshot、质量或导出。
- 新增 `agent:a003-provider-gateway-smoke`、`desktop:a003-provider-connection-smoke` 与 Rust 兼容/脱密测试。最终本机回归通过：A003 两个 Gate、G1–G7、受影响的 Q003/G809/G819/G825/C102、Agent 18 项单测、Rust 6 项、desktop typecheck/build/Tauri check、T002 14/14、T003、Agent-first r3、contracts、docs walkthrough、repository integrity、安全范围、密钥文件与 `git diff --check`。首次 T002 暴露旧前端只读取第一个 `tool_result`，已改为按 `MechanicalConceptPlan` payload 识别并用“Provider trace 在前”夹具回归；首轮远端 packaged macOS 又暴露 sidecar smoke 统计所有 `tool_result`，同样改为只选非空 `payload.result`。生产返回均正常，两个测试都不再把 Provider 审计 Item 当计划；不得删除这些覆盖。所有联网语义由本机 fake Provider 验证；没有读取真实 Key、执行 E003 或宣称真实模型质量/费用。commit/push 与 PR checks 以本节后续最终记录为准。
- 下一唯一可领取任务是 `FGC-F025`：只隔离 Agent 主流程中的 legacy 参数、旧导出和 Graph Inspector，并继续拆薄 `CadWorkbenchPanel`；不得在该任务提前改简洁布局、几何、材质或加入第二 renderer。

## 2026-07-15：FGC-G826 表面完成与稳定 Material Zone 面事实（已完成；A003 ready）

- `GeometryCompileReadback@1` 已增加 `tangent_primitive_count`、逐 primitive surface completion 和 `material_zone_faces`。Worker 在同一 GLB 中写出 `TANGENT`、`_FORGECAD_FACE_ID`、`_FORGECAD_SOURCE_FACE_ID`、稳定 `primitive_id`/`part_instance_id`/zone extras；readback 验证单位 normal/tangent、正交性、handedness、UV0 非退化、face ID 完整唯一、zone 非空不重叠和来源 operation。每个三角面拆分顶点并携带 face 属性，因此顶点/索引重排不能靠顺序丢失映射。
- 受控 edge finish 仍只复用 `bevel_approx`：X/Z 周边四边、半径比例 `<= 0.25`、1–3 级细分，readback 名称为 `bevel_approximation`。`surface_panel` 明确回读为 `trim`。没有加入纹理、HDRI、clearcoat、工程材质或精确 fillet；M108 后续只能消费这些几何前置事实。
- 新增 `agent:g826-surface-readback-smoke`，覆盖基础 primitive、Profile/Extrude/Revolve/Loft/Sweep、edge finish/trim、mirror/array、Manifold CSG、重复 GLB/hash，以及缺失/损坏 tangent、UV 退化、空/重叠 zone、face ID 损坏和半径/细分/三角预算失败。严格 Gate 同时暴露并修复了 legacy Extrude cap/side 的退化 UV 和 legacy Revolve 负 V 坐标；没有降级或跳过旧运行时操作。
- 本轮通过：G1–G7、G802–G806、G819、Q003、G821–G823、G825、G826、R2 export、M101–M107、18 个 Agent unit、Ruff/compile、contracts、desktop M104–M106、typecheck/build、T002/T003、Agent-first r3、Tauri cargo check、docs walkthrough、repository integrity、安全范围、密钥文件与 `git diff --check`。Vite 仍只有既有 dynamic-import/chunk warning；M103/jsonschema 仍只有既有 `RefResolver` deprecation warning。当前 8000 端口上的用户 Agent 进程未被终止，r3 使用现有运行环境并通过。
- 下一唯一可领取任务是 `FGC-A003`：DeepSeek Provider metadata/Keychain preflight、真实网络调用标记、流式/取消/用量与稳定错误分类。G826 完成不代表 DeepSeek 已配置，也不代表真实纹理、单一最佳候选或简洁工作台已完成。提交、push 与 PR checks 以本节后续最终命令记录为准。
- G826 已以 `293a49e` 提交并推送到 `codex/repository-integrity`；PR #3 的 backend/contracts、desktop、macOS packaged、Windows CSG、依赖审计、三平台 cargo、integrity、secrets 和 workbench E2E 全部通过。2026-07-15 已领取 A003；开始前工作区与远端 SHA 一致且干净。

## 2026-07-15：FGC-G825 单一生产 CSG 与不可变 Feature History（已完成；G826 ready）

- 只按 ADR-0013 将 `manifold3d==3.5.2` 与 NumPy 2.4.6 接入现有 Python sidecar；`manifold_csg.py` 是 union/subtract 的唯一生产 handler，旧 G805 box 路径不再作为 fallback。输入须封闭并满足 8 层深度、32 solids、200,000 输入 triangles 与既有 ShapeProgram 预算；隔离子进程支持取消/5 秒 timeout，且不接收数据库、对象库、Snapshot、URL 或文件路径。
- `GeometryCompileReadback@1.feature_history` 和 GLB root extras 现在按 ShapeProgram 顺序保存 node/op、input IDs/hashes、规范参数/node input/result/provenance hash、runtime/kernel version、CSG depth、triangle/closed、material/zone/surface role；布尔逐三角保留 source operation/part/material/zone/face/backside，并将切面标为 `boolean_cut`。旧 G824 证据 GLB 仍可只读，但新 Worker 编译缺少 Feature History 会失败。
- `agent:g825-feature-csg-smoke` 覆盖闭合壳体 union、窗洞/轮拱/凹槽 subtract、coplanar、近退化、非封闭、超深度、输入/三角预算、取消/超时、重复 GLB/result hash、旧 G805、preview 零版本副作用、confirm 不可变子版本、父版本不改写、质量和导出 GLB 同源 readback。失败保留稳定 CSG code/node ID 且不输出部分 GLB。
- 生产 pyproject/release lock、PyInstaller collect/hidden import、frozen `multiprocessing.freeze_support()`、生成 Schema/TypeScript/OpenAPI、CI backend Gate 和许可证账本已同步。重建的 macOS arm64 sidecar 为 31,622,192 bytes；扩展后的 `desktop:packaged-sidecar-alpha-smoke` 已让 frozen binary 实际执行 subtract 子进程，GLB 回读确认 Manifold 3.5.2、closed 和 `boolean_cut`，随后通过重启恢复。该能力仍是受限概念级 CSG；Planner/UI 尚未自动采用，不提供自由 mesh 修复、B-Rep、工程实体或制造结论。
- 本轮已通过：G1–G7、G805、G819/Q003、G820–G825、18 个 Agent unit、Ruff/compile、contracts、docs walkthrough、repository integrity、安全范围、密钥文件、license/SBOM、desktop typecheck/build、Agent-first r3、sidecar preflight/build/真实 frozen CSG Alpha、cargo check 与 `git diff --check`。Vite 仍只有既有 chunk/dynamic-import warning。`release:packaging-readiness` 按预期继续失败，因为 Intel macOS/Windows/Linux sidecar 仍为空占位；不得删除该发布阻断。`desktop:packaged-tauri-alpha-smoke` 未运行，因为用户当前打开的既有 CAD 工作台占用固定 8000 端口；没有擅自终止该应用。
- 下一唯一可领取任务是 `FGC-G826`：受控 edge finish、法线、UV0、tangent 与稳定 Material Zone face provenance。提交、push 与 PR checks 以本节后续最终命令记录为准。

## 2026-07-15：FGC-G824D Windows packaged evidence（历史交接；其后 G825 已完成）

- GitHub 登录已恢复，用户明确授权 commit/push 当前工作区；分支 `codex/repository-integrity` 已推送到 Draft PR #3。主工作区提交为 `f12aa381`，后续 CI 修复截至 `6a9edefa`。
- GitHub Actions run `29383382978` 的真实 `windows-2022` frozen sidecar job 已通过并上传 `g824d-windows-packaged-candidate`。报告保存为 `evaluations/csg-g824d/windows-report.json`，再次运行 `check_g824d_windows_packaged_candidate.py` 通过。
- Windows AMD64/Python 3.11.9：executable 35,788,283 bytes，健康冷启动 2,528.125 ms；五组有效 fixture 的 provenance/GLB readback 通过，near-degenerate 在写出前拒绝；三个中断窗口均回收进程、清理 staging、保持 SQLite/对象库不变，Version/head/Snapshot 原子回滚/提交通过，Provider 调用为零。
- ADR-0013 在该任务时选择 `manifold3d==3.5.2` 作为 G825 唯一生产候选；生产依赖和默认 handler 当时仍未改变。该历史边界已由上节 G825 集成取代。
- 为使干净 runner 与 Windows 语义一致，修复了 PyInstaller `_MEIPASS` 资源定位、C104 完整 ShapeProgram fixture、F006 10px 文本、desktop/Agent CI 隔离、sidecar 空输入临时夹具、D003 历史迁移夹具及 Windows SQLite 清理。backend、desktop 与 G824D 已在同一 run 通过；完整 PR checks 仍应以 `gh pr checks 3` 的最终状态为准。

## 2026-07-15：FGC-G824D Windows packaged evidence runner（历史阻断记录；已由上节解除）

- 新增 Windows x64 PyInstaller runner：实际当前 sidecar 入口通过 runtime hook 在 frozen binary 内运行 Manifold Python 六组 provenance/readback、near-degenerate 拒绝，以及 busy cancel、busy timeout、valid GLB ready-before-promotion 三个窗口；父进程用真实临时 SQLite、对象库和 UnitOfWork 验证零部分提升与原子回滚/提交。
- `.github/workflows/forgecad-core.yml` 新增独立 `windows-2022` job，固定 Python 3.11、PyInstaller 6.16.0、Manifold 3.5.2、NumPy 2.4.6；成功或失败均尝试上传 `g824d-windows-packaged-candidate` JSON artifact。候选进程只接收 staging marker/result/GLB 路径，不接收权威路径或 Provider 配置。
- 本机 runtime hook 已用隔离候选目录验证六组 fixture：五组真实 GLB readback/provenance 通过，near-degenerate 以 `CSG_DEGENERATE_OUTPUT` 在写出前拒绝；Ruff、compile 和 diff check 通过。连续三个目标回合均确认本机没有 Wine/QEMU Windows 环境、GitHub CLI 凭证失效且未获 commit/push 授权，所以不存在真实 Windows artifact，任务标记 `blocked`。恢复认证并明确授权发布该工作区，或提供 Windows x64 环境后，先运行/校验 artifact；在此之前不得新增采用 ADR 或领取 G825。

## 2026-07-15：FGC-G824C macOS packaged candidate（已完成；建议 Python，仍未正式采用）

- 隔离临时目录使用当前 `sidecar_entry.py` 实际构建并启动含 `manifold3d==3.5.2`/NumPy 的 arm64 PyInstaller onefile；archive、runtime hook 强制 import 与真实健康检查通过。构建没有覆盖仓库 sidecar，也没有修改生产依赖、lock、manifest 或 handler。
- 当前基线/候选包体为 19,445,536/24,207,728 bytes，增量 4,762,192 bytes；同轮冷启动 18,250.329/19,243.281 ms，相对回归 992.951 ms；候选完整进程树峰值 RSS 87,376 KiB。均通过固定的 48 MiB 总包体、28 MiB 增量、5 秒相对冷启动和 300 MiB RSS 预算。
- Manifold Apache-2.0 与 NumPy BSD-3-Clause/捆绑许可证文件已记录版本和 SHA-256；PyInstaller 需要显式 hidden import `numpy._core._exceptions`。WASM 不适配当前 Python sidecar 执行宿主，因此建议唯一候选为 Python，但状态仍是 `recommended_pending_windows_runtime`。
- 新 Gate `agent:g824c-packaged-candidate-smoke` 校验提交报告及生产依赖隔离。Windows x64 packaged sidecar 同 fixture 与 superseding ADR 仍未完成，G825 继续 blocked。本轮未 commit、未 push。

## 2026-07-15：FGC-G824B CSG staging/权威状态提升补证（已完成；仍未采用候选）

- 全量迁移建立真实临时 SQLite 和 `ContentAddressedStore`，保存活动 Agent v1/head/Snapshot/proposed ChangeSet。Python/WASM 候选子进程不接收任何权威路径，只能写事务外 staging。
- 两个候选在 kernel cancel、kernel timeout、valid GLB ready-before-promotion 三个窗口终止；Version/head/ChangeSet/Snapshot/quality/import/idempotency 和对象库 fingerprint 均不变，staging GLB 清理。真实 `SQLiteUnitOfWork` 注入 Version/head/Snapshot 提升失败会整体回滚，成功会整体提交到 v2/revision 2。
- 新 Gate `agent:g824b-csg-promotion-boundary-smoke` 校验报告。该时点 Windows x64 packaged sidecar、唯一候选 packaged 预算/许可证与 superseding ADR 尚未完成；后续 G824C 已补齐 macOS packaged 证据并建议 Python，但 Windows 与正式采用仍阻断 G825。本轮未 commit、未 push。

## 2026-07-15：FGC-G824A CSG provenance/readback/取消补证（已完成；仍未采用候选）

- Manifold Python/WASM 的输入使用不同 source/material/zone property channel；四领域 union/subtract、coplanar 与 near-degenerate 在 `simplify` 后按 original ID、face ID 和 backside 建立逐三角 provenance。五组有效 fixture 生成相同 GLB hash 并通过 ForgeCAD triangle/material/surface/custom provenance readback；near-degenerate 以 `CSG_DEGENERATE_OUTPUT` 在部分 GLB 前拒绝。
- 两个候选分别用隔离进程验证 `CSG_CANCELLED`/`CSG_TIMEOUT`：marker 后终止、进程回收、无候选 GLB，隔离 Snapshot/Version/cache sentinel 不变。它不等于真实生产 Worker/数据库事务已经验证。
- 新 Gate `agent:g824a-csg-adoption-evidence-smoke` 校验报告及生产依赖继续隔离。Windows x64 packaged sidecar 仍未实机执行，且没有 superseding ADR 选择唯一候选；G825 继续 blocked。本轮未 commit、未 push。

## 2026-07-15：FGC-G824 CSG 内核隔离 benchmark（已完成；未采用候选）

- 临时目录固定比较当前 handler、`manifold3d==3.5.2`（commit `11235e6...`）和 `manifold-3d@3.5.1`（commit `cc8a7f66...`）；报告记录 macOS arm64 环境、安装/运行命令、Apache-2.0、包增量、冷/热时间、峰值内存、四领域 fixture、coplanar/near-degenerate 和重复 mesh hash。
- Python/WASM 在本机 fixture 中都成功且产生相同 hash；但 ForgeCAD material/surface/zone provenance、operation 取消/稳定错误码和 Windows 实机 packaged runtime 均未证明。ADR-0012 因此明确不采用候选，G825 保持 blocked，并列出解除条件。
- `agent:g824-csg-benchmark-smoke` 校验报告和生产依赖隔离；没有修改 production manifest、Worker handler、Python/npm 依赖或锁文件。临时目录删除即可移除候选。本轮未 commit、未 push。

## 2026-07-15：FGC-G823 受限路径 Sweep（已完成）

- `sweep` 已进入唯一 runtime manifest，只消费 canonical closed/hole-free `ProfileSketch@1` 与 2–32 点有界 3D path。Worker 使用确定性 parallel-transport frame，支持开放路径有限 twist、开/闭 path、固定 sample seam 和显式 cap。
- 运行边界拒绝零长度、短于截面视觉比例的段、接近 180° frame 翻转、明显路径自交、闭合 path cap/twist、点数/bounds/triangle 超限；GLB readback 验证 `sweep_side/seam/start_cap/end_cap`、UV0、normal、closed/boundary/non-manifold/degenerate 和连续 triangle ranges。
- 新 Gate `agent:g823-sweep-smoke` 覆盖直线、折线、多点平滑近似、有限 twist、开/闭 path、封盖、重复字节、拓扑和失败预算；G819/Q003/G820–G822、contracts 与 Agent 回归通过。下一项唯一 ready 为 G824 布尔 benchmark/ADR；Planner/UI 尚未自动采用 Sweep。本轮未 commit、未 push。

## 2026-07-15：FGC-G822 受限多截面 Loft（已完成）

- `loft` 已加入 `ShapeProgramRuntimeManifest@1` 的唯一 operation 真值，并由 Schema、Pydantic/semantic validator 和 Worker 共用；只接受 canonical `ProfileSectionSet@1`、2–12 个统一采样闭合无孔截面、严格 section 顺序、有界二维 scale/axis length、有限 twist、固定采样 seam、`linear` continuity 和首尾 cap。
- Worker 新增确定性多截面网格，支持 x/y/z 主轴和截面 scale/twist；GLB 保留 `loft_side/seam/start_cap/end_cap` 连续三角范围、UV0、normal 和 profile provenance。编译在 GLB 写出前先拒绝三角预算，readback 再验证 triangle/bounds、闭合、boundary/non-manifold/degenerate 与 accessor/range 一致性。
- 新 Gate `agent:g822-loft-smoke` 覆盖汽车、飞机、家电和机械臂四类壳体 fixture、曲线/矩形截面、尺寸/位置/扭转/封盖、重复字节与真实 readback，以及排序、混合采样数、翻转风险、自交、退化、bounds、损坏 hash 和预算失败。G819/Q003/G820/G821/G807/G818、G1–G7（含 Agent asset commit）、contracts、Agent unit/check 已通过。
- 下一项唯一 ready 为 `FGC-G823`，只实现受限 Sweep path/frame runtime。当前 Planner/UI 不自动生成 Loft，用户指南没有新增自由轮廓或放样入口；孔洞 Loft、Sweep、稳健 CSG、NURBS/B-Rep、PBR/Recipe 均仍未实现。本轮未 commit、未 push，并保留既有脏工作区。

## 2026-07-15：FGC-G821 增强 Profile/Extrude/Revolve（已完成）

- 现有 `profile` operation 新增 canonical `profile_input_id` + 二维 `profile_scale` 分支，直接消费 G820 重采样结果；旧 `args.points` 保持兼容且禁止混用新参数。Extrude 支持曲线、孔洞、独立首尾 cap 和明确开放 ribbon；Revolve 支持轴点、完整/部分角度、8–64 radial segments 与部分角 seam cap。
- 服务端三角化保留外轮廓/孔洞方向；轴点 Revolve 使用单三角扇，避免退化四边形。GLB primitive extras 保存 side/hole_wall/start_cap/end_cap/seam 的连续 triangle ranges；真实 readback 解析 POSITION/NORMAL/UV0/index，校验 accessor 对齐、UV 范围、range 覆盖、closed/boundary/non-manifold/degenerate topology。profile 结果出现退化面会失败，不写部分资产。
- 新 Gate `agent:g821-profile-solid-fidelity-smoke` 覆盖带孔/无孔/开放 Extrude、完整/部分 Revolve、轴点、封盖、seam、UV0、表面区间、拓扑、重复 GLB、损坏 hash、负半径、孔洞 Revolve 和 triangle budget。G1–G7、G802/G803、G819/Q003/G820、contracts、Agent unit（16 passed）/compile/ruff、desktop typecheck/build、文档 walkthrough、repository integrity、安全范围、密钥文件和 `git diff --check` 均已通过；Vite 仍只报告既有大 chunk/dynamic import 警告。
- 下一项唯一 ready 为 `FGC-G822`，只实现受限多截面 Loft。当前 Planner/UI 尚未自动采用新 Profile，用户指南不增加自由轮廓能力。本轮未 commit、未 push，继续保留既有脏工作区。

## 2026-07-15：FGC-G820 ProfileSketch 与截面合同（已完成）

- 新增 `ProfileSketch@1` 与 `ProfileSectionSet@1` JSON Schema、Pydantic 模型和生成 TypeScript/Python registry。合同限制 normalized `[-1,1]` 的 line/quadratic/cubic、闭合/开放和实际绕序、最多 8 个孔洞、统一重采样，以及 2–12 个严格排序截面的有限 position/scale/twist/cap；自由 SVG、URL/路径、非有限、退化、自交、孔洞越界/重叠和预算失败均在 Worker 前拒绝。
- `profile_contracts.py` 提供确定性曲线采样、规范化和 canonical SHA-256：外轮廓统一 counter-clockwise，孔洞统一 clockwise，等价顺逆输入产生同一 hash。ShapeProgram 新增可选 `profile_inputs` provenance，保存 canonical payload、合同版本和 input hash；不一致即拒绝，旧 ShapeProgram 不带该字段仍原样通过。
- 新 Gate `agent:g820-profile-sketch-contract-smoke` 已通过；任务前/后 G819、Q003、G802、G803、`contracts:types:check`、`agent:unit`（16 passed）、`agent:check`、`.venv/bin/ruff check apps/agent`、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files` 和 `git diff --check` 均已通过。本轮没有新增 Loft/Sweep/Worker operation 或用户 UI，当前用户指南不变。
- 下一项唯一 ready 为 `FGC-G821`：只让现有 Profile/Extrude/Revolve 消费 G820 合同并补曲线、孔洞、封盖、UV0 和 surface provenance。工作区继续保留用户既有大量未提交修改；本轮未 commit、未 push。

## 2026-07-15：FGC-Q003 真实编译/GLB readback 质量真值（已完成）

- 新增 `GeometryCompileReadback@1` JSON Schema/Pydantic 合同，同一次编译产生 program/GLB hash、字节数、triangle、bounds、mesh/primitive/material 数、operation 与 output role 事实。生成类型和 OpenAPI 已同步。
- 质量检查已删除 box/cylinder 常数估算，并将 readback 嵌入不可变报告；导出使用同一 compile/readback 结果。损坏回读生成 `compile_failure/unavailable` 质量或 `GEOMETRY_READBACK_FAILED` 导出拒绝，未知操作仍由 G819 无副作用拒绝。
- 旧 `legacy_estimate` 报告读取时隔离为 unavailable，且不再成为组件来源质量证据。Q002 的 Snapshot ETag/Idempotency-Key 重放保留，新 quality request hash 防止旧估算响应被当作 Q003 报告重放；真实 legacy v1 readback 只有在精确清单、authored→canonical 映射和 material index 一致时才能迁移缺失字段，当前 v2 导出下的旧结论以 `stale_compile_readback/unavailable` 隔离。
- 新 Gate `agent:q003-compile-readback-quality-smoke` 已通过，覆盖四领域、导出 hash/数字一致、损坏 readback、未知操作、旧报告隔离与重启幂等。G801–G818、G819、G5/G6/G7、Q002、C102、T002（14 场景）、T003、r3、desktop typecheck/build、`contracts:types:check`、`agent:check`、文档/安全/密钥 Gate 和 `git diff --check` 均已通过。
- 下一项唯一 ready 任务为 `FGC-G820`。本轮未 commit、未 push，保留用户现有脏工作区。

## 2026-07-15：FGC-G819 运行时操作 manifest 单一真值（已完成）

- 已新增 `packages/concept-spec/fixtures/shape-program-runtime-manifest.json`（`ShapeProgramRuntimeManifest@1`），唯一声明 14 个当前可执行操作与其 executor；`scripts/generate_schema_types.py` 从此文件生成 `shape-program.schema.json` 的 operation enum，`contracts:types:check` 会拒绝 schema/manifest 漂移。
- `ShapeProgramPayload` 让 Pydantic Agent response/version 输入复用同一 Schema/manifest 校验；`shape_program.py` 在 JSON Schema 前拒绝未知操作；Geometry Worker 在每次编译前核对 manifest executor coverage，所有原先的执行循环静默 `continue` 已改为明确拒绝。`build_blockout` 也改走同一个 GLB 编译/readback 入口。
- preview、confirm、质量入口和导出在写入/输出前统一编译并检查运行时兼容性，未知、非法或缺少执行器返回 `UNSUPPORTED_RUNTIME_OPERATION`；损坏的持久化 ShapeProgram 也会在 Pydantic 读取边界拒绝。质量入口在本任务只使用该 compile/readback 作为拒绝门，仍不消费其 readback 数字。`agent:g819-runtime-operation-manifest-smoke` 覆盖 manifest 中每项操作、未知 `pivot` 与故意移除 executor，分别验证 preview/confirm/quality/export 零副作用。
- 本轮已通过：G1–G7（含外部 GLB 只读参考回归）、G3、G801–G807、G812–G815、G817/G818、G819、Q002、`agent:unit`（16 passed）、`agent:check`、`contracts:types:check`、`desktop:typecheck`、`desktop:build`、T002、T003、r3、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files` 和 `git diff --check`。组件及 r3 smoke 现从服务器已声明的角色/参数/连接器读取 fixture，不再假定 `upper_link` 或“长度比例”。未 commit、未 push；继续保留用户既有脏工作区。
- 下一项唯一 ready 是 `FGC-Q003`：质量报告仍以 manifest 声明的旧估算模式计算 box/cylinder 数字；必须改为同一次真实 compile/readback 的只读事实，不能把 G819 的拒绝边界误写为 Q003 已完成。

## 2026-07-15：3D 机械设计系统、混合建模语法与目标操作手册（仅文档设计）

- 用户确认不以 HTML/CSS 六面拼接或单一 box 连续裁剪作为最终路线，而是借鉴 UI 组件库思想建立 3D 机械设计系统：HTML/React 负责工作台，SVG 只编辑规范化 Profile，GSAP 只做状态过渡；主形体使用 Profile/Extrude/Loft/Revolve/Sweep，CSG 负责局部开孔/组合，Recipe 负责复用，PBR/GLB readback 负责真实外观与质量证据。
- `DESIGN.md` 已升级到 v6，新增 `MechanicalStyleToken@1`、`ProfileSketch@1`、`ProfileSectionSet@1`、`EditableComponentRecipe@1`、建模语法路由、不可变 feature node、CSG benchmark/单一内核、edge/normal/UV/tangent/zone provenance、GSAP 与可丢弃 SDF 边界。ADR-0011 接受该路线；`MECHANICAL_DESIGN_OPERATIONS.md` 是新的目标操作手册，不替代当前 USER_GUIDE。
- 原 G819/Q003 仍不可跳过，且当前唯一 `ready` 仍是 G819。新增原子几何子链 G820–G826；完整目标顺序为 `G819 → Q003 → G820 → G821 → G822 → G823 → G824 → G825 → G826 → A003 → F025 → D005 → A004 → M108 → C105 → V003 → F026 → A005 → R007 → D006`。M108 消费 G826 的真实表面事实，C105 组合 Profile/feature/连接/材质 Recipe，V003 最后自动选择建模语法、Recipe 和唯一最佳候选。
- 当前运行能力没有因本轮文档变化而扩大：Alpha 仍显示三方向，仍以低多边形 blockout、有限组合操作和多数单材质区为主；Loft、Sweep、稳健 CSG、轮廓编辑、真实多区 PBR、Recipe 和单一最佳结果均是目标设计。USER_GUIDE 与能力—Gate 矩阵未被改写为已实现。
- 使用 `documents:documents` 的结构/可读性规则整理 Markdown，使用 `game-studio:web-3d-asset-pipeline` 固化 GLB/readback/纹理和单 renderer 边界，使用 `gsap-core` 固化动画只反映状态、不成为几何或版本真值；没有生成 DOCX、没有新增依赖或代码。
- 本轮 Gate：`release:docs-walkthrough` PASS（任务索引 111 项、无 issue）、`repository:integrity` PASS、`release:safety-scope` PASS、`release:secrets-files` PASS（557 文件、0 匹配）、`agent:check` PASS、`git diff --check` PASS。工作区继续保留用户已有大量未提交修改；本轮未 commit、未 push。

## 2026-07-14：视觉真实度、单一最佳结果、Codex 式工作台与 DeepSeek 诊断（历史目标；已由 ADR-0011 扩展）

- 用户明确取消“三方向供选择”的目标：Agent 应在内部生成/编译/readback/渲染/评审候选，只展示一个最佳结果；3D 默认缩到左上 mini viewport，点击后把同一个 canvas 移到中央 focus。ADR-0010 已接受该决策，`FGC-V002` 已标为 `superseded`，USER_GUIDE 仍保留当前三方向事实直到 V003 真正完成。
- 本机实时检查：`CAD 工作台.app` 与本地 Uvicorn 正在运行，`GET /api/health` 返回 `status=ok, mode=sqlite_mock`；`~/Library/Application Support/ForgeCAD/provider.json` 缺失，Keychain service/account `ForgeCAD Agent Provider/default` 也缺失。Rust supervisor 因而没有注入 `FORGECAD_AGENT_PROVIDER=openai_compatible`，`mechanical_planner_from_env()` 选择确定性离线 Planner。`.wushen-agent.log` 有普通 Agent Turn，但没有 `provider:check`/DeepSeek 请求；一次 409 是同 Thread Turn in progress，不是 DeepSeek 错误。
- 已核对 DeepSeek 官方文档：`https://api.deepseek.com` 和 `deepseek-v4-pro` 当前有效；模型名不是此次根因。官方 JSON Output 仍可能返回空 content，thinking Tool Calls 的后续子请求必须续传 `reasoning_content`；400/401/402/422/429/500/503 应分别处理。当前 adapter 拒绝 Tool Calls并泛化部分错误，前端再把失败压成“暂时无法连接/测试未完成”，这是独立的可观察性缺陷。
- 已用 GitHub connector/官方文档核验 OpenAI Codex app-server 的 Thread/Turn/Item 事件生命周期和 `SKILL.md` loader、Claude Code 的专用 subagent/Skill/hook/tool restriction、Zoo Design Studio 的 code-as-model/XState、glTF PBR/clearcoat/KTX2 与 glTF Transform inspect/validate/优化。只采用模式，不复制通用 shell Agent、云几何引擎或完整上游运行时。
- 新主链：`G819 → Q003 → A003 → F025 → D005 → A004 → V003 → F026 → A005 → M108 → C105 → R007 → D006`。G819 仍是唯一 ready；其余均 blocked。A003 处理 DeepSeek Provider Gateway，A004 处理受限产品 Action Loop，V003 处理内部最佳候选，F026 处理简洁布局，A005 处理专属 Skill，M108/C105/R007/D006 依次处理高真实度 PBR、组件配方、参考引导重建与新机械领域晋级。
- 本条只更新目标设计、计划、任务和审计文档，不实现 Provider、UI、几何、材质、Skill 或新领域，不修改 USER_GUIDE/能力矩阵为已实现。`release:docs-walkthrough`（任务索引 104 项、无 issue）、`repository:integrity`、`release:safety-scope`、`release:secrets-files`、`agent:check` 与 `git diff --check` 均通过。工作区继续保留原有大量未提交修改；未 commit、未 push。

## 2026-07-14：原用户优先 CAD 设计能力任务链（历史；已由 ADR-0010 更新）

- 当时用户指定顺序：`FGC-G819 → FGC-Q003 → FGC-F025 → FGC-D005 → FGC-V002`。后续同日“不要三方向选择”的新指示已将 V002 标为 superseded，并按本文件上一节扩展主链；当前仍只可领取 G819。P009 保持独立发布回归任务，不与该链混合实施。
- G819 的核心退出条件是 Schema、Pydantic、Worker、GLB 编译/readback 与质量检查共同消费一个运行时操作白名单；未实现操作必须在任何持久副作用前明确拒绝，不能跳过后继续成功。Q003 随后才将质量事实改为读取该次真实编译/readback。
- F025、D005、V002 分别限定为 Agent-first/legacy 只读隔离、四领域非工程语义比例配方、三方向的解释/单维临时重混/Brief 覆盖反馈。多材质区、可编辑组件配方、参考模型引导重建必须等 V002 后另拆原子任务。
- 本条目不实现运行时、合同、迁移或 UI；所有新能力仍是目标设计，未写入用户指南或能力矩阵。本轮已通过 `release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files`、`agent:check` 与 `git diff --check`；首次文档门曾因 G819 表格依赖引用了未登记的 G818 而失败，已将索引依赖改为已登记的完成基线后通过。

## 2026-07-14：FGC-A001 DeepSeek 多轮上下文与缓存账本

- 已实现 `ForgeCADProviderConversation@1`：固定 Provider 前缀、四组近期历史、当前 Snapshot 摘要、已绑定领域复用和确定性 `ThreadMemorySummary@1`；它不拥有任何资产或 Snapshot 真值。
- OpenAI-compatible Planner 现解析 DeepSeek `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`，只使用 Schema JSON，收到 Tool Calls 就拒绝，未保存 `reasoning_content`。
- Provider HTTP 已移出 SQLite 事务；每个 Thread 限制一个运行中 Turn。DeepSeek 20 元日预算预留/结算与缺失 usage 停止已实现。真实 Provider 测试仍未执行。
- 本轮已通过：`npm run agent:unit`（16 passed）、`npm run agent:g1-kernel-smoke`、`npm run agent:g4-mechanical-planner-smoke`、`npm run agent:check`、`npm run contracts:types:check`。仍须执行完整文档/安全 Gate 与桌面回归。

## 1. 先刷新，不盲信快照

本文件记录 2026-07-13 的已验证状态。开始新任务时先运行：

```bash
git status -sb
git diff --check
git log --oneline --decorate -8
```

当前工作区已有大量未提交产品和文档修改。它们属于用户正在推进的工作，禁止 reset、checkout、清理或覆盖无关文件。

## 2. 产品现状

产品已经在文档和最小运行时层面从 Weapon Concept Agent 升级为通用机械概念 3D Agent。四个首批领域包是未来武器概念道具、汽车、飞机和机械臂。

当前不是生产软件。准确定位：本机 Alpha + 轻量纵向切片。

文档已按当前权威、历史 ADR、历史 evidence 和 legacy 兼容资料分层。开始前先读 [DOCUMENTATION_MAP.md](DOCUMENTATION_MAP.md)；已删除的本地神经 3D、Unity、Blender Starter 和旧 Weapon 工作台文档不得从 Git 历史恢复到主路径。

## 3. 当前已验证通过

最近一次文档阶段验证：

```text
release:docs-walkthrough   PASS
repository:integrity       PASS
release:safety-scope       PASS
release:secrets-files      PASS
agent:check                PASS
agent:q002-active-design-contract-smoke PASS（bootstrap、CORS ETag/If-Match、质量重放/冲突/stale）
agent:s8-active-design-navigation-smoke PASS
contracts:types:check      PASS
git diff --check           PASS
desktop:f001-workbench-characterization PASS（本机 Chrome）
desktop:f004-workbench-drawers-smoke PASS
desktop:f006-accessibility-smoke PASS
desktop:c101-part-role-labels-smoke PASS（四领域 role、关节角色和未知回退）
desktop:f003-agent-selection-card-smoke PASS（中文角色显示边界）
desktop:t003-performance-smoke PASS（单 canvas/context 与资源预算）
agent:c102-component-compatibility-smoke PASS（HTTP/服务候选结论、质量/领域/role/停用负例和 ChangeSet 拦截）
agent:c104-part-display-smoke PASS（CAS/幂等、锁定 ChangeSet 拦截、隐藏/隔离选择保护、版本状态归一化）
agent:g808-editable-parameter-bindings-smoke PASS（JSON/Pydantic、旧资产兼容、路径/单位/范围/步长/唯一性）
desktop:typecheck PASS
desktop:build PASS（存在既有 bundle >500 kB warning；T003 预算门禁 PASS）
desktop:r3-concept-workbench-smoke PASS（Agent-first + 抽屉焦点/Escape + C104 锁定重启、隐藏/隔离恢复）
desktop:t002-workbench-e2e-scenarios PASS（12/12 场景）
agent:g801-shape-primitive-smoke PASS
agent:g802-profile-extrude-smoke PASS
agent:g803-revolve-smoke PASS
agent:g804-transform-arrays-smoke PASS
agent:g805-boolean-smoke PASS
agent:g806-bevel-surface-panel-smoke PASS
agent:g807-blockout-diversity-smoke PASS（四领域 48 个结构）
agent:r002-render-views-smoke PASS（四视图 PNG provenance/readback/fingerprint）
agent:r003-exploded-views-smoke PASS（条件式爆炸候选、透明 alpha、稳定 Part ID 与拒绝伪造分件）
agent:r004-render-package-smoke PASS（PNG/manifest ZIP、hash/readback、stale 拒绝和字节级重复性）
agent:m101-material-contract-smoke PASS（旧 payload 迁移、完整 PBR 字段与失败边界）
agent:m102-material-catalog-smoke PASS（13 个六类视觉材质预设）
agent:m103-material-texture-smoke PASS（内容寻址纹理对象、来源/许可证、路径边界和参数回退）
agent:unit PASS（13 passed；jsonschema RefResolver 仅有弃用警告）
desktop:typecheck PASS
desktop:build PASS（存在既有 bundle >500 kB warning；T003 Alpha 预算门禁仍通过）
```

本轮新增的 `desktop:f001-workbench-characterization` 已在本机 Chrome 通过并登记到 CI。它覆盖首次项目加载、legacy 显式重建 hand-off、含糊输入澄清、预览不写盘、Agent 资产提交、Snapshot/导出一致、重启恢复和单 WebGL canvas。F006 的 `desktop:f006-accessibility-smoke` 与 r3 浏览器断言增加了质量/组件抽屉初始焦点、Escape 关闭和导出关闭后的焦点返回。legacy starter 在未执行“让 Agent 重建可编辑资产”时保存仍会返回 `ACTIVE_DESIGN_INVALID`，这是必须保留的写入屏障；本次测试已验证显式 hand-off 后再提交。CI runner 的远程结果仍以对应 commit 为准。

上一轮技术审计中，G1–G7 独立 smoke、contracts、desktop typecheck 和 cargo check 通过。开始代码任务时仍需针对当前工作区重新运行，不能直接复用旧结果。

## 4. 当前已知限制与发布阻断

### 工作台状态正确性

```bash
npm run desktop:r3-concept-workbench-smoke
```

历史核心 smoke 覆盖 legacy 显式重建授权、Agent asset 提升、preview/确认、持久化质量 ID、不可变 undo→redo、preview/quality/selection 的 revision 竞争、重启恢复和 GLB 导出不回退 Concept；当前 `desktop:r3-concept-workbench-smoke` 的 Agent-first 路径已通过（参考 GLB v1、可编辑资产 v2–v5、质量、导出、C104 锁定重启恢复、单独查看、隐藏清选择和显示全部）。原生安装恢复、多客户端压力矩阵和 legacy UI 退出仍未完成；即使 Snapshot S008/C104 已退出，也不能据此宣布整个工作台已生产就绪。

### 打包

```bash
npm run release:packaging-readiness
```

预期失败：四个平台 `wushen-agent-*` 是 0 字节占位文件。当前 Tauri 使用 `local-dev-python`，不是独立安装包。

## 5. P0 正确性缺陷

- Agent 路径已由 Snapshot 统一恢复、选择、preview、质量、回退/前进和 GLB 导出；F002–F004 已将 Agent 对话、步骤、选择卡和四类抽屉拆出，F005 已将四类抽屉收敛到 `WorkbenchDrawerStack` 组合层，F006 已完成可访问性收敛（控件尺寸、焦点、aria-live、Escape/焦点返回）；legacy 兼容 UI、父层状态与副作用仍待后续状态机任务处理；
- Q002 已收紧兼容 bootstrap 和质量写入：`GET /active-design` 仅从有效 Agent head 或 legacy current version创建 Snapshot，空项目不写；active-design/navigation 均 `no-store`，navigation 无独立 ETag；公共 `POST :quality` 要求当前 Snapshot `If-Match` 与 `Idempotency-Key`，同键同请求重放、冲突键拒绝、旧 revision 不写报告。CORS 明确允许 `If-Match` 并暴露 `ETag`，避免桌面开发壳丢失 revision；广泛多客户端压力和生产缓存策略仍未验证；
- legacy Concept 仍是兼容只读 UI，不得被重新作为 Agent 写入真值；
- 非 GLB 的旧 Concept 导出只属 legacy，不得被宣传为 Agent 导出；
- 含糊/不支持领域已在服务端阻断并持久化为单个 clarification Item；D003 focused UI smoke、F001 characterization 与当前工作台 r3 Agent-first 路径已有通过证据；
- backup 已枚举并恢复 `agent_imported_glbs.object_path`；`agent:r3-library-backup-restore-smoke` 还通过 `/active-design` 验证了恢复后的 Agent head、Snapshot 和 export source/version 同链。

## 6. 当前代码热点

```text
apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx   约 2473 行（F002–F005 已提取 AgentConversation/AgentSelectionCard/四类抽屉/组合层）
apps/desktop/src/features/cad-workbench/AgentConversation.tsx   Agent 输入、Provider、澄清、步骤和方向
apps/desktop/src/features/cad-workbench/AgentStepItem.tsx       单个 Kernel Item 展示
apps/desktop/src/features/cad-workbench/AgentSelectionCard.tsx  分件选择和部件动作
apps/desktop/src/features/cad-workbench/agentAssetWorkspaceState.ts F010 已提交资产读取投影 reducer
apps/desktop/src/features/cad-workbench/useAgentAssetWorkspace.ts F010 已提交资产读取投影 hook
apps/desktop/src/features/cad-workbench/legacyCompatibilityDisplay.ts F011 legacy 只读显示模型
apps/desktop/src/features/cad-workbench/LegacyCompatibilityNotice.tsx F011 legacy 转换提示组件
apps/desktop/src/features/cad-workbench/componentLibraryPreferencesState.ts F012 组件库本机偏好 reducer/filter
apps/desktop/src/features/cad-workbench/useComponentLibraryPreferences.ts F012 组件库本机偏好 hook
apps/desktop/src/features/cad-workbench/viewportDisplayPreferencesState.ts F013 项目隔离的视口显示偏好 reducer
apps/desktop/src/features/cad-workbench/useViewportDisplayPreferences.ts F013 视口显示偏好 hook
apps/desktop/src/features/cad-workbench/legacyModuleGraphWorkspaceState.ts F014 legacy ModuleGraph 工作区会话 reducer
apps/desktop/src/features/cad-workbench/useLegacyModuleGraphWorkspace.ts F014 legacy ModuleGraph 工作区会话 hook
apps/desktop/src/features/cad-workbench/legacyModuleGraphOverlayState.ts F015 legacy ModuleGraph 临时叠层 reducer
apps/desktop/src/features/cad-workbench/useLegacyModuleGraphOverlay.ts F015 legacy ModuleGraph 临时叠层 hook
apps/desktop/src/features/cad-workbench/agentRenderPresentationState.ts F016 Agent 概念图展示 reducer
apps/desktop/src/features/cad-workbench/useAgentRenderPresentation.ts F016 Agent 概念图展示 hook
apps/desktop/src/features/cad-workbench/agentEditAssistPresentationState.ts F017 Agent 编辑辅助读取 reducer
apps/desktop/src/features/cad-workbench/useAgentEditAssistPresentation.ts F017 Agent 编辑辅助读取 hook
apps/desktop/src/features/cad-workbench/agentMaterialCatalogPresentationState.ts F018 视觉材质目录读取 reducer
apps/desktop/src/features/cad-workbench/useAgentMaterialCatalogPresentation.ts F018 视觉材质目录读取 hook
apps/desktop/src/features/cad-workbench/partRoleLabels.ts       内部 role 的中文显示与安全回退
scripts/smoke_c102_component_compatibility.py                   项目内组件候选与拦截 Gate
apps/desktop/src/features/cad-workbench/ComponentDrawer.tsx      组件目录和替换检视
apps/desktop/src/features/cad-workbench/MaterialDrawer.tsx       视觉材质与细节密度
apps/desktop/src/features/cad-workbench/QualityDrawer.tsx        Agent/legacy 质量检查摘要
apps/desktop/src/features/cad-workbench/ExportDrawer.tsx         按用途选择导出
apps/desktop/src/features/cad-workbench/WorkbenchDrawerStack.tsx 四类抽屉组合层；只转发 props/callback，不拥有状态真值
scripts/smoke_workbench_accessibility.mjs                         F006 可访问性静态/组件 Gate
scripts/smoke_workbench_e2e_scenarios.mjs                         T002 12 场景 E2E 报告
scripts/smoke_workbench_performance.mjs                           T003 单 WebGL/内存/bundle 门禁
scripts/smoke_g801_wedge_capsule.py                               G801 wedge/capsule GLB readback
scripts/smoke_g802_profile_extrude.py                             G802 profile/extrude GLB readback
scripts/smoke_g803_revolve.py                                     G803 revolve GLB readback
scripts/smoke_g804_transform_arrays.py                            G804 mirror/array/radial_array readback
scripts/smoke_g805_boolean.py                                     G805 restricted union/subtract readback
scripts/smoke_g806_bevel_surface_panel.py                         G806 bevel/surface panel readback
scripts/smoke_g807_blockout_diversity.py                          G807 48 blockout diversity/readback gate
apps/desktop/src/features/cad-workbench/ModuleGraphViewport.tsx 约 883 行
apps/desktop/src/features/cad-workbench/cad-workbench.css        约 1993 行
apps/agent/forgecad_agent/application/agent_asset_editing.py     约 1104 行
apps/agent/forgecad_agent/application/agent_kernel.py            约 659 行
```

不要在没有 characterization tests 的情况下整体重写这些文件。

## 7. 当前几何和材料边界

- Geometry Worker 当前执行受控 `box`/`cylinder`/`capsule`/`wedge`/`profile`/`extrude`/`revolve`/`mirror`/`array`/`radial_array`、受限 union/subtract，以及受控 `bevel_approx`/`surface_panel`；
- 四领域后端共 48 个确定性 blockout 变体（每个领域 12 个）；工作台仍只展示 3 个零基础方向，但 G812 已让每张方向卡稳定匹配其中一项，不展示完整技术目录或自由参数；
- ShapeProgram Schema 中的复杂操作多数尚未实现；
- 当前有 13 个、覆盖六类的完整字段视觉材质预设；M103 已完成受控纹理对象目录、来源/许可证边界和参数回退；M104 已完成 Material Zone 检视、中文分类筛选、关键词搜索、对象存在性和来源摘要；M105 已完成稳定 zone 选择、部件槽绑定、带 zone 的 ChangeSet 预览和非法 zone 后端拒绝；M106 已完成基于真实 `allowed_domains` 的四领域兼容筛选；M107 已将 zone 选择写入 Snapshot，并覆盖重启、版本切换和 undo/redo；C101 已将候选部件、材质上下文和组件保存名称中的稳定 role 映射为中文，未知值不显示内部标识而回退为“未命名部件”；
- 外部 GLB 是只读参考，不会自动变成 ShapeProgram；
- Agent 资产正式支持 GLB 导出，以及 R002/R003 的四视图和条件式透明爆炸概念 PNG 派生预览；R004 还支持下载当前、指纹一致的 PNG/manifest 概念图包。转台视频、OBJ/MP4 和源包仍不支持。

## 8. 推荐下一个任务

当前交接补充：`FGC-S001`–`FGC-S008` 的 ActiveDesignSnapshot 单一真值链保持不变；`FGC-G805` 已通过受限 disjoint union、贯穿槽 subtract、重叠/非贯穿失败和布尔输入数量校验；`FGC-G806` 已通过 1/3 段 bevel、±Y surface panel、面板适配和 GLB readback 失败边界；`FGC-G807` 已通过四领域各 12 个、跨领域共 48 个结构签名唯一的 blockout gate；`FGC-G816` 已让同一主视口的 display-only ShapeProgram 适配器完整显示 `box`/`cylinder`/`wedge`/`capsule`，并以柔化展示边缘、阴影和工作室环境改善概念观察，不写入几何、版本或 Snapshot；`FGC-G817/G818` 增加 `quick_sketch`/`showcase` 的有限外观质量档：展示档把面板、分缝视觉线、护板、孔洞/紧固件、灯带和线缆槽视觉线及有限 PBR 映射写入同源 ShapeProgram、GLB、AssemblyGraph、分件候选和候选 JSON，快速草图保持旧输出；工作台默认展示模型，切换只重建未保存预览。上述均为非功能概念外观，不是实际孔槽、散热、电气、工程材料或照片级渲染，且仍只有一个 WebGL canvas/context。`FGC-R001` 已通过 Snapshot 相机/灯光预设；`FGC-R002/R003` 已通过四视图、条件式透明爆炸候选、来源/alpha readback/fingerprint smoke 和桌面导出抽屉预览/单图下载接线；`FGC-R004` 已通过以当前 fingerprint 约束的 PNG/manifest ZIP 下载、stale 拒绝、固定 ZIP member/时间戳与浏览器下载断言。爆炸候选只在 GLB primitive 组与稳定 Part/AssemblyGraph 一一对应时出现；图包不得扩展成装配说明、源包、转台视频或工程渲染。

不要恢复 localStorage Agent 版本头或让 GLB 导出回退到 Concept。后续任务必须保持转换授权、Agent asset head、Snapshot、选择、质量、导出和 C104 part display 跨重启仍保持同一资产版本，并补齐广泛并发与原生安装验证。`FGC-M101`–`FGC-M107`、`FGC-C101`–`FGC-C104`、`FGC-G808`–`FGC-G812` 与 `FGC-Q002` 已完成；G812 让三方向的 build/segment/candidate/已确认资产保持同一受限视觉变体来源，仍不开放自由目录。Q002 的 `agent:q002-active-design-contract-smoke` 已覆盖空库、Agent/legacy bootstrap、no-store、质量重放、冲突键和 stale 拒绝。AgentComponent 没有正式 Module Asset 的审阅状态，不能在 UI 伪装为“已审”。当前没有可独立领取的 `ready` 任务；后续必须先定义新的原子任务。打包 sidecar、真实 Provider、广泛并发、正式审阅和签名仍是独立阻断项。

R005 更新（2026-07-13）：Agent 下载抽屉已收敛为直接 GLB、概念单图和指纹受限图包，旧用途/OBJ/源包不再出现在 Agent 路径；抽屉、12 场景浏览器 E2E 和 r3 回归通过。`FORGECAD_LOCAL_VISUAL_PACK=0 ./script/build_and_run.sh --verify` 已通过本机 `.app` 启动和 `local-dev-python` Agent 健康检查，但 `osascript -l JavaScript` 返回“osascript 不允许辅助访问”，因此原生 WebView 下载点击仍是已记录的辅助功能授权阻断，不能宣称已通过。该更新覆盖上文关于 R005 等待原生下载 E2E 的旧快照。

F007 更新（2026-07-13，脏工作区，未提交）：`useWorkbenchLifecycle` 已从 `CadWorkbenchPanel` 提取请求编号、取消/乱序响应屏障、既有错误映射和抽屉互斥/焦点返回状态；父层仍拥有 API、Snapshot hydration、ETag、ChangeSet、质量与下载副作用。新增 `desktop:f007-workbench-lifecycle-smoke`，并将其接入 desktop CI；同轮将一处 10px 辅助文字修正为 11px，F006 未被放宽。完整回归通过：typecheck/build、F001–F007、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check。该记录当时的下一项为 `FGC-F008`；其后续状态以本文件较新的 F008 更新和任务索引为准。

F008 更新（2026-07-14，脏工作区，未提交）：新增 `agentConversationState` 与 `useAgentConversationPresentation`，将输入、模式、提示、项目内 Agent thread、Kernel steps、澄清和方向卡从 `CadWorkbenchPanel` 提取为纯展示状态；项目切换会原子清空，project/request 双重检查拒绝旧项目或已取消 Turn 的迟到响应。父层仍是唯一 Agent API/SSE、legacy fallback、blockout/segmentation、提交、Snapshot、ETag、ChangeSet、质量与下载副作用入口。新增 `desktop:f008-agent-conversation-state-smoke` 并接入 CI。F008、F001、F002、F007、D003、T002（12/12）、T003、r3、typecheck、build 均通过；T003 确认单 canvas/context 与 bundle 预算保持通过。当前唯一 `ready` 为 `FGC-F009`：只抽取 blockout 候选展示协调，不得移动 AgentAssetVersion 或 Snapshot 真值。

F009 更新（2026-07-14，脏工作区，未提交）：新增 `agentBlockoutDisplayState` 与 `useAgentBlockoutDisplay`，将 GLB、ShapeProgram、分件候选和方向加载的显示缓冲从 `CadWorkbenchPanel` 提取；重选方向清空旧候选，分件失败保留仅供观察的未提交外观，项目切换/旧请求不能写回。该层不保存 AgentAssetVersion、Snapshot、ChangeSet、质量或导出 ID；父层仍是唯一 build/segment/commit、hydration 和持久写入入口。新增 `desktop:f009-agent-blockout-display-state-smoke` 并接入 CI。typecheck/build、F001、D003、T002、T003、r3 通过。当前唯一 `ready` 为 `FGC-F010`：只提取已提交资产工作区投影，不得让缓存成为版本 head。

F009 复验（2026-07-14，脏工作区，未提交）：修复首次加载时项目尚未绑定便可提交 Agent 的竞态；发送按钮现在等待项目就绪，E2E 也等待同一可交互状态。新回合会清空旧澄清/方向，避免已选类别继续遮挡新方向。`desktop:f002-agent-conversation-smoke`、F008、F009、typecheck、build、T002（12/12）与 T003 通过；r3 仍为已知基线失败，当前在 C104 重启后的 `active-design:part-display` 锁定请求等待超时，未删除或放宽该断言。当前唯一 `ready` 仍为 `FGC-F010`；开始前必须先处理或明确记录 r3 的独立基线阻断。

F010 更新（2026-07-14，脏工作区，未提交）：新增 `agentAssetWorkspaceState` 与 `useAgentAssetWorkspace`，从 `CadWorkbenchPanel` 提取当前 Snapshot 已选 Agent 资产的只读投影、选中部件、质量摘要与导航摘要。缓存只接受匹配当前 project、asset source 和 request 的读取响应；项目/source 切换清空旧投影，旧 selection/quality/navigation 无法写回。它明确不保存 asset head、Snapshot revision、ETag、ChangeSet、质量写入或导出身份；父层继续唯一负责 API、hydration、CAS、preview/confirm、undo/redo、质量写入和下载。新增 `desktop:f010-agent-asset-workspace-state-smoke` 并接入 CI；F003、F008、F009、F010、typecheck、build、T002（12/12）、T003、r3 已通过。r3 的先前 C104 重启动作超时被定位为 UI hydration/action-ready 竞态：现在等待 Snapshot 与已加载资产一致再允许动作，保留并通过锁定重启、隔离、隐藏/恢复与单 canvas 断言。当前唯一 `ready` 为 F011：只提取 legacy 只读兼容显示边界，不改变 Snapshot 或写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F011 更新（2026-07-14，脏工作区，未提交）：新增 `legacyCompatibilityDisplay` 与 `LegacyCompatibilityNotice`，将旧 Concept source 的只读说明和“让 Agent 重建可编辑资产”引导从 Agent 会话主体抽为纯显示边界。显示模型只由当前 Snapshot source 与 operation 派生；它不保存转换授权、asset head、Snapshot revision、ETag、ChangeSet、质量写入或导出身份，父层仍是唯一发起 legacy conversion authorization、CAS 和所有写入的入口。新增 `desktop:f011-legacy-compatibility-display-smoke` 并接入 CI；F002、F011、typecheck、build、F001、T002（12/12）、T003、r3 通过。当前唯一 `ready` 为 F012：仅提取组件库本机筛选/收藏/最近使用/抽屉高度偏好，禁止把偏好变成资产或版本真值。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F012 更新（2026-07-14，脏工作区，未提交）：新增 `componentLibraryPreferencesState` 与 `useComponentLibraryPreferences`，将组件库分类、关键词、审阅状态筛选、收藏、最近使用、抽屉模式与高度改为按 Project+Domain Pack 隔离的本机偏好。损坏或缺失的 localStorage 安全回退，收藏/最近使用有去重和长度边界；纯过滤 adapter 只消费真实 Module Asset 元数据，不制造审阅、许可证、质量或兼容结论。父层仍唯一读取资产目录、质量与缩略图，并唯一拥有组件替换 ChangeSet、Snapshot/CAS、API、版本和导出。新增 `desktop:f012-component-library-preferences-smoke` 并接入 CI；F004、F006、F012、typecheck、build、T002（12/12）、T003、r3 通过。当前唯一 `ready` 为 F013：只提取本机视口显示偏好，不能移动 Snapshot 相机/灯光、测量记录、renderer 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F013 更新（2026-07-14，脏工作区，未提交）：新增 `viewportDisplayPreferencesState` 与 `useViewportDisplayPreferences`，将工具、网格、线框、X 光、Connector、爆炸系数和截面偏移改为按 Project 隔离的本机显示偏好；缺失/损坏 localStorage 安全回退，工具白名单与数值边界由纯 reducer 固定。`CadWorkbenchPanel` 的 v6 通用 session 不再保存这些字段，也不再保存相机/灯光；相机/灯光继续仅由 `ActiveDesignSnapshot` 的 R001 CAS 路径读写。该层明确不持有 asset head、Snapshot revision、ETag、选择、质量、ChangeSet、导出或 renderer 身份。新增 `desktop:f013-viewport-display-preferences-smoke` 并接入 CI；R001、F006、F012、F013、typecheck、build、T002（12/12）、T003、r3、contracts、agent check 通过；T003 保持单 canvas/context，R3 重启恢复通过。当前唯一 `ready` 为 F014：仅提取 legacy ModuleGraph 本机工作区会话，不得移动 Agent Snapshot 选择、测量记录或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F014 更新（2026-07-14，脏工作区，未提交）：新增 `legacyModuleGraphWorkspaceState` 与 `useLegacyModuleGraphWorkspace`，将 legacy ModuleGraph 的 inspector tab、旧图节点/模块定位、变换坐标/吸附与测量模式改为按 Project 隔离的本机会话；损坏/缺失 localStorage 安全回退，返回图后只从现存节点恢复有效选择。旧全局 CAD session 读写已经删除；Agent source 打开空 context，不读取或写入 legacy session，当前 Agent part selection/quality/export 继续只读 Snapshot，测量标注仍使用原有项目/版本 key。新增 `desktop:f014-legacy-module-graph-workspace-smoke` 并接入 CI；F010、F011、F013、F014、F006、typecheck、build、T002（12/12）、T003、r3、contracts、agent check 通过；T003 仍保持单 canvas/context，r3 重启恢复通过。当前唯一 `ready` 为 F015：仅提取 legacy ModuleGraph 展示叠层，不得移动 Snapshot、Agent part display、质量、测量记录或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F015 更新（2026-07-14，脏工作区，未提交）：新增 `legacyModuleGraphOverlayState` 与 `useLegacyModuleGraphOverlay`，把 legacy ModuleGraph 的隐藏节点、聚焦节点、质量高亮/几何引用和组件缩略图失败记录移为纯瞬态显示层。该层以 Project+Graph context 绑定且不写 localStorage；切换 project、graph 或切到 Agent source 时会清空，图节点重载会过滤过期节点和几何引用。Agent source 的空 context 会拒绝旧图叠层动作，旧 `hiddenNodeIds` 从不与 Snapshot `part_display` 合并；Quality API、质量结果、Snapshot/CAS、版本、导出、ChangeSet、renderer props 和 Agent 部件显示仍由现有父层/服务端拥有。新增 `desktop:f015-legacy-module-graph-overlay-smoke` 并接入 CI；F010、F011、F013、F014、F015、F006、typecheck、build、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check 通过。当前唯一 `ready` 为 F016：只提取 Agent 概念图请求/展示状态，不得移动下载、Snapshot 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F016 更新（2026-07-14，脏工作区，未提交）：新增 `agentRenderPresentationState` 与 `useAgentRenderPresentation`，把当前 Agent 的四视图/概念图包 render-set、渲染/图包 loading 与请求屏障从 `CadWorkbenchPanel` 抽为纯内存展示状态。它只接受同一 project、当前 Agent asset version 与当前 request 的响应；切换 asset/source 会清空旧图，关闭抽屉会取消未完成请求并拒绝迟到响应，图包只允许使用当前 render-set fingerprint。父层仍唯一拥有 Render API、PNG/ZIP 浏览器下载、GLB 导出、Snapshot/CAS、质量、ChangeSet 和 renderer；该层没有 Snapshot、质量、ChangeSet、导出、图片 URL 或 asset head。新增 `desktop:f016-agent-render-presentation-smoke` 并接入 CI；R002–R004、F010、F015、F016、F006、typecheck、build、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check 通过。当前唯一 `ready` 为 F017：只提取 Agent 组件/结构建议读取状态，不得移动 preview→confirm 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F017 更新（2026-07-14，脏工作区，未提交）：新增 `agentEditAssistPresentationState` 与 `useAgentEditAssistPresentation`，把当前 Agent asset+selected Part 的组件替换候选、事实驱动结构建议、loading/不可用说明和请求屏障从 `CadWorkbenchPanel` 抽为纯内存展示状态。它只接受同一 project、当前 asset 与当前 Part 的候选/建议；source、project、asset 或 selection 切换即清空，迟到成功/失败均被拒绝，读取失败只显示“暂时无法读取”而不伪造结构建议。父层仍唯一拥有候选/建议 API、组件保存、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer；该层没有 Snapshot、质量、ChangeSet、导出、asset head 或 renderer。新增 `desktop:f017-agent-edit-assist-presentation-smoke` 并接入 CI；C102、C103、F010、F016、F006、F003、typecheck、build、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check 通过。当前唯一 `ready` 为 F018：只提取视觉材质目录只读加载状态，不得移动 Material Zone、preview→confirm 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

F018 更新（2026-07-14，脏工作区，未提交）：新增 `agentMaterialCatalogPresentationState` 与 `useAgentMaterialCatalogPresentation`，把视觉材质目录、loading/真实回退说明和请求屏障从 `CadWorkbenchPanel` 抽为纯内存展示状态。它只接受同一 project、asset、domain pack 与 source 的目录响应；切换 context 即清空，迟到成功/失败均被拒绝。服务目录失败时只使用传入的本机内置视觉预设并明确说明；无回退预设才显示目录不可用。父层仍唯一拥有 Material Zone、preview→confirm ChangeSet、Snapshot/CAS、质量、导出和 renderer；该层没有 Snapshot、选择、质量、ChangeSet、导出、asset head 或 renderer。新增 `desktop:f018-agent-material-catalog-presentation-smoke` 并接入 CI；M101–M107、F010、F017、F006、typecheck、build、T002（12/12）、T003、r3、contracts、agent check、文档/安全 Gate 与 diff check 通过。当前唯一 `ready` 为 F019：只提取视觉材质筛选展示状态，不得移动选中材质、Material Zone、preview→confirm 或任何写入。真实 Provider、原生安装/WebView 下载、多客户端压力和签名发布仍分别受外部或未实现 Gate 阻断。

G812/G813 更新（2026-07-14，脏工作区，未提交）：`resolve_blockout_variant()` 现将三方向按当前 Domain Pack、silhouette 与 direction ID 稳定映射到同一领域 12 项预审视觉 blockout 中的一项；G813 再以受限 `variation_index=0..2` 在同一三项族轮换。`BuildAgentBlockoutRequest`/`SegmentAgentBlockoutRequest` 与响应携带可选/实际 `variant_id` 和默认安全的 index；工作台只在未保存候选显示“换一版外观 / 当前第 N / 3 版”，不泄露 ID。Build 返回实际 ID/index 后，父层将其原样提交 Segment，确保 GLB、ShapeProgram、AssemblyGraph、分件候选与保存候选同源；轮换只替换临时 preview，不写 AgentAssetVersion、Snapshot、ChangeSet、质量或导出。跨包 ID、越界 index 和同幂等键改选视觉结果均在服务端拒绝，不引入自由几何、技术目录、制造/功能信息、第二 renderer 或新的 Snapshot 真值。新增并接入 backend CI 的 `agent:g813-variant-regeneration-smoke` 覆盖四领域三版轮换、候选保存、幂等和越界；F003 覆盖零基础按钮 callback。G812、G807、G6、G809/G810、F003、F009、T002（12/12）、T003、r3、contracts、typecheck/build、ruff、文档、安全、integrity、secrets 与 `git diff --check` 已通过。当前唯一 ready 是 F022，只抽取方向预览展示状态；不得把 G813 扩展为自由外观编辑、工程 CAD 或真实 Provider 质量评测。

F022 更新（2026-07-14，脏工作区，未提交）：`agentBlockoutDisplayState` / `useAgentBlockoutDisplay` 现在只保存 project/request 屏障下的 `directionId`、`variationIndex`、GLB/ShapeProgram/分件候选、加载和两种可恢复预览错误；开始轮换会原子清空旧候选，分件失败保留只供观察的 GLB，project switch/clear 会丢弃方向与轮换上下文。它不保存 AgentAssetVersion、Snapshot、ChangeSet、质量、导出或 renderer；`CadWorkbenchPanel` 仍唯一执行 build/segment API、提交和所有永久写入。扩展 F009 smoke 覆盖轮换、迟到响应、失败和 clear；F003、typecheck/build、T002（12/12）、T003（单 canvas/context、1.11 MB 主 JS 在 1.2 MB 预算内）、r3、contracts、文档/安全/integrity/secrets 与 diff check 已通过。当前唯一 ready 为 F023，只收敛普通语言预览提示，不能把展示状态扩展为任务中心、Mode 或新的 Agent 真值。

F023 更新（2026-07-14，脏工作区，未提交）：新增纯 `selectAgentBlockoutPreviewPresentation()`，只从 F022 已有预览展示状态翻译“正在生成完整外观预览”“完整外观预览已准备好”“完整外观已生成但暂不能整理部件”与“这次预览没有生成成功”。对话区和候选卡共用该来源，保留 r3 稳定的“预览状态”标识；用户看不到 variant ID、轮换 index、API 错误码或几何术语。selector 不调用 Provider、不自动重试、不创建版本或写 Snapshot；父层仍拥有 API/Turn/版本/质量/导出/renderer。新增并接入 desktop CI 的 `desktop:f023-agent-blockout-preview-presentation-smoke`，覆盖 idle、生成中、ready 与两类失败；F002/F003/F009、T002（12/12）、T003（单 canvas/context、1.11 MB 主 JS 在 1.2 MB 预算内）、r3、typecheck/build、contracts、文档/安全/integrity/secrets 与 diff check 已通过。当前唯一 ready 为 F024，只展示离线规划或真实 Provider 来源，不得触发调用、费用或泄露密钥。

F024 更新（2026-07-14，脏工作区，未提交）：新增纯 `selectAgentPlanSourcePresentation()`，只从已返回 `MechanicalConceptPlan.provider_id` 翻译“本机离线规划”“已连接模型服务生成”或“规划来源待确认”。确定性 plan 明确提示“尚未调用模型服务”，不会冒充真实模型结果；普通工作台的已配置、连接成功和失败提示也不再回显 Provider、模型、Base URL、Key、token、原始错误或费用信息。selector 不读取 Key、不联网、不创建版本、不写 Snapshot/质量/导出；父层仍拥有 Provider 配置、连接测试、Turn/API、版本与 renderer。新增并接入 desktop CI 的 `desktop:f024-agent-plan-source-presentation-smoke`，并扩展 F002 防止已配置状态泄露模型标识；F024/F002、typecheck、T002（12/12）、build、T003、r3、contracts、agent check、文档/安全/integrity/secrets 与 diff check 已通过。`FORGECAD_LOCAL_VISUAL_PACK=0 ./script/build_and_run.sh --verify` 另确认本机 `CAD 工作台.app` 可构建并启动，`local-dev-python` Agent 健康；它不等于真实 Provider 调用、外部安装、签名或公证验证。当前唯一 ready 为 `FGC-E001`：只冻结真实 Provider 四领域 truth-set 的显式、可计费评测合同，不得自动调用用户的 Provider。

E001 更新（2026-07-14，脏工作区，未提交）：新增 [AGENT_PROVIDER_EVALUATION.md](AGENT_PROVIDER_EVALUATION.md)、`evaluations/agent-provider-v1/contract.json` 与 `truth_set.json`，明确四领域各 20 条正常完整外观 Brief、20 条含糊/越界安全停止输入、固定 100 个测试条目、零默认费用、无自动重试、45 秒单请求上限、token 上限、脱敏证据和逐次人工授权。`agent:e001-provider-evaluation-dry-run` 与 contract smoke 均只读取静态 JSON：报告 `network_calls_made=0`、`asset_or_snapshot_writes=0`，并拒绝非零默认预算、CI 自动调用和截断 fixture；它们已加入 backend CI。真实 Provider baseline 仍为 external/NOT RUN，旧 Weapon R4 evaluator 不能作为通用四领域质量证据。

E002 更新（2026-07-14，脏工作区，未提交）：修正 E001 合同语义为“100 个测试条目 = 80 次正常 Provider 请求 + 20 条本地安全停止”，避免把越界输入发送给外部模型。新增隔离的 `provider_evaluation.py`、`run_agent_provider_evaluation.py` 与合成 Provider smoke：默认命令只 dry-run；真实路径同时要求三项固定 flag、正值且不超过 100 元的人工批准、操作者/时间/preflight 和有效 OpenAI-compatible 本机配置，缺配置在任何网络调用前拒绝。执行器不接触 Project、Thread/Turn、AgentAssetVersion、Snapshot、质量或导出；它仅输出内存中的脱敏 run report，固定映射 timeout/限流/鉴权/传输/结构化/策略/预算/取消，且不保存 Key、Base URL、模型 ID、原始 Brief/Response 或账单。`agent:e002-provider-evaluation-runner-smoke` 覆盖无凭据、缺确认、零/超额预算、超时、取消、无 usage、输出 token 上限、完整 telemetry 和脱敏；CI 只运行 no-call Gate。真实 Provider baseline 仍为 `EXTERNAL / NOT RUN`。当前唯一 ready 为 `FGC-G814`：把已隔离评测的概念范围预检提升为普通 Agent Turn 的 Planner 前屏障；`FGC-E003` 保持用户逐次授权且人工审阅的 external run，不是可自动领取的代码任务。

G814 更新（2026-07-14，脏工作区，未提交）：新增版本化 `ConceptScopeDecision@1` 与有限、可解释的本地策略，正常 Turn 固定经过 DomainInference → ScopeDecision → Planner。明确现实武器/制造、加工或材料配方、工程性能，以及车辆安全、适航/飞行、机器人控制/扭矩/认证请求得到 `unsupported`：Kernel 只写 completed Thread/Turn/一个 `kind=scope` clarification Item/幂等记录，绝不调用 Planner 或 Provider，也不写 Plan、blockout、AgentAssetVersion、Snapshot、质量或导出；已选领域不能绕过。普通含糊类别仍走 D003 单问题，四个非功能完整外观 Brief 仍可规划。工作台将 scope stop 显示为“请换一种外观创意描述”，不显示选择按钮或方向卡。`agent:g814-concept-scope-smoke`（10 条越界、选择绕过、D003、四领域正常）、G1/D2/D3、F002/F008、typecheck/contracts/agent check、T002（13/13，含 scope-stop 浏览器场景）、r3、desktop build、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files` 与 `git diff --check` 均通过。当前唯一 `ready` 为 `FGC-G815`：只将安全完整外观意图映射到既有视觉族，不得引入任意几何或工程参数。真实 Provider baseline 仍为 `EXTERNAL / NOT RUN`。

G815 更新（2026-07-14，脏工作区，未提交）：新增 `VisualIntentMapping@1` 与本机 `visual_intent.py`。确定性与 OpenAI-compatible Planner 输出均会用安全 Brief 的有限轮廓、细节、色彩和展示姿态分类归一化；该 mapping 只选择同一 Domain Pack 既有 0–3 视觉族，Geometry Worker 继续使用 G812/G813 catalog、现有 ShapeProgram、triangle budget、分件、preview→confirm 与 Snapshot 链。mapping 缺失或损坏时回退旧的 silhouette family，不会解释文本为尺寸、操作、脚本、自由网格或工程参数。新增 `agent:g815-visual-intent-projection-smoke`，覆盖四领域各两条 Brief、GLB/ShapeProgram 指纹分化和重复性、坏 mapping 回退；G2/G4/G5/G812/G813/G814、F002、typecheck/contracts/agent check、T002（13/13）、r3、desktop build、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files` 与 `git diff --check` 均通过。方向卡只显示普通语言，不显示视觉族 index 或字段名。当前唯一 `ready` 为 `FGC-R006`：只为未保存方向提供同源低分辨率概念图预览，不得持久化候选、增加 renderer 或调用真实 Provider。

R006 更新（2026-07-14，脏工作区，未提交）：新增 `AgentBlockoutConceptPreview@1`、`POST /api/v1/agent/blockouts:concept-preview`、纯内存的方向概念图展示状态与工作台方向卡图片。用户在保存前会看到三个同源、320×240 的软件概念图；它们只来自既有确定性 blockout 渲染，不创建候选、`AgentAssetVersion`、`ActiveDesignSnapshot`、质量报告或导出记录，也不调用真实 Provider 或增加 WebGL renderer。方向卡选择、重新生成或新 Agent 请求都会清空这组临时图片。新增 R006 后端/前端 smoke 并接入 CI；`agent:r006-blockout-concept-preview-smoke`、`desktop:r006-direction-concept-preview-state-smoke`、contracts、agent check、typecheck、F002、G815、R002、T002（14/14，含保存前无写入场景）、T003、desktop build、r3、文档/安全/integrity/secrets 与 diff check 通过。r3 首次曾因现有参数按钮等待时序超时，立即重跑通过，已如实保留为回归观察项。当前唯一 `ready` 为 `FGC-P008`：只实现本机 packaged sidecar 输入/预检合同，不下载或构建未知二进制，不接入真实 Provider、签名或发布；`FGC-P002` 仍受空 packaged sidecar 阻断。

P008 更新（2026-07-14，脏工作区，未提交）：新增 `apps/desktop/src-tauri/binaries/sidecar-inputs.json` 的 `ForgeCADPackagedSidecarInput@1`、无密钥 `packaged_sidecar_preflight.py` 与 smoke，并接入 backend CI、`release:packaging-readiness-smoke` 和 production packaging report。清单当前只声明 macOS arm64 目标、相对 sidecar 路径、`agent serve`、受限运行环境名称、health URL/响应与本机 Alpha 检查项；不含 Provider Key、Base URL、模型或用户数据。预检从不读取 secret、不联网或执行二进制：空占位稳定输出 `blocked_missing_sidecar`，临时正确的 Mach-O arm64 输入输出 `ready_for_local_alpha`，错误架构和凭据样式合同值被拒绝。`release:packaging-readiness` 仍按预期因四个空 sidecar 失败，这个失败没有被隐藏。`release:packaged-sidecar-preflight-smoke`、预检报告、agent check、desktop tauri check、docs walkthrough、integrity、安全、密钥和 diff check 通过。当前唯一 `ready` 为 `FGC-P002`：只构建当前 macOS arm64 的真实 frozen sidecar，并实际验证 packaged Alpha 启动、无 Key 初始化、工作台、GLB 导出和重启恢复；不得把 P008 结构性绿色称为安装、签名、公证或外部发布完成。

P002 完成（2026-07-14，脏工作区，未提交）：修复 packaged supervisor 在日志目录不存在时会在 spawn 前失败的问题；release 默认 `packaged-sidecar`，并将 PyInstaller onefile sidecar 放入独立进程组，正常窗口关闭会回收 wrapper 与实际 listener。为保证所有 macOS LaunchServices 路径都可靠，sidecar 在 Tauri setup 内同步完成健康检查后再交给 WebView 做幂等状态读取。`npm run desktop:packaged-sidecar-build`、`npm run desktop:packaged-sidecar-alpha-smoke`、`npm run desktop:packaged-tauri-alpha-smoke` 均通过：后者从真实 `.app` 验证 `mode=packaged-sidecar`、受管后代、临时空 Library 初始化、确定性机械臂可编辑 GLB 导出与重启恢复，输出 `provider_calls: 0`。真实界面复测还确认工作台加载以及正常关闭后端口 8000 不遗留 sidecar。没有调用 Provider、读取 Keychain、签名、公证、安装或外部发布结论；`release:packaging-readiness` 仍因其他平台 sidecar 未构建而阻断。下一项为 `FGC-P009`：仅把现有无密钥 macOS native smoke 接入专用 macOS CI/构建机，不能扩展 Provider、安装或发布范围。

可以独立并行但不得混入 S001 的任务：

- `FGC-T001`：把 G1–G7 纳入 CI；
- `FGC-B001`–`FGC-B002` 已完成：备份覆盖 imported GLB 对象，恢复后通过 API 回读 Agent head、ActiveDesignSnapshot 和 export source/version。P001/P007 已完成并解除依赖审计阻断；F006、T002、T003、G801、G802、G803、G804、G805、G806 与 G807 已完成，必须保持 F001/r3/T002/T003/G801/G802/G803/G804/G805/G806/G807 回归门以及 F002/F003/F004/F006 组件与可访问性 smoke。

若任务涉及 Agent 架构、开源依赖或开发工具，先读 [AGENT_GITHUB_REFERENCE_ARCHITECTURE.md](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md) 和 [AGENT_PLUGINS_SKILLS_DESIGN.md](AGENT_PLUGINS_SKILLS_DESIGN.md)。参考项目只提供模式；实际依赖必须经过 benchmark、许可证、体积、平台打包和退出方案审查。

## 9. 首轮基线命令

```bash
npm run agent:check
npm run contracts:types:check
npm run desktop:typecheck
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run agent:r004-render-package-smoke
npm run desktop:f004-workbench-drawers-smoke
npm run desktop:build
npm run desktop:r3-concept-workbench-smoke
```

2026-07-13 本轮结果：上述合同、Agent 检查、文档/完整性/安全/密钥门、R004 图包 smoke、抽屉 smoke、桌面 build、T002 浏览器下载断言和 r3 工作台 smoke 均通过；`desktop:build` 仍有 Vite 大 chunk warning。工作区保持用户已有的脏修改，未提交、未合并、未 push。

随后运行与任务直接相关的 smoke。不要一开始运行包含 legacy Unity/ComfyUI 环境的完整旧 release gate，除非任务就是迁移这些门。

## 10. 密钥和外部输入

- 不从聊天或历史输出复制 API Key；
- 原生运行使用 Keychain；浏览器开发使用 0600 secret file；
- 真实 Provider 评测会产生费用，必须获得明确授权；
- 刘邦已被指定为独立资产 reviewer，但“已指派”不等于已批准；
- 签名账户在外部发布阶段才需要。

2026-07-14 A002 更新（脏工作区，未提交）：`scripts/run_agent_provider_evaluation.py` 新增显式 `--provider-config-source macos-keychain`，只在获授权的隔离评测进程内读取 Tauri 使用的 `ForgeCAD Agent Provider/default` Keychain 项和非敏感 metadata；密钥不会进入环境、报告、ledger、日志或普通 Agent。`npm run agent:e001-provider-evaluation-dry-run`、`agent:e001-provider-evaluation-contract-smoke`、`agent:e002-provider-evaluation-runner-smoke`、`release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files`、`agent:check` 与 `git diff --check` 均通过；当前本机 Provider metadata/Keychain 为空，`--provider-config-source macos-keychain` 在任何网络调用前返回 `E002_PROVIDER_UNCONFIGURED`。下一项仍为 `FGC-E003` external：用户须在工作台保存已轮换的 Keychain 密钥、为一次具体 run 确认预算与操作者，然后由非执行者审阅脱敏报告和 Provider 控制台账单；本轮未执行真实 Provider 请求、未提交或 push。

## 11. 交接给下一位 Codex

结束任务时更新：

- `CODEX_TASK_INDEX.md` 任务状态；
- 本文件的已知失败或新阻断；
- `CAPABILITY_GATE_MATRIX.md` 的能力证据；
- 任务相关的 API、状态、测试或操作文档。

交接必须列出真实命令结果、工作区是否干净、是否提交/推送，以及下一项已解除阻断的任务 ID。
