# FGC-M108 独立视觉基准操作协议

状态：当前可执行；评分结果尚未收集。

目的：验证四领域同源 GLB 在 ForgeCAD 工作台的实际 PBR 视口中，是否达到任务卡所要求的比例、材质可读性和表面细节中位数门槛。它不是 Provider 评测、工程验证、制造评价或“照片级真实”声明。

## 1. 生成不可变审阅包

在仓库根目录执行：

```bash
npm run agent:m108-visual-benchmark-kit
```

该命令只从 `smoke_m108_visual_pbr.py` 的确定性 showcase 编译路径选择四份 GLB（每个已启用领域一份），写入被忽略的 `output/m108-visual-benchmark/`。`manifest.json` 记录每份 GLB 的 SHA-256、字节数、triangle、zone、纹理集和固定工作室环境；`review-responses.json` 必须初始为空。

执行前或 CI 中可运行：

```bash
npm run agent:m108-visual-benchmark-kit-smoke
```

它只验证审阅包可复现、四领域齐全、每份 GLB 具有多 zone 和多套内置 PBR 纹理，不产生评分。

## 2. 独立评审步骤

1. 至少邀请 3 位未实现本任务的评审者；记录其独立性声明，但不在工件中记录个人敏感信息。
2. 每位评审者在同一版本的 ForgeCAD 工作台逐一导入四份 `fixtures/*.glb`，使用 `cad_neutral`、等轴相机、默认工作室环境。不得用软件概念 PNG、外部 glTF 查看器截图或参数化 ShapeProgram 回退代替。
3. 对每份资产确认视口属性 `data-blockout-load-state=ready`、`data-blockout-render-source=glb_pbr`，且 `data-blockout-embedded-pbr-material-count` 大于 0。任一失败使本次 run 无效，不可用“看起来接近”补分。
4. 对每份资产独立给出 1–5 分：`proportion`（主次体比例与轮廓）、`material_readability`（多材质/PBR 区域的区分）和 `surface_detail`（接缝、边缘、重复视觉细节是否服从部件边界）。可附简短失败原因和截图 SHA，但不应记录 API Key、外部 URL、原始 Provider 内容或工程材料性能推断。
5. 将每位评审的声明和 4×3 分数写入 `review-responses.json`；提交前核对 `kit_manifest_sha256` 与本次 `manifest.json` 一致。

## 3. 通过口径与禁止项

有效 run 需要 3 位或以上独立评审者，并且全部有效 asset-review 中每个维度的中位数均至少为 4/5。任意同源 PBR GLB 加载失败、评分来源不独立、少于四领域、少于三位评审者或参数外观回退冒充嵌入纹理，均为未通过，而非缺失值补齐。

通过后才可在 `CODEX_TASK_INDEX`、`DOCUMENTATION_STATUS` 和能力—Gate 矩阵把 M108 的人工基准项更新为有证据；在此之前，M108 仍为 `in_progress`，C105 继续 blocked。
