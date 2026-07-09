# 固定 Prompt 质量集

本文件定义 release G4 的固定 prompt 质量集。当前 gate 是 deterministic mock planner 级别检查，用于防止 WeaponDesignSpec、negative prompt、Unity 3D 契约和安全边界回退；它不替代真实图像 / 3D provider 的人工与自动质量验收。

执行命令：

```bash
npm run release:prompt-quality
```

验收阈值：

| 指标 | 阈值 |
| --- | --- |
| Prompt 数量 | 20/20 |
| 被判定为可执行的幻想战斗物体产物 | 至少 18/20 |
| 具备 3渲2、国风、强剪影、材质层次 | 至少 16/20 |
| 包含现实制造参数或工艺指导 | 0/20 |
| 水印、乱码文字、主体破碎、主体缺失风险未被 negative prompt 覆盖 | 不超过 2/20 |
| 适合单图转 3D 并声明 GLB / Unity material / quality report 输出 | 至少 15/20 |

当前门禁兼容声明：

- 本脚本当前仍以 `WeaponDesignSpec@1` 为强制检查对象，`creative_graph` 与 `skill_graph` 为目标态扩展位。
- `recast` 目标是输出 2~3 候选与稳定排序，当前门禁通过 `script interpretation_ready` 指标软检查记录该能力。
- 固定 prompt 集禁止现实制造语义；非制造边界仍以 `safety_boundary.real_world_manufacturing_details=false` 为判据。

固定 prompt 覆盖“高自由度对象神化”样式，既包含传统武器外观，也包含非典型输入（防弹裤、木棍、椅子、镜子、铃铛、树枝等）的结构解释目标。源数据位于 `scripts/check_release_prompt_quality.py`，以保证文档阈值和 release gate 使用同一套样本。

当前 gate 检查：

- `WeaponDesignSpec@1` JSON Schema 合法（兼容阶段目标）。
- `safety_boundary.real_world_manufacturing_details=false`。
- `unity_target.scale_contract.forbid_real_world_dimensions=true`。
- positive design fields 不包含制造图纸、制造尺寸、材料配方、加工流程或装配指导，并优先覆盖结构-功能重构提示（skeleton/affordance/skill seeds）。
- negative prompt 包含制造安全排除项，以及 watermark、unreadable text、broken subject、missing subject 等图像质量排除项。
- `unity_target.model_3d` 保持单图转 3D 所需的 `concept_image` 输入和 `rough_raw_glb`、`rough_optimized_glb`、`unity_material_json`、`quality_report` 输出契约。
- `creative_graph` 与 `skill_graph` 仍为目标态字段，当前阶段不作为硬性门禁，但脚本应保留扩展位。
- `interpretation` 必须有 2~3 条候选、可稳定排序（rank + confidence），且每条候选包含结构锚点（anchor）与 1~2 项结构性风险标签；当前 gate 保留 `script` 字段中 `interpretation_ready` 作为未来扩展指标。
- 若 mock/real planner 首次输出低于 2 条候选，质量记录必须标记 `resample_attempted=true`；重采样后仍低于 2 条时计入 `prompt_coverage_gap`，不得把单一候选当作通过。
- 非武器对象至少覆盖 40% 样本时，至少 30% 的样本要求返回 2 个不同的 `combat_affordances` 组合，避免“默认化类别映射”。
- 强制样本新增：同一输入（如 pants/椅子/树枝）重复跑 2 次，必须至少有一项候选顺序保持不回退（用于检查解释排序可复现性）。

新增“对象-能力映射”质量组（用于人工验收）：

| 输入对象 | 示例结构解释 | 典型 combat_affordances |
| --- | --- | --- |
| 防弹裤 | 腰部环形炮台 + 防御域核心 | `shield`, `area_control`, `mobility` |
| 木棍 | 锁链炮杖 + 可折叠发射结构 | `ranged`, `summon`, `chain` |
| 椅子 | 王座炮台 + 折叠式护盾面 | `shield`, `area_control`, `reflect` |
| 镜子 | 反射法阵 + 召唤镜门 | `reflect`, `teleport`, `summon` |
| 伞 | 天幕阵 + 针雨围拢炮 | `shield`, `ranged`, `disrupt` |
| 戒指 | 玄纹护符场 + 触发环 | `summon`, `heal`, `passive` |
| 树枝 | 龙骨杵 + 牵引骨链 | `melee`, `mobility`, `chain` |
| 钥匙 | 目标定位阀门 / 传送触发枢纽 | `teleport`, `control`, `ultimate` |
| 花盆 | 风系护域器 + 守护触须 | `area_control`, `shield`, `summon` |
| 风车 | 风切环 + 螺旋反射器 | `reflect`, `damage_over_time`, `ranged` |
| 铃铛 | 警戒域 + 干扰触发阵 | `control`, `teleport`, `defense` |
| 门 | 空间折叠枢纽 / 召回站 | `teleport`, `summon`, `defense` |
| 雨伞 | 屏障伞幕 + 领域压缩口 | `shield`, `mobility`, `area_control` |
| 书卷 | 符文弹幕 + 咒印释放架 | `summon`, `ranged`, `control` |
| 画框 | 视域捕捉口 + 伤害反射面 | `teleport`, `reflect`, `passive` |
| 舞鞋 / 鞋底 | 冲击踏点 + 位移蹬射器 | `mobility`, `melee`, `control` |
| 花环 | 持续增益场 + 牵引环 | `heal`, `control`, `summon` |
| 贝壳 | 共鸣护罩 + 回响音爆 | `control`, `reflect`, `area_control` |
| 笼子 | 围合限制 + 控制域框 | `control`, `shield`, `trap` |
| 梯子 | 长度扩展/拉锯杠臂 | `mobility`, `melee`, `transform` |

