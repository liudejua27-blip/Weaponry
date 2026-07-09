from __future__ import annotations

import itertools
from typing import Dict, Iterable, List, Optional

from .models import JobDetail, JobEvent, ProviderSettings, WeaponSummary, utc_now


class InMemoryJobStore:
    """M1-only mock store. M2 replaces this with SQLite + immutable AssetStore."""

    def __init__(self) -> None:
        self._job_seq = itertools.count(1)
        self._weapon_seq = itertools.count(1)
        self.jobs: Dict[str, JobDetail] = {}
        self.weapons: Dict[str, WeaponSummary] = {}
        self.providers = [
            ProviderSettings(
                provider_id="mock_llm",
                kind="llm",
                type="openai_compatible",
                display_name="Mock OpenAI-compatible LLM",
                has_secret=False,
            ),
            ProviderSettings(
                provider_id="mock_comfyui",
                kind="image",
                type="comfyui",
                display_name="Mock ComfyUI",
                base_url="mock://comfyui",
            ),
            ProviderSettings(
                provider_id="mock_3d",
                kind="three_d",
                type="mock",
                display_name="Mock 3D Provider",
            ),
        ]

    def create_mock_job(self, job_type: str) -> JobDetail:
        weapon_id = f"weapon_{next(self._weapon_seq):04d}"
        job_id = f"job_{next(self._job_seq):04d}"
        now = utc_now()
        summary = WeaponSummary(
            weapon_id=weapon_id,
            display_name="未命名国风神兵",
            weapon_family="sword",
            stage="draft",
            updated_at=now,
        )
        events = [
            JobEvent(
                id=f"evt_{job_id}_0001",
                seq=1,
                job_id=job_id,
                weapon_id=weapon_id,
                step="request_guard",
                status="succeeded",
                message="Request accepted by mock Agent.",
                progress=0.1,
            ),
            JobEvent(
                id=f"evt_{job_id}_0002",
                seq=2,
                job_id=job_id,
                weapon_id=weapon_id,
                step="weapon_spec_planner",
                status="succeeded",
                message="Mock WeaponDesignSpec created.",
                artifact_asset_id="file_mock_spec",
                progress=0.35,
            ),
            JobEvent(
                id=f"evt_{job_id}_0003",
                seq=3,
                job_id=job_id,
                weapon_id=weapon_id,
                step="image_submit",
                status="succeeded",
                message="Mock concept image recorded.",
                artifact_asset_id="file_mock_concept",
                progress=0.65,
            ),
            JobEvent(
                id=f"evt_{job_id}_0004",
                seq=4,
                job_id=job_id,
                weapon_id=weapon_id,
                step="rough3d_submit",
                status="succeeded",
                message="Mock rough GLB recorded.",
                artifact_asset_id="file_mock_glb",
                progress=0.9,
            ),
            JobEvent(
                id=f"evt_{job_id}_0005",
                seq=5,
                job_id=job_id,
                weapon_id=weapon_id,
                step="finalize_job",
                status="succeeded",
                message="Mock job completed.",
                progress=1,
            ),
        ]
        detail = JobDetail(
            job_id=job_id,
            weapon_id=weapon_id,
            type=job_type,
            status="succeeded",
            current_step="finalize_job",
            created_at=now,
            updated_at=now,
            outputs={
                "weapon_spec_id": f"spec_{weapon_id}",
                "current_version_id": "version_0001",
                "asset_ids": ["file_mock_spec", "file_mock_concept", "file_mock_glb"],
            },
            events=events,
        )
        self.jobs[job_id] = detail
        self.weapons[weapon_id] = summary.model_copy(
            update={"stage": "rough_3d", "current_version_id": "version_0001", "updated_at": now}
        )
        return detail

    def list_weapons(self) -> List[WeaponSummary]:
        return list(self.weapons.values())

    def get_job(self, job_id: str) -> JobDetail:
        return self.jobs[job_id]

    def iter_events(self, job_id: str, after: Optional[str] = None) -> Iterable[JobEvent]:
        seen_after = after is None
        for event in self.jobs[job_id].events:
            if not seen_after:
                seen_after = event.id == after
                continue
            yield event
