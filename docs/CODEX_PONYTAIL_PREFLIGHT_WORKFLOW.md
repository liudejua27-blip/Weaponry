# Codex Ponytail 前置设计流程

版本：2026-08-13  
状态：已配置到 current MCP source；first-party declarative Skill，非第三方插件

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
-> user approval
-> confirm/export
```

`skill_get` 成功只表示前置规则已被读取，不表示任一 Skill 的 operator 可执行，也不
表示几何、视觉相似度、PBR、人评或 360 度质量已通过。

## 3. 供应链与维护

- Source receipt：`docs/evidence/adoption/ponytail/2ed6c52c9d7e5e56942508591085fd45dea277d3.yaml`。
- Bundle 由 `packages/forgecad-skills/registry.json` 和
  `scripts/materialize_mcp006_bundles.py` 生成，Runtime 只嵌入 registry 声明的静态
  bundle 内容。
- 不执行 upstream `package.json` scripts，不运行 hook，不安装 npm dependency，不把
  上游 prompt、代码或文件路径写进几何真值。
- 更新该 Skill 时必须同时通过 Bundle integrity、MCP order test、文档/许可证 Gate，且
  不得改写 `QUALITY_TARGET_NOT_MET` 或 `INCOMPLETE_TRUTH_BINDING`。当前
  `boolean@1` 仅以已验收 receipt 支持 bounded same-Part union/difference/intersection；
  更早 cohort 中的 unavailable/deferred 结论必须保留为历史事实，不能被 Skill 更新
  伪造为通用 Boolean 能力。
