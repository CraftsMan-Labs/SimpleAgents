"""SimpleAgents Python bindings (native module + pure-Python helpers)."""

from . import models
from .simple_agents_py import *  # noqa: F403
from .workflow_stream import default_on_event as default_on_event  # noqa: F401
from .models import *  # noqa: F403

__doc__ = simple_agents_py.__doc__  # noqa: F405
_native_all = getattr(simple_agents_py, "__all__", None)
if _native_all is not None:
    __all__ = list(_native_all) + [n for n in models.__all__ if n not in _native_all]  # type: ignore[misc]  # noqa: F405
