# ADR-0005：旧数据导入与环境变量迁移

- 状态：Accepted
- 日期：2026-07-10

## 决策

1. 旧数据库在 tag 后只读，新版本只写新 CAD/DFM 表。
2. importer 可重复运行、使用事务，并为每条记录生成 imported/skipped/failed 报告。
3. 旧 Weapon 记录最多导入为 legacy project，旧文件作为 `legacy_reference_asset`。
4. WeaponDesignSpec、CreativeWeaponGraph、SkillGraph 和 Unity manifest 不转换为 DesignSpec/FeatureGraph。
5. 新环境变量以 `FORGECAD_*` 为准，兼容期允许回退读取 `WUSHEN_*`。
6. 使用旧变量时打印一次脱敏弃用警告；密钥值和用户绝对路径不得进入日志。
7. 不双写环境配置、不自动修改用户 shell 配置。

## 退出条件

- importer 对同一输入重复执行不会产生重复 Design/Asset；
- 任一失败可回滚且不修改源库；
- 兼容期结束版本和移除清单写入发布说明；
- 新 sidecar 在脱离源码目录时只依赖打包配置或用户配置目录。
