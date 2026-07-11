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

### 视觉层级增强版（当前 starter）

同日用同一已验签 Blender 重建的 visual-v2 starter 保留全部 Module/Asset/Connector 合同，但增加了非功能性的顶轨、外观条带、装甲带、信号标记和差异化 front 轮廓。

| Module | bounds mm | triangles | GLB SHA-256 |
| --- | --- | ---: | --- |
| `module_core_shell_01` | 100.0 × 56.0 × 47.5 | 2256 | `1e9c14148e6ff6cac19c9fc3ec3c72506cb7b36b4dbc0d120596ebff5af80c88` |
| `module_front_shell_01` | 67.0 × 39.0 × 35.1 | 1316 | `f70a3b094972230bae4de2b63b99ad4d2d43569dbbdc25bb58b912dfb7527fe8` |
| `module_front_shell_02` | 76.0 × 41.0 × 43.5 | 1504 | `9f7fc19d7ff293eb98bb685ef0d3e04ee93c8fe0ca34756a523a7a94ad7a5bb8` |

三份 visual-v2 source 前后 SHA-256 保持 `f9ee89b8e4839cfde9bf15a65123ee4cd78f358722c378893196fc8600366408`、`f81526f0f578c57fe747ddd0f257e2fd2c43f9f981ad57334d0727f883129f3e`、`7cac011212660be9ae7f6674371b66e9e764ce8c7d0608068f65e8fad8c1ec8b`。只读 re-export、三模块 Connector baseline 和 core DCC 往返均通过；core 为 5354 顶点 / 2256 三角。

通过 `smoke_blender_starter_workbench.py` 将这个实际 Pack 导入临时 ForgeCAD Library 后，core/front Graph 验证、front01 → front02 ChangeSet 预览/确认、子 Version、`weapon-concept-geometry/1.3` 质量检查（`passed`）、combined GLB 下载和 Agent 重启回读均通过。导出 combined GLB SHA-256 为 `415e7ac6fbf8403ded0cd3f73963f8802e41e755ddb5fa5e1c1d5cb1405b3351`，328732 bytes，Blender 往返后保持 8980 顶点 / 3760 三角。

上文 940 三角与 2178 顶点对应初始 toolchain run；当前 starter 的权威数值以 visual-v2 段落为准。两次运行均保持 Asset/Category/Connector 按 ID 规范化后与 reference baseline 相等。

本次真实执行还暴露并修复了：factory startup 无 World、Blender Python 异常未映射为非零退出码、Blender Z-up 与 ForgeCAD/glTF Y-up 坐标基变换、Connector 基线的 ±24 mm 偏差、float32 表示噪声以及缩略图过曝。

## 尚未证明

- 人工修改后的最终轮廓、表面层级、UV 和材质分区质量；
- 最终许可证与独立 reviewer 批准；
- 正式人工 Blender 资产的全装配 DCC 往返；
- 三模块正式替换矩阵和 Tauri GPU 指标。
