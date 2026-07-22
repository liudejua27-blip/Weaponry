from __future__ import annotations

import copy
import threading

import pytest

from forgecad_agent.application.agent_action_loop import (
    AgentActionContext,
    ProductToolDefinition,
    ProductToolRegistry,
)
from forgecad_agent.application.domain_packs import domain_pack_by_id
from forgecad_agent.application.k002_port_contracts import ProductToolExecutionRequest
from forgecad_agent.application.k002_port_security import (
    K002PortBoundaryError,
    canonical_json_sha256,
)
from forgecad_agent.application.k002_python_ports import ProductToolExecutorPort
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner
from forgecad_agent.application.product_tool_registry import forgecad_product_tool_registry


def _request(
    *,
    registry: ProductToolRegistry,
    tool_name: str,
    arguments: dict,
    execution_id: str = "executor_run_1",
    call_id: str = "call_1",
    cancellation_id: str = "cancel_1",
    cancellation_token: str = "cancel_token_1",
    input_schema: dict | None = None,
) -> dict:
    tool = registry.require(tool_name)
    schema = input_schema or tool.input_schema
    return {
        "schema_version": "ProductToolExecutionRequest@1",
        "execution_id": execution_id,
        "turn_id": "turn_1",
        "call_id": call_id,
        "tool_id": tool.tool_id,
        "tool_name": tool.name,
        "registry_schema_version": "ForgeCADProductToolRegistry@1",
        "idempotency_key": canonical_json_sha256(
            {
                "turn_id": "turn_1",
                "call_id": call_id,
                "tool_id": tool.tool_id,
                "arguments": arguments,
            }
        ),
        "validated_arguments": {
            "schema_id": f"{tool.tool_id}:input",
            "schema_sha256": canonical_json_sha256(schema),
            "value": arguments,
        },
        "approval_policy": tool.approval_policy,
        "cancellation_id": cancellation_id,
        "cancellation_token": cancellation_token,
    }


def _path_registry(*, forbidden_output: bool = False) -> ProductToolRegistry:
    def handler(arguments: dict, context: AgentActionContext) -> dict:
        context.state["path_seen"] = arguments["path"]
        if forbidden_output:
            return {"accepted": True, "provider_id": "must_not_escape"}
        return {"accepted": True}

    return ProductToolRegistry(
        (
            ProductToolDefinition(
                tool_id="forgecad.test.geometric_path.v1",
                name="accept_geometric_path",
                description="Test-only code-owned geometric path tool.",
                input_schema={
                    "type": "object",
                    "properties": {"path": {}},
                    "required": ["path"],
                    "additionalProperties": False,
                },
                output_schema={
                    "type": "object",
                    "properties": {"accepted": {"type": "boolean"}},
                    "required": ["accepted"],
                    "additionalProperties": True,
                },
                approval_policy="candidate_only",
                handler=handler,
            ),
        )
    )


def test_product_tool_executes_only_the_code_owned_schema_bound_tool() -> None:
    registry = forgecad_product_tool_registry()
    port = ProductToolExecutorPort(registry, environment={})
    arguments = {"brief": "设计一个非功能性未来机械道具外观"}
    request = _request(
        registry=registry,
        tool_name="infer_product_domain",
        arguments=arguments,
    )

    result = port.execute(request)

    assert result.status == "completed"
    assert result.validated_output is not None
    tool = registry.require("infer_product_domain")
    assert result.validated_output.schema_id == f"{tool.tool_id}:output"
    assert result.validated_output.schema_sha256 == canonical_json_sha256(tool.output_schema)
    assert result.validated_output.value["status"] in {
        "bound",
        "clarification_required",
        "unsupported",
    }
    assert result.permanent_side_effects == 0
    assert "tool_name" not in result.model_dump(mode="json")
    assert "idempotency_key" not in result.model_dump(mode="json")
    assert "late_result_discarded" not in result.model_dump(mode="json")

    replay = port.execute(request)
    assert replay == result


@pytest.mark.parametrize(
    "forbidden",
    [
        "thread_id",
        "session",
        "session_id",
        "history",
        "messages",
        "provider",
        "provider_id",
        "provider_key",
        "api_key",
        "base_url",
        "endpoint_url",
        "model",
        "reasoning",
        "reasoning_content",
        "database_path",
        "file_path",
        "filesystem_path",
        "object_path",
        "snapshot_write_token",
        "asset_write_token",
    ],
)
def test_product_tool_rejects_nested_authority_context(forbidden: str) -> None:
    registry = forgecad_product_tool_registry()
    port = ProductToolExecutorPort(registry, environment={})
    arguments = {"brief": "safe", forbidden: "forbidden"}
    request = _request(
        registry=registry,
        tool_name="infer_product_domain",
        arguments=arguments,
    )

    with pytest.raises(K002PortBoundaryError):
        port.execute(request)


