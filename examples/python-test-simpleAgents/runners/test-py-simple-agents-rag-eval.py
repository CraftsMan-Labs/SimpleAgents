import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_paths import eval_suite, workflows
from simple_agents_py import Client, EvalCase, EvalResult, run_eval_suite


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

client = Client(
    "openai",
    api_base="https://example.invalid/v1",
    api_key="sk-mocked-rag-eval-000000000000",
)

report = run_eval_suite(
    client,
    workflow_path=workflows("rag", "rag-eval-workflow.yaml"),
    dataset_path=eval_suite("rag", "rag-eval.dataset.jsonl"),
    evaluator=rag_chunks_match,
    execution={"node_llm_streaming": False},
    workflow_options={"telemetry": {"enabled": False}},
)

print(json.dumps(report.model_dump(mode="json"), indent=2))
raise SystemExit(0 if report.status == "passed" else 1)
