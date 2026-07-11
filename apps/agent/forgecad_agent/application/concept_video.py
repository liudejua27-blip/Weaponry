from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Sequence


class ConceptVideoError(RuntimeError):
    pass


TURNTABLE_VIDEO_FPS = 8
TURNTABLE_VIDEO_MIME_TYPE = "video/mp4"


def find_ffmpeg() -> Path | None:
    candidates: list[Path] = []
    configured = os.environ.get("FORGECAD_FFMPEG_EXECUTABLE")
    if configured:
        candidates.append(Path(configured).expanduser())
    discovered = shutil.which("ffmpeg")
    if discovered:
        candidates.append(Path(discovered))
    candidates.extend((Path("/opt/homebrew/bin/ffmpeg"), Path("/usr/local/bin/ffmpeg")))
    for candidate in candidates:
        resolved = candidate.resolve()
        if resolved.is_file() and os.access(resolved, os.X_OK):
            return resolved
    return None


def encode_turntable_mp4(frames: Sequence[bytes], *, fps: int = TURNTABLE_VIDEO_FPS) -> bytes:
    if len(frames) < 2:
        raise ConceptVideoError("turntable video requires at least two PNG frames")
    if not 1 <= fps <= 60:
        raise ConceptVideoError("turntable video fps must be between 1 and 60")
    ffmpeg = find_ffmpeg()
    if ffmpeg is None:
        raise ConceptVideoError(
            "FFmpeg is not configured; install it or set FORGECAD_FFMPEG_EXECUTABLE"
        )
    with tempfile.TemporaryDirectory(prefix="forgecad-turntable-") as temporary:
        root = Path(temporary)
        for index, payload in enumerate(frames):
            if not payload.startswith(b"\x89PNG\r\n\x1a\n"):
                raise ConceptVideoError(f"turntable frame {index} is not a PNG")
            (root / f"frame-{index:03d}.png").write_bytes(payload)
        output = root / "turntable.mp4"
        command = [
            str(ffmpeg),
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-framerate",
            str(fps),
            "-start_number",
            "0",
            "-i",
            str(root / "frame-%03d.png"),
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "slow",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "1",
            "-map_metadata",
            "-1",
            "-metadata",
            "creation_time=1970-01-01T00:00:00Z",
            "-movflags",
            "+faststart",
            str(output),
        ]
        completed = subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
            timeout=120,
        )
        if completed.returncode != 0 or not output.is_file():
            diagnostic = (completed.stderr or completed.stdout).strip()[-2000:]
            raise ConceptVideoError(f"FFmpeg turntable encoding failed: {diagnostic}")
        payload = output.read_bytes()
        if len(payload) < 12 or payload[4:8] != b"ftyp":
            raise ConceptVideoError("FFmpeg output is not a valid MP4 container")
        return payload