def test_product_tool_rejects_thread_identity_at_the_top_level() -> None:
    registry = forgecad_product_tool_registry()
    port = ProductToolExecutorPort(registry, environment={})
    request = _request(
        registry=registry,
        tool_name="infer_product_domain",
        arguments={"brief": "safe"},
    )
    request["thread_id"] = "thread_forbidden"

    with pytest.raises(K002PortBoundaryError) as error:
        port.execute(request)

    assert error.value.code == "K002_PRODUCT_TOOL_REQUEST_INVALID"


@pytest.mark.parametrize(
    ("name", "value"),
    [
        ("DEEPSEEK_API_KEY", "not-visible-to-python"),
        ("DEEPSEEK_CREDENTIAL", "not-visible-to-python"),
        ("MY_PROVIDER_CREDENTIAL", "not-visible-to-python"),
        ("DASHSCOPE_API_KEY", "not-visible-to-python"),
        ("ANTHROPIC_API_KEY", "not-visible-to-python"),
        ("GITHUB_TOKEN", "not-visible-to-python"),
        # Keep the secret-like boundary case without checking a scanner-shaped
        # credential literal into the repository.
        ("UNKNOWN_VALUE", "sk-" + "0123456789abcdefghijklmnop"),
    ],
)
def test_product_tool_environment_cannot_contain_provider_configuration(
    name: str,
    value: str,
) -> None:
    registry = forgecad_product_tool_registry()
    port = ProductToolExecutorPort(
        registry,
        environment={name: value},
    )
    request = _request(
        registry=registry,
        tool_name="infer_product_domain",
        arguments={"brief": "safe"},
    )

    with pytest.raises(K002PortBoundaryError) as error:
        port.execute(request)

    assert error.value.code == "K002_PROVIDER_ENVIRONMENT_FORBIDDEN"


def test_product_tool_environment_allows_only_non_provider_runtime_context() -> None:
    registry = forgecad_product_tool_registry()
    port = ProductToolExecutorPort(
        registry,
        environment={
            "PATH": "/usr/bin:/bin",
            "HOME": "/tmp/forgecad-test-home",
            "LANG": "zh_CN.UTF-8",
            "FORGECAD_K002_INTERNAL_CAPABILITY_TOKEN": "capability-not-provider-key",
        },
    )
    request = _request(
        registry=registry,
        tool_name="infer_product_domain",
        arguments={"brief": "safe"},
    )

    assert port.execute(request).status == "completed"


def test_geometric_path_is_allowed_but_machine_and_network_locations_are_not() -> None:
    registry = _path_registry()
    port = ProductToolExecutorPort(registry, environment={})
    geometric_path = {"points": [[0, 0, 0], [1, 0, 0]], "closed": False}
    accepted = _request(
        registry=registry,
        tool_name="accept_geometric_path",
        arguments={"path": geometric_path},
    )
    assert port.execute(accepted).status == "completed"

    for index, location in enumerate(
        ["/tmp/asset.glb", "~/asset.glb", "file:///tmp/asset.glb", "https://x.test/a"]
    ):
        rejected = _request(
            registry=registry,
            tool_name="accept_geometric_path",
            arguments={"path": location},
            execution_id=f"executor_location_{index}",
            call_id=f"call_location_{index}",
            cancellation_id=f"cancel_location_{index}",
            cancellation_token=f"cancel_token_location_{index}",
        )
        with pytest.raises(K002PortBoundaryError) as error:
            port.execute(rejected)
        assert error.value.code == "K002_MACHINE_LOCATION_FORBIDDEN"


def test_complete_plan_scrubs_provider_authority_and_reconstructs_fixed_local_fields() -> None:
    registry = forgecad_product_tool_registry()
    port = ProductToolExecutorPort(registry, environment={})
    tool = registry.require("plan_complete_concept")
    plan = DeterministicMechanicalPlanner().plan_complete_concept(
        brief="用于游戏美术的非功能性概念道具",
        pack=domain_pack_by_id("pack_future_weapon_prop"),
        project_id=None,
        action_loop_enabled=False,
    ).model_dump(mode="json")
    plan.pop("provider_id")
    plan.pop("model")
    arguments = {"plan": plan}
    boundary_schema = port._boundary_input_schema(tool)
    request = _request(
        registry=registry,
        tool_name=tool.name,
        arguments=arguments,
        input_schema=boundary_schema,
    )

    result = port.execute(request)

    assert result.status == "completed"
    assert result.validated_output is not None
    output_plan = result.validated_output.value["plan"]
    assert "provider_id" not in output_plan
    assert "model" not in output_plan


