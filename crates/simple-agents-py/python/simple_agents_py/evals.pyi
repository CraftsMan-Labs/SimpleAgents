from __future__ import annotations

from pathlib import Path
from typing import Any

from .eval_request import EvalCase, EvalEvaluator, EvalReport, EvalResult, EvalSuiteRequest
from .simple_agents_py import Client

def run_eval_suite(
    client: Client,
    request: EvalSuiteRequest | dict[str, Any] | None = None,
    evaluator: EvalEvaluator | None = None,
    *,
    workflow_path: str | Path | None = None,
    dataset_path: str | Path | None = None,
    suite_id: str | None = None,
    execution: dict[str, Any] | None = None,
    workflow_options: dict[str, Any] | None = None,
) -> EvalReport: ...

def terminal_output_exact(case: EvalCase) -> EvalResult: ...
def terminal_node_exact(case: EvalCase) -> EvalResult: ...
def output_subset(case: EvalCase) -> EvalResult: ...
