"""Structured workflow stream hooks for :meth:`simple_agents_py.Client.stream`.

Events are the same dicts the Rust runner emits; shape matches
:class:`simple_agents_py.WorkflowEvent` in ``simple_agents_py.pyi``.

**Execution flags (explicit keys).** Under the top-level ``execution`` mapping (or
:class:`~simple_agents_py.workflow_request.WorkflowExecutionRequest`), these keys are
supported:

- ``model`` (optional ``str``): not a Rust execution flag; if set, merged into
  ``workflow_options.model`` as the default model for LLM nodes (same as top-level
  workflow run options).
- ``healing`` (bool, default ``False``): JSON healing path for structured LLM output
  (Rust ``YamlWorkflowExecutionFlags.healing``).
- ``workflow_streaming`` (bool, default ``False``): when ``False`` with a stream sink,
  token delta events are not forwarded (lifecycle events still are).
- ``node_llm_streaming`` (bool, default ``True``): when ``False``, LLM nodes never use
  provider streaming.
- ``split_stream_deltas`` (bool, default ``False``): when ``True``, emit
  ``node_stream_thinking_delta`` and ``node_stream_output_delta`` in addition to
  ``node_stream_delta``; when ``False``, rely on merged ``node_stream_delta`` only.

**Requests:** Pass a :class:`~simple_agents_py.workflow_request.WorkflowExecutionRequest`
(needs ``pip install simple-agents-py[pydantic]``) or any ``mapping``; see
:func:`workflow_payload.workflow_execution_request_to_mapping`.

**Explicit defaults (DX):** Use :func:`default_workflow_execution_bools` or
:func:`merge_workflow_execution` so ``execution`` always contains every flag value that
Rust applies (nothing is “invisible default”). :func:`stream_workflow` merges partial
``execution`` dicts by default (see *merge_execution_defaults*).
"""

from __future__ import annotations

from typing import Any, Callable, Mapping, Protocol

from .workflow_payload import workflow_execution_request_to_mapping

WorkflowStreamEvent = Mapping[str, Any]

# Explicit event_type values used when ``execution.split_stream_deltas`` is True vs False.
STREAM_EVENT_TYPES_SPLIT_DELTAS: frozenset[str] = frozenset(
    ("node_stream_thinking_delta", "node_stream_output_delta")
)
STREAM_EVENT_TYPES_MERGED_DELTA: frozenset[str] = frozenset(("node_stream_delta",))


def default_workflow_execution_bools() -> dict[str, bool]:
    """Rust ``YamlWorkflowExecutionFlags::default()`` as an explicit dict (bool fields only).

    Keys: ``healing``, ``workflow_streaming``, ``node_llm_streaming``, ``split_stream_deltas``.
    """

    return {
        "healing": False,
        "workflow_streaming": False,
        "node_llm_streaming": True,
        "split_stream_deltas": False,
    }


def merge_workflow_execution(execution: Mapping[str, Any] | None) -> dict[str, Any]:
    """Return *execution* merged on top of :func:`default_workflow_execution_bools`.

    Later keys win. Non-bool values for the four flag keys are preserved as-is so
    ``model`` can still be merged separately by the Rust layer if present under
    ``execution``.
    """

    base: dict[str, Any] = dict(default_workflow_execution_bools())
    if execution is None:
        return base
    merged = {**base, **dict(execution)}
    return merged


def split_stream_execution(*, enabled: bool = True) -> dict[str, Any]:
    """Explicit execution mapping with only ``split_stream_deltas`` overridden."""

    m = merge_workflow_execution(None)
    m["split_stream_deltas"] = enabled
    return m


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
    merge_execution_defaults: bool = True,
) -> Any:
    """Stream a workflow with optional structured *hooks* or a raw *on_event* callback.

    *request* may be a mapping or a Pydantic
    :class:`~simple_agents_py.workflow_request.WorkflowExecutionRequest`.

    Pass **only one** of *hooks* or *on_event*. If both are omitted, uses
    ``Client.stream`` without a live callback (events recorded on the result when
    applicable).

    When *merge_execution_defaults* is True (default), if the payload has an
    ``execution`` key that is a mapping, it is merged with
    :func:`default_workflow_execution_bools` so every flag is present in the wire
    mapping (better logs and no “silent” defaults).
    """
    payload = workflow_execution_request_to_mapping(request)
    if merge_execution_defaults:
        ex = payload.get("execution")
        if isinstance(ex, Mapping):
            payload["execution"] = merge_workflow_execution(ex)
    if hooks is not None and on_event is not None:
        raise ValueError("pass only one of hooks or on_event")
    cb: Callable[[WorkflowStreamEvent], Any] | None
    if hooks is not None:
        cb = workflow_event_callback(hooks)
    else:
        cb = on_event
    return client.stream(payload, on_event=cb)

