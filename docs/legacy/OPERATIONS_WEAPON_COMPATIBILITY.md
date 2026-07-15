# 旧运行与回归命令

状态：legacy，只供迁移维护者使用

旧 Weapon/Concept 回归仍由 package scripts 提供。默认产品开发不需要运行全部 legacy 门。

```bash
npm run r1:create-weapon-gate
npm run r1:generate3d-gate
npm run r1:worker-gate
npm run r1:patch-gate
npm run r1:unity-export-gate
npm run r2:gate
npm run r3:workbench-gate
npm run r4:planner-gate
npm run r5:quality-gate
```

旧外部 ComfyUI、本地神经 3D 和 Unity 专用操作文档已删除。默认不安装、不运行这些环境；若兼容迁移任务必须追溯，只从 Git 历史读取相应版本，并在隔离环境执行，不得恢复到主操作路径。

这些门主要证明历史 Weapon reference pack、Concept ModuleGraph、旧 Provider 和 Unity 交接没有在迁移中被意外破坏。它们不能替代 G1–G7、Agent 工作台 E2E、packaged sidecar 或真实 Provider 评测。

当 [兼容迁移计划](../COMPATIBILITY_MIGRATION.md) 的 M5 完成后，这些门应移到手动 compatibility workflow，不再阻断通用机械 Agent 默认发布。
