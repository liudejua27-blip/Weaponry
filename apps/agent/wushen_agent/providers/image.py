from __future__ import annotations

import json
import os
import copy
import struct
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Dict, Optional, Protocol, TypeVar

from ..models import CreateWeaponRequest, PatchWeaponRequest, ProviderSettings, utc_now


class ImageProviderError(Exception):
    def __init__(self, code: str, message: str, recoverable: bool = True) -> None:
        super().__init__(message)
        self.code = code
        self.recoverable = recoverable


class ImageProvider(Protocol):
    provider_id: str

    def generate_concept(
        self,
        request: CreateWeaponRequest,
        spec: Dict[str, Any],
        *,
        weapon_id: str,
        version_id: str,
    ) -> "ConceptImageResult":
        ...

    def generate_patch(
        self,
        request: PatchWeaponRequest,
        patch_prompt: Dict[str, Any],
        *,
        weapon_id: str,
        version_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_filename: str,
        source_width: int,
        source_height: int,
        mask_bytes: bytes,
        manifest: Dict[str, Any],
    ) -> "PatchImageResult":
        ...


@dataclass(frozen=True)
class ConceptImageResult:
    image_bytes: bytes
    mime_type: str
    ext: str
    width: int
    height: int
    provider_task_id: Optional[str]
    workflow: Dict[str, Any]
    metadata: Dict[str, Any]


@dataclass(frozen=True)
class PatchImageResult:
    image_bytes: bytes
    mime_type: str
    ext: str
    width: int
    height: int
    provider_task_id: Optional[str]
    workflow: Dict[str, Any]
    metadata: Dict[str, Any]


@dataclass(frozen=True)
class ComfyUIConfig:
    base_url: str
    workflow_template_path: Optional[str] = None
    patch_workflow_template_path: Optional[str] = None
    checkpoint_name: Optional[str] = None
    width: int = 1280
    height: int = 720
    timeout_seconds: float = 30
    poll_interval_seconds: float = 1
    max_wait_seconds: float = 180
    retry_attempts: int = 3
    retry_backoff_seconds: float = 0.5
    client_id: str = "wushen-forge-agent"


T = TypeVar("T")


class MockComfyUIProvider:
    provider_id = "mock_comfyui"

    def generate_concept(
        self,
        request: CreateWeaponRequest,
        spec: Dict[str, Any],
        *,
        weapon_id: str,
        version_id: str,
    ) -> ConceptImageResult:
        workflow = build_mock_workflow(spec, weapon_id=weapon_id, version_id=version_id)
        return ConceptImageResult(
            image_bytes=mock_concept_svg(weapon_id, str(spec.get("name") or "未命名国风神兵"), request.text).encode("utf-8"),
            mime_type="image/svg+xml",
            ext=".svg",
            width=1280,
            height=720,
            provider_task_id=f"mock_prompt_{weapon_id}",
            workflow=workflow,
            metadata={
                "provider": "mock_comfyui",
                "provider_task_id": f"mock_prompt_{weapon_id}",
                "mock": True,
                "visual_realism": "high_game_art_only",
                "workflow_template_id": "wushen_mock_comfyui",
                "workflow_template_version": "0.1.0",
                "workflow_template_path": "mock://comfyui",
                "checkpoint_name": "mock_toon_renderer",
                "width": 1280,
                "height": 720,
                "generation_provenance": mock_generation_provenance(),
            },
        )

    def generate_patch(
        self,
        request: PatchWeaponRequest,
        patch_prompt: Dict[str, Any],
        *,
        weapon_id: str,
        version_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_filename: str,
        source_width: int,
        source_height: int,
        mask_bytes: bytes,
        manifest: Dict[str, Any],
    ) -> PatchImageResult:
        workflow = build_mock_patch_workflow(
            patch_prompt,
            weapon_id=weapon_id,
            version_id=version_id,
            source_width=source_width,
            source_height=source_height,
        )
        return PatchImageResult(
            image_bytes=mock_patch_svg(
                weapon_id,
                request.instruction,
                request.target_area,
                width=source_width,
                height=source_height,
            ).encode("utf-8"),
            mime_type="image/svg+xml",
            ext=".svg",
            width=source_width,
            height=source_height,
            provider_task_id=f"mock_patch_{version_id}",
            workflow=workflow,
            metadata={
                "provider": "mock_comfyui",
                "provider_task_id": f"mock_patch_{version_id}",
                "mock": True,
                "visual_realism": "high_game_art_only",
                "workflow_template_id": "wushen_mock_patch_inpaint",
                "workflow_template_version": "0.1.0",
                "workflow_template_path": "mock://comfyui/patch",
                "checkpoint_name": "mock_toon_inpaint_renderer",
                "width": source_width,
                "height": source_height,
                "generation_provenance": {
                    "source_canvas": {"width": source_width, "height": source_height},
                    "mask_policy": "white_repaint_black_preserve",
                    "non_manufacturing_asset": True,
                },
            },
        )


