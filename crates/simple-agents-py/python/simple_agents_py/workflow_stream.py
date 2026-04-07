"""Structured workflow stream hooks for :meth:`simple_agents_py.Client.stream`.

Events are the same dicts the Rust runner emits; shape matches
:class:`~simple_agents_py.models.WorkflowEvent`.

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

import json
import sys
from pathlib import Path
from typing import Any, Callable, Literal, Mapping, Protocol

from .workflow_payload import workflow_execution_request_to_mapping

StreamDisplayMode = Literal["off", "merged", "split"]

WorkflowStreamEvent = Mapping[str, Any]

# Explicit event_type values used when ``execution.split_stream_deltas`` is True vs False.
STREAM_EVENT_TYPES_SPLIT_DELTAS: frozenset[str] = frozenset(
    ("node_stream_thinking_delta", "node_stream_output_delta")
)
STREAM_EVENT_TYPES_MERGED_DELTA: frozenset[str] = frozenset(("node_stream_delta",))


def _node_stream_snapshot_log_line(event: WorkflowStreamEvent) -> str:
    """Build one stderr log line for a ``node_stream_snapshot`` event."""

    node = event.get("node_id") or event.get("step_id") or "?"
    parts: list[str] = [f"[snapshot] node={node!s}"]
    meta = event.get("metadata")
    if isinstance(meta, dict):
        if "confidence" in meta:
            parts.append(f"confidence={meta['confidence']!r}")
        if "is_complete" in meta:
            parts.append(f"is_complete={meta['is_complete']!r}")
    snap = event.get("snapshot")
    if snap is not None:
        try:
            preview = json.dumps(snap, ensure_ascii=False)
        except (TypeError, ValueError):
            preview = ""
        if len(preview) > 120:
            preview = preview[:117] + "..."
        if preview:
            parts.append(f"json={preview}")
    return " ".join(parts)


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
    # Canonical wire name emitted by the Rust runner (node_execution.rs).
    "resolved_llm_input": "on_node_llm_input_resolved",
    "node_completed": "on_node_completed",
    "node_stream_delta": "on_stream_delta",
    "node_stream_snapshot": "on_stream_snapshot",
    "node_stream_thinking_delta": "on_stream_thinking_delta",
    "node_stream_output_delta": "on_stream_output_delta",
    "workflow_completed": "on_workflow_completed",
}


def default_on_event(event: WorkflowStreamEvent) -> None:
    """Print streamed tokens to stdout; log structured snapshots to stderr.

    A ready-made ``on_event`` callback suitable for quick scripts and demos.
    Pass it directly wherever a callback is accepted::

        from simple_agents_py.workflow_stream import default_on_event
        client.stream_workflow(payload, on_event=default_on_event)

    Prints ``node_stream_delta``, ``node_stream_thinking_delta``, and
    ``node_stream_output_delta`` tokens inline on **stdout** (no newline between
    tokens). Emits a single line per ``node_stream_snapshot`` event on **stderr**
    (healing / structured JSON snapshot progress: node id, optional metadata, JSON
    preview). Silently ignores ``workflow_started`` and ``workflow_completed``;
    all other event types are also silently ignored by this handler.
    """
    event_type = event.get("event_type")
    if event_type == "node_stream_snapshot":
        print(_node_stream_snapshot_log_line(event), file=sys.stderr, flush=True)
        return
    delta = event.get("delta")
    if event_type in (
        "node_stream_delta",
        "node_stream_thinking_delta",
        "node_stream_output_delta",
    ) and isinstance(delta, str):
        print(delta, end="", flush=True)
        return


class WorkflowStreamHooks(Protocol):
    """Structural hook surface: implement any subset of ``on_*`` methods or only ``on_event``."""

    pass


def make_terminal_stream_printer(
    mode: Literal["merged", "split"],
) -> Callable[[WorkflowStreamEvent], None]:
    """Print LLM stream tokens to stdout (CLI-style), similar to ``run_with_chat_history --stream``.

    * **merged** — print ``node_stream_delta`` only.
    * **split** — print ``node_stream_thinking_delta`` and ``node_stream_output_delta`` only
      (set ``execution.split_stream_deltas=True`` on the request, or use ``stream_workflow`` with
      ``stream_display='split'`` which sets it when merging execution defaults).
    """

    state: dict[str, Any] = {"current_node": None, "line_open": False, "last_token_label": None}

    def on_event(event: WorkflowStreamEvent) -> None:
        event_type = event.get("event_type")
        node_id = event.get("node_id")
        step_id = event.get("step_id")
        delta = event.get("delta")
        token_kind = event.get("token_kind")
        is_terminal_node_token = event.get("is_terminal_node_token")

        if mode == "merged":
            is_stream = event_type == "node_stream_delta"
        else:
            is_stream = event_type in (
                "node_stream_thinking_delta",
                "node_stream_output_delta",
            )

        if is_stream and isinstance(delta, str):
            display_node_id: str | None = None
            if isinstance(node_id, str):
                display_node_id = node_id
            elif isinstance(step_id, str):
                display_node_id = step_id
            step_name = display_node_id or "?"
            current_node = state.get("current_node")
            line_open = bool(state.get("line_open", False))
            if current_node != display_node_id:
                if line_open:
                    print(file=sys.stdout)
                print(f"\nStep: {step_name}", file=sys.stdout)
                print("Streaming:", end=" ", flush=True, file=sys.stdout)
                state["current_node"] = display_node_id
                state["line_open"] = True
                state["last_token_label"] = None

            if mode == "split":
                token_label_parts: list[str] = []
                if isinstance(token_kind, str) and token_kind.strip():
                    token_label_parts.append(token_kind.strip())
                if is_terminal_node_token is True:
                    token_label_parts.append("terminal")
                token_label = (
                    f"[{' '.join(token_label_parts)}] " if token_label_parts else ""
                )
                last_token_label = state.get("last_token_label")
                if token_label and token_label != last_token_label:
                    if line_open:
                        print(file=sys.stdout)
                    print(f"{token_label}{step_name}: ", end="", flush=True, file=sys.stdout)
                    state["last_token_label"] = token_label
                    state["line_open"] = True
                print(delta, end="", flush=True, file=sys.stdout)
            else:
                print(delta, end="", flush=True, file=sys.stdout)
            return

        if event_type in {"workflow_started", "workflow_completed"}:
            return

    return on_event


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
    stream_display: StreamDisplayMode | None = None,
    merge_execution_defaults: bool = True,
) -> Any:
    """Stream a workflow with optional structured *hooks*, *on_event*, or terminal *stream_display*.

    *request* may be a mapping or a Pydantic
    :class:`~simple_agents_py.workflow_request.WorkflowExecutionRequest`.

    Pass **only one** of *hooks*, *on_event*, or a non-off *stream_display*. If all are
    omitted (or *stream_display* is ``\"off\"``), uses ``Client.stream_workflow`` without a
    live callback (events recorded on the result when applicable).

    *stream_display* ``\"merged\"`` prints merged ``node_stream_delta`` tokens;
    ``\"split\"`` prints thinking vs output deltas and forces ``split_stream_deltas`` when
    *merge_execution_defaults* is True.

    When *merge_execution_defaults* is True (default), if the payload has an
    ``execution`` key that is a mapping, it is merged with
    :func:`default_workflow_execution_bools` so every flag is present in the wire
    mapping (better logs and no “silent” defaults).
    """
    payload = workflow_execution_request_to_mapping(request)
    display = stream_display or "off"
    if display not in ("off", "merged", "split"):
        raise ValueError('stream_display must be "off", "merged", or "split"')

    if display != "off":
        if hooks is not None or on_event is not None:
            raise ValueError(
                "stream_display is incompatible with hooks and on_event; pick one"
            )

    if merge_execution_defaults:
        ex = payload.get("execution")
        if isinstance(ex, Mapping):
            payload["execution"] = merge_workflow_execution(ex)
        elif display == "split":
            payload["execution"] = merge_workflow_execution(None)

    if display == "split" and merge_execution_defaults:
        ex2 = payload.get("execution")
        if isinstance(ex2, dict):
            ex2["split_stream_deltas"] = True
            payload["execution"] = ex2

    if hooks is not None and on_event is not None:
        raise ValueError("pass only one of hooks or on_event")
    cb: Callable[[WorkflowStreamEvent], Any] | None
    if hooks is not None:
        cb = workflow_event_callback(hooks)
    elif on_event is not None:
        cb = on_event
    elif display == "merged":
        cb = make_terminal_stream_printer("merged")
    elif display == "split":
        cb = make_terminal_stream_printer("split")
    else:
        cb = None
    return client.stream_workflow(payload, on_event=cb)


def run_workflow_yaml_stream_typed(
    client: Any,
    request: Any,
    *,
    workflow_path: Path | str | None = None,
    hooks: Any | None = None,
    on_event: Callable[[WorkflowStreamEvent], Any] | None = None,
    stream_display: StreamDisplayMode | None = None,
    merge_execution_defaults: bool = True,
) -> Any:
    """Like :func:`stream_workflow` but optionally overrides ``workflow_path`` (e.g. pass a :class:`pathlib.Path`)."""
    payload = workflow_execution_request_to_mapping(request)
    if workflow_path is not None:
        payload["workflow_path"] = str(workflow_path)
    return stream_workflow(
        client,
        payload,
        hooks,
        on_event=on_event,
        stream_display=stream_display,
        merge_execution_defaults=merge_execution_defaults,
    )

