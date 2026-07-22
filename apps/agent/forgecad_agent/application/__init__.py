"""Lazy application-layer compatibility exports.

The production K003 sidecar imports only the restricted geometry executor.
Historically this package initializer imported the entire product service
graph, including Agent lifecycle and persistence-facing services, as a side
effect.  Lazy PEP 562 exports preserve existing callers and the explicit
legacy test oracle without granting those modules to the default facet.
"""

from __future__ import annotations

from importlib import import_module
from typing import Any


_EXPORT_MODULES = {
    "ConceptProjectError": ".concept_projects",
    "ConceptProjectIdempotencyConflict": ".concept_projects",
    "ConceptProjectService": ".concept_projects",
    "ConceptModuleError": ".concept_modules",
    "ConceptModuleIdempotencyConflict": ".concept_modules",
    "ConceptModuleService": ".concept_modules",
    "ConceptWorkbenchBootstrapService": ".concept_workbench_bootstrap",
    "ConceptChangeSetError": ".concept_change_sets",
    "ConceptChangeSetIdempotencyConflict": ".concept_change_sets",
    "ConceptChangeSetService": ".concept_change_sets",
    "ConceptQualityError": ".concept_quality",
    "ConceptQualityIdempotencyConflict": ".concept_quality",
    "ConceptQualityService": ".concept_quality",
    "ConceptBriefError": ".concept_briefs",
    "ConceptBriefIdempotencyConflict": ".concept_briefs",
    "ConceptBriefService": ".concept_briefs",
    "concept_planner_from_env": ".concept_planner",
    "ConceptJobError": ".concept_jobs",
    "ConceptJobService": ".concept_jobs",
    "ConceptExportError": ".concept_exports",
    "ConceptExportIdempotencyConflict": ".concept_exports",
    "ConceptExportService": ".concept_exports",
    "ChangeSetAuditExportError": ".concept_change_set_audits",
    "ChangeSetAuditExportIdempotencyConflict": ".concept_change_set_audits",
    "ConceptChangeSetAuditService": ".concept_change_set_audits",
    "AgentKernelError": ".agent_kernel",
    "AgentKernelIdempotencyConflict": ".agent_kernel",
    "AgentKernelService": ".agent_kernel",
    "AgentAssetEditingService": ".agent_asset_editing",
    "AgentAssetError": ".agent_asset_editing",
    "AgentAssetIdempotencyConflict": ".agent_asset_editing",
    "ActiveDesignApiError": ".active_design",
    "ActiveDesignIdempotencyConflict": ".active_design",
    "ActiveDesignService": ".active_design",
    "DomainPackManifest": ".domain_packs",
    "domain_pack_by_id": ".domain_packs",
    "list_domain_packs": ".domain_packs",
    "DomainInferenceResult": ".domain_inference",
    "DomainInferenceService": ".domain_inference",
    "DomainInferenceStatus": ".domain_inference",
    "infer_domain": ".domain_inference",
    "ConceptScopeDecision": ".concept_scope",
    "decide_concept_scope": ".concept_scope",
    "VisualIntentMapping": ".visual_intent",
    "build_visual_intent_mapping": ".visual_intent",
    "ArmDesignIntent": ".agent_models",
    "infer_arm_design_intent": ".arm_design_intent",
    "AssemblyDeltaProgram": ".assembly_delta",
    "ShapeProgramValidationError": ".shape_program",
    "validate_shape_program": ".shape_program",
    "ConceptDirection": ".mechanical_planner",
    "DeterministicMechanicalPlanner": ".mechanical_planner",
    "MechanicalConceptPlan": ".mechanical_planner",
    "MechanicalPlannerError": ".mechanical_planner",
    "mechanical_planner_from_env": ".mechanical_planner",
    "GeometryBuildResult": ".geometry_worker",
    "build_blockout": ".geometry_worker",
    "MaterialTextureError": ".material_textures",
    "MaterialTextureIdempotencyConflict": ".material_textures",
    "MaterialTextureService": ".material_textures",
}

__all__ = list(_EXPORT_MODULES)


def __getattr__(name: str) -> Any:
    module_name = _EXPORT_MODULES.get(name)
    if module_name is None:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    value = getattr(import_module(module_name, __name__), name)
    globals()[name] = value
    return value


def __dir__() -> list[str]:
    return sorted({*globals(), *__all__})
