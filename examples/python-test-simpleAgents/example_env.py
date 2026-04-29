from __future__ import annotations

import os
from pathlib import Path

_ROOT_DOTENV_LOADED = False


def load_monorepo_root_dotenv() -> None:
    """Populate ``os.environ`` from ``<SimpleAgents-repo-root>/.env`` if present."""
    global _ROOT_DOTENV_LOADED
    if _ROOT_DOTENV_LOADED:
        return
    _ROOT_DOTENV_LOADED = True
    try:
        from dotenv import load_dotenv
    except ImportError:
        return
    env_file = Path(__file__).resolve().parents[2] / ".env"
    if env_file.is_file():
        load_dotenv(env_file, override=False)


def require_env(name: str) -> str:
    load_monorepo_root_dotenv()
    value = os.environ.get(name)
    if not value:
        raise SystemExit(
            f"Required environment variable {name} is not set. "
            "Set it in your shell or in ``<SimpleAgents-repo-root>/.env`` (loaded automatically). "
            "You can also export variables from ``examples/python-test-simpleAgents/.env`` manually."
        )
    return value
