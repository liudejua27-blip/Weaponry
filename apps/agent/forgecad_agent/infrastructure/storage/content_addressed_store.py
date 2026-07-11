from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


class ObjectStoreError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class StoredObject:
    relative_path: str
    sha256: str
    byte_size: int


class ContentAddressedStore:
    """Store immutable payloads under their SHA-256 digest."""

    def __init__(self, library_root: Path) -> None:
        self.library_root = library_root.expanduser().resolve()
        self.objects_root = self.library_root / "objects" / "sha256"
        self.objects_root.mkdir(parents=True, exist_ok=True)

    def put(self, payload: bytes, *, extension: str) -> StoredObject:
        digest = hashlib.sha256(payload).hexdigest()
        normalized_extension = self._normalize_extension(extension)
        relative_path = (
            Path("objects")
            / "sha256"
            / digest[:2]
            / digest[2:4]
            / f"{digest}{normalized_extension}"
        )
        target = self.resolve(relative_path.as_posix())
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists():
            target.write_bytes(payload)
        return StoredObject(
            relative_path=relative_path.as_posix(),
            sha256=digest,
            byte_size=len(payload),
        )

    def read(self, relative_path: str, *, expected_sha256: Optional[str] = None) -> bytes:
        target = self.resolve(relative_path)
        if not target.is_file():
            raise ObjectStoreError("OBJECT_MISSING", f"Object file is missing: {relative_path}")
        payload = target.read_bytes()
        if expected_sha256 is not None:
            actual = hashlib.sha256(payload).hexdigest()
            if actual != expected_sha256:
                raise ObjectStoreError("OBJECT_HASH_MISMATCH", f"Object sha256 mismatch: {relative_path}")
        return payload

    def resolve(self, relative_path: str) -> Path:
        object_path = Path(relative_path)
        if object_path.is_absolute() or ".." in object_path.parts:
            raise ObjectStoreError("OBJECT_PATH_DENIED", "Object path is outside the library.")
        target = (self.library_root / object_path).resolve()
        try:
            target.relative_to(self.library_root)
        except ValueError as exc:
            raise ObjectStoreError("OBJECT_PATH_DENIED", "Object path is outside the library.") from exc
        return target

    @staticmethod
    def _normalize_extension(extension: str) -> str:
        value = extension.strip()
        if not value:
            return ""
        suffix = value if value.startswith(".") else f".{value}"
        if any(character in suffix for character in ("/", "\\", "\x00")):
            raise ObjectStoreError("OBJECT_EXTENSION_INVALID", "Object extension is invalid.")
        return suffix.lower()
