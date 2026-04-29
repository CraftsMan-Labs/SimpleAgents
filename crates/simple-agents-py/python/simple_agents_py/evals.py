"""Callback-based workflow eval runner."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, cast

from .eval_request import (
    EvalCase,
    EvalCaseResult,
    EvalDatasetRecord,
    EvalErrorInfo,
    EvalEvaluator,
    EvalReport,
    EvalResult,
    EvalRunStatus,
    EvalSuiteRequest,
    EvalSummary,
)
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
) -> EvalReport:
    """Run workflow goldens and judge each case with user code.

    The dataset is JSONL. Each row contains `input` and a normalized
    `expected_output`; the evaluator receives both plus the new actual workflow
    output and returns pass/fail/score/reason.
    """

    req = _coerce_request(
        request,
        workflow_path=workflow_path,
        dataset_path=dataset_path,
        suite_id=suite_id,
        execution=execution,
        workflow_options=workflow_options,
    )
    if evaluator is None:
        raise ValueError("evaluator is required")

    records = _load_dataset(Path(req.dataset_path))
    native_cases = _native_cases_by_id(client, req)
    cases: list[EvalCaseResult] = []
    for record in records:
        cases.append(
            _run_case(
                record=record,
                evaluator=evaluator,
                native_case=native_cases.get(record.id),
            )
        )

    return _build_report(
        req.suite_id or Path(req.dataset_path).stem,
        cases,
    )


def terminal_output_exact(case: EvalCase) -> EvalResult:
    """Built-in evaluator for exact terminal output matching."""

    expected = case.expected_output.get("terminal_output")
    actual = case.actual_output.get("terminal_output")
    if expected == actual:
        return EvalResult.passed_result(id="terminal_output_exact")
    return EvalResult.failed(
        "terminal_output changed",
        id="terminal_output_exact",
        expected=expected,
        actual=actual,
    )


def terminal_node_exact(case: EvalCase) -> EvalResult:
    """Built-in evaluator for exact terminal node matching."""

    expected = case.expected_output.get("terminal_node")
    actual = case.actual_output.get("terminal_node")
    if expected == actual:
        return EvalResult.passed_result(id="terminal_node_exact")
    return EvalResult.failed(
        "terminal_node changed",
        id="terminal_node_exact",
        expected=expected,
        actual=actual,
    )


def output_subset(case: EvalCase) -> EvalResult:
    """Built-in evaluator: expected output must be an actual-output subset."""

    mismatch = _first_mismatch(case.expected_output, case.actual_output)
    if mismatch is None:
        return EvalResult.passed_result(id="output_subset")
    path, expected, actual = mismatch
    return EvalResult.failed(
        f"first mismatch at {path}",
        id="output_subset",
        expected=expected,
        actual=actual,
    )


def _coerce_request(
    request: EvalSuiteRequest | dict[str, Any] | None,
    *,
    workflow_path: str | Path | None,
    dataset_path: str | Path | None,
    suite_id: str | None,
    execution: dict[str, Any] | None,
    workflow_options: dict[str, Any] | None,
) -> EvalSuiteRequest:
    if request is not None:
        if any(
            value is not None
            for value in (
                workflow_path,
                dataset_path,
                suite_id,
                execution,
                workflow_options,
            )
        ):
            raise ValueError("pass either request or keyword config, not both")
        return (
            request
            if isinstance(request, EvalSuiteRequest)
            else EvalSuiteRequest.model_validate(request)
        )
    return EvalSuiteRequest.model_validate(
        {
            "workflow_path": workflow_path,
            "dataset_path": dataset_path,
            "suite_id": suite_id,
            "execution": execution,
            "workflow_options": workflow_options,
        }
    )


def _load_dataset(path: Path) -> list[EvalDatasetRecord]:
    records: list[EvalDatasetRecord] = []
    seen: set[str] = set()
    lines = path.read_text(encoding="utf-8").splitlines()
    for line_number, line in enumerate(lines, start=1):
        line = line.strip()
        if not line:
            continue
        try:
            record = EvalDatasetRecord.model_validate(json.loads(line))
        except Exception as exc:  # noqa: BLE001
            raise ValueError(
                f"failed to parse eval dataset {path} "
                f"line {line_number}: {exc}"
            ) from exc
        if record.id in seen:
            raise ValueError(f"duplicate eval record id {record.id!r}")
        seen.add(record.id)
        records.append(record)

    if not records:
        raise ValueError(
            f"eval dataset {path} must contain at least one record"
        )
    return records


def _run_case(
    record: EvalDatasetRecord,
    evaluator: EvalEvaluator,
    native_case: Any | None,
) -> EvalCaseResult:
    if native_case is None:
        return EvalCaseResult(
            case_id=record.id,
            status=EvalRunStatus.ERROR,
            evaluations=[
                EvalResult.errored("native eval runner did not return this case")
            ],
            error=EvalErrorInfo(
                code="eval_case_error",
                message="native eval runner did not return this case",
            ),
        )
    if not isinstance(native_case, dict):
        return EvalCaseResult(
            case_id=record.id,
            status=EvalRunStatus.ERROR,
            evaluations=[
                EvalResult.errored(
                    "native eval runner returned malformed case payload"
                )
            ],
            error=EvalErrorInfo(
                code="eval_case_error",
                message="native eval runner returned malformed case payload",
            ),
        )
    native_error = native_case.get("error")
    workflow_output = native_case.get("workflow_output")
    if native_error is not None or not isinstance(workflow_output, dict):
        message = (
            native_error.get("message")
            if isinstance(native_error, dict)
            else "native eval workflow execution failed"
        )
        text = str(message or "native eval workflow execution failed")
        return EvalCaseResult(
            case_id=record.id,
            status=EvalRunStatus.ERROR,
            evaluations=[EvalResult.errored(text)],
            error=EvalErrorInfo(code="eval_case_error", message=text),
        )
    try:
        actual_output = cast(dict[str, Any], workflow_output)
        case = EvalCase(
            id=record.id,
            input=record.input,
            expected_output=record.expected_output,
            actual_output=actual_output,
            record=record,
        )
        evaluation = _coerce_eval_result(evaluator(case))
        return EvalCaseResult(
            case_id=record.id,
            status=evaluation.status,
            expected=evaluation.expected,
            actual=evaluation.actual,
            evaluations=[evaluation],
            workflow_output=actual_output,
            error=(
                EvalErrorInfo(
                    code="evaluator_error",
                    message=evaluation.reason or "evaluator error",
                )
                if evaluation.status == EvalRunStatus.ERROR
                else None
            ),
        )
    except Exception as exc:  # noqa: BLE001
        evaluation = EvalResult.errored(str(exc))
        return EvalCaseResult(
            case_id=record.id,
            status=EvalRunStatus.ERROR,
            evaluations=[evaluation],
            error=EvalErrorInfo(code="eval_case_error", message=str(exc)),
        )


def _native_cases_by_id(
    client: Client,
    request: EvalSuiteRequest,
) -> dict[str, Any]:
    native_payload = request.to_client_payload()
    native_report = cast(dict[str, Any], client.run_eval_suite(native_payload))
    native_cases = native_report.get("cases")
    if not isinstance(native_cases, list):
        raise ValueError(
            "native run_eval_suite returned malformed report.cases"
        )
    case_map: dict[str, Any] = {}
    for native_case in native_cases:
        if not isinstance(native_case, dict):
            continue
        case_id = native_case.get("case_id")
        if isinstance(case_id, str):
            case_map[case_id] = native_case
    return case_map


def _coerce_eval_result(
    value: EvalResult | dict[str, Any] | bool,
) -> EvalResult:
    if isinstance(value, EvalResult):
        return value
    if isinstance(value, bool):
        return (
            EvalResult.passed_result()
            if value
            else EvalResult.failed("evaluator returned false")
        )
    return EvalResult.model_validate(value)


def _build_report(suite_id: str, cases: list[EvalCaseResult]) -> EvalReport:
    total = len(cases)
    passed = sum(1 for case in cases if case.status == EvalRunStatus.PASSED)
    failed = sum(1 for case in cases if case.status == EvalRunStatus.FAILED)
    errors = sum(1 for case in cases if case.status == EvalRunStatus.ERROR)
    status = (
        EvalRunStatus.ERROR
        if errors
        else EvalRunStatus.FAILED
        if failed
        else EvalRunStatus.PASSED
    )
    return EvalReport(
        suite_id=suite_id,
        status=status,
        summary=EvalSummary(
            total_cases=total,
            passed_cases=passed,
            failed_cases=failed,
            error_cases=errors,
            pass_rate=passed / total if total else 0.0,
        ),
        cases=cases,
    )


def _first_mismatch(
    expected: Any,
    actual: Any,
    path: str = "$",
) -> tuple[str, Any, Any] | None:
    if expected == actual:
        return None
    if isinstance(expected, dict) and isinstance(actual, dict):
        for key, expected_value in expected.items():
            if key not in actual:
                return (f"{path}.{key}", expected_value, None)
            mismatch = _first_mismatch(
                expected_value,
                actual[key],
                f"{path}.{key}",
            )
            if mismatch is not None:
                return mismatch
        return None
    if isinstance(expected, list) and isinstance(actual, list):
        for index, expected_value in enumerate(expected):
            if index >= len(actual):
                return (f"{path}[{index}]", expected_value, None)
            mismatch = _first_mismatch(
                expected_value,
                actual[index],
                f"{path}[{index}]",
            )
            if mismatch is not None:
                return mismatch
        return None
    return (path, expected, actual)


__all__ = [
    "output_subset",
    "run_eval_suite",
    "terminal_node_exact",
    "terminal_output_exact",
]
