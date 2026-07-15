# Third-Party License and SBOM Gate

This document is the release-facing license ledger for Wushen Forge. It does not replace full legal review, but it gives the release gate a concrete inventory to check.

## Release Commands

```text
npm run release:license-sbom
npm run release:gate
```

`release:license-sbom` checks machine-readable dependency inventories and this ledger. A production release remains blocked while any release dependency is unlocked, missing a license, uses a disallowed license, or has pending external model/runtime review.

## Automated Coverage

| Surface | Source | Gate status |
| --- | --- | --- |
| Desktop npm dependencies | `package-lock.json` | Automated license expression check. |
| Desktop Rust/Tauri dependencies | `apps/desktop/src-tauri/Cargo.lock` | Lockfile is committed; final Rust transitive license/NOTICE SBOM remains a release task. |
| Agent Python dependencies | `apps/agent/requirements-release.lock` | Automated pin and license expression check. |
| Unity import package | Legacy smoke only | Not part of the target ForgeCAD release; review separately only if Unity export is reintroduced. |

Allowed license atoms for current integrated code dependencies:

- `MIT`
- `Apache-2.0`
- `Apache-2.0 OR MIT`
- `BSD-2-Clause`
- `BSD-3-Clause`
- `ISC`
- `MPL-2.0`
- `CC-BY-4.0` for metadata/data packages only

Blocked license families unless explicitly approved for this product:

- `AGPL`
- `GPL`
- `LGPL`
- `SSPL`
- `BUSL`
- `Commons-Clause`
- custom commercial or unknown licenses

## External Runtime and Model Ledger

These projects are referenced by the product design or supported as external adapters. They are not automatically safe to bundle. Before production release, each selected runtime/model must have code license, model-weight license, commercial-use terms, attribution, redistribution, and output-ownership reviewed.

| Item | Role | Status | Release note |
| --- | --- | --- | --- |
| ComfyUI | Legacy external image workflow server | Not a product dependency | Not bundled; retained only for legacy regression until the old runtime is removed. |
| Stable Fast 3D | Legacy local 3D experiment | Rejected for P0 | Not bundled and not part of the zero-beginner product route. |
| TripoSR | Legacy local 3D experiment | Rejected for P0 | Not bundled and not part of the zero-beginner product route. |
| Hunyuan3D | Research reference only | Rejected for P0 | Not integrated; model weight, VRAM and install cost conflict with the lightweight product. |
| TRELLIS | Research reference only | Rejected for P0 | Not integrated; model weight and GPU runtime conflict with the lightweight product. |
| Manifold Python 3.5.2 | Production CSG kernel | Integrated only by G825 through the existing Python sidecar; exact direct dependency and packaged collection are frozen | Apache-2.0. G824C/G824D record macOS/Windows packaged evidence; G825 adds the exact production lock/SBOM and no JS/WASM fallback. |
| Manifold WASM 3.5.1 | Evaluated geometry runtime candidate | Not recommended for current host; not integrated | Apache-2.0. Smaller payload does not justify a second JS/WASM host or moving authoritative geometry into the WebView. |
| Trimesh | Candidate mesh analysis/export runtime | Candidate review | MIT upstream; exact pinned dependency graph and release lock must be reviewed before integration. |
| Unity glTFast | Legacy Unity import verifier | Not a product dependency | Used only by the legacy smoke through Package Manager and not bundled; review separately if Unity export becomes a supported product feature again. |
| Tauri | Desktop shell | Automated | `Cargo.lock` is committed; Rust transitive license reporting still belongs in the final SBOM. |
| FastAPI | Agent API framework | Automated | Covered by `apps/agent/requirements-release.lock`; transitive Python runtime dependencies are pinned with license metadata. |
| Phosphor Icons for React | CAD 工作台图标 | Automated | `@phosphor-icons/react@2.1.10`，MIT；由 `package-lock.json` 固定并进入 npm license gate。 |
| Khronos glTF-Validator | 开发/CI GLB 标准合规检查 | Automated dev dependency | `gltf-validator@2.0.0-dev.3.10`，Apache-2.0；锁定在 `package-lock.json`，只读取原始 GLB 并输出报告，不成为资产或运行时真值。 |
| glTF Transform core/extensions | M108 GLB reader/writer 采用边界评估 | Automated dev dependency, evaluation-only | `@gltf-transform/core@4.4.1`、`@gltf-transform/extensions@4.4.1`，MIT；锁定在 `package-lock.json`，仅对四份原始 showcase GLB 进行受扩展注册的读写和拒绝决策验证。评估确认标准读取阶段仍保留 Part/zone/material 映射，但 writer 会删除 ForgeCAD 真实 readback 必需的显式默认 PBR 参数；该写出必须被拒绝，不能替换不可变编译 GLB，也不打包进桌面运行时。 |

## Reference-only GitHub projects

OpenAI Codex、OpenCode、goose、Zoo Design Studio、Aider 和 JSCAD 当前只作为设计/架构参考，不是 ForgeCAD 的安装依赖或派生代码，因此不因“被引用”进入产品 SBOM。glTF-Validator 与 glTF Transform core/extensions 已作为开发/CI 依赖列入上表，但都不打包进入桌面运行时。若后续实际复制、链接、安装或打包其他参考代码，必须先在本台账增加固定版本、许可证、NOTICE、二进制来源和传递依赖，再修改 lockfile。

参考用途和采用门见 [AGENT_GITHUB_REFERENCE_ARCHITECTURE.md](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md)。

## Current Production Blockers

- G825 has added the exact Manifold Python/NumPy production lock and packaged handler under ADR-0013. Future changes must preserve the single-kernel boundary and rerun license/SBOM plus packaged sidecar Gates.
- Review Trimesh packaging and add exact pinned dependencies before it enters the release build.
- Add an attribution bundle for licenses that require notices.
