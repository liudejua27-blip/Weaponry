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

## 十模块 visual candidate（待人工审阅）

同日扩展 `weapon_concept_starter.py --module-set full_candidate`，保持 reference Pack 的十个稳定 Module/Asset/Connector ID。官方 Blender 4.2.22 构建、只读 re-export 与 Pack 校验均通过，全部 source 在导出前后保持不变；其许可证仍为 `LicenseRef-ForgeCAD-Authoring-Starter`，所以不具备正式资产资格。

| Module | triangles | GLB SHA-256 |
| --- | ---: | --- |
| core | 2256 | `1e9c14148e6ff6cac19c9fc3ec3c72506cb7b36b4dbc0d120596ebff5af80c88` |
| front 01 / 02 | 1316 / 1504 | `f70a3b094972230bae4de2b63b99ad4d2d43569dbbdc25bb58b912dfb7527fe8` / `9f7fc19d7ff293eb98bb685ef0d3e04ee93c8fe0ca34756a523a7a94ad7a5bb8` |
| rear / grip | 1128 / 1128 | `4830a532411087591b22f31eca2601f80052320b617c852a70079d91c8751119` / `70a29778e9cc6ba1c033193793c94d29899e02f79936d7f92d6dd1676547c20a` |
| top / side / lower | 940 / 940 / 940 | `7a07373a5e3c6a69b882972acfa2c030cb4dc9fbd2fa3946d50d58db1d6f239d` / `4f0597551dd7b8ce0be73c89fba402b3e8005c38ae2ae99b734dd9740b865bb6` / `67bc4c79fc7b07152be514dc0f34d7124551467c07fddf7db53d8a5037f00ea4` |
| storage / armor | 1128 / 940 | `8a5ea4f625cdd6e4eb49a1b3400ed234c697f32a429b551276e4eb90fb7764db` / `14dd65442087f41ac6680138ccb9d53ac95857cfb2716f7e5120f55ccdc29aee` |

首次完整组装暴露 storage 内部包裹几何、grip 与 lower/storage 的非直连相交；候选脚本修正空间分区后，隔离 Agent 导入 10 模块、9 节点/8 Connector Graph、Version 绑定、质量检查、combined GLB 下载与重启回读均通过。最终质量状态为 `passed`，唯一 finding 是 `geometry.ruleset` 的通过信息。combined GLB 为 943456 bytes，SHA-256 `d0515dbbd6b00ff789c7ea02a1ad0f14fd5b11bcb50a4b1adb6613faded88d51`；Blender DCC round-trip 返回 source 不变，输出为 25808 vertices / 10716 triangles。`release_10_12` 审阅草稿已生成，验证如期因 starter 许可证、占位 reviewer/批准、人工 checklist 和低评分被拒绝；没有生成 promotion report。

## 尚未证明

- 人工修改后的最终轮廓、表面层级、UV 和材质分区质量；
- 最终许可证与独立 reviewer 批准；
- 最终许可证与独立 reviewer 批准后的正式 Blender 资产全装配 DCC 往返；
- 正式资产替换矩阵和 Tauri GPU 指标。