class ComfyUIHTTPProvider:
    provider_id = "comfyui"

    def __init__(self, config: ComfyUIConfig) -> None:
        self.config = config

    def generate_concept(
        self,
        _request: CreateWeaponRequest,
        spec: Dict[str, Any],
        *,
        weapon_id: str,
        version_id: str,
    ) -> ConceptImageResult:
        workflow, workflow_metadata = build_comfyui_workflow(
            spec,
            weapon_id=weapon_id,
            version_id=version_id,
            config=self.config,
        )
        prompt_id = self._submit_prompt(workflow)
        history = self._wait_for_history(prompt_id)
        image_ref = first_image_ref(history, prompt_id)
        image_bytes = self._download_image(image_ref)
        mime_type = image_ref.get("mime_type") or infer_mime_type(str(image_ref.get("filename") or ""))
        width, height = image_dimensions(image_bytes, mime_type=mime_type, filename=str(image_ref.get("filename") or ""))
        return ConceptImageResult(
            image_bytes=image_bytes,
            mime_type=mime_type,
            ext=infer_ext(str(image_ref.get("filename") or "")),
            width=width,
            height=height,
            provider_task_id=prompt_id,
            workflow=workflow,
            metadata={
                "provider": "comfyui",
                "provider_task_id": prompt_id,
                **workflow_metadata,
                "filename": image_ref.get("filename"),
                "subfolder": image_ref.get("subfolder", ""),
                "type": image_ref.get("type", "output"),
            },
        )

    def generate_patch(
        self,
        request: PatchWeaponRequest,
        patch_prompt: Dict[str, Any],
        *,
        weapon_id: str,
        version_id: str,
        source_image_bytes: bytes,
        source_image_mime_type: str,
        source_image_filename: str,
        source_width: int,
        source_height: int,
        mask_bytes: bytes,
        manifest: Dict[str, Any],
    ) -> PatchImageResult:
        source_upload = self._upload_image(
            source_image_bytes,
            filename=source_image_filename,
            mime_type=source_image_mime_type,
            upload_type="input",
        )
        mask_upload = self._upload_image(
            mask_bytes,
            filename=f"{version_id}_patch_mask.png",
            mime_type="image/png",
            upload_type="input",
        )
        workflow, workflow_metadata = build_comfyui_patch_workflow(
            request,
            patch_prompt,
            weapon_id=weapon_id,
            version_id=version_id,
            source_upload=source_upload,
            mask_upload=mask_upload,
            source_width=source_width,
            source_height=source_height,
            manifest=manifest,
            config=self.config,
        )
        prompt_id = self._submit_prompt(workflow)
        history = self._wait_for_history(prompt_id)
        image_ref = first_image_ref(history, prompt_id)
        image_bytes = self._download_image(image_ref)
        mime_type = image_ref.get("mime_type") or infer_mime_type(str(image_ref.get("filename") or ""))
        width, height = image_dimensions(image_bytes, mime_type=mime_type, filename=str(image_ref.get("filename") or ""))
        return PatchImageResult(
            image_bytes=image_bytes,
            mime_type=mime_type,
            ext=infer_ext(str(image_ref.get("filename") or "")),
            width=width,
            height=height,
            provider_task_id=prompt_id,
            workflow=workflow,
            metadata={
                "provider": "comfyui",
                "provider_task_id": prompt_id,
                **workflow_metadata,
                "filename": image_ref.get("filename"),
                "subfolder": image_ref.get("subfolder", ""),
                "type": image_ref.get("type", "output"),
            },
        )

    def _submit_prompt(self, workflow: Dict[str, Any]) -> str:
        payload = {"prompt": workflow, "client_id": self.config.client_id}
        data = json.dumps(payload).encode("utf-8")
        request = urllib.request.Request(self._url("/prompt"), data=data, method="POST")
        request.add_header("Content-Type", "application/json")
        def submit() -> Dict[str, Any]:
            with urllib.request.urlopen(request, timeout=self.config.timeout_seconds) as response:
                return json.loads(response.read().decode("utf-8"))

        parsed = self._retry_provider_call("prompt submission", submit)
        prompt_id = parsed.get("prompt_id")
        if not isinstance(prompt_id, str) or not prompt_id:
            raise ImageProviderError("PROVIDER_BAD_OUTPUT", "ComfyUI /prompt response did not include prompt_id.")
        return prompt_id

    def _wait_for_history(self, prompt_id: str) -> Dict[str, Any]:
        deadline = time.time() + self.config.max_wait_seconds
        while time.time() < deadline:
            def read_history() -> Dict[str, Any]:
                with urllib.request.urlopen(self._url(f"/history/{prompt_id}"), timeout=self.config.timeout_seconds) as response:
                    return json.loads(response.read().decode("utf-8"))

            parsed = self._retry_provider_call("history polling", read_history)
            if prompt_id in parsed:
                return parsed
            time.sleep(self.config.poll_interval_seconds)
        raise ImageProviderError("PROVIDER_TIMEOUT", f"ComfyUI prompt {prompt_id} did not finish before timeout.")

    def _download_image(self, image_ref: Dict[str, Any]) -> bytes:
        query = urllib.parse.urlencode(
            {
                "filename": image_ref.get("filename", ""),
                "subfolder": image_ref.get("subfolder", ""),
                "type": image_ref.get("type", "output"),
            }
        )
        def download() -> bytes:
            with urllib.request.urlopen(self._url(f"/view?{query}"), timeout=self.config.timeout_seconds) as response:
                return response.read()

        return self._retry_provider_call("image download", download)

    def _upload_image(self, payload: bytes, *, filename: str, mime_type: str, upload_type: str) -> Dict[str, str]:
        safe_name = safe_upload_filename(filename)
        boundary = f"----wushen-forge-{int(time.time() * 1000)}"
        body = multipart_form_data(
            boundary,
            fields={"type": upload_type, "overwrite": "true"},
            files={"image": (safe_name, mime_type, payload)},
        )
        request = urllib.request.Request(self._url("/upload/image"), data=body, method="POST")
        request.add_header("Content-Type", f"multipart/form-data; boundary={boundary}")
        request.add_header("Content-Length", str(len(body)))

        def upload() -> Dict[str, Any]:
            with urllib.request.urlopen(request, timeout=self.config.timeout_seconds) as response:
                return json.loads(response.read().decode("utf-8"))

        parsed = self._retry_provider_call("image upload", upload)
        name = parsed.get("name") or parsed.get("filename") or safe_name
        if not isinstance(name, str) or not name:
            raise ImageProviderError("PROVIDER_BAD_OUTPUT", "ComfyUI /upload/image response did not include an image name.")
        subfolder = parsed.get("subfolder", "")
        image_type = parsed.get("type", upload_type)
        return {
            "name": name,
            "subfolder": str(subfolder) if subfolder is not None else "",
            "type": str(image_type) if image_type is not None else upload_type,
        }

    def _url(self, path: str) -> str:
        return self.config.base_url.rstrip("/") + path

    def _retry_provider_call(self, label: str, operation: Callable[[], T]) -> T:
        attempts = max(1, self.config.retry_attempts)
        last_error: Optional[BaseException] = None
        for attempt in range(1, attempts + 1):
            try:
                return operation()
            except urllib.error.HTTPError as exc:
                if not is_retryable_http_status(exc.code):
                    raise ImageProviderError("PROVIDER_BAD_OUTPUT", f"ComfyUI {label} failed with HTTP {exc.code}.", False) from exc
                last_error = exc
            except (urllib.error.URLError, TimeoutError, OSError) as exc:
                last_error = exc
            except json.JSONDecodeError as exc:
                raise ImageProviderError("PROVIDER_BAD_OUTPUT", f"ComfyUI {label} returned invalid JSON.") from exc
            if attempt < attempts:
                time.sleep(self.config.retry_backoff_seconds * attempt)
        raise ImageProviderError("PROVIDER_TIMEOUT", f"ComfyUI {label} failed after {attempts} attempts.") from last_error


