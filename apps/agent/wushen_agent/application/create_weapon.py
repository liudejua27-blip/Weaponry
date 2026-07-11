from __future__ import annotations

import hashlib
import json
import uuid
from typing import Any, Callable, Dict

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory

from ..models import CreateWeaponRequest, JobDetail, utc_now
from ..providers.image import ImageProvider, ImageProviderError
from ..providers.llm import (
    LLMProvider,
    LLMProviderError,
    derive_display_name,
    derive_weapon_family,
)
from ..providers.three_d import ThreeDProvider, ThreeDProviderError
from ..spec_validation import (
    WeaponSpecValidationError,
    validate_quality_report,
    validate_weapon_design_spec,
)


class CreateWeaponIdempotencyConflict(RuntimeError):
    pass


class LegacyCreateWeaponService:
    """Orchestrates the frozen legacy create flow behind the AssetStore facade."""

    def __init__(
        self,
        *,
        connection_factory: SQLiteConnectionFactory,
        llm_provider: LLMProvider,
        image_provider: ImageProvider,
        three_d_provider: ThreeDProvider,
        write_asset: Callable[..., str],
        write_rough_model_assets: Callable[..., Dict[str, str]],
        record_provider_task: Callable[..., str],
        upsert_checkpoint: Callable[..., None],
        insert_step: Callable[..., None],
        insert_event: Callable[..., None],
        get_job: Callable[[str], JobDetail],
        concept_quality_report: Callable[..., Dict[str, Any]],
    ) -> None:
        self.llm_provider = llm_provider
        self.image_provider = image_provider
        self.three_d_provider = three_d_provider
        self._connect = connection_factory.connect
        self._write_asset = write_asset
        self._write_rough_model_assets = write_rough_model_assets
        self._record_provider_task = record_provider_task
        self._upsert_checkpoint = upsert_checkpoint
        self._insert_step = insert_step
        self._insert_event = insert_event
        self.get_job = get_job
        self._concept_quality_report = concept_quality_report

    def create_weapon(self, request: CreateWeaponRequest, idempotency_key: str) -> JobDetail:
        idempotency_scope = "POST /api/weapons"
        request_json = _canonical_json(request.model_dump())
        request_hash = _sha256_bytes(request_json.encode("utf-8"))

        with self._connect() as conn:
            existing = conn.execute(
                """
                SELECT job_id, request_hash
                FROM generation_jobs
                WHERE idempotency_scope = ? AND idempotency_key = ?
                ORDER BY created_at DESC
                LIMIT 1
                """,
                (idempotency_scope, idempotency_key),
            ).fetchone()
            if existing is not None:
                if existing["request_hash"] != request_hash:
                    raise CreateWeaponIdempotencyConflict(
                        "Idempotency-Key was reused with a different request body."
                    )
                return self.get_job(existing["job_id"])

            now = utc_now()
            weapon_id = _new_id("weapon")
            job_id = _new_id("job")
            version_id = _new_id("ver")
            model_id = _new_id("model")
            spec = self.llm_provider.plan_weapon_spec(request, weapon_id=weapon_id)
            try:
                spec = validate_weapon_design_spec(spec, provider_id=self.llm_provider.provider_id)
            except WeaponSpecValidationError as exc:
                raise LLMProviderError("PROVIDER_BAD_OUTPUT", str(exc), recoverable=True) from exc
            display_name = str(spec.get("name") or derive_display_name(request.text))
            weapon_family = str(spec.get("weapon_family") or derive_weapon_family(request.text))
            try:
                concept = self.image_provider.generate_concept(
                    request, spec, weapon_id=weapon_id, version_id=version_id
                )
            except ImageProviderError as exc:
                raise LLMProviderError(exc.code, str(exc), recoverable=exc.recoverable) from exc

            conn.execute(
                """
                INSERT INTO weapons (
                  weapon_id, name, weapon_family, fantasy_category, style, status,
                  current_version_id, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?)
                """,
                (
                    weapon_id,
                    display_name,
                    weapon_family,
                    "mythic_weapon",
                    "3渲2国风神兵",
                    version_id,
                    now,
                    now,
                ),
            )
            conn.execute(
                """
                INSERT INTO generation_jobs (
                  job_id, weapon_id, job_type, status, current_step, idempotency_scope, idempotency_key,
                  request_hash, request_json, created_at, updated_at, finished_at
                )
                VALUES (?, ?, 'create_weapon', 'succeeded', 'finalize_job', ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    job_id,
                    weapon_id,
                    idempotency_scope,
                    idempotency_key,
                    request_hash,
                    request_json,
                    now,
                    now,
                    now,
                ),
            )
            self._insert_step(conn, job_id, "request_guard", "succeeded", "mock_guard")
            self._insert_step(
                conn, job_id, "weapon_spec_planner", "succeeded", self.llm_provider.provider_id
            )
            self._insert_step(
                conn, job_id, "prompt_builder", "succeeded", self.llm_provider.provider_id
            )
            self._insert_step(
                conn, job_id, "image_submit", "succeeded", self.image_provider.provider_id
            )
            self._insert_step(conn, job_id, "image_quality_check", "succeeded", "quality_checker")
            self._insert_step(conn, job_id, "rough3d_submit", "succeeded", "mock_3d")
            self._insert_step(conn, job_id, "finalize_job", "succeeded", "asset_store")
            conn.execute(
                """
                INSERT INTO weapon_versions (
                  version_id, weapon_id, parent_version_id, job_id, version_no,
                  version_type, status, summary, created_at
                )
                VALUES (?, ?, NULL, ?, 1, 'rough_3d', 'committed', ?, ?)
                """,
                (version_id, weapon_id, job_id, "Mock concept and rough 3D asset slice.", now),
            )

            spec_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="weapon_spec",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/weapon_spec.json",
                payload=_canonical_json(spec).encode("utf-8"),
                ext=".json",
                mime_type="application/json",
                metadata={"schema_version": "WeaponDesignSpec@1"},
            )
            prompt_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="prompt",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/concept_prompt.json",
                payload=_canonical_json(
                    {
                        "schema_version": "PromptArchive@1",
                        "weapon_id": weapon_id,
                        "version_id": version_id,
                        "provider": self.image_provider.provider_id,
                        "prompt": spec["generation"]["concept_prompt"],
                        "seed": spec["generation"].get("seed"),
                    }
                ).encode("utf-8"),
                ext=".json",
                mime_type="application/json",
                metadata={
                    "provider": self.image_provider.provider_id,
                    "schema_version": "PromptArchive@1",
                },
            )
            negative_prompt_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="negative_prompt",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/negative_prompt.json",
                payload=_canonical_json(
                    {
                        "schema_version": "PromptArchive@1",
                        "weapon_id": weapon_id,
                        "version_id": version_id,
                        "provider": self.image_provider.provider_id,
                        "negative_prompt": spec["generation"]["negative_prompt"],
                    }
                ).encode("utf-8"),
                ext=".json",
                mime_type="application/json",
                metadata={
                    "provider": self.image_provider.provider_id,
                    "schema_version": "PromptArchive@1",
                },
            )
            workflow_payload = _canonical_json(concept.workflow).encode("utf-8")
            workflow_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="comfyui_workflow",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/comfyui_workflow.json",
                payload=workflow_payload,
                ext=".json",
                mime_type="application/json",
                metadata={
                    "provider": self.image_provider.provider_id,
                    "provider_task_id": concept.provider_task_id,
                    "workflow_sha256": _sha256_bytes(workflow_payload),
                    "seed": spec["generation"].get("seed"),
                    "workflow_template_id": concept.metadata.get("workflow_template_id"),
                    "workflow_template_version": concept.metadata.get("workflow_template_version"),
                    "workflow_template_path": concept.metadata.get("workflow_template_path"),
                    "checkpoint_name": concept.metadata.get("checkpoint_name"),
                    "width": concept.metadata.get("width"),
                    "height": concept.metadata.get("height"),
                    "generation_provenance": concept.metadata.get("generation_provenance"),
                },
            )
            concept_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="concept_image",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/concept{concept.ext}",
                payload=concept.image_bytes,
                ext=concept.ext,
                mime_type=concept.mime_type,
                metadata={
                    **concept.metadata,
                    "workflow_asset_id": workflow_file_id,
                    "prompt_asset_id": prompt_file_id,
                    "negative_prompt_asset_id": negative_prompt_file_id,
                },
                width=concept.width or None,
                height=concept.height or None,
            )
            concept_sha256 = conn.execute(
                "SELECT sha256 FROM asset_files WHERE file_id = ?", (concept_file_id,)
            ).fetchone()["sha256"]
            quality_report = validate_quality_report(
                self._concept_quality_report(
                    concept_file_id,
                    concept_sha256=concept_sha256,
                    provider_id=self.image_provider.provider_id,
                    workflow_file_id=workflow_file_id,
                    provider_task_id=concept.provider_task_id,
                ),
                provider_id="quality_checker",
            )
            if quality_report["status"] not in {"passed", "warning"}:
                raise LLMProviderError(
                    "PROVIDER_BAD_OUTPUT",
                    "Concept image quality gate did not pass.",
                    recoverable=True,
                )
            quality_report_file_id = self._write_asset(
                conn,
                weapon_id=weapon_id,
                version_id=version_id,
                job_id=job_id,
                role="quality_report",
                logical_path=f"weapons/{weapon_id}/versions/{version_id}/concept_quality_report.json",
                payload=_canonical_json(quality_report).encode("utf-8"),
                ext=".json",
                mime_type="application/json",
                metadata={
                    "schema_version": "QualityReport@1",
                    "target_asset_id": concept_file_id,
                    "target_sha256": concept_sha256,
                    "provider": "quality_checker",
                },
            )
            try:
                model_assets = self._write_rough_model_assets(
                    conn,
                    weapon_id=weapon_id,
                    version_id=version_id,
                    job_id=job_id,
                    model_id=model_id,
                    source_image_file_id=concept_file_id,
                    source_image_bytes=concept.image_bytes,
                    source_image_mime_type=concept.mime_type,
                    source_image_logical_path=f"weapons/{weapon_id}/versions/{version_id}/concept{concept.ext}",
                    target_format=request.target.output_format,
                    style="stylized_toon_weapon",
                    orientation_policy={
                        "forward_axis": "+Z",
                        "long_axis": "+Y",
                        "pivot": "grip_center",
                    },
                    scale_policy="normalized_game_asset_scale",
                    gated_by_quality_report_file_id=quality_report_file_id,
                )
            except ThreeDProviderError as exc:
                raise LLMProviderError(exc.code, str(exc), recoverable=exc.recoverable) from exc

            self._record_provider_task(
                conn,
                job_id=job_id,
                step_name="image_submit",
                attempt=1,
                provider_kind="image",
                provider_id=self.image_provider.provider_id,
                provider_task_id=concept.provider_task_id,
                status="succeeded",
                metadata={
                    "artifact_asset_id": concept_file_id,
                    "workflow_asset_id": workflow_file_id,
                },
                updated_at=now,
            )
            model_task_record_id = self._record_provider_task(
                conn,
                job_id=job_id,
                step_name="rough3d_submit",
                attempt=1,
                provider_kind="three_d",
                provider_id=self.three_d_provider.provider_id,
                provider_task_id=model_assets.get("provider_task_id") or None,
                status="succeeded",
                metadata={
                    "model_id": model_id,
                    "raw_model_file_id": model_assets["raw_model_file_id"],
                },
                updated_at=now,
            )
            self._upsert_checkpoint(
                conn,
                job_id=job_id,
                step_name="rough3d_submit",
                attempt=1,
                status="completed",
                resume_policy="skip_completed",
                provider_task_record_id=model_task_record_id,
                state={
                    "model_id": model_id,
                    "provider_task_id": model_assets.get("provider_task_id") or None,
                    "raw_model_file_id": model_assets["raw_model_file_id"],
                    "optimized_model_file_id": model_assets["optimized_model_file_id"],
                },
                updated_at=now,
            )

            spec_sha256 = conn.execute(
                "SELECT sha256 FROM asset_files WHERE file_id = ?", (spec_file_id,)
            ).fetchone()["sha256"]
            conn.execute(
                """
                INSERT INTO weapon_specs (
                  spec_id, weapon_id, version_id, schema_version, spec_json,
                  spec_sha256, safety_policy_version, created_at
                )
                VALUES (?, ?, ?, 'WeaponDesignSpec@1', ?, ?, 'safety_boundary@1', ?)
                """,
                (_new_id("spec"), weapon_id, version_id, _canonical_json(spec), spec_sha256, now),
            )
            conn.execute(
                """
                UPDATE weapons
                SET current_version_id = ?, updated_at = ?
                WHERE weapon_id = ?
                """,
                (version_id, now, weapon_id),
            )

            event_rows = [
                (
                    "request_guard",
                    "succeeded",
                    "Request accepted and persisted in SQLite.",
                    None,
                    0.1,
                    {},
                ),
                (
                    "weapon_spec_planner",
                    "succeeded",
                    "WeaponDesignSpec stored in immutable AssetStore.",
                    spec_file_id,
                    0.3,
                    {},
                ),
                (
                    "prompt_builder",
                    "succeeded",
                    "Concept prompt and ComfyUI workflow archived for reproducibility.",
                    prompt_file_id,
                    0.45,
                    {
                        "negative_prompt_asset_id": negative_prompt_file_id,
                        "workflow_asset_id": workflow_file_id,
                        "seed": spec["generation"].get("seed"),
                    },
                ),
                (
                    "image_submit",
                    "succeeded",
                    "Concept image stored with ComfyUI workflow provenance.",
                    concept_file_id,
                    0.62,
                    {
                        "provider": self.image_provider.provider_id,
                        "provider_task_id": concept.provider_task_id,
                        "workflow_asset_id": workflow_file_id,
                    },
                ),
                (
                    "image_quality_check",
                    "succeeded",
                    "Concept image quality report passed.",
                    quality_report_file_id,
                    0.72,
                    {
                        "target_asset_id": concept_file_id,
                        "target_sha256": concept_sha256,
                        "quality_report_asset_id": quality_report_file_id,
                    },
                ),
                (
                    "rough3d_submit",
                    "succeeded",
                    "Rough GLB variants and Unity metadata recorded.",
                    model_assets["raw_model_file_id"],
                    0.9,
                    {
                        "gated_by": quality_report_file_id,
                        "model_id": model_id,
                        "optimized_model_file_id": model_assets["optimized_model_file_id"],
                    },
                ),
                (
                    "finalize_job",
                    "succeeded",
                    "Job completed with SQLite-backed assets.",
                    quality_report_file_id,
                    1.0,
                    {},
                ),
            ]
            for index, (step, status, message, artifact_id, progress, metadata) in enumerate(
                event_rows, start=1
            ):
                self._insert_event(
                    conn,
                    event_id=f"evt_{job_id}_{index:04d}",
                    seq=index,
                    job_id=job_id,
                    weapon_id=weapon_id,
                    step=step,
                    status=status,
                    message=message,
                    artifact_asset_id=artifact_id,
                    progress=progress,
                    created_at=now,
                    metadata=metadata,
                )

            conn.commit()
            return self.get_job(job_id)


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _new_id(prefix: str) -> str:
    return f"{prefix}_{uuid.uuid4().hex}"
