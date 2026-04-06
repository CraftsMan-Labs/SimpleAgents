"""SimpleAgents Python bindings (native module + pure-Python helpers)."""

from .simple_agents_py import *  # noqa: F403

__doc__ = simple_agents_py.__doc__  # noqa: F405
if hasattr(simple_agents_py, "__all__"):
    __all__ = simple_agents_py.__all__  # type: ignore[misc]  # noqa: F405

# region agent log
import json as _json, time as _time, inspect as _inspect, os as _os

def _dbg(msg, data=None, hyp=None):
    entry = {"runId": "post-fix", "timestamp": int(_time.time()*1000), "location": "__init__.py", "message": msg, "hypothesisId": hyp, "data": data or {}}
    _log = "/home/rishub/Desktop/projects/rishub/SimpleAgents/.cursor/debug.log"
    _os.makedirs(_os.path.dirname(_log), exist_ok=True)
    with open(_log, "a") as _f: _f.write(_json.dumps(entry) + "\n")

# Hypothesis B: log all exported symbols
_exported = [s for s in dir(simple_agents_py) if not s.startswith("_")]  # noqa: F405
_dbg("exported_symbols", {"symbols": _exported}, "B")

# Hypothesis B: check specific missing symbols
_missing = [s for s in ["heal_json","coerce_to_schema","ParseResult","CoercionResult","HealedJsonResult","StreamingParser","PyStructuredEvent"] if not hasattr(simple_agents_py, s)]  # noqa: F405
_dbg("missing_symbols", {"missing": _missing}, "B")

# Hypothesis A/D/E: log Client.__init__ signature
try:
    _sig = str(_inspect.signature(Client.__new__))  # noqa: F405
    _dbg("client_signature", {"signature": _sig}, "A")
except Exception as _e:
    _dbg("client_signature_error", {"error": str(_e)}, "A")

# Hypothesis A/E: try constructing with old API and capture error
try:
    _c = Client("openai", api_key="test-key-00000000000000", api_base="http://localhost:1/v1")  # noqa: F405
    _dbg("old_api_construct_success", {}, "A")
except TypeError as _te:
    _dbg("old_api_construct_type_error", {"error": str(_te)}, "A")
except Exception as _ex:
    _dbg("old_api_construct_other_error", {"error": str(_ex), "type": type(_ex).__name__}, "E")

# Hypothesis C: try constructing with "unknown" provider
try:
    _c2 = Client("unknown")  # noqa: F405
    _dbg("unknown_construct_success", {}, "C")
except Exception as _ex2:
    _dbg("unknown_construct_error", {"error": str(_ex2), "type": type(_ex2).__name__}, "C")

# Hypothesis D: log complete() signature
try:
    _csig = str(_inspect.signature(Client.complete))  # noqa: F405
    _dbg("client_complete_signature", {"signature": _csig}, "D")
except Exception as _e2:
    _dbg("client_complete_signature_error", {"error": str(_e2)}, "D")
# endregion
