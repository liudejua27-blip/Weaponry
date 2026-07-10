# R3 Blender Authoring Starter Evidence

日期：2026-07-10

## 已证明

- 三模块 authoring script 可通过 Python 编译和 Ruff；
- core/front01/front02 稳定 ID、UV0、三材质和 Blender GLB 导出调用存在；
- runner 会查找显式参数、`FORGECAD_BLENDER_EXECUTABLE`、PATH 和 macOS 默认位置；
- runner 只在 Blender 成功、三份 `.blend` 存在且导出 Pack 通过真实校验后报告 `built_and_validated`；
- 输出隔离在 `output/blender/weapon-concept-v1-starter`，不会覆盖 reference Pack。
- runner 和 Blender source 都拒绝 `assets/module-packs` 输出；非空临时输出必须显式 `--force` 才可重建。

## 当前环境结果

```bash
npm run assets:blender-starter-preflight
```

结果：`ok=true`、`status=blocked_blender_not_configured`、`build_ready=false`。这只说明 source/preflight 正常；没有真实 Blender 构建证据。

## 尚未证明

- `.blend` 可被目标 Blender 版本打开；
- Blender glTF exporter 输出通过 Module Pack 门；
- thumbnail 渲染与人工视觉质量；
- 人工修改后的无覆盖重新导出；
- 三模块正式替换矩阵和 Tauri GPU 指标。
