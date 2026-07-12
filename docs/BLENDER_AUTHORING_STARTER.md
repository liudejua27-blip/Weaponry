# Blender Authoring Starter

状态：三模块 authoring source、十模块 visual candidate、只读导出器和正式人工审阅门已实现；authoring builder 同时兼容历史 Blender 4.x 的 EEVEE Next 与本机 Blender 5.x 的 EEVEE 枚举，已真实构建 `.blend`/GLB/thumbnail，完成完整组合质量检查和 DCC 往返。这些仍是候选资产，未经人工最终编辑、权属确认和独立 reviewer 批准。

候选脚本会产生真实的倒角、楔形轮廓、表面轨道/接缝、握持纹理和非功能性前端视觉管件；它们导出在 GLB 中并接受同一模块包与组合质量检查，绝不是工作台中的静态伪预览。候选不应在未经人工美术确认时标为正式资产或“已批准”。

## 1. 作用

`scripts/blender/weapon_concept_starter.py` 默认在 Blender 中建立三个可编辑起点：

- `module_core_shell_01`；
- `module_front_shell_01`；
- `module_front_shell_02`。

它创建 Metric 场景、LOD0 mesh、UV0、三个语义材质、Connector Empty、相机和灯光，保存独立 `.blend`，再导出 GLB、512×512 thumbnail、Module Manifest 和三模块 `pack.json`。这些是可编辑 starter，不是最终人工美术，也不是制造设计。

当三模块替换链已经稳定后，`--module-set full_candidate` 会扩展为与 reference Pack 相同稳定 ID/Connector 语义的 10 模块视觉候选包。它仍使用 `LicenseRef-ForgeCAD-Authoring-Starter`，故正式审阅门会故意拒绝它，直到人类确认权属、完成编辑并独立审批。

## 2. 安装前预检

```bash
cd "/Users/liuchongjiang/Documents/武神"
npm run assets:blender-starter-preflight
```

预检验证 Python 语法、三个稳定 ID、GLB 导出调用、UV0 与材质合同。没有 Blender 时返回：

```text
status: blocked_blender_not_configured
build_ready: false
```

该状态不是代码成功生成资产的证据。

## 3. 真实生成

要求 Blender 4.2 LTS 或更新版本：

```bash
FORGECAD_BLENDER_EXECUTABLE=/Applications/Blender.app/Contents/MacOS/Blender \
  npm run assets:blender-starter-build
```

也可以直接指定路径：

```bash
.venv/bin/python scripts/build_blender_starter_pack.py \
  --require-blender \
  --blender-executable /absolute/path/to/Blender \
  --output-root "$PWD/output/blender/weapon-concept-v1-starter"
```

执行器只有在 Blender 退出码为 0、三份 `.blend` 存在且整个输出通过 `validate_module_pack` 后才返回 `built_and_validated`。

当前 macOS 开发机也可以使用不安装到 `/Applications` 的用户缓存版：

```bash
export FORGECAD_BLENDER_EXECUTABLE="$HOME/Library/Caches/ForgeCAD/Blender/4.2.22/Blender.app/Contents/MacOS/Blender"
npm run assets:blender-starter-build
```

2026-07-11 视觉层级增强版真实运行返回 `built_and_validated`；三模块三角数分别为 2256 / 1316 / 1504，core 的外观顶轨、侧向条带、信号标记和下部护板已作为可编辑 starter 层次。它们均超过 formal 三角下限，但仍不证明最终资产：最终许可证和独立人工审阅仍是必要条件。

默认拒绝非空输出目录，也永久拒绝把输出指向 `assets/module-packs`。只有确认不需要保留现有临时 starter 时，才可在自定义命令中显式加入 `--force`；它会删除并重建指定的 `output/` 目录。

### 3.1 十模块视觉候选包

三模块链稳定后，使用独立输出目录生成完整候选包：

```bash
export FORGECAD_BLENDER_EXECUTABLE="$HOME/Library/Caches/ForgeCAD/Blender/4.2.22/Blender.app/Contents/MacOS/Blender"

.venv/bin/python scripts/build_blender_starter_pack.py \
  --module-set full_candidate --require-blender \
  --output-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-full-candidate"

.venv/bin/python scripts/export_blender_starter_pack.py \
  --module-set full_candidate --execute --require-blender \
  --source-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-full-candidate/sources" \
  --output-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-full-candidate-reexport"
```

候选包包含 core、两个 front、rear、grip、top、side、lower、storage 和 armor，共十个模块；runner 仍要求 source SHA-256 不变、Pack 合同通过。不要把此命令用于已人工编辑的 source。

## 4. 输出

```text
output/blender/weapon-concept-v1-starter/
├── pack.json
├── LICENSES/PACK.txt
├── sources/
│   ├── module_core_shell_01.blend
│   ├── module_front_shell_01.blend
│   └── module_front_shell_02.blend
└── modules/<module_id>/
    ├── module.json
    ├── model.glb
    ├── thumbnail.png
    └── LICENSE.txt
```

`output/` 不进入 Git，也不会覆盖 `assets/module-packs/weapon-concept-v1-reference`。

## 5. 人工设计顺序

1. 打开 `module_core_shell_01.blend`，只调整外轮廓、大倒角、面板节奏和三材质分区；
2. 保持 core Connector Empty 的稳定名称和语义，不增加真实机构；
3. 分别打开两个 front source，让轮廓明显不同，但保持 `front.core + shell_mount`；
4. 所有新增 mesh 按 `GEO_<module_id>_LOD0_NN` / `MESH_<module_id>_LOD0_NN` 命名；
5. 应用 Rotation/Scale，维护 UV0，检查三材质实际被 primitive 使用；
6. 人工审阅三分之四缩略图后，运行只读 re-export 与 Pack 校验。

