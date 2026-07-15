from __future__ import annotations

import os
from dataclasses import dataclass, field
from typing import Mapping, Optional

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware


DEFAULT_CORS_ORIGINS = (
    # The production Tauri frontend is served from this custom origin.  Keep
    # it explicit rather than widening the local Agent to arbitrary origins.
    "tauri://localhost",
    "http://127.0.0.1:1420",
    "http://127.0.0.1:5173",
    "http://127.0.0.1:5174",
    "http://localhost:1420",
    "http://localhost:5173",
    "http://localhost:5174",
)


@dataclass(frozen=True)
class LocalApiSettings:
    title: str = "ForgeCAD Local Agent"
    version: str = "0.1.0"
    cors_origins: tuple[str, ...] = field(default_factory=lambda: DEFAULT_CORS_ORIGINS)
    allowed_headers: tuple[str, ...] = (
        "Content-Type",
        "If-Match",
        "Idempotency-Key",
        "Last-Event-ID",
        "X-Wushen-Client-Version",
        "X-ForgeCAD-Client-Version",
    )

    @classmethod
    def from_env(
        cls,
        *,
        title: str = "ForgeCAD Local Agent",
        version: str = "0.1.0",
        environ: Optional[Mapping[str, str]] = None,
    ) -> "LocalApiSettings":
        values = environ if environ is not None else os.environ
        configured = values.get("FORGECAD_CORS_ORIGINS") or values.get("WUSHEN_CORS_ORIGINS", "")
        extras = tuple(
            origin.strip().rstrip("/")
            for origin in configured.split(",")
            if origin.strip()
        )
        origins = tuple(dict.fromkeys((*DEFAULT_CORS_ORIGINS, *extras)))
        return cls(title=title, version=version, cors_origins=origins)


def create_local_api(settings: LocalApiSettings) -> FastAPI:
    app = FastAPI(title=settings.title, version=settings.version)
    app.add_middleware(
        CORSMiddleware,
        allow_origins=list(settings.cors_origins),
        allow_credentials=False,
        allow_methods=["GET", "POST", "OPTIONS"],
        allow_headers=list(settings.allowed_headers),
        # Snapshot writes use optimistic concurrency.  The browser must be
        # allowed to send If-Match and read the returned ETag; otherwise a
        # local Vite/Tauri origin silently loses the Snapshot revision even
        # though the server itself emitted it.
        expose_headers=["ETag", "Content-Disposition", "X-ForgeCAD-Render-Set-SHA256"],
    )
    return app
