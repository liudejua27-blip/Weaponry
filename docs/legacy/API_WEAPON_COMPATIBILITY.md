# 旧 Weapon API 兼容说明

状态：legacy，只用于迁移和回归

当前 `wushen_agent.main` 仍挂载以下旧入口：

- `POST /api/weapons`
- `GET /api/weapons`
- `GET /api/weapons/{weapon_id}`
- Weapon interpretation/recast/skill graph
- Patch 与旧图像 Provider
- `POST /api/weapons/{weapon_id}/generate-3d`
- `POST /api/weapons/{weapon_id}/export-unity`
- 旧 Job、Asset 和 Provider Settings

完整机器合同保存在 `packages/weapon-spec/generated/openapi.json`。迁移代码需要旧请求/响应时应读取该版本化快照，不应把旧端点重新复制进 [当前 Agent API](../API.md)。

兼容规则：

1. 旧数据只读或按原合同回放；
2. 不把 `WeaponDesignSpec` 当作通用机械 Core；
3. 不把旧 ConceptVersion 版本号显示为 AgentAssetVersion；
4. 不从新工作台新增旧 API 功能；
5. 旧图像、神经 3D 和 Unity 结果仅限虚构游戏美术资产，不包含 manufacturing drawings、制造尺寸或现实功能设计；
6. 删除端点前完成 [兼容迁移计划](../COMPATIBILITY_MIGRATION.md) 的对应退出条件。
