"""Shared filesystem layout for ``examples/python-test-simpleAgents/``."""

from pathlib import Path

PACKAGE_ROOT = Path(__file__).resolve().parent
# SimpleAgents monorepo root (parent of ``examples/``).
REPO_ROOT = PACKAGE_ROOT.parent.parent


def workflows(*parts: str) -> Path:
    return PACKAGE_ROOT.joinpath("workflows", *parts)


def eval_suite(*parts: str) -> Path:
    return PACKAGE_ROOT.joinpath("evals", *parts)


def asset(*parts: str) -> Path:
    return PACKAGE_ROOT.joinpath("assets", *parts)