def image_provider_from_env() -> ImageProvider:
    provider = os.environ.get("WUSHEN_IMAGE_PROVIDER", "mock").strip().lower()
    if provider == "comfyui":
        return ComfyUIHTTPProvider(
            ComfyUIConfig(
                base_url=os.environ.get("WUSHEN_COMFYUI_BASE_URL", "http://127.0.0.1:8188"),
                workflow_template_path=os.environ.get("WUSHEN_COMFYUI_WORKFLOW_TEMPLATE"),
                patch_workflow_template_path=os.environ.get("WUSHEN_COMFYUI_PATCH_WORKFLOW_TEMPLATE"),
                checkpoint_name=os.environ.get("WUSHEN_COMFYUI_CHECKPOINT"),
                width=int(os.environ.get("WUSHEN_COMFYUI_WIDTH", "1280")),
                height=int(os.environ.get("WUSHEN_COMFYUI_HEIGHT", "720")),
                timeout_seconds=float(os.environ.get("WUSHEN_COMFYUI_TIMEOUT_SECONDS", "30")),
                poll_interval_seconds=float(os.environ.get("WUSHEN_COMFYUI_POLL_INTERVAL_SECONDS", "1")),
                max_wait_seconds=float(os.environ.get("WUSHEN_COMFYUI_MAX_WAIT_SECONDS", "180")),
                retry_attempts=int(os.environ.get("WUSHEN_COMFYUI_RETRY_ATTEMPTS", "3")),
                retry_backoff_seconds=float(os.environ.get("WUSHEN_COMFYUI_RETRY_BACKOFF_SECONDS", "0.5")),
                client_id=os.environ.get("WUSHEN_COMFYUI_CLIENT_ID", "wushen-forge-agent"),
            )
        )
    return MockComfyUIProvider()


