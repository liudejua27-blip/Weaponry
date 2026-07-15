# ForgeCAD 故障恢复手册

版本：2026-07-13
适用对象：开发与发布维护人员

## 1. 恢复目标

目标是保护以下本机真值：

- SQLite 元数据和版本链；
- 内容寻址对象；
- Project、Agent Thread、AgentAssetVersion、ChangeSet 和组件；
- 审计与导出引用。

API Key、Keychain、Provider secret file、WAL/SHM 临时文件和缓存不进入 Library 备份。

当前备份已枚举并复制 `agent_imported_glbs.object_path`，并由备份/恢复 smoke 验证哈希、外键、Agent head、`ActiveDesignSnapshot` 与 export source/version 一致。仍应保留原始外部文件作为额外保全副本；备份不包含 Provider 密钥和未引用对象候选。

## 2. 立即处置

出现数据库错误、对象缺失、迁移失败或版本头异常时：

1. 停止 Tauri 和本地 Agent；
2. 不继续确认 ChangeSet、导出或运行迁移；
3. 复制错误信息和时间，不复制密钥；
4. 记录当前应用版本、commit 和 `WUSHEN_LIBRARY_ROOT`；
5. 对原 Library 只读保全，不直接修表；
6. 在新目录中验证备份或恢复。

## 3. 创建备份

目标目录必须在 Library 外部：

```bash
npm run library:backup -- \
  --library-root "$PWD/WushenForgeLibrary" \
  --output "$HOME/ForgeCADBackups/snapshot-$(date +%Y%m%d-%H%M%S)"
```

备份工具创建独立 SQLite 快照、引用对象和 `backup-manifest.json`，并排除 secret、WAL/SHM 和无引用对象。

## 4. 验证备份

```bash
npm run library:verify-backup -- \
  --backup "$HOME/ForgeCADBackups/<snapshot>"
```

验证必须检查：

- SQLite integrity；
- manifest 和对象 hash；
- 引用对象完整；
- 无额外或被篡改对象；
- 不包含配置和密钥。

验证失败的目录不能用于恢复。

## 5. 恢复到新目录

恢复目标必须不存在，且不能位于备份目录内部：

```bash
npm run library:restore -- \
  --backup "$HOME/ForgeCADBackups/<snapshot>" \
  --destination "$HOME/ForgeCADLibraries/restored-$(date +%Y%m%d-%H%M%S)"
```

恢复后：

```bash
export WUSHEN_LIBRARY_ROOT="$HOME/ForgeCADLibraries/<restored>"
script/build_and_run.sh --verify
```

在切换真实用户目录前验证：

- `/api/health`；
- 项目列表和活动版本；
- AgentAssetVersion 与对象 hash；
- 一个已有 GLB 的读取和导出；
- 对外部导入 GLB 单独核对原文件；
- 不创建新的无关版本。

## 6. 自动恢复演练

```bash
npm run agent:r3-library-backup-restore-smoke
npm run agent:r3-library-recovery-drill-smoke
npm run r3:library-backup-gate
```

发布候选必须在临时 Library 上运行，不得对真实用户库执行 smoke。

## 7. 常见故障

### Agent 无法启动

- 检查端口 8000 是否被其他服务占用；
- 检查 `/api/health` 的 service 标识；
- 检查迁移目录和 Library 权限；
- 当前开发版检查本机 Python 和 `.venv`；
- 生产版必须检查 packaged sidecar，不允许退回用户 Python。

### 数据库 integrity 失败

- 停止写入；
- 保全原目录；
- 使用最近一次验证通过的备份恢复到新目录；
- 不直接删除 WAL、表或迁移记录尝试“修复”。

### 对象缺失或 hash 不一致

- 运行备份验证确定缺失范围；
- 从已验证备份恢复；
- 不用同名文件替换内容寻址对象；
- 记录引用表、对象 hash 和受影响版本。

### 当前版本显示不一致

如果界面同时显示 legacy Concept 与 Agent 资产，先确认 `ActiveDesignSnapshot.source`；停止导出和确认，不要手工修改 localStorage 或数据库 head。Agent source 必须只使用 AgentAssetVersion、Snapshot 和 Agent export 链；legacy source 只能走只读/显式转换路径，异常时恢复到已验证备份。

### Provider 认证失败

Provider 失败不应损坏已有资产。检查配置、轮换密钥并重新测试连接；不要把 Key 写进事故记录。

## 8. 事故记录

最小记录：

- 发现时间和操作者；
- 应用版本、commit、平台和运行模式；
- Library 标识和最近备份 ID；
- 受影响 Project/Version/Object hash；
- 触发步骤和错误码；
- 停写时间；
- 恢复来源和验证结果；
- 后续回归测试。

事故关闭前必须补自动化回归或明确说明为何无法自动化。
