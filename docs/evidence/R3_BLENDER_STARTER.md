# R3 Blender Authoring Starter Evidence

日期：2026-07-11

## 已证明

- 三模块 authoring script 可通过 Python 编译和 Ruff；
- core/front01/front02 稳定 ID、UV0、三材质和 Blender GLB 导出调用存在；
- runner 会查找显式参数、`FORGECAD_BLENDER_EXECUTABLE`、PATH 和 macOS 默认位置；
- runner 只在 Blender 成功、三份 `.blend` 存在且导出 Pack 通过真实校验后报告 `built_and_validated`；
- 输出隔离在 `output/blender/weapon-concept-v1-starter`，不会覆盖 reference Pack。
- runner 和 Blender source 都拒绝 `assets/module-packs` 输出；非空临时输出必须显式 `--force` 才可重建。
- starter 将 `ForgeCADBlenderAuthoring@1` Module/Connector metadata 保存进每份 `.blend`；
- re-export 只读打开三份 source，不含 `save_as_mainfile`，并在外层比较导出前后 source SHA-256；
- re-export 阻断 source/output 重叠、非法文件头、错误 Mesh/UV/Material/Connector/Transform/Modifier，并在成功后运行 Module Pack 校验。

## 当前环境结果

```bash
export FORGECAD_BLENDER_EXECUTABLE="$HOME/Library/Caches/ForgeCAD/Blender/4.2.22/Blender.app/Contents/MacOS/Blender"
.venv/bin/python scripts/build_blender_starter_pack.py \
  --require-blender --force \
  --blender-executable "$FORGECAD_BLENDER_EXECUTABLE" \
  --output-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-starter-4.2.22"

.venv/bin/python scripts/export_blender_starter_pack.py \
  --execute --require-blender --force \
  --blender-executable "$FORGECAD_BLENDER_EXECUTABLE" \
  --source-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-starter-4.2.22/sources" \
  --output-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-reexport-proof-4.2.22"

PYTHONPATH=apps/agent .venv/bin/python scripts/check_dcc_roundtrip.py \
  --require-dcc --force \
  --blender-executable "$FORGECAD_BLENDER_EXECUTABLE" \
  --input-glb "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-reexport-proof-4.2.22/modules/module_core_shell_01/model.glb" \
  --output-root "$HOME/Library/Caches/ForgeCAD/Builds/dcc-roundtrip-core-4.2.22"
```

官方 Apple Silicon DMG SHA-256 为 `d177dc0f99024a51c6cc770e7920b302b2740ccc177ae8437c9294fdbd749e8f`；`codesign --verify --deep --strict` 通过，`spctl` 返回 `accepted` / `Notarized Developer ID`。Blender 版本为 4.2.22 LTS，构建返回 `built_and_validated`。

| Module | bounds mm | triangles | GLB SHA-256 |
| --- | --- | ---: | --- |
| `module_core_shell_01` | 100.0 × 51.5 × 43.0 | 940 | `9417647f32077e44636d8fdaaa5d73f8209bbd715fde532e72eb5bef8f1f0b3b` |
| `module_front_shell_01` | 67.0 × 35.0 × 34.25 | 752 | `1f623e8f49f4a987ed49fb0574e17456fe319962815bc77d9db224e7748f7064` |
| `module_front_shell_02` | 76.0 × 37.0 × 40.25 | 940 | `2f3100a59c0118e36ad57dd4ea9097cee33f85d4e50f06cbd2855648e6c258f2` |

真实只读 re-export 返回 `edited_sources_exported_and_validated` 与 `source_unchanged: true`；三份 `.blend` 前后 SHA-256 分别保持 `3ae9a0e630399357fe80eddd7bd3be9d3488c4e09f7889d204f261d91073bbed`、`2287f80d5797cb9130106ff238fb5054e159bdaf4ddffb3daa0e0438b00e1d27`、`c9a259c923a8a5007ea79a95c5c3903afce510f807c98a07bf898af9fc490442`。三模块 Asset/Category/Connector 按 ID 规范化后与 reference baseline 相等。core GLB 通过 Blender DCC 往返，2178 顶点和 940 三角保持不变。

本次真实执行还暴露并修复了：factory startup 无 World、Blender Python 异常未映射为非零退出码、Blender Z-up 与 ForgeCAD/glTF Y-up 坐标基变换、Connector 基线的 ±24 mm 偏差、float32 表示噪声以及缩略图过曝。

## 尚未证明

- 人工修改后的最终轮廓、表面层级、UV 和材质分区质量；
- core 超过 1000 三角下限、最终许可证与独立 reviewer 批准；
- 工作台 combined GLB 的真实全装配 DCC 往返；
- 三模块正式替换矩阵和 Tauri GPU 指标。
