"""SimpleAgents Python bindings (native module + pure-Python helpers)."""

from . import models
from . import eval_request
from . import workflow_request
from .simple_agents_py import *  # noqa: F403
from .eval_request import *  # noqa: F403
from . import evals
from .evals import *  # noqa: F403
from .workflow_stream import default_on_event as default_on_event  # noqa: F401
from .models import *  # noqa: F403
from .workflow_request import *  # noqa: F403

__doc__ = simple_agents_py.__doc__  # noqa: F405
_native_all = getattr(simple_agents_py, "__all__", None)
if _native_all is not None:
    _exported = list(_native_all)
    _exported += [n for n in models.__all__ if n not in _exported]  # type: ignore[misc]  # noqa: F405
    _exported += [n for n in workflow_request.__all__ if n not in _exported]
    _exported += [n for n in eval_request.__all__ if n not in _exported]
    _exported += [n for n in evals.__all__ if n not in _exported]
    __all__ = _exported
