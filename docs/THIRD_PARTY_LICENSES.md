# MCP002/MCP003 License / SBOM Ledger

This ledger is deliberately small for the reset. No model SDK, model weight,
remote mesh service, DCC plugin or runtime sidecar is part of MCP002.

| Component | Role | License/source | Status |
| --- | --- | --- | --- |
| Rust standard library | Runtime implementation | Rust project terms | tracked |
| Tauri 2 | Desktop viewer shell | MIT/Apache-2.0 | tracked in Cargo lock after build |
| rusqlite bundled SQLite | Runtime store | MIT | tracked before adoption |
| React/Vite | Viewer build | MIT | tracked in npm lock |
| Three.js | reserved viewer capability, not loaded by MCP002 | MIT | not adopted by runtime yet |

MCP002: every future Skill, asset, texture, renderer, geometry operator or
GitHub adoption must add a pinned source, license, SBOM entry, signature and
benchmark receipt before entering a release build.
