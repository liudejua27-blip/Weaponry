# Local 3D Runtime

This document defines how Wushen Forge runs image-to-3D providers as local sidecar services.

The product boundary remains unchanged: outputs are fictional game art assets, Unity asset handoff files, and non-manufacturing descriptions. The runtime must not generate real-world weapon blueprints, manufacturing dimensions, material recipes, or process instructions.

## Current Runtime Shape

Wushen Forge uses two layers:

1. `LocalHTTPThreeDProvider` inside the Python Agent.
2. `scripts/wushen_local_3d_runtime.py` as a separate local HTTP process.

The process boundary is intentional. SF3D, TripoSR, Hunyuan3D, CUDA, MPS, and model weights are heavier and less predictable than the desktop Agent. Keeping them outside FastAPI/Tauri lets the Agent restart, classify failures, and avoid shipping every model dependency in the desktop bundle.

Agent-side provider selection:

```bash
export WUSHEN_3D_PROVIDER=local_http
export WUSHEN_3D_HTTP_BASE_URL=http://127.0.0.1:8787
export WUSHEN_GENERATE3D_ASYNC=1
```

The local HTTP service must implement `POST /v1/rough-models`, `GET /v1/rough-models/{task}`, `GET /v1/rough-models/{task}/result`, and `POST /v1/rough-models/{task}/cancel`.

## Implemented Backends

| Backend | Status | Purpose |
| --- | --- | --- |
| `mock` | Automated gate | Deterministic subprocess validation with legal GLB output. |
| `sf3d-cli` | Manual gate | Calls a local Stable Fast 3D checkout through `run.py`. |
| `triposr-cli` | Manual gate | Lower-license-friction fallback if SF3D license or install constraints block release. |
| `hunyuan3d-http` | Planned | Higher-quality path after wrapper, license, VRAM, and texture workflow are proven. |

## Why SF3D First

Stable Fast 3D is the first concrete backend because the official project is a single-image 3D reconstruction codebase, produces GLB through manual inference, and targets game-engine-friendly assets through UV unwrapping, delighting, texture, and material prediction.

Operational caveats:

- The SF3D model is gated on Hugging Face.
- The official setup expects Python, PyTorch, and optional CUDA/MPS.
- The official manual inference path is `python run.py <image> --output-dir <dir>`.
- Default single-image inference is documented as using about 6GB VRAM.
- Mac MPS and Windows support are documented as experimental; CPU fallback exists but should be treated as slow validation, not the target production path.
- The Stability AI Community License allows commercial use only under its stated revenue threshold; above that threshold an enterprise license is required.

## Install Sketch

### Stable Fast 3D

Use an external checkout rather than vendoring SF3D into this repo:

```bash
mkdir -p ~/Models/Wushen
cd ~/Models/Wushen
git clone https://github.com/Stability-AI/stable-fast-3d.git
cd stable-fast-3d
python3 -m venv .venv
source .venv/bin/activate
pip install -U setuptools==69.5.1
pip install wheel
pip install -r requirements.txt
huggingface-cli login
```

Install PyTorch according to the local GPU/OS before `requirements.txt`. For Apple Silicon, set:

```bash
export PYTORCH_ENABLE_MPS_FALLBACK=1
```

For forced CPU validation:

```bash
export SF3D_USE_CPU=1
```

### TripoSR

Use a separate external checkout rather than vendoring TripoSR into this repo:

```bash
mkdir -p ~/Models/Wushen
cd ~/Models/Wushen
git clone https://github.com/VAST-AI-Research/TripoSR.git
cd TripoSR
python3 -m venv .venv
source .venv/bin/activate
pip install -U pip wheel
pip install -r requirements.txt
```

TripoSR's official manual inference supports `python run.py <image> --output-dir <dir>` and a `--model-save-format glb` option. The Wushen wrapper always requests GLB so the Agent can keep the same Unity handoff contract.

On Apple silicon, upstream `run.py` falls back to CPU because it only probes CUDA. Set `WUSHEN_TRIPOSR_RUNNER` to this repository's `scripts/triposr_mps_runner.py` to keep neural inference on PyTorch MPS while moving only marching-cubes extraction to CPU. The adapter imports the external TripoSR checkout without modifying it and writes `triposr-run.json` beside each candidate output.

## Manual SF3D Smoke

From the Wushen Forge repo:

```bash
source smoke-gate07.env.example
npm run agent:p0-local-3d-runtime-sf3d-manual
```

Optional inputs:

```bash
export WUSHEN_SF3D_INPUT_IMAGE="/absolute/path/to/source.png"
export WUSHEN_SF3D_TEXTURE_RESOLUTION=1024
export WUSHEN_SF3D_REMESH_OPTION=triangle
export WUSHEN_SF3D_SMOKE_MAX_WAIT_SECONDS=900
export WUSHEN_SF3D_SMOKE_KEEP_WORK_DIR=1
```

If `WUSHEN_SF3D_INPUT_IMAGE` is not set, the smoke generates and uses a built-in
reference PNG so you can start a smoke run with only `run.py` ready.

Expected outputs:

```text
output/sf3d-manual/manual_sf3d_raw.glb
output/sf3d-manual/manual_sf3d_normalized.glb
output/sf3d-manual/manual_sf3d_optimized.glb
output/sf3d-manual/manual_sf3d_unity_material.json
```

The smoke starts `scripts/wushen_local_3d_runtime.py --backend sf3d-cli`, connects through `LocalHTTPThreeDProvider`, waits for provider success, validates GLB headers inside the adapter, and writes the GLB variants to the output directory.

## Manual TripoSR Smoke

From the Wushen Forge repo:

```bash
source smoke-gate07.env.example
npm run agent:p0-local-3d-runtime-triposr-manual
```

## Gate-07 一键复现（推荐）

从项目根目录执行：

```bash
# 1) 先写入最小必需环境变量
cp smoke-gate07.env.example .env.gate07
source .env.gate07

# 2) 先确认本地 Python 与模型路径可达，再分步跑每个 backend
test -x "$WUSHEN_SF3D_PYTHON" && echo "sf3d python ok" || echo "set WUSHEN_SF3D_PYTHON"
test -x "$WUSHEN_TRIPOSR_PYTHON" && echo "triposr python ok" || echo "set WUSHEN_TRIPOSR_PYTHON"

# 3) 运行 SF3D 与 TripoSR 手动 smoke
npm run agent:p0-local-3d-runtime-sf3d-manual
npm run agent:p0-local-3d-runtime-triposr-manual

# 4) 直接落地输出目录
ls -la "$WUSHEN_SF3D_SMOKE_OUTPUT_DIR" "$WUSHEN_TRIPOSR_SMOKE_OUTPUT_DIR"

# 5) Gate-10 前，避免 local_http 注入，先切回 mock 模式
WUSHEN_3D_PROVIDER=mock WUSHEN_GENERATE3D_ASYNC=1 npm run unity:import:gate
```

如需复现失败信息，优先保存：

- `.env.gate07`
- 命令返回 JSON（含 `ok:false` 和 `error`）
- 配置目录下的 GLB 与 `*material.json`

Optional inputs:

```bash
export WUSHEN_TRIPOSR_INPUT_IMAGE="/absolute/path/to/source.png"
export WUSHEN_TRIPOSR_DEVICE="mps"
export WUSHEN_TRIPOSR_RUNNER="$PWD/scripts/triposr_mps_runner.py"
export WUSHEN_TRIPOSR_PRETRAINED_MODEL="stabilityai/TripoSR"
export WUSHEN_TRIPOSR_CHUNK_SIZE=8192
export WUSHEN_TRIPOSR_MC_RESOLUTION=256
export WUSHEN_TRIPOSR_BAKE_TEXTURE=1
export WUSHEN_TRIPOSR_TEXTURE_RESOLUTION=2048
export WUSHEN_TRIPOSR_NO_REMOVE_BG=1
export WUSHEN_TRIPOSR_SMOKE_MAX_WAIT_SECONDS=900
export WUSHEN_TRIPOSR_SMOKE_KEEP_WORK_DIR=1
```

If `WUSHEN_TRIPOSR_INPUT_IMAGE` is not set, the smoke generates and uses a built-in
reference PNG.

Expected outputs:

```text
output/triposr-manual/manual_triposr_raw.glb
output/triposr-manual/manual_triposr_normalized.glb
output/triposr-manual/manual_triposr_optimized.glb
output/triposr-manual/manual_triposr_unity_material.json
```

The smoke starts `scripts/wushen_local_3d_runtime.py --backend triposr-cli`, forces TripoSR `--model-save-format glb`, connects through `LocalHTTPThreeDProvider`, and writes the same three GLB stages expected by the desktop 3D exhibition rig.

## Release Criteria For SF3D Backend

SF3D is not production-ready for Wushen until all criteria below are satisfied:

- `npm run agent:p0-local-3d-runtime-sf3d-manual` succeeds on the target developer machine.
- Output GLB loads in the existing Three.js exhibition rig.
- Output GLB survives `npm run unity:import:gate` with `unity_import_status=imported`.
- License decision is recorded for the intended business model and distribution plan.
- Runtime failures are classified into install error, auth/model access error, timeout, no GLB output, invalid GLB output, and cancellation.
- Provider task state can be resumed or safely invalidated after runtime restart.

## Provider Selection

| Provider | Use when | Blockers |
| --- | --- | --- |
| SF3D | Fast single-image rough model, game-friendly GLB, first production candidate. | Gated model access, Stability AI license threshold, GPU/MPS variability. |
| TripoSR | Fallback when permissive MIT licensing and simpler GLB/OBJ path matter more than SF3D material quality. | Less complete material pipeline; manual quality comparison still required. |
| Hunyuan3D | Higher-quality shape/texture path after the basic wrapper pattern is proven. | Heavier model family, more install modes, license review, VRAM and texture complexity. |

## Next Engineering Work

1. Run the SF3D manual smoke on a real local checkout and capture output evidence.
2. Run the TripoSR manual smoke and compare output quality/license friction against SF3D.
3. Add runtime restart behavior: persisted task ids should either resume polling or mark provider state as stale with a clear retry path.
4. Add model-quality checks beyond GLB validity: material slot count, texture presence, bounds, triangle count, orientation, pivot, and Unity import warnings.