def image_provider_settings_from_env() -> list[ProviderSettings]:
    selected = os.environ.get("WUSHEN_IMAGE_PROVIDER", "mock").strip().lower()
    base_url = os.environ.get("WUSHEN_COMFYUI_BASE_URL", "http://127.0.0.1:8188")
    return [
        ProviderSettings(
            provider_id="mock_comfyui",
            kind="image",
            type="mock",
            display_name="Mock ComfyUI",
            enabled=selected == "mock",
            status="configured" if selected == "mock" else "available",
            base_url="mock://comfyui",
        ),
        ProviderSettings(
            provider_id="comfyui",
            kind="image",
            type="comfyui",
            display_name="ComfyUI HTTP API",
            enabled=selected == "comfyui",
            status="configured" if selected == "comfyui" else "available",
            base_url=base_url,
            has_secret=False,
            updated_at=utc_now(),
        ),
    ]


def build_mock_workflow(spec: Dict[str, Any], *, weapon_id: str, version_id: str) -> Dict[str, Any]:
    return {
        "workflow_format": "wushen_mock_comfyui@1",
        "weapon_id": weapon_id,
        "version_id": version_id,
        "prompt": spec["generation"]["concept_prompt"],
        "negative_prompt": spec["generation"]["negative_prompt"],
        "seed": spec["generation"].get("seed"),
        "output": {"width": 1280, "height": 720, "format": "svg"},
    }


def build_mock_patch_workflow(
    patch_prompt: Dict[str, Any],
    *,
    weapon_id: str,
    version_id: str,
    source_width: int,
    source_height: int,
) -> Dict[str, Any]:
    return {
        "workflow_format": "wushen_mock_patch_comfyui@1",
        "weapon_id": weapon_id,
        "version_id": version_id,
        "prompt": patch_prompt,
        "source": {"width": source_width, "height": source_height},
        "mask_policy": "white_repaint_black_preserve",
        "output": {"width": source_width, "height": source_height, "format": "svg"},
    }


