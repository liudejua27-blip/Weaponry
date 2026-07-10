# R0 基线冻结证据

日期：2026-07-10

## Git 恢复点

- 旧基线 commit：`1511794713fbcc4cc58ff05edec99c13b38e1849`
- 旧基线 tag：`legacy-wushen-v0.1`
- 重构分支：`codex/refactor-cad-dfm-agent`

tag 指向文档与代码重构前的已提交旧产品基线。当前未提交的 ForgeCAD 文档和后续代码只存在于重构分支工作区。

## 旧领域回归

命令：

```bash
npm run m6:gate
```

结果：通过。

覆盖证据：

- contract check；
- generated Schema/OpenAPI drift check；
- Python compileall；
- M6 structure recast smoke；
- desktop TypeScript check。

该结果只证明迁移前 CreativeWeaponGraph/SkillGraph 基线未被文档重构破坏，不证明 CAD/DFM 能力。

## 旧 release gate

命令：

```bash
npm run release:gate
```

结果：失败，首个失败项为 `release:safety-scope`。

原因：旧门要求 README 保留“虚构游戏美术资产 / 非制造说明 / 不输出可用于现实制造武器的精确图纸”等旧产品文案；ForgeCAD 文档已经采用真实功能件 CAD/DFM 新边界。

结论：

- 失败是产品边界切换的预期结果，不应通过把旧文案塞回新 README 规避；
- 旧 gate 保留为 legacy 历史证据；
- ForgeCAD 发布由计划中的 C01–C10、许可证/SBOM 和 clean-machine desktop package 门替代。

## R0 决策证据

- `ADR-0001`：产品转向与旧域冻结；
- `ADR-0002`：单一 CAD 内核；
- `ADR-0003`：FeatureGraph 安全；
- `ADR-0004`：第三方与分发边界；
- `ADR-0005`：旧数据与配置迁移。
