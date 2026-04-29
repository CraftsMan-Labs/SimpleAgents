from __future__ import annotations

import json

from simple_agents_py.evals import output_subset, run_eval_suite


class _FakeClient:
    def __init__(self, native_cases: list[dict]) -> None:
        self._native_cases = native_cases

    def run_eval_suite(self, request: dict) -> dict:
        assert "workflow_path" in request
        assert "dataset_path" in request
        return {
            "suite_id": request.get("suite_id", "fake"),
            "status": "passed",
            "summary": {},
            "cases": self._native_cases,
        }


def test_run_eval_suite_accepts_dataset_without_messages(
    tmp_path,
) -> None:
    dataset_path = tmp_path / "dataset.jsonl"
    dataset_path.write_text(
        json.dumps(
            {
                "id": "case-1",
                "input": {"payload": {"k": "v"}},
                "expected_output": {"terminal_node": "done"},
            }
        )
        + "\n",
        encoding="utf-8",
    )

    client = _FakeClient(
        [
            {
                "case_id": "case-1",
                "status": "passed",
                "workflow_output": {"terminal_node": "done"},
            }
        ]
    )

    report = run_eval_suite(
        client,  # type: ignore[arg-type]
        workflow_path="workflow.yaml",
        dataset_path=dataset_path,
        evaluator=output_subset,
    )

    assert report.status.value == "passed"
    assert report.summary.passed_cases == 1


def test_run_eval_suite_marks_native_execution_errors(tmp_path) -> None:
    dataset_path = tmp_path / "dataset.jsonl"
    dataset_path.write_text(
        json.dumps(
            {
                "id": "case-1",
                "input": {"payload": {"k": "v"}},
                "expected_output": {"terminal_node": "done"},
            }
        )
        + "\n",
        encoding="utf-8",
    )

    client = _FakeClient(
        [
            {
                "case_id": "case-1",
                "status": "error",
                "error": {"code": "workflow_run_failed", "message": "boom"},
            }
        ]
    )

    report = run_eval_suite(
        client,  # type: ignore[arg-type]
        workflow_path="workflow.yaml",
        dataset_path=dataset_path,
        evaluator=output_subset,
    )

    assert report.status.value == "error"
    assert report.summary.error_cases == 1
    assert report.cases[0].error is not None
    assert report.cases[0].error.message == "boom"
