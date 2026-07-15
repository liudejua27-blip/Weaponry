"""Code-owned ForgeCAD Product Tool Registry for AgentActionLoop@1."""

from __future__ import annotations

import hashlib
from typing import Any

from .agent_action_loop import (
    AgentActionContext,
    AgentActionLoopError,
    ProductToolDefinition,
    ProductToolRegistry,
)
from .agent_rendering import render_agent_views
from .domain_inference import infer_domain
from .geometry_worker import build_blockout, compile_shape_program
from .mechanical_planner import MechanicalConceptPlan
from .profile_contracts import validate_profile_sketch
from .semantic_proportions import recipes_for_domain, style_token_map
from .shape_program import validate_shape_program
from .shape_program_runtime import UnsupportedRuntimeOperationError


_OBJECT = {"type": "object"}


def _closed(properties: dict[str, Any], required: list[str] | None = None) -> dict[str, Any]:
    return {
        "type": "object",
        "properties": properties,
        "required": required or [],
        "additionalProperties": False,
    }


def _text(max_length: int = 2000) -> dict[str, Any]:
    return {"type": "string", "minLength": 1, "maxLength": max_length}


def _json_result_schema(required: list[str]) -> dict[str, Any]:
    return {
        "type": "object",
        "required": required,
        "additionalProperties": True,
    }


def forgecad_product_tool_registry() -> ProductToolRegistry:
    plan_schema = MechanicalConceptPlan.model_json_schema()
    plan_defs = plan_schema.pop("$defs", {})
    plan_input_schema = _closed({"plan": plan_schema}, ["plan"])
    if plan_defs:
        plan_input_schema["$defs"] = plan_defs
    tools = (
        ProductToolDefinition(
            tool_id="forgecad.domain.inference.v1",
            name="infer_product_domain",
            description="Infer one supported ForgeCAD concept domain without choosing an ambiguous default.",
            input_schema=_closed({"brief": _text(8000)}, ["brief"]),
            output_schema=_json_result_schema(["schema_version", "status"]),
            approval_policy="read_only",
            handler=_infer_product_domain,
        ),
        ProductToolDefinition(
            tool_id="forgecad.reference.research.v1",
            name="research_approved_references",
            description="Search only bundled documentation or the reviewed local catalog; URLs and paths are forbidden.",
            input_schema=_closed(
                {
                    "query": _text(500),
                    "domain_pack_id": _text(120),
                    "source_scope": {"enum": ["bundled_docs", "approved_catalog"]},
                },
                ["query", "domain_pack_id", "source_scope"],
            ),
            output_schema=_json_result_schema(["source_scope", "matches", "network_call_made"]),
            approval_policy="read_only",
            handler=_research_approved_references,
        ),
        ProductToolDefinition(
            tool_id="forgecad.style.recipe_selection.v1",
            name="select_style_recipe",
            description="Select a versioned Style Token and semantic proportion recipe for the bound domain.",
            input_schema=_closed(
                {"domain_pack_id": _text(120), "intent": _text(500)},
                ["domain_pack_id", "intent"],
            ),
            output_schema=_json_result_schema(["style_token", "recipe", "fallback_used"]),
            approval_policy="read_only",
            handler=_select_style_recipe,
        ),
        ProductToolDefinition(
            tool_id="forgecad.profile.author.v1",
            name="author_profile_sketch",
            description="Accept and normalize one bounded ProfileSketch@1 candidate; no file or SVG execution.",
            input_schema=_closed({"profile_sketch": _OBJECT}, ["profile_sketch"]),
            output_schema=_json_result_schema(["profile_sketch", "validated"]),
            approval_policy="candidate_only",
            handler=_author_profile_sketch,
        ),
        ProductToolDefinition(
            tool_id="forgecad.profile.validate.v1",
            name="validate_profile_sketch",
            description="Validate one ProfileSketch@1 against the shipped Schema and semantic limits.",
            input_schema=_closed({"profile_sketch": _OBJECT}, ["profile_sketch"]),
            output_schema=_json_result_schema(["profile_sketch", "validated"]),
            approval_policy="read_only",
            handler=_author_profile_sketch,
        ),
        ProductToolDefinition(
            tool_id="forgecad.shape.author.v1",
            name="author_shape_program",
            description="Accept one candidate ShapeProgram@1 through the G819 runtime manifest boundary.",
            input_schema=_closed({"shape_program": _OBJECT}, ["shape_program"]),
            output_schema=_json_result_schema(["shape_program", "validated"]),
            approval_policy="candidate_only",
            handler=_author_shape_program,
        ),
        ProductToolDefinition(
            tool_id="forgecad.shape.validate.v1",
            name="validate_shape_program",
            description="Validate ShapeProgram@1; unknown or unimplemented G819 operations fail closed.",
            input_schema=_closed({"shape_program": _OBJECT}, ["shape_program"]),
            output_schema=_json_result_schema(["shape_program", "validated"]),
            approval_policy="read_only",
            handler=_author_shape_program,
        ),
        ProductToolDefinition(
            tool_id="forgecad.plan.complete_concept.v1",
            name="plan_complete_concept",
            description="Register one schema-valid complete exterior concept plan for this bound brief and domain.",
            input_schema=plan_input_schema,
            output_schema=_json_result_schema(["plan", "accepted"]),
            approval_policy="candidate_only",
            handler=_plan_complete_concept,
        ),
        ProductToolDefinition(
            tool_id="forgecad.geometry.build.v1",
            name="build_candidate_geometry",
            description="Build one bounded candidate with the shipped Geometry Worker; never persists an asset version.",
            input_schema=_closed(
                {
                    "direction_id": {"type": "string", "pattern": "^direction_[a-z0-9_\\-]+$"},
                    "variant_id": {"type": ["string", "null"], "maxLength": 120},
                    "presentation_profile": {"enum": ["quick_sketch", "showcase"]},
                },
                ["direction_id", "presentation_profile"],
            ),
            output_schema=_json_result_schema(
                ["direction_id", "topology_hash", "triangle_count", "bounds_mm", "candidate_only"]
            ),
            approval_policy="candidate_only",
            handler=_build_candidate_geometry,
        ),
        ProductToolDefinition(
            tool_id="forgecad.geometry.compile_readback.v1",
            name="compile_readback_candidate",
            description="Compile the candidate ShapeProgram and return facts read back from the actual GLB.",
            input_schema=_closed({}, []),
            output_schema=_json_result_schema(
                ["triangle_count", "bounds_mm", "mesh_count", "primitive_count", "material_count", "evidence_source"]
            ),
            approval_policy="candidate_only",
            handler=_compile_readback_candidate,
        ),
        ProductToolDefinition(
            tool_id="forgecad.render.concept.v1",
            name="render_candidate_views",
            description="Render four deterministic concept views from the compiled candidate GLB.",
            input_schema=_closed({}, []),
            output_schema=_json_result_schema(["view_ids", "view_sha256", "renderer_id"]),
            approval_policy="candidate_only",
            handler=_render_candidate_views,
        ),
        ProductToolDefinition(
            tool_id="forgecad.candidate.evaluate.v1",
            name="evaluate_candidate",
            description="Evaluate hard candidate evidence from actual readback and rendered views; no aesthetic truth claim.",
            input_schema=_closed({}, []),
            output_schema=_json_result_schema(["hard_gate_passed", "checks", "evidence_source"]),
            approval_policy="read_only",
            handler=_evaluate_candidate,
        ),
        ProductToolDefinition(
            tool_id="forgecad.preview.prepare.v1",
            name="prepare_candidate_preview",
            description="Prepare an ephemeral best-candidate preview descriptor; confirmation remains a separate user action.",
            input_schema=_closed({}, []),
            output_schema=_json_result_schema(
                ["preview_id", "topology_hash", "view_sha256", "requires_user_confirmation", "permanent_side_effects"]
            ),
            approval_policy="candidate_only",
            handler=_prepare_candidate_preview,
        ),
    )
    return ProductToolRegistry(tools)


