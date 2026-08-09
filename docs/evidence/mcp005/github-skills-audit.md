# MCP005 GitHub Skill/Tool Audit

日期：2026-08-08
范围：只读研究，不下载、安装、执行或复制第三方仓库。

## `img2threejs/img2threejs`

- URL：<https://github.com/img2threejs/img2threejs>
- 观察到的 HEAD：`d6673386f89673a58736f8d398dd16ece67874f5`
- 仓库声明许可证：Apache-2.0。
- 可借鉴：分阶段 `blockout → structural → form → material → surface → lighting → interaction → optimization`；先做 detail inventory，再生成；每阶段用固定比较图和 per-region confidence 做门；把机械验证交给确定性脚本，把视觉判断留给 Codex。
- 不直接采用：仓库产物是 Three.js/TypeScript 工厂，且附带 Python/浏览器工作流；这些不是 ForgeCAD 的 GeometryProgram、Runtime receipt 或 GLB 真值。ForgeCAD 只做 first-party typed reimplementation：声明式 DAG、预注册 Rust Operator、固定预算、hash/lineage 和独立 Quality Compiler。

## `javierbyte/img2css`

- URL：<https://github.com/javierbyte/img2css>
- 观察到的 HEAD：`5dce10537589422c254a01a1d71aa805697d2c7f`
- 仓库声明许可证：BSD-3-Clause。
- 可借鉴：把像素颜色和区域作为低成本 reference preview；用于 Codex 的颜色采样、轮廓/遮罩草图和 compare sheet 辅助。
- 不直接采用：box-shadow CSS 矩阵和 base64 HTML 输出只适合网页展示，不能进入 Runtime 几何真值；ForgeCAD 不执行仓库 JavaScript，不保存 HTML/CSS，不允许它读写文件或网络。

## 采用状态

当前两项均为 `approved-for-evaluation` / `reference-only`，没有修改 lockfile，也没有安装第三方 Skill。仓库内的 `reference-to-typed-plan` 仅作为 superseded provenance 保留；MCP006 registry/十个独立 Bundle 是 first-party、声明式、development-only，已通过 Bundle/DAG/operator/validator/benchmark/SBOM Gate，但仍不能作为 Geometry/Render 结果或第三方插件。正式签名和第三方安装留给 MCP012–013。