def mock_generation_provenance() -> Dict[str, Any]:
    return {
        "checkpoint": "mock_toon_renderer",
        "sampler": {
            "node_id": "mock_sampler",
            "seed": 1,
            "steps": 24,
            "cfg": 7,
            "sampler_name": "euler",
            "scheduler": "normal",
            "denoise": 1,
        },
        "latent_image": {
            "node_id": "mock_canvas",
            "width": 1280,
            "height": 720,
            "batch_size": 1,
        },
        "save_image": {
            "node_id": "mock_save",
            "filename_prefix": "wushen/mock",
        },
    }


def mock_patch_svg(weapon_id: str, instruction: str, target_area: str, *, width: int, height: int) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 1280 720">
  <rect width="1280" height="720" fill="#121212"/>
  <path d="M210 390 C420 290 620 260 1060 160 C930 310 690 428 230 548 Z" fill="#2d2a27" stroke="#d6a84e" stroke-width="10"/>
  <path d="M315 397 C510 338 694 292 920 232" fill="none" stroke="#ff6a2a" stroke-width="22" stroke-linecap="round" opacity="0.72"/>
  <circle cx="820" cy="270" r="88" fill="#263f55" stroke="#74d2ff" stroke-width="9" opacity="0.82"/>
  <text x="72" y="96" fill="#f3e5c0" font-size="42" font-family="serif">Patch Preview</text>
  <text x="74" y="146" fill="#b7aa8f" font-size="24" font-family="sans-serif">target={escape_xml(target_area)} · fictional Unity game asset patch</text>
  <text x="74" y="188" fill="#696252" font-size="18" font-family="monospace">{escape_xml(weapon_id)}</text>
  <text x="74" y="650" fill="#8f8574" font-size="22" font-family="sans-serif">{escape_xml(instruction[:180])}</text>