def _infer_product_domain(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    result = infer_domain(arguments["brief"])
    payload = result.model_dump(mode="json")
    context.state["domain_inference"] = payload
    return payload


def _research_approved_references(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    del context
    # A004 deliberately has no network research executor.  R007 will add
    # user-authorized reference evidence; this tool can only report reviewed,
    # already-packaged sources.
    return {
        "source_scope": arguments["source_scope"],
        "matches": [],
        "network_call_made": False,
        "message": "当前没有与查询匹配的已审阅本地参考；未访问任意 URL。",
    }


def _select_style_recipe(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    recipes = list(recipes_for_domain(arguments["domain_pack_id"]))
    if not recipes:
        raise AgentActionLoopError(
            "STYLE_RECIPE_DOMAIN_UNAVAILABLE",
            "No reviewed semantic proportion recipe exists for this domain.",
            category="unsupported",
        )
    folded = arguments["intent"].casefold()
    selected = next(
        (recipe for recipe in recipes if any(phrase.casefold() in folded for phrase in recipe.intent_phrases)),
        recipes[-1],
    )
    token = style_token_map()[selected.style_token_id]
    payload = {
        "style_token": token.model_dump(mode="json"),
        "recipe": selected.model_dump(mode="json"),
        "fallback_used": not any(phrase.casefold() in folded for phrase in selected.intent_phrases),
    }
    context.state["style_recipe"] = payload
    return payload


def _author_profile_sketch(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    profile = validate_profile_sketch(arguments["profile_sketch"])
    context.state["profile_sketch"] = profile
    return {"profile_sketch": profile, "validated": True}


def _author_shape_program(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    try:
        program = validate_shape_program(arguments["shape_program"])
    except UnsupportedRuntimeOperationError as exc:
        raise AgentActionLoopError(
            exc.code,
            str(exc),
            category="unsupported",
        ) from exc
    context.state["shape_program"] = program
    return {"shape_program": program, "validated": True}


def _plan_complete_concept(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    plan = MechanicalConceptPlan.model_validate(arguments["plan"])
    expected_pack = context.state.get("domain_pack_id")
    expected_brief = context.state.get("brief")
    if expected_pack is not None and plan.domain_pack_id != expected_pack:
        raise AgentActionLoopError(
            "ACTION_LOOP_DOMAIN_CONFLICT",
            "Concept plan changed the domain bound to this Turn.",
            category="conflict",
        )
    normalized = plan.model_copy(
        update={
            "domain_pack_id": expected_pack or plan.domain_pack_id,
            "brief": expected_brief or plan.brief,
            "provider_id": str(context.state.get("provider_id") or plan.provider_id),
            "model": context.state.get("model"),
            "shape_program_ready": False,
        }
    )
    context.state["plan"] = normalized
    return {"plan": normalized.model_dump(mode="json"), "accepted": True}


def _require_state(context: AgentActionContext, key: str, code: str) -> Any:
    value = context.state.get(key)
    if value is None:
        raise AgentActionLoopError(code, f"Required Action Loop state {key!r} is missing.", category="conflict")
    return value


def _build_candidate_geometry(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    plan = _require_state(context, "plan", "ACTION_LOOP_PLAN_REQUIRED")
    context.check_cancelled()
    result = build_blockout(
        plan,
        arguments["direction_id"],
        variant_id=arguments.get("variant_id"),
        presentation_profile=arguments["presentation_profile"],
    )
    context.state["build"] = result
    context.state["shape_program"] = result.shape_program
    return {
        "direction_id": result.direction_id,
        "topology_hash": result.topology_hash,
        "triangle_count": result.triangle_count,
        "bounds_mm": result.bounds_mm,
        "candidate_only": True,
    }


def _compile_readback_candidate(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    del arguments
    program = _require_state(context, "shape_program", "ACTION_LOOP_SHAPE_PROGRAM_REQUIRED")
    compiled = compile_shape_program(
        program,
        cancel_check=lambda: bool(context.cancel_event and context.cancel_event.is_set()),
    )
    context.state["compiled"] = compiled
    readback = compiled.readback.model_dump(mode="json")
    return {
        **readback,
        "evidence_source": "geometry_compile_glb_readback",
    }


def _render_candidate_views(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    del arguments
    compiled = _require_state(context, "compiled", "ACTION_LOOP_COMPILE_READBACK_REQUIRED")
    context.check_cancelled()
    rendered = render_agent_views(compiled.glb_bytes, width=320, height=320)
    hashes = {name: hashlib.sha256(data).hexdigest() for name, data in rendered.views.items()}
    context.state["rendered"] = rendered
    context.state["view_sha256"] = hashes
    return {
        "view_ids": sorted(rendered.views),
        "view_sha256": hashes,
        "renderer_id": "forgecad-agent-software-raster@1",
    }


def _evaluate_candidate(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    del arguments
    compiled = _require_state(context, "compiled", "ACTION_LOOP_COMPILE_READBACK_REQUIRED")
    hashes = _require_state(context, "view_sha256", "ACTION_LOOP_RENDER_REQUIRED")
    readback = compiled.readback
    checks = {
        "has_triangles": readback.triangle_count > 0,
        "has_meshes": readback.mesh_count > 0,
        "four_views_read_back": set(hashes) == {"iso", "front", "side", "top"},
        "surface_provenance_present": bool(readback.surface_provenance),
    }
    payload = {
        "hard_gate_passed": all(checks.values()),
        "checks": checks,
        "evidence_source": "geometry_compile_glb_readback+concept_render_readback",
    }
    context.state["evaluation"] = payload
    return payload


def _prepare_candidate_preview(arguments: dict[str, Any], context: AgentActionContext) -> dict[str, Any]:
    del arguments
    evaluation = _require_state(context, "evaluation", "ACTION_LOOP_EVALUATION_REQUIRED")
    if not evaluation["hard_gate_passed"]:
        raise AgentActionLoopError(
            "CANDIDATE_HARD_GATE_FAILED",
            "Candidate failed a readback or render hard gate.",
            category="execution",
        )
    build = _require_state(context, "build", "ACTION_LOOP_BUILD_REQUIRED")
    hashes = _require_state(context, "view_sha256", "ACTION_LOOP_RENDER_REQUIRED")
    preview_id = "preview_" + hashlib.sha256(
        f"{context.parent_turn_id}:{build.topology_hash}".encode("utf-8")
    ).hexdigest()[:24]
    payload = {
        "preview_id": preview_id,
        "topology_hash": build.topology_hash,
        "view_sha256": hashes,
        "requires_user_confirmation": True,
        "permanent_side_effects": 0,
    }
    context.state["preview"] = payload
    return payload
