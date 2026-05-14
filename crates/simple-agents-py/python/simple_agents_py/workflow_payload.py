"""Convert WorkflowExecutionRequest to a JSON-safe mapping.

Note: ``Client.run_workflow`` and ``Client.stream_workflow`` now only accept
:class:`~simple_agents_py.workflow_request.WorkflowExecutionRequest` directly.
Plain dicts are rejected at the Rust layer.  This helper remains available for
serialising a request for logging, caching, or other purposes.
"""

from __future__ import annotations

from typing import Any


def workflow_execution_request_to_mapping(request: Any) -> dict[str, Any]:
    """Serialise a :class:`~simple_agents_py.workflow_request.WorkflowExecutionRequest` to a dict.

    Calls ``model_dump(mode="json", exclude_none=True)`` so nested values are
    JSON-safe.  Raises :exc:`TypeError` if *request* does not expose
    ``model_dump`` (i.e. is not a Pydantic model).
    """
    dump = getattr(request, "model_dump", None)
    if callable(dump):
        raw = dump(mode="json", exclude_none=True)
        if isinstance(raw, dict):
            return raw
        raise TypeError("model_dump(mode='json') must return a dict for WorkflowExecutionRequest")
    raise TypeError(
        "workflow_execution_request_to_mapping expects a WorkflowExecutionRequest "
        "(simple_agents_py.workflow_request.WorkflowExecutionRequest). "
        "Plain dicts are no longer accepted by Client.run_workflow / stream_workflow."
    )
