"""Structured workflow stream hooks for :meth:`simple_agents_py.Client.stream`.

Events are the same dicts the Rust runner emits; shape matches
:class:`simple_agents_py.WorkflowEvent` in ``simple_agents_py.pyi``.

**Split thinking vs. merged deltas.** Set ``execution.split_stream_deltas=True`` on a
:class:`~simple_agents_py.workflow_request.WorkflowExecutionRequest` (or the equivalent
dict key), or set env ``SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW`` (legacy). The two
are OR'd in the Rust runner.

**Requests:** Pass a :class:`~simple_agents_py.workflow_request.WorkflowExecutionRequest`
(needs ``pip install simple-agents-py[pydantic]``) or any ``mapping``; see
:func:`workflow_payload.workflow_execution_request_to_mapping`.
"""

from __future__ import annotations

from typing import Any, Callable, Mapping, Protocol

from .workflow_payload import workflow_execution_request_to_mapping

WorkflowStreamEvent = Mapping[str, Any]

EVENT_TYPE_TO_METHOD: dict[str, str] = {
    "workflow_started": "on_workflow_started",
    "node_started": "on_node_started",
    "node_llm_input_resolved": "on_node_llm_input_resolved",
    "node_completed": "on_node_completed",
    "node_stream_delta": "on_stream_delta",
    "node_stream_thinking_delta": "on_stream_thinking_delta",
    "node_stream_output_delta": "on_stream_output_delta",
    "workflow_completed": "on_workflow_completed",
}


class WorkflowStreamHooks(Protocol):
    """Structural hook surface: implement any subset of ``on_*`` methods or only ``on_event``."""

    pass


def workflow_event_callback(hooks: Any) -> Callable[[WorkflowStreamEvent], Any]:
    """Build an ``on_event`` handler that dispatches to named methods on *hooks*.

    For each incoming event, if ``event_type`` maps in `EVENT_TYPE_TO_METHOD`, the
    corresponding method (when callable) is invoked first, then ``on_event`` (when
    callable) is invoked, so narrow hooks and a catch-all can coexist.
    """

    def on_event(event: WorkflowStreamEvent) -> Any:
        et = event.get("event_type")
        if isinstance(et, str):
            method_name = EVENT_TYPE_TO_METHOD.get(et)
            if method_name is not None:
                fn = getattr(hooks, method_name, None)
                if callable(fn):
                    fn(event)
        catch_all = getattr(hooks, "on_event", None)
        if callable(catch_all):
            return catch_all(event)
        return None

    return on_event


def run_workflow_request(client: Any, request: Any) -> Any:
    """``client.run(workflow_execution_request_to_mapping(request))``."""
    return client.run(workflow_execution_request_to_mapping(request))


def run_workflow_request_async(client: Any, request: Any) -> Any:
    """``client.run_async(workflow_execution_request_to_mapping(request))``."""
    return client.run_async(workflow_execution_request_to_mapping(request))


def stream_workflow(
    client: Any,
    request: Any,
    hooks: Any | None = None,
    *,
    on_event: Callable[[WorkflowStreamEvent], Any] | None = None,
) -> Any:
    """Stream a workflow with optional structured *hooks* or a raw *on_event* callback.

    *request* may be a mapping or a Pydantic
    :class:`~simple_agents_py.workflow_request.WorkflowExecutionRequest`.

    Pass **only one** of *hooks* or *on_event*. If both are omitted, uses
    ``Client.stream`` without a live callback (events recorded on the result when
    applicable).
    """
    payload = workflow_execution_request_to_mapping(request)
    if hooks is not None and on_event is not None:
        raise ValueError("pass only one of hooks or on_event")
    cb: Callable[[WorkflowStreamEvent], Any] | None
    if hooks is not None:
        cb = workflow_event_callback(hooks)
    else:
        cb = on_event
    return client.stream(payload, on_event=cb)

