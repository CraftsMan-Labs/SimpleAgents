"""Pytest hooks: load monorepo root ``.env`` before collection."""

from __future__ import annotations

import os

from repo_dotenv import load_root_dotenv_into


def pytest_configure(config) -> None:
    load_root_dotenv_into(os.environ, override=False)
