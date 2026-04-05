"""Structured workflow stream hooks for :meth:`simple_agents_py.Client.stream`.

Events are the same dicts the Rust runner emits; shape matches
:class:`simple_agents_py.WorkflowEvent` in ``simple_agents_py.pyi``.

**Split thinking vs. merged deltas.** Set ``execution["split_stream_deltas"] = True`` on the
request (preferred) or set env ``SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW`` to a truthy
value (legacy). The two are OR'd in the Rust runner.
"""

from __future__ import annotations

from typing import Any, Callable, Mapping, Protocol

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


def stream_workflow(client: Any, request: Mapping[str, Any], hooks: Any) -> Any:
    """Run ``client.stream(request, on_event=workflow_event_callback(hooks))``.

    **Return value** — same ``WorkflowRunOutput`` mapping as other workflow APIs, including:

    - ``workflow_id``, ``entry_node``, ``trace``, ``outputs``
    - ``terminal_node``, ``terminal_output``
    - ``step_timings``, ``llm_node_metrics``, ``llm_node_models``
    - ``total_elapsed_ms``, ``ttft_ms``
    - ``total_input_tokens``, ``total_output_tokens``, ``total_tokens``,
      ``total_reasoning_tokens``, ``tokens_per_second``
    - ``trace_id``, ``metadata``
    - ``events`` — when the runner records them (e.g. callback-free stream path)

    See ``WorkflowRunOutput`` in ``simple_agents_py.pyi`` for the canonical shape.
    """
    return client.stream(request, on_event=workflow_event_callback(hooks))
