#!/usr/bin/env python3
"""Read-only validation for ForgeCAD's local macOS application identity.

Keychain ACLs identify the requesting application by its code-signing
requirement. An ad-hoc signature is effectively tied to one CodeDirectory hash,
so rebuilding the binary makes it a different requester and can trigger a new
password prompt. Live Provider acceptance must therefore use a valid,
certificate-signed bundle with a stable Team ID and bundle identifier.

This module never reads a Keychain item or signing private key. It invokes only
the read-only `codesign --display` and `codesign --verify` operations.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import re
import subprocess


EXPECTED_BUNDLE_IDENTIFIER = "local.wushen.forge"
_FIELD = re.compile(r"^(Identifier|TeamIdentifier|Signature)=(.*)$", re.MULTILINE)


@dataclass(frozen=True)
class MacOsStableIdentityEvidence:
    status: str
    bundle_identifier_bound: bool
    team_identifier_bound: bool
    certificate_signature: bool
    designated_requirement_bound: bool
    strict_bundle_valid: bool

    @property
    def ready(self) -> bool:
        return self.status == "ready"


def _run(*arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        arguments,
        capture_output=True,
        text=True,
        check=False,
    )


def _fields(display: str) -> dict[str, str]:
    return {key: value.strip() for key, value in _FIELD.findall(display)}


def evaluate_identity_text(
    *,
    display: str,
    requirement: str,
    strict_bundle_valid: bool,
    expected_identifier: str = EXPECTED_BUNDLE_IDENTIFIER,
) -> MacOsStableIdentityEvidence:
    fields = _fields(display)
    identifier_bound = fields.get("Identifier") == expected_identifier
    team = fields.get("TeamIdentifier")
    team_bound = isinstance(team, str) and bool(team) and team != "not set"
    signature = fields.get("Signature")
    certificate_signature = signature != "adhoc" and team_bound
    requirement_bound = (
        "designated =>" in requirement
        and f'identifier "{expected_identifier}"' in requirement
        and ("certificate leaf[subject.OU]" in requirement or "anchor apple" in requirement)
    )
    ready = all(
        (
            strict_bundle_valid,
            identifier_bound,
            team_bound,
            certificate_signature,
            requirement_bound,
        )
    )
    return MacOsStableIdentityEvidence(
        status="ready" if ready else "blocked_stable_identity_missing",
        bundle_identifier_bound=identifier_bound,
        team_identifier_bound=team_bound,
        certificate_signature=certificate_signature,
        designated_requirement_bound=requirement_bound,
        strict_bundle_valid=strict_bundle_valid,
    )


def inspect_stable_app_identity(app_bundle: Path) -> MacOsStableIdentityEvidence:
    if not app_bundle.is_dir():
        return MacOsStableIdentityEvidence(
            status="blocked_app_missing",
            bundle_identifier_bound=False,
            team_identifier_bound=False,
            certificate_signature=False,
            designated_requirement_bound=False,
            strict_bundle_valid=False,
        )
    verify = _run(
        "/usr/bin/codesign",
        "--verify",
        "--deep",
        "--strict",
        "--verbose=2",
        str(app_bundle),
    )
    display = _run("/usr/bin/codesign", "--display", "--verbose=4", str(app_bundle))
    requirement = _run("/usr/bin/codesign", "--display", "-r-", str(app_bundle))
    return evaluate_identity_text(
        display=f"{display.stdout}\n{display.stderr}",
        requirement=f"{requirement.stdout}\n{requirement.stderr}",
        strict_bundle_valid=verify.returncode == 0,
    )


__all__ = [
    "EXPECTED_BUNDLE_IDENTIFIER",
    "MacOsStableIdentityEvidence",
    "evaluate_identity_text",
    "inspect_stable_app_identity",
]
