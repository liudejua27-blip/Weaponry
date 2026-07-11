# R3 Module Naming Evidence

日期：2026-07-10

范围：证明 `ForgeCADModuleNaming@1` 已从文字约定变为 Module Pack 机器门。它不证明人工 Blender 最终资产已经完成。

## 实现

- Module：`module_<P0 category>_<01-99>`；
- Asset：必须与 Module 的 category/序号一致；
- Connector：`connector_<owner>_<interface>` 小写 snake_case；
- Material：跨模块语义 `MAT_` 槽；
- GLB mesh-bearing node：`GEO_<module_id>_LOD0[_NN]`；
- GLB mesh：`MESH_<module_id>_LOD0[_NN]`；
- P0 Pack ID：`pack_<name>_v<N>`。

## 自动证据

```bash
npm run assets:module-pack-gate
```

结果：通过。仓库 10 模块参考包保持可重生成；9 模块最小 fixture 完成 dry-run、导入、幂等和重启恢复；hash、unsafe path、license、duplicate connector、pack mismatch 以及非法 Module/Asset/Connector/Material 命名负例全部被拒绝。

设计者操作标准见 [MODULE_NAMING_STANDARD.md](../MODULE_NAMING_STANDARD.md)。
