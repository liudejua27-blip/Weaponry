# Codex Ponytail 前置设计流程

> 2026-08-26 商业 preflight 补充：每次设计必须先声明 `goal_object`、当前 `ProductionStage`、目标 typed artifact、固定 candidate/reference/camera hash、promotion policy 和所需 human/engine gate。用户要求“跳过当前阶段”只允许 `preview_only=true`，不得把后序预览写成前序批准或自动 promotion。

> 2026-08-25 当前 Native High 前置判定：public MCP source/focused 与同 cohort Runtime restart **1/1 PASS**，但 packaged/candidate quality Gate 未通过，所以 preflight 仍必须返回“不可用于商业 High 阶段推进”，不得跳过 FormQuality 或激活 proposal。

版本：2026-08-26
状态：已配置到 current MCP source；first-party declarative Skill，非第三方插件

2026-08-25 商业资产路线边界：preflight 只决定是否需要动作、复用何种现有能力和最小 typed action；它不能跳过 `FormQuality → AuthoringMesh → High → Low → UV → Cage/Bake → Material → FPS/LOD → engine → human` 顺序，也不能把尚未实现的 Worker、GitHub 项目或外部 DCC 视为可调用能力。完整路线见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。

## 1. 目的与边界

`ponytail-preflight@0.1.0` 是 ForgeCAD 自有的前置设计 Skill。它将从
DietrichGebert/ponytail 学到的“先判断必要性、再复用现有能力、最后做最小动作”
改写为 ForgeCAD 的有界规则。它不是上游 Ponytail 的安装包、Node hook、MCP server
或任何可执行代码。

它不生成几何、不改变候选、不写入 CAS/SQLite，也不替代参考、质量、批准或导出
合同。它只要求 Codex 在进入 3D 设计链路前先读取以下内容并按约束规划：

1. 此次改变是否真的需要；没有必要时停止或只读检查；
2. 当前项目、参考、candidate、snapshot、已启用 Skill 和 Operator 是否已有可复用的
   有界路径；
3. 是否能够使用已有 product-owned typed Operator；
4. 若必须行动，选择单一、最小的 prepare/readback/quality 步骤；
5. 不绕过用户授权、reference unknown、candidate lineage、固定相机证据和用户 confirm。

未知或遮挡的参考区域仍必须保留为 unknown。此流程不允许把简化设计当作降低质量门、
删除证据或跳过批准的理由。

Preflight 只能选择已注册的 ForgeCAD-owned typed Operator。它不能激活未注册的 Native
High proposal，也不能把 `HighMeshArtifact@1`、`HighMeshArtifactGlb@1` 或
`NativeHighDurable*` 的 source/structural/durable readback 当作 active Skill、商业 High
Gate 或视觉 PASS；当前它们仍保持 `registered=false`、integration=`unavailable`，并且
不推进 Stage、confirm、version 或 export。Blender、Substance Designer/Painter 和其他
DCC 仅作 workflow/material 参考，不能成为 preflight 可调用的 binary、工程 graph、插件
或脚本来源。

## 2. MCP 强制顺序

每个新的 MCP stdio 会话在进行 ForgeCAD 设计调用前必须先成功调用：

```json
{
  "name": "skill_get",
  "arguments": {
    "skill_id": "ponytail-preflight",
    "version": "0.1.0"
  }
}
```

成功响应含 `SkillGetResult@1.knowledge` 的 overview、constraints、examples 和
canonical hash。MCP adapter 只允许 `capabilities_get`、`runtime_status`、`doctor`
作为无状态诊断例外；在读取前，其他 MCP tool 或其他 Skill 会返回
`PONYTAIL_PREFLIGHT_REQUIRED`，不会触达 Runtime 业务操作。

读取成功后，Codex 仍按现有严格顺序工作：

```text
ponytail-preflight
-> capabilities/project/reference evidence
-> active Operator/Skill discovery
-> typed prepare
-> strict readback + fixed render/quality evidence
-> High/Low/Hero UV 后调用 production_weapon_high_low_bake_preflight_get
-> 仅 ready_for_formal_bake=true 且 formal producer available 时允许 Cage/Bake prepare
-> user approval
-> confirm/export
```

`production_weapon_high_low_bake_preflight_get` 是只读阻断检查，不生成 Cage、maps 或正式 receipt。若返回 `ready_for_formal_bake=false`，或随后 prepare 返回 `PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE`，Codex 必须停止该阶段并记录零写；不得启动未计入 Gate 的 2K 长测、不得转入 Material、不得推进 Stage。

`skill_get` 成功只表示前置规则已被读取，不表示任一 Skill 的 operator 可执行，也不
表示几何、视觉相似度、PBR、人评或 360 度质量已通过。

即使后续有 Runtime-owned Native High durable prepare/get，preflight 也只记录其
AuthoringMesh exact binding、High/GLB replay、hash/readback 和 no-stage limitation；
它不是候选确认或质量批准。只有独立的 typed quality、人评、引擎和 export/restart
receipts 才能改变对应 gate 状态。

## 3. 供应链与维护

- Source receipt：`docs/evidence/adoption/ponytail/2ed6c52c9d7e5e56942508591085fd45dea277d3.yaml`。
- Bundle 由 `packages/forgecad-skills/registry.json` 和
  `scripts/materialize_mcp006_bundles.py` 生成，Runtime 只嵌入 registry 声明的静态
  bundle 内容。
- 不执行 upstream `package.json` scripts，不运行 hook，不安装 npm dependency，不把
  上游 prompt、代码、DCC 工程 graph 或文件路径写进几何真值。
- 更新该 Skill 时必须同时通过 Bundle integrity、MCP order test、文档/许可证 Gate，且
  不得改写 `QUALITY_TARGET_NOT_MET` 或 `INCOMPLETE_TRUTH_BINDING`。当前
  `boolean@1` 仅以已验收 receipt 支持 bounded same-Part union/difference/intersection；
  更早 cohort 中的 unavailable/deferred 结论必须保留为历史事实，不能被 Skill 更新
  伪造为通用 Boolean 能力。
