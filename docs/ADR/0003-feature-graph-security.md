# ADR-0003：受控 FeatureGraph 与 CAD Runtime 安全边界

- 状态：Accepted
- 日期：2026-07-10

## 背景

让 LLM 直接生成并执行 build123d/CadQuery Python 能快速演示，但会暴露任意代码执行、文件/网络访问、资源耗尽、不可审计修改和不可稳定重建等风险。

## 决策

1. 最终用户 Agent 不提供 `execute_python`、`run_shell`、`open_file`、`install_package` 或任意网络工具。
2. LLM 只提交通过 JSON Schema 的 `DesignSpec`、`FeatureGraph` 和 `ChangeSet`。
3. 首版 Feature allowlist：SketchRectangle、SketchCircle、SketchPolyline、Extrude、Revolve、Hole、Shell、Rib、Fillet、Chamfer、LinearPattern、CircularPattern、Mirror、Union、Difference。
4. 参数必须声明类型、单位、范围；内部长度统一为 mm。
5. 拓扑引用使用 owner feature、几何类型、法向、语义标签、interface id 和 anchor，禁止长期依赖 `Face[n]` / `Edge[n]`。
6. CAD Runtime 使用临时目录、无网络默认值，以及 CPU、内存、时间、特征数和输出大小限制。
7. 每次构建产生结构化诊断；自动修复最多 2–3 次。
8. locked interface 和 critical dimension 在应用 ChangeSet 前后都必须复测。

## 后果

- 需要自行实现 FeatureGraph Compiler 和语义选择器。
- 功能覆盖会比任意 Python 慢，但安全、审计、版本和失败定位可成立。
- 内部开发调试可以使用独立实验工具，但不能进入生产用户路径。

## 安全验收

FeatureGraph 必须无法读取任意文件、访问网络、执行 shell、逃离工作目录、无限循环、创建无限特征或生成超大文件。
