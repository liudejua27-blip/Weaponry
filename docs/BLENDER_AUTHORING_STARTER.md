# Blender Authoring Starter

状态：三模块 authoring source 与执行器已实现；当前开发机未检测到 Blender，因此真实 `.blend`/GLB/thumbnail 构建尚未执行。

## 1. 作用

`scripts/blender/weapon_concept_starter.py` 在 Blender 中建立三个可编辑起点：

- `module_core_shell_01`；
- `module_front_shell_01`；
- `module_front_shell_02`。

它创建 Metric 场景、LOD0 mesh、UV0、三个语义材质、Connector Empty、相机和灯光，保存独立 `.blend`，再导出 GLB、512×512 thumbnail、Module Manifest 和三模块 `pack.json`。这些是可编辑 starter，不是最终人工美术，也不是制造设计。

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

默认拒绝非空输出目录，也永久拒绝把输出指向 `assets/module-packs`。只有确认不需要保留现有临时 starter 时，才可在自定义命令中显式加入 `--force`；它会删除并重建指定的 `output/` 目录。

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
6. 人工审阅三分之四缩略图后，再执行受控重新导出与 Pack 校验。

当前 starter generator 会重建输出，不能用它覆盖已经人工修改的 `.blend`。人工 source 的无覆盖重新导出入口是下一资产管线切片；在该入口完成前，保留 `.blend` 备份并按 `MODULE_ASSET_GUIDE.md` 手工导出。

## 6. 不可变注册边界

reference Pack 和 authoring starter 使用相同的预发布稳定 Module ID。Module Registry 不允许同 ID 更换 hash，因此：

- 不得把 starter 导入已注册 reference 模块的同一 Library；
- 人工制作期间使用单独的测试 Library；
- 正式晋级在干净 Library 或显式预发布迁移中完成；
- 一旦对外发布，破坏兼容性的资产改动必须创建新 Module ID，不覆盖历史记录。
