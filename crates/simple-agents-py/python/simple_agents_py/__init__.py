"""SimpleAgents Python bindings (native module + pure-Python helpers)."""

from .simple_agents_py import *  # noqa: F403
from .workflow_stream import default_on_event as default_on_event  # noqa: F401

__doc__ = simple_agents_py.__doc__  # noqa: F405
if hasattr(simple_agents_py, "__all__"):
    __all__ = simple_agents_py.__all__  # type: ignore[misc]  # noqa: F405
