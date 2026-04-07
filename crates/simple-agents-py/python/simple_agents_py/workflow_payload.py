"""Convert workflow execution requests to JSON-like mappings for Rust ``Client`` methods."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any


def workflow_execution_request_to_mapping(request: Any) -> dict[str, Any]:
    """Normalize *request* for :meth:`~simple_agents_py.Client.run_workflow` / ``stream_workflow``.

    Accepts:

    - Any object with ``model_dump`` (e.g. :class:`workflow_request.WorkflowExecutionRequest`)
    - A :class:`collections.abc.Mapping` (typically a ``dict``)

    Pydantic models should use ``model_dump(mode=\"json\", exclude_none=True)`` so nested
    values are JSON-safe.
    """
    dump = getattr(request, "model_dump", None)
    if callable(dump):
        return dump(mode="json", exclude_none=True)
    if isinstance(request, Mapping):
        return dict(request)
    raise TypeError(
        "workflow request must be a mapping or support model_dump(mode=..., exclude_none=...) "
        "(install optional `pydantic` and use simple_agents_py.workflow_request.WorkflowExecutionRequest)"
    )
