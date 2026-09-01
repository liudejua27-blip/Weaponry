#!/usr/bin/env python3
"""Extract and verify one closed Weaponry browser capture bundle.

The browser preview writes a JSON bundle into the hidden
``weaponry-capture-bundle`` output node.  This bounded exporter decodes the
exact 8 x 7 PNG payloads, verifies every byte count and SHA-256 against the
capture manifest, and writes a portable directory.  It never evaluates visual
quality and refuses to overwrite an existing non-empty destination.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
from typing import Any


VIEW_IDS = (
    "FRONT",
    "BACK",
    "TOP",
    "BOTTOM",
    "LEFT",
    "RIGHT",
    "REAR_THREE_QUARTER",
    "FPS_HOLD",
)
AOV_IDS = (
    "beauty",
    "silhouette",
    "depth",
    "normal",
    "part-id",
    "material-id",
    "wireframe",
)
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


class CaptureExportError(ValueError):
    """The browser bundle is missing, malformed, or hash-inconsistent."""


class CaptureNodeParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.active = False
        self.parts: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "output" and dict(attrs).get("id") == "weaponry-capture-bundle":
            if self.active or self.parts:
                raise CaptureExportError("capture output node is duplicated")
            self.active = True

    def handle_endtag(self, tag: str) -> None:
        if tag == "output" and self.active:
            self.active = False

    def handle_data(self, data: str) -> None:
        if self.active:
            self.parts.append(data)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise CaptureExportError(message)


def object_value(value: Any, label: str) -> dict[str, Any]:
    require(isinstance(value, dict), f"{label} must be an object")
    return value


def read_bundle(path: Path) -> tuple[dict[str, Any], bytes]:
    try:
        source = path.read_bytes()
        text = source.decode("utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise CaptureExportError(f"cannot read browser DOM: {error}") from error
    parser = CaptureNodeParser()
    parser.feed(text)
    encoded = "".join(parser.parts)
    require(encoded != "", "weaponry-capture-bundle output is empty or missing")
    try:
        bundle = json.loads(encoded)
    except json.JSONDecodeError as error:
        raise CaptureExportError(f"capture output is not JSON: {error}") from error
    return object_value(bundle, "capture bundle"), source


def validate_bundle(bundle: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], list[tuple[str, str, bytes]]]:
    require(set(bundle) == {"schema_version", "capture", "payloads"}, "capture bundle keys are not closed")
    require(bundle["schema_version"] == "WeaponryThreeJsBrowserExportBundle@1", "capture bundle schema drifted")
    capture = object_value(bundle["capture"], "capture")
    require(set(capture) == {"manifest", "receipt"}, "capture result keys are not closed")
    manifest = object_value(capture["manifest"], "capture manifest")
    receipt = object_value(capture["receipt"], "capture receipt")
    require(manifest.get("schema_version") == "WeaponryThreeJsCaptureManifest@1", "capture manifest schema drifted")
    require(receipt.get("schema_version") == "WeaponryThreeJsBrowserCaptureReceipt@1", "capture receipt schema drifted")
    require(tuple(manifest.get("view_ids", ())) == VIEW_IDS, "capture view order drifted")
    require(tuple(manifest.get("aov_ids", ())) == AOV_IDS, "capture AOV order drifted")
    views = manifest.get("views")
    require(isinstance(views, list) and len(views) == len(VIEW_IDS), "capture must contain eight views")

    metadata: dict[tuple[str, str], dict[str, Any]] = {}
    for view_id, view_value in zip(VIEW_IDS, views):
        view = object_value(view_value, f"view {view_id}")
        require(view.get("view_id") == view_id, f"view order drifted at {view_id}")
        aovs = view.get("aovs")
        require(isinstance(aovs, list) and len(aovs) == len(AOV_IDS), f"view {view_id} does not contain seven AOVs")
        for aov_id, aov_value in zip(AOV_IDS, aovs):
            aov = object_value(aov_value, f"AOV {view_id}/{aov_id}")
            require(aov.get("aov_id") == aov_id, f"AOV order drifted at {view_id}/{aov_id}")
            metadata[(view_id, aov_id)] = aov

    payloads = bundle["payloads"]
    require(isinstance(payloads, list) and len(payloads) == len(VIEW_IDS) * len(AOV_IDS), "capture must contain 56 PNG payloads")
    decoded: list[tuple[str, str, bytes]] = []
    seen: set[tuple[str, str]] = set()
    for index, payload_value in enumerate(payloads):
        payload = object_value(payload_value, f"payload {index}")
        require(set(payload) == {"view_id", "aov_id", "mime_type", "base64"}, f"payload {index} keys are not closed")
        key = (payload.get("view_id"), payload.get("aov_id"))
        require(key in metadata and key not in seen, f"payload {index} identity is missing or duplicated")
        require(payload.get("mime_type") == "image/png", f"payload {index} is not a PNG")
        try:
            raw = base64.b64decode(payload.get("base64", ""), validate=True)
        except (ValueError, TypeError) as error:
            raise CaptureExportError(f"payload {index} base64 is invalid") from error
        info = metadata[key]
        require(raw.startswith(PNG_SIGNATURE), f"payload {index} PNG signature is invalid")
        require(len(raw) == info.get("png_size_bytes"), f"payload {index} byte size drifted")
        require(sha256_bytes(raw) == info.get("png_sha256"), f"payload {index} SHA-256 drifted")
        seen.add(key)
        decoded.append((str(key[0]), str(key[1]), raw))
    require(seen == set(metadata), "capture payload coverage is incomplete")
    return manifest, receipt, decoded


def export_capture(dom_path: Path, output: Path) -> dict[str, Any]:
    bundle, source = read_bundle(dom_path)
    manifest, receipt, decoded = validate_bundle(bundle)
    if output.exists():
        require(output.is_dir(), "output exists and is not a directory")
        require(not any(output.iterdir()), "output directory must be empty")
    else:
        output.mkdir(parents=True)
    for view_id, aov_id, raw in decoded:
        (output / f"{view_id.lower()}--{aov_id}.png").write_bytes(raw)
    (output / "capture-manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (output / "capture-receipt.json").write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    summary = {
        "schema_version": "WeaponryThreeJsExtractedCaptureSummary@1",
        "source_dom_sha256": sha256_bytes(source),
        "manifest_sha256": receipt.get("manifest_sha256"),
        "receipt_sha256": receipt.get("canonical_sha256"),
        "rig_fingerprint": receipt.get("rig_fingerprint"),
        "program_fingerprint": receipt.get("program_fingerprint"),
        "scene_fingerprint": receipt.get("scene_fingerprint"),
        "view_count": len(VIEW_IDS),
        "aov_count_per_view": len(AOV_IDS),
        "png_count": len(decoded),
        "render_status": receipt.get("render_status"),
        "quality_status": receipt.get("quality_status"),
        "visual_status": receipt.get("visual_status"),
    }
    (output / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return summary


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dom", required=True, type=Path, help="Chrome --dump-dom output containing the closed capture bundle")
    parser.add_argument("--output", required=True, type=Path, help="new or empty output directory")
    args = parser.parse_args()
    try:
        summary = export_capture(args.dom, args.output)
    except CaptureExportError as error:
        raise SystemExit(f"WEAPONRY_BROWSER_CAPTURE_EXPORT_REFUSED: {error}") from error
    print(json.dumps(summary, ensure_ascii=False, sort_keys=True))


if __name__ == "__main__":
    main()