</svg>
"""


def build_comfyui_workflow(
    spec: Dict[str, Any],
    *,
    weapon_id: str,
    version_id: str,
    config: ComfyUIConfig,
) -> tuple[Dict[str, Any], Dict[str, Any]]:
    template = load_workflow_template(config.workflow_template_path)
    workflow = copy.deepcopy(template["prompt"])
    bindings = template["bindings"]
    seed = spec["generation"].get("seed")
    if seed is None:
        seed = 1
    values = {
        "positive_prompt": spec["generation"]["concept_prompt"],
        "negative_prompt": spec["generation"]["negative_prompt"],
        "seed": int(seed),
        "width": int(config.width),
        "height": int(config.height),
        "filename_prefix": f"wushen/{weapon_id}/{version_id}",
        "checkpoint": config.checkpoint_name or template.get("default_checkpoint") or "v1-5-pruned-emaonly.safetensors",
    }
    for key, value in values.items():
        binding = bindings.get(key)
        if binding:
            apply_binding(workflow, binding, value)

    generation_provenance = extract_workflow_provenance(workflow)
    return workflow, {
        "workflow_template_id": template["template_id"],
        "workflow_template_version": template["template_version"],
        "workflow_template_path": template["template_path"],
        "checkpoint_name": values["checkpoint"],
        "width": values["width"],
        "height": values["height"],
        "seed": values["seed"],
        "generation_provenance": generation_provenance,
    }


def load_workflow_template(path: Optional[str]) -> Dict[str, Any]:
    template_path = Path(path).expanduser() if path else default_workflow_template_path()
    return load_workflow_template_file(template_path)


def load_patch_workflow_template(path: Optional[str]) -> Dict[str, Any]:
    template_path = Path(path).expanduser() if path else default_patch_workflow_template_path()
    return load_workflow_template_file(template_path)


def load_workflow_template_file(template_path: Path) -> Dict[str, Any]:
    try:
        raw = json.loads(template_path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise ImageProviderError("PROVIDER_UNCONFIGURED", f"ComfyUI workflow template not found: {template_path}") from exc
    except json.JSONDecodeError as exc:
        raise ImageProviderError("PROVIDER_BAD_OUTPUT", f"ComfyUI workflow template is not valid JSON: {template_path}") from exc
    for key in ["template_id", "template_version", "prompt", "bindings"]:
        if key not in raw:
            raise ImageProviderError("PROVIDER_BAD_OUTPUT", f"ComfyUI workflow template missing {key}: {template_path}")
    if not isinstance(raw["prompt"], dict) or not isinstance(raw["bindings"], dict):
        raise ImageProviderError("PROVIDER_BAD_OUTPUT", f"ComfyUI workflow template has invalid prompt/bindings: {template_path}")
    raw["template_path"] = str(template_path)
    return raw


def default_workflow_template_path() -> Path:
    return Path(__file__).resolve().parents[4] / "workflows" / "comfyui" / "concept_api_template.json"


def default_patch_workflow_template_path() -> Path:
    return Path(__file__).resolve().parents[4] / "workflows" / "comfyui" / "patch_inpaint_api_template.json"


def build_comfyui_patch_workflow(
    request: PatchWeaponRequest,
    patch_prompt: Dict[str, Any],
    *,
    weapon_id: str,
    version_id: str,
    source_upload: Dict[str, str],
    mask_upload: Dict[str, str],
    source_width: int,
    source_height: int,
    manifest: Dict[str, Any],
    config: ComfyUIConfig,
) -> tuple[Dict[str, Any], Dict[str, Any]]:
    template = load_patch_workflow_template(config.patch_workflow_template_path)
    workflow = copy.deepcopy(template["prompt"])
    bindings = template["bindings"]
    seed = int(manifest.get("seed") or 1)
    positive_prompt = (
        f"3渲2国风神兵，虚构 Unity 游戏资产，逼真外观，局部重绘区域：{request.target_area}。"
        f"局部修改要求：{request.instruction}。"
        f"保持元素：{', '.join(request.preserve) if request.preserve else 'overall silhouette, chinese motifs, toon outline'}。"
        "只生成游戏美术外观，不输出真实武器图纸、尺寸、材料配方或制造工艺。"
    )
    values = {
        "checkpoint": config.checkpoint_name or template.get("default_checkpoint") or "v1-5-pruned-emaonly.safetensors",
        "positive_prompt": positive_prompt,
        "negative_prompt": (
            "real weapon blueprint, manufacturing drawing, exact dimensions, material formula, machining steps, "
            "technical assembly instructions, real-world fabrication"
        ),
        "seed": seed,
        "width": int(source_width),
        "height": int(source_height),
        "denoise": denoise_for_patch_strength(request.strength),
        "source_image": source_upload["name"],
        "mask_image": mask_upload["name"],
        "filename_prefix": f"wushen/{weapon_id}/{version_id}/patch",
    }
    for key, value in values.items():
        binding = bindings.get(key)
        if binding:
            apply_binding(workflow, binding, value)

    generation_provenance = extract_workflow_provenance(workflow)
    return workflow, {
        "workflow_template_id": template["template_id"],
        "workflow_template_version": template["template_version"],
        "workflow_template_path": template["template_path"],
        "checkpoint_name": values["checkpoint"],
        "width": values["width"],
        "height": values["height"],
        "seed": values["seed"],
        "denoise": values["denoise"],
        "source_upload": source_upload,
        "mask_upload": mask_upload,
        "patch_prompt": patch_prompt,
        "generation_provenance": generation_provenance,
        "non_manufacturing_asset": True,
    }


def denoise_for_patch_strength(strength: str) -> float:
    if strength == "subtle":
        return 0.35
    if strength == "strong":
        return 0.75
    return 0.55


def apply_binding(workflow: Dict[str, Any], binding: Dict[str, Any], value: Any) -> None:
    node_id = str(binding["node"])
    input_name = str(binding["input"])
    try:
        workflow[node_id]["inputs"][input_name] = value
    except KeyError as exc:
        raise ImageProviderError(
            "PROVIDER_BAD_OUTPUT",
            f"ComfyUI workflow binding failed for node {node_id} input {input_name}.",
        ) from exc


def extract_workflow_provenance(workflow: Dict[str, Any]) -> Dict[str, Any]:
    provenance: Dict[str, Any] = {"nodes": {}}
    for node_id, node in workflow.items():
        if not isinstance(node, dict):
            continue
        class_type = node.get("class_type")
        inputs = node.get("inputs") if isinstance(node.get("inputs"), dict) else {}
        if class_type == "CheckpointLoaderSimple":
            provenance["checkpoint"] = inputs.get("ckpt_name")
            provenance["nodes"]["checkpoint_loader"] = node_id
        elif class_type == "KSampler":
            provenance["sampler"] = {
                "node_id": node_id,
                "seed": inputs.get("seed"),
                "steps": inputs.get("steps"),
                "cfg": inputs.get("cfg"),
                "sampler_name": inputs.get("sampler_name"),
                "scheduler": inputs.get("scheduler"),
                "denoise": inputs.get("denoise"),
            }
        elif class_type == "EmptyLatentImage":
            provenance["latent_image"] = {
                "node_id": node_id,
                "width": inputs.get("width"),
                "height": inputs.get("height"),
                "batch_size": inputs.get("batch_size"),
            }
        elif class_type == "LoadImage":
            provenance.setdefault("load_images", {})[node_id] = {
                "image": inputs.get("image"),
            }
        elif class_type == "VAEEncodeForInpaint":
            provenance["inpaint_encode"] = {
                "node_id": node_id,
                "grow_mask_by": inputs.get("grow_mask_by"),
            }
        elif class_type == "SaveImage":
            provenance["save_image"] = {
                "node_id": node_id,
                "filename_prefix": inputs.get("filename_prefix"),
            }
    return provenance


def first_image_ref(history: Dict[str, Any], prompt_id: str) -> Dict[str, Any]:
    entry = history.get(prompt_id)
    if not isinstance(entry, dict):
        raise ImageProviderError("PROVIDER_BAD_OUTPUT", "ComfyUI history did not include prompt entry.")
    outputs = entry.get("outputs")
    if not isinstance(outputs, dict):
        raise ImageProviderError("PROVIDER_BAD_OUTPUT", "ComfyUI history did not include outputs.")
    for output in outputs.values():
        if not isinstance(output, dict):
            continue
        images = output.get("images")
        if isinstance(images, list) and images:
            image = images[0]
            if isinstance(image, dict) and image.get("filename"):
                return image
    raise ImageProviderError("PROVIDER_BAD_OUTPUT", "ComfyUI history did not include a downloadable image.")


def infer_ext(filename: str) -> str:
    _, ext = os.path.splitext(filename)
    return ext if ext else ".png"


def infer_mime_type(filename: str) -> str:
    ext = infer_ext(filename).lower()
    if ext == ".jpg" or ext == ".jpeg":
        return "image/jpeg"
    if ext == ".webp":
        return "image/webp"
    return "image/png"


def is_retryable_http_status(status_code: int) -> bool:
    return status_code == 408 or status_code == 409 or status_code == 425 or status_code == 429 or 500 <= status_code <= 599


def image_dimensions(payload: bytes, *, mime_type: str, filename: str = "") -> tuple[int, int]:
    try:
        if payload.startswith(b"\x89PNG\r\n\x1a\n"):
            return png_dimensions(payload)
        if payload.startswith(b"\xff\xd8"):
            return jpeg_dimensions(payload)
        if payload.startswith(b"RIFF") and payload[8:12] == b"WEBP":
            return webp_dimensions(payload)
    except (IndexError, struct.error, ValueError) as exc:
        raise ImageProviderError("PROVIDER_BAD_OUTPUT", f"Could not parse image dimensions for {filename or mime_type}.") from exc
    raise ImageProviderError("PROVIDER_BAD_OUTPUT", f"Unsupported image payload for dimension detection: {filename or mime_type}.")


def png_dimensions(payload: bytes) -> tuple[int, int]:
    if len(payload) < 24 or payload[12:16] != b"IHDR":
        raise ValueError("invalid PNG IHDR")
    width, height = struct.unpack(">II", payload[16:24])
    return positive_dimensions(width, height)


def jpeg_dimensions(payload: bytes) -> tuple[int, int]:
    index = 2
    while index < len(payload):
        while index < len(payload) and payload[index] == 0xFF:
            index += 1
        if index >= len(payload):
            break
        marker = payload[index]
        index += 1
        if marker in {0xD8, 0xD9, 0x01} or 0xD0 <= marker <= 0xD7:
            continue
        if index + 2 > len(payload):
            break
        segment_length = struct.unpack(">H", payload[index:index + 2])[0]
        if segment_length < 2:
            raise ValueError("invalid JPEG segment length")
        segment_start = index + 2
        segment_end = index + segment_length
        if marker in {0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF}:
            if segment_start + 5 > len(payload):
                break
            height, width = struct.unpack(">HH", payload[segment_start + 1:segment_start + 5])
            return positive_dimensions(width, height)
        index = segment_end
    raise ValueError("JPEG SOF marker not found")


def webp_dimensions(payload: bytes) -> tuple[int, int]:
    chunk = payload[12:16]
    if chunk == b"VP8X":
        if len(payload) < 30:
            raise ValueError("invalid VP8X payload")
        width = 1 + int.from_bytes(payload[24:27], "little")
        height = 1 + int.from_bytes(payload[27:30], "little")
        return positive_dimensions(width, height)
    if chunk == b"VP8L":
        if len(payload) < 25:
            raise ValueError("invalid VP8L payload")
        bits = int.from_bytes(payload[21:25], "little")
        width = (bits & 0x3FFF) + 1
        height = ((bits >> 14) & 0x3FFF) + 1
        return positive_dimensions(width, height)
    if chunk == b"VP8 ":
        if len(payload) < 30:
            raise ValueError("invalid VP8 payload")
        width = struct.unpack("<H", payload[26:28])[0] & 0x3FFF
        height = struct.unpack("<H", payload[28:30])[0] & 0x3FFF
        return positive_dimensions(width, height)
    raise ValueError("unsupported WebP chunk")


def positive_dimensions(width: int, height: int) -> tuple[int, int]:
    if width <= 0 or height <= 0:
        raise ValueError("image dimensions must be positive")
    return width, height


def safe_upload_filename(filename: str) -> str:
    safe_name = os.path.basename(filename).strip().replace("\\", "_")
    return safe_name or "wushen_upload.png"


def multipart_form_data(
    boundary: str,
    *,
    fields: Dict[str, str],
    files: Dict[str, tuple[str, str, bytes]],
) -> bytes:
    chunks: list[bytes] = []
    for name, value in fields.items():
        chunks.append(f"--{boundary}\r\n".encode("utf-8"))
        chunks.append(f'Content-Disposition: form-data; name="{name}"\r\n\r\n'.encode("utf-8"))
        chunks.append(str(value).encode("utf-8"))
        chunks.append(b"\r\n")
    for name, (filename, mime_type, payload) in files.items():
        chunks.append(f"--{boundary}\r\n".encode("utf-8"))
        chunks.append(
            f'Content-Disposition: form-data; name="{name}"; filename="{safe_upload_filename(filename)}"\r\n'.encode("utf-8")
        )
        chunks.append(f"Content-Type: {mime_type}\r\n\r\n".encode("utf-8"))
        chunks.append(payload)
        chunks.append(b"\r\n")
    chunks.append(f"--{boundary}--\r\n".encode("utf-8"))
    return b"".join(chunks)


def mock_concept_svg(weapon_id: str, title: str, description: str) -> str:
    safe_title = escape_xml(title)
    safe_description = escape_xml(description[:180])
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">
  <rect width="1280" height="720" fill="#121212"/>
  <path d="M210 390 C420 290 620 260 1060 160 C930 310 690 428 230 548 Z" fill="#2d2a27" stroke="#d6a84e" stroke-width="10"/>
  <path d="M315 397 C510 338 694 292 920 232" fill="none" stroke="#ff6a2a" stroke-width="22" stroke-linecap="round" opacity="0.72"/>
  <circle cx="380" cy="425" r="48" fill="#c42e24" stroke="#ffcf70" stroke-width="8"/>
  <path d="M182 560 L350 420 L421 492 L245 622 Z" fill="#1f1e1c" stroke="#d6a84e" stroke-width="8"/>
  <text x="72" y="96" fill="#f3e5c0" font-size="42" font-family="serif">{safe_title}</text>
  <text x="74" y="146" fill="#b7aa8f" font-size="24" font-family="sans-serif">Fictional Unity game asset concept · visual realism only</text>
  <text x="74" y="188" fill="#696252" font-size="18" font-family="monospace">{escape_xml(weapon_id)}</text>
  <text x="74" y="650" fill="#8f8574" font-size="22" font-family="sans-serif">{safe_description}</text>
</svg>
"""


def escape_xml(value: str) -> str:
    return (
        value.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
        .replace("'", "&apos;")
    )