starter generator 会重建输出，不能用它覆盖已经人工修改的 `.blend`。编辑完成后先运行：

```bash
npm run assets:blender-reexport-preflight
```

预检显示 `ready_for_read_only_export` 后执行：

```bash
FORGECAD_BLENDER_EXECUTABLE=/Applications/Blender.app/Contents/MacOS/Blender \
  npm run assets:blender-reexport
```

re-export 默认从 starter 的 `sources/` 读取，输出到独立的 `output/blender/weapon-concept-v1-edited-export`。它不调用 Blender 保存 API；执行器在导出前后计算三份 `.blend` SHA-256，任何 source 变化都会失败。非空导出目录需要自定义命令显式 `--force`，但 source 与 output 不能相同、互为父子目录或位于 committed Pack。

2026-07-11 真实 re-export 返回 `edited_sources_exported_and_validated` 与 `source_unchanged: true`；三份 source 前后 SHA-256 一致，三模块 Connector 按 ID 比较后与 reference baseline 数值语义一致。Blender 是 Z-up，ForgeCAD/glTF 合同是 Y-up，脚本负责 `(x,y,z) ↔ (x,-z,y)` 的基变换；不要手工二次旋转资产或 Connector。

导出前会阻断：丢失/额外 Mesh、错误 Object/Mesh 名、未应用 Modifier/Transform、缺失 UV0、错误材质集合、Connector Empty 与 metadata 不一致、Connector scale 未应用、相机缺失。输出必须再次通过完整 Module Pack 校验。

需要验证实际产品链时，可将已重导出的三模块 Pack 导入隔离 Library，执行 core/front Graph、front01 → front02 替换、质量检查、导出和重启回读：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/smoke_blender_starter_workbench.py \
  --pack-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-reexport-visual-v2-20260711"
```

如需把工作台生成的 combined GLB 交给 DCC runner，可额外传入一个不存在的绝对 `.glb` 路径的 `--combined-output`；它拒绝覆盖和 committed Pack 路径。2026-07-11 的 visual-v2 运行通过：质量状态 `passed`、front 替换创建子版本、导出 combined GLB 为 8980 顶点 / 3760 三角，并通过 Blender 4.2.22 往返。

十模块候选包使用完整 9 节点 Graph 的隔离验证：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/smoke_blender_full_candidate_workbench.py \
  --pack-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-full-candidate-reexport"
```

它导入 10 模块、验证 9 节点/8 Connector 图、绑定新 Version、运行 `weapon-concept-geometry/1.3`、导出 combined GLB 并重启回读。它不替代桌面 Tauri 性能、最终美术评分或人工审批。

默认 smoke 保持快速 GLB 回归。要对十模块候选实际生成 OBJ/MTL、PNG、8 帧转台和 MP4，并以较长本地超时校验所有独立下载 hash，显式加入：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/smoke_blender_full_candidate_workbench.py \
  --pack-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-full-candidate-reexport" \
  --include-presentation
```

2026-07-11 的本机候选运行耗时 16.4 s，验证 OBJ 3,090,148 bytes、preview PNG 22,221 bytes 和 MP4 70,984 bytes；这只是一台开发机的技术观察值，不是正式资产或性能 SLA。

## 6. 正式人工审阅与晋级

技术导出通过后仍不能称为最终资产。默认 re-export 许可证故意保留 starter/not-final 标记，必须在输出副本中换成已确认权属的最终美术 SPDX 和许可证文本。然后运行 `assets:formal-review-draft`，由非作者 reviewer 完成 `FormalModuleReview@1`：所有 checklist 为 true，五项视觉评分均 ≥4，确认非功能性概念/游戏/影视道具用途。

为避免 reviewer 直接在长 JSON 中遗漏模块，可从完整性已锁定的 draft 生成只读 Markdown 交接单。它列出每个模块的 source 文件名、缩略图相对路径、GLB hash 和待勾选项，**不能**批准或晋级资产：

```bash
npm run assets:formal-review-handoff -- \
  --pack-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-full-candidate-reexport" \
  --source-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-full-candidate/sources" \
  --review "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-full-candidate-review.json" \
  --output "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-review-handoff.md"
```

若 source、Manifest、GLB、thumbnail 或 license hash 已偏离 draft，或输出文件已存在，该命令拒绝生成。reviewer 仍必须把真实结论填写回原始 JSON，并由 `assets:formal-review-validate` 生成唯一的 promotion report。

`assets:formal-review-validate` 会重新核验三份 Blender source、module Manifest、GLB、thumbnail、Pack/Module license 的 hash，以及 Blender generator、三语义材质、anti-placeholder triangle floor、最终许可证和 reference Pack 的稳定 Module/Asset/Connector 合同。报告不含绝对路径；attestation 是审计记录，不是密码学签名。具体命令见 [操作手册](OPERATIONS.md)。

## 7. 不可变注册边界

reference Pack 和 authoring starter 使用相同的预发布稳定 Module ID。Module Registry 不允许同 ID 更换 hash，因此：

- 不得把 starter 导入已注册 reference 模块的同一 Library；
- 人工制作期间使用单独的测试 Library；
- 正式晋级在干净 Library 或显式预发布迁移中完成；
- 一旦对外发布，破坏兼容性的资产改动必须创建新 Module ID，不覆盖历史记录。
