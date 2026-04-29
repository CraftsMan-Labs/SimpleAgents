"""Run two text-only eval suites: friendly (real API) and RAG (mocked HTTP)."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import eval_suite, workflows
from simple_agents_py import Client, EvalCase, EvalResult, output_subset, run_eval_suite

def rag_chunks_match(case: EvalCase) -> EvalResult:
    chunks = (
        case.actual_output.get("outputs", {})
        .get("retrieve_chunks", {})
        .get("output", [])
    )
    actual_ids = {
        chunk.get("source_id")
        for chunk in chunks
        if isinstance(chunk, dict) and chunk.get("source_id")
    }
    expected_ids = set(case.record.custom.get("expected_sources", []))
    matched = actual_ids.intersection(expected_ids)
    score = len(matched) / len(expected_ids) if expected_ids else 1.0
    if score >= 0.8:
        return EvalResult.passed_result(id="rag_chunks", score=score)
    return EvalResult.failed(
        f"{len(matched)}/{len(expected_ids)} expected sources matched",
        id="rag_chunks",
        score=score,
        expected=sorted(expected_ids),
        actual=sorted(actual_ids),
    )


# (label, workflow path, dataset path, mock client, evaluator)
SUITES = (
    (
        "friendly-eval",
        workflows("friendly", "friendly.yaml"),
        eval_suite("friendly", "friendly-eval.dataset.jsonl"),
        False,
        output_subset,
    ),
    (
        "rag-eval",
        workflows("rag", "rag-eval-workflow.yaml"),
        eval_suite("rag", "rag-eval.dataset.jsonl"),
        True,
        rag_chunks_match,
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
for label, workflow_path, dataset_path, use_mock, evaluator in SUITES:
    p = dataset_path.resolve()
    client = _client(use_mock)
    scope = "mocked HTTP" if use_mock else "real API"
    print("", file=sys.stderr)
    print(f"[{label}] running… ({p.name}, {scope})", file=sys.stderr, flush=True)
    report = run_eval_suite(
        client,
        workflow_path=workflow_path,
        dataset_path=p,
        evaluator=evaluator,
        execution={"node_llm_streaming": False},
        workflow_options={"telemetry": {"enabled": False}},
    )
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
