from __future__ import annotations

from pathlib import Path
from typing import Any


def coerce_path(value: Any, *, field_name: str) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, Path):
        return str(value)
    fspath = getattr(value, "__fspath__", None)
    if callable(fspath):
        return str(fspath())
    raise TypeError(
        f"{field_name} must be str, pathlib.Path, or os.PathLike[str], "
        f"not {type(value).__name__}"
    )
