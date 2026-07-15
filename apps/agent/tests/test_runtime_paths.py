from pathlib import Path

from forgecad_agent.runtime_paths import runtime_resource_root


def test_runtime_resource_root_uses_pyinstaller_bundle(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.delenv("FORGECAD_RUNTIME_RESOURCE_ROOT", raising=False)
    monkeypatch.setattr("forgecad_agent.runtime_paths.sys._MEIPASS", str(tmp_path), raising=False)

    assert runtime_resource_root() == tmp_path.resolve()


def test_runtime_resource_root_prefers_explicit_override(monkeypatch, tmp_path: Path) -> None:
    explicit = tmp_path / "explicit"
    monkeypatch.setenv("FORGECAD_RUNTIME_RESOURCE_ROOT", str(explicit))
    monkeypatch.setattr("forgecad_agent.runtime_paths.sys._MEIPASS", str(tmp_path / "bundle"), raising=False)

    assert runtime_resource_root() == explicit.resolve()
