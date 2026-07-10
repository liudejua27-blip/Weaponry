# ADR-0002：build123d + OpenCascade 作为唯一 MVP CAD 内核

- 状态：Accepted
- 日期：2026-07-10

## 背景

ForgeCAD 需要精确、可参数化、可重新构建的 B-Rep，并能验证尺寸、导出 STEP/3MF/STL/GLB。build123d 与 CadQuery 都基于 OpenCascade，若 MVP 同时维护两套编译结果，会引入拓扑、圆角和布尔差异。

## 决策

1. MVP 权威 CAD 后端固定为 build123d + OpenCascade。
2. 定义 `CadBackend` Port，但 R3 只实现 `Build123dBackend`。
3. CadQuery 仅作为未来兼容后端，不参与双写、双构建或交叉裁决。
4. `DesignSpec → FeatureGraph → Compiler → B-Rep` 是权威链路。
5. STEP 是工程交换产物；3MF 是默认打印交付；STL 和 GLB 是派生产物。
6. 生产 STEP 和 3MF 必须回读并复测实体、单位、包围盒和关键尺寸。
7. CAD Runtime 必须独立进程运行，记录 build123d、OCCT、compiler 和 runtime 版本。

## 后果

- 可以集中建立拓扑选择、几何回归和失败诊断。
- 版本升级需要固定真值集与 round-trip 证据。
- 神经 3D 网格只能作为 `reference_mesh` 或概念资产，不能升级为权威 CAD。

## 被否决方案

- 同时用 build123d、CadQuery、OpenSCAD 生成并择优：结果不可稳定复现。
- 以 GLB/STL 为主模型再逆向 STEP：丢失参数和特征语义。
- Fork 完整 CAD 应用：产品架构会被通用编辑器反向约束。
