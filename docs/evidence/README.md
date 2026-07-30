# ForgeCAD 当前证据入口

版本：2026-07-29
状态：精简后的当前证据索引

当前树不再保存 R0–R6、旧 Weapon、Unity、早期 Planner、旧包装和一次性阶段报告。它们已经被当前合同、任务和 Gate 取代，详细内容从 Git 历史或被忽略的 `output/` 工件追溯。

当前只保留：

- [能力—Gate 矩阵](CAPABILITY_GATE_MATRIX.md)：当前实现、目标、阻断和可复现命令的唯一总表；
- [U002 类别开放 author Gate](U002_UNIVERSAL_AUTHOR_GATE.md)：通用理解/表示规划、typed limitation、机械臂正路径与零副作用边界；
- [F026 工作台视觉规格](f026/F026_VISUAL_SPEC.md)：仍参与单 renderer、docked/focus 与单结果工作台回归；
- `f026/*.png`：冻结视觉参考，不是运行成功或模型质量证据。

证据规则：

1. 当前能力必须同时有代码入口、可复现 Gate 和当前状态标签；
2. `output/` 中的 report、截图和模型只证明对应 run/commit，不自动升级文档状态；
3. 自动 Gate、视觉 Provider、真人盲评和商业指标分别记录；
4. 历史 PASS 不能证明当前脏工作区，目标设计不能写入用户指南。
