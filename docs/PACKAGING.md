# ForgeCAD Runtime 打包合同

版本：2026-08-08
状态：目标设计；旧 sidecar 已从当前树移除，MCP003 仅有签名路径/环境变量配置基线，尚未生成可发布安装包

## 1. 发布组件

同一 release manifest 包含：

- ForgeCAD Runtime Viewer；
- `forgecad-runtime`；
- `forgecad-mcp`；
- geometry/render workers；
- 可选 Blender worker；
- first-party Skill/asset packs；
- Runtime V1 migration；
- contracts/tool manifest/license/NOTICE/SBOM/provenance/signatures；
- Codex Desktop/CLI/IDE 配置助手。

组件合同版本和签名必须一致。旧 sidecar、App Server/Protocol、Python FastAPI、端口 8000、模型 Key 配置和 legacy packs 不进入安装包。

## 2. macOS 边界

安装包完成 code signing、hardened runtime、entitlements 最小化、notarization/stapling。Workers/MCP 也是签名可执行文件；Runtime 只开放 authenticated local IPC。Viewer CSP/Tauri capabilities 不允许 broad filesystem/network。

Codex MCP 配置只写本机签名二进制路径、timeout 和 write approval policy，不包含 secret 或项目绝对路径。卸载默认保留用户 Library，数据删除需独立选择。

## 3. 可选 Blender worker

若打包，必须固定版本、隔离进程、无网络、固定 Recipe、完成 GPL/源码提供/NOTICE 法律审查；不得运行 Codex/Skill 提供的 Python/addon。若不打包，capabilities 明确 unavailable，核心版本/几何真值不受影响。

## 4. 安装/升级

安装前验证磁盘和兼容性；升级前备份 Runtime V1 DB/CAS manifest；在副本跑 migration；原子替换整套组件；失败回滚二进制和数据库。禁止不同版本 MCP/Runtime/Viewer/Worker 混跑写路径。

## 5. 发布 Gate

clean-room 构建可复现、签名/notarization、SBOM/license、安全扫描、无绝对路径/secret、无 legacy/model/8000、Codex 三宿主 E2E、Viewer 关闭运行、升级/回滚、离线启动、灾难恢复、跨类别质量和真人门。