新增约束（目标态）：

- 每个 `source_object` 至少应给出一个可执行主 affordance（非 `passive`-only）。
- 同一 `interpretation` 内至少两个候选应在 affordance 方向上互斥（例如 `shield` 与 `control` / `ranged` 与 `melee`）。
- 同一 non-weapon 样本的第二次复测中，至少有一个候选保留稳定 `rank`，避免完全随机漂移。
- `PROVIDER_BAD_OUTPUT` 的样本需要保留原始候选、重采样候选和失败原因，方便判断是 prompt 质量问题还是 provider JSON 稳定性问题。

质量扩展建议（按 GPT Pro 复核）：

- 核查 `recast_summary` 与 `combat_affordances` 的互斥关系是否一致（同一输入应有不同能力主轴）。
- 对 `pants/chair/mirror/tree` 一类对象，要求至少 30% 的候选在 `source_object` 变体重采样后仍保留同一核心能力（用于判定排序稳定性）。
- 将候选的 `risk_tags` 与 `protected_regions` 也纳入人工复核项；禁止候选只给 `visual_style` 而无结构约束。

新增约束（非具像化）

- 示例词库不得被限制为“剑/刀/枪/弓”类型。
- 至少 40% 的测试样本必须来自非武器对象（服饰、家具、日用品、工具、自然物等）转神化。
- 目标样例必须明确可见 `combat_affordances` 结论（哪怕最终仍是幻想风格）。
- 每条生成记录应尽量覆盖 2 层：结构解释层（骨架/握持/攻击源/可动） + 玩法映射层（affordance / 技能槽位）。

建议非武器样例（用于手工补充回归，优先新增）：

防弹裤、木棍、椅子、镜子、伞、铃铛、树枝、钥匙、卷轴、灯笼、茶壶、戒指、羽毛、花盆、木梳、琴弦、鞋底。

可扩展对象池（建议每周新增 3 条）：

- 护符纸、鼓、书架、手套、靶纸、花环、雨衣、吊牌、折纸、算盘、草帽
- 工具/器物：铲子、锥子、铃铛、梳子、提灯、风车、围栏、挂钩、风铃、算盘
- 日常空间/交通：车座、车把、地台、柱墩、台阶、栏杆、船锚、地钉
- 自然形态：冰层、珊瑚、藤蔓、松针、贝壳、贝叶、砂砾、海藻、树洞
- 抽象形态：环、网、折线、阶梯、弧体、悬浮球、镜面框架、双曲面

对象-能力映射建议（供 quality set 复核）：

- “椅子” -> 防御域/领域控制/召唤站点
- “钥匙” -> 选择目标/位移/召回
- “茶壶” -> 范围投射/状态转移/治疗
- “网” -> 封控/束缚/反弹
- “花盆” -> 召唤/护罩/增益
- “风车” -> 反射/持续伤害/变形
- “贝壳” -> 音波/护盾/反冲击

禁止提示词白名单（不能出现的制造语义）：

- 可制造尺寸、材料配比、热处理参数、装配工艺、零件清单、铸造流程
- 真实安全规范参数（如弹道压强、临界结构安全值）
- 任何可复现现实制造行为的指令化表述

判定禁令：

- 当词条出现“现实”+“可实施/可生产/可复制”的制造导向语气时，视为 `non_manufacturing_drift`。
- 当 negative prompt 未覆盖“破碎/缺失主体/乱码文字/水印/边缘拉伸”任意 2 项以上时，按 `negative_prompt_missing` 计分。

更高细节要求（real provider 以前）：

- 用实际 OpenAI-compatible LLM 输出跑同一组 prompt 并留存报告。
- 用真实 ComfyUI / 图像 provider 生成概念图，人工或视觉模型复核水印、乱码、主体破碎、主体缺失。
- 用真实 3D provider 输出 GLB，并结合 Unity 导入 gate 校准材质、bounds、朝向和三角面数阈值。

可选增强项（第二级门禁）：

- 同一输入要求至少输出 2 个不同风格形态（例如“保守/奇异”）
- 允许同一 `source_object` 生成 2 套不同 `combat_affordances`，只要都能附带可执行技能映射
- 输出 `creative_graph` 与 `skill_graph` 的字段完整性：版本链、受保护区域、技能锚点、技能槽位可重生说明
