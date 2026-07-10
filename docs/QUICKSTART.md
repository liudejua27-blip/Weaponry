# Quickstart

操作说明已经统一迁移到 [OPERATIONS.md](OPERATIONS.md)。

最短的迁移前基线检查：

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"
npm run m6:gate
```

注意：当前仓库仍运行 Weapon/Unity 旧基线；build123d、STEP/3MF、DFM 和 Print Doctor 尚未实现。浏览器 `127.0.0.1:5173` 只是 Vite 开发壳，Tauri 才是本地桌面产品路径。
