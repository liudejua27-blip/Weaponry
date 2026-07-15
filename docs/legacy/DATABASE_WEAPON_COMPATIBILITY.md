# 旧 Weapon/Concept 数据兼容说明

状态：legacy，只用于迁移与历史回放

当前 Library 仍包含旧 Weapon、Job、Asset、CreativeWeaponGraph/SkillGraph、Concept Project/Version、ModuleGraph、Concept ChangeSet、Quality 和 Export 表。

维护规则：

- 不原地重命名为通用 Agent 表；
- 不把旧 `version_no` 显示为 AgentAssetVersion 编号；
- 不把旧质量报告附着到新 Agent 资产；
- 旧大文件仍由 `asset_files`/`concept_assets` 和内容寻址对象保存；
- 迁移通过显式 adapter 创建新对象，并保留 source ID、schema 和 hash；
- 历史 schema 细节以 migrations、生成 OpenAPI 和 Git 历史为准。

退出顺序见 [兼容迁移计划](../COMPATIBILITY_MIGRATION.md)。
