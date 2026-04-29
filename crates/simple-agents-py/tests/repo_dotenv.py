"""Load ``<monorepo-root>/.env`` for live tests (WORKFLOW_* / CUSTOM_*)."""

from __future__ import annotations

from pathlib import Path
from typing import MutableMapping


def repo_root() -> Path:
    """SimpleAgents repository root (parent of ``crates/``)."""
    return Path(__file__).resolve().parents[3]


def load_root_dotenv_into(
    environ: MutableMapping[str, str],
    *,
    override: bool = False,
) -> None:
    """Merge monorepo root ``.env`` into *environ*.

    *override*: if ``True``, keys present in the file replace existing values.
    """
    try:
        from dotenv import dotenv_values
    except ImportError:
        return
    path = repo_root() / ".env"
    if not path.is_file():
        return
    for key, value in dotenv_values(path).items():
        if value is None:
            continue
        if override or key not in environ or environ.get(key) == "":
            environ[key] = value