def test_registry_identity_schema_policy_and_idempotency_are_revalidated() -> None:
    registry = forgecad_product_tool_registry()
    port = ProductToolExecutorPort(registry, environment={})
    base = _request(
        registry=registry,
        tool_name="infer_product_domain",
        arguments={"brief": "safe"},
    )
    mutations = [
        ("tool_id", "forgecad.unknown.v1"),
        ("tool_name", "unknown_tool"),
        ("approval_policy", "candidate_only"),
        ("idempotency_key", "f" * 64),
    ]
    for field, value in mutations:
        request = copy.deepcopy(base)
        request[field] = value
        if field == "tool_id":
            request["validated_arguments"]["schema_id"] = f"{value}:input"
        with pytest.raises(K002PortBoundaryError):
            port.execute(request)

    for field, value in [("schema_id", "wrong:input"), ("schema_sha256", "e" * 64)]:
        request = copy.deepcopy(base)
        request["validated_arguments"][field] = value
        with pytest.raises(K002PortBoundaryError):
            port.execute(request)


def test_forbidden_output_is_failed_and_ephemeral_state_is_not_committed() -> None:
    registry = _path_registry(forbidden_output=True)
    port = ProductToolExecutorPort(registry, environment={})
    request = _request(
        registry=registry,
        tool_name="accept_geometric_path",
        arguments={"path": {"points": []}},
    )

    result = port.execute(request)

    assert result.status == "failed"
    assert result.failure_category == "permission"
    assert result.validated_output is None
    assert port._runs["executor_run_1"].state == {}


class _BlockingProductToolPort(ProductToolExecutorPort):
    def __init__(self, registry: ProductToolRegistry) -> None:
        super().__init__(registry, environment={})
        self.started = threading.Event()
        self.release = threading.Event()

    def _invoke_tool(
        self,
        tool: ProductToolDefinition,
        arguments: dict,
        context: AgentActionContext,
    ) -> dict:
        context.state["must_not_commit"] = True
        self.started.set()
        assert self.release.wait(timeout=5)
        return super()._invoke_tool(tool, arguments, context)


def test_cancel_discards_late_result_and_never_commits_local_state() -> None:
    registry = forgecad_product_tool_registry()
    port = _BlockingProductToolPort(registry)
    request = _request(
        registry=registry,
        tool_name="infer_product_domain",
        arguments={"brief": "safe"},
    )
    result_holder: list = []
    worker = threading.Thread(target=lambda: result_holder.append(port.execute(request)))
    worker.start()
    assert port.started.wait(timeout=5)

    assert port.cancel(cancellation_id="cancel_1", cancellation_token="cancel_token_1") is True
    port.release.set()
    worker.join(timeout=5)

    assert not worker.is_alive()
    result = result_holder[0]
    assert result.status == "cancelled"
    assert result.failure_category == "cancelled"
    assert result.validated_output is None
    assert result.permanent_side_effects == 0
    assert port._runs["executor_run_1"].state == {}
    assert port.execute(request) == result


def test_cancel_before_start_is_bounded_and_token_bound() -> None:
    registry = forgecad_product_tool_registry()
    port = ProductToolExecutorPort(registry, environment={}, max_cancel_tombstones=2)
    assert port.cancel(cancellation_id="cancel_1", cancellation_token="cancel_token_1") is False
    with pytest.raises(K002PortBoundaryError):
        port.cancel(cancellation_id="cancel_1", cancellation_token="other_token")

    request = _request(
        registry=registry,
        tool_name="infer_product_domain",
        arguments={"brief": "safe"},
    )
    assert port.execute(request).status == "cancelled"


def test_request_model_matches_final_result_contract_fields() -> None:
    registry = forgecad_product_tool_registry()
    raw = _request(
        registry=registry,
        tool_name="infer_product_domain",
        arguments={"brief": "safe"},
    )
    parsed = ProductToolExecutionRequest.model_validate(raw)
    assert parsed.cancellation_token == "cancel_token_1"
    assert "thread_id" not in parsed.model_dump(mode="json")
