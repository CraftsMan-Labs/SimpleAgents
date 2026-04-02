from __future__ import annotations

from pathlib import Path


def resolve_workflow_path(raw_path: str, *, caller_file: str) -> Path:
    candidate = Path(raw_path)
    if candidate.exists():
        return candidate

    local = Path(caller_file).resolve().parent / raw_path
    if local.exists():
        return local

    repo_relative = Path(caller_file).resolve().parents[2] / raw_path
    if repo_relative.exists():
        return repo_relative

    if raw_path.startswith("examples/"):
        trimmed = raw_path[len("examples/") :]
        trimmed_path = Path(caller_file).resolve().parents[1] / trimmed
        if trimmed_path.exists():
            return trimmed_path

    raise FileNotFoundError(f"workflow file not found: {raw_path}")
