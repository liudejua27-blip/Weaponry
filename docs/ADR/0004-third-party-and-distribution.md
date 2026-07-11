# ADR-0004：第三方依赖、许可证与分发边界

- 状态：Accepted
- 日期：2026-07-10

## 决策

直接依赖候选：

| 项目 | 许可证 | 方式 |
| --- | --- | --- |
| build123d | Apache-2.0 | CAD Runtime 直接依赖 |
| three-cad-viewer | MIT | 桌面前端直接依赖 |
| three-mesh-bvh | MIT | 桌面前端直接依赖 |
| trimesh | MIT | Print Doctor 直接依赖 |
| Manifold | Apache-2.0 | 按需 Adapter |
| lib3mf | BSD-2-Clause | 3MF Adapter |
| Instructor | MIT | 结构化 LLM 输出 |
| CADGenBench | Apache-2.0 | 测试参考或测试依赖 |

需要隔离或专项审查：

- PrusaSlicer（AGPL-3.0）：只通过外部 CLI Adapter 调用，打包前单独审查分发方式。
- NopSCADlib（GPL-3.0）：只参考 BOM/爆炸图思想，不复制实现。
- Fusion 360 Gallery Dataset：非商业研究限制，不进入商业训练或分发管线。

每个依赖 PR 必须：

1. 固定 tag、版本或 commit；
2. 保存许可证和 copyright notice；
3. 记录是否修改；
4. 说明直接依赖、动态/静态链接、独立进程或仅参考；
5. 更新 `THIRD_PARTY_NOTICES` 与 SBOM；
6. 完成漏洞、维护状态和平台兼容检查。

## 后果

- PrusaSlicer 未安装或未获准分发时，系统降级为无切片估算模式，不阻断核心几何和 DFM。
- 第三方许可证变化会阻断发布，不由 README 声明替代专项审查。
- 本 ADR 不是法律意见。
