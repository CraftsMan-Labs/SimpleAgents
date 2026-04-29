"""Run two text-only eval suites: friendly (real API) and RAG (mocked HTTP)."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import eval_suite
from simple_agents_py import Client, EvalSuiteRequest, run_eval_suite

# (label, yaml path, mock client for RAG offline run)
SUITES: tuple[tuple[str, Path, bool], ...] = (
    (
        "friendly-eval",
        eval_suite("friendly", "friendly-eval.yaml"),
        False,
    ),
    (
        "rag-eval",
        eval_suite("rag", "rag-eval.yaml"),
        True,
    ),
)

real_client = Client(
    require_env("WORKFLOW_PROVIDER"),
    api_base=require_env("WORKFLOW_API_BASE"),
    api_key=require_env("WORKFLOW_API_KEY"),
)
mock_rag_client = Client(
    "openai",
    api_base="https://example.invalid/v1",
    api_key="sk-mocked-rag-eval-000000000000",
)


def _client(use_mock: bool) -> Client:
    return mock_rag_client if use_mock else real_client

print(f"Text evals: starting ({len(SUITES)} suites)…", file=sys.stderr, flush=True)

ok = True
for label, suite_path, use_mock in SUITES:
    p = suite_path.resolve()
    client = _client(use_mock)
    scope = "mocked HTTP" if use_mock else "real API"
    print("", file=sys.stderr)
    print(f"[{label}] running… ({p.name}, {scope})", file=sys.stderr, flush=True)
    report = run_eval_suite(client, EvalSuiteRequest(suite_path=str(p)))
    passed = report.status == "passed"
    ok = ok and passed
    verdict = "PASSED" if passed else str(report.status).upper()
    print(f"[{label}] {verdict}", file=sys.stderr, flush=True)
    print(json.dumps(report.model_dump(mode="json"), indent=2))

print("", file=sys.stderr)
msg = (
    "Text evals: OVERALL PASSED." if ok else "Text evals: OVERALL FAILED."
)
print(msg, file=sys.stderr, flush=True)
raise SystemExit(0 if ok else 1)
