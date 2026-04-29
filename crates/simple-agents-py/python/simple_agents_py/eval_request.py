"""Typed eval models for callback-based workflow evals."""

from __future__ import annotations

from enum import Enum
from typing import Annotated, Any, Callable

from pydantic import (
    BaseModel,
    BeforeValidator,
    ConfigDict,
    Field,
    model_validator,
)

from ._path_utils import coerce_path


def _coerce_workflow_path(value: Any) -> str:
    return coerce_path(value, field_name="workflow_path")


def _coerce_dataset_path(value: Any) -> str:
    return coerce_path(value, field_name="dataset_path")


EvalPath = Annotated[str, BeforeValidator(_coerce_dataset_path)]
EvalWorkflowPath = Annotated[str, BeforeValidator(_coerce_workflow_path)]


class EvalRunStatus(str, Enum):
    PASSED = "passed"
    FAILED = "failed"
    ERROR = "error"


class EvalSuiteRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    workflow_path: EvalWorkflowPath
    dataset_path: EvalPath
    suite_id: str | None = None
    execution: dict[str, Any] | None = None
    workflow_options: dict[str, Any] | None = None

    @model_validator(mode="after")
    def _paths_not_empty(self) -> "EvalSuiteRequest":
        if not self.workflow_path.strip():
            raise ValueError("workflow_path cannot be empty")
        if not self.dataset_path.strip():
            raise ValueError("dataset_path cannot be empty")
        return self

    def to_client_payload(self) -> dict[str, Any]:
        return self.model_dump(mode="json", exclude_none=True)


class EvalDatasetRecord(BaseModel):
    model_config = ConfigDict(extra="forbid")

    id: str
    input: dict[str, Any]
    expected_output: dict[str, Any]
    rubric: Any | None = None
    custom: Any | None = None
    metadata: Any | None = None

    @model_validator(mode="after")
    def _id_not_empty(self) -> "EvalDatasetRecord":
        if not self.id.strip():
            raise ValueError("record id cannot be empty")
        return self


class EvalCase(BaseModel):
    model_config = ConfigDict(extra="forbid")

    id: str
    input: dict[str, Any]
    expected_output: dict[str, Any]
    actual_output: dict[str, Any]
    record: EvalDatasetRecord


class EvalSummary(BaseModel):
    model_config = ConfigDict(extra="forbid")

    total_cases: int
    passed_cases: int
    failed_cases: int
    error_cases: int
    pass_rate: float


class EvalErrorInfo(BaseModel):
    model_config = ConfigDict(extra="forbid")

    code: str
    message: str


class EvalResult(BaseModel):
    model_config = ConfigDict(extra="forbid")

    id: str = "evaluator"
    status: EvalRunStatus
    passed: bool
    score: float | None = None
    expected: Any | None = None
    actual: Any | None = None
    reason: str | None = None
    metadata: Any | None = None

    @classmethod
    def passed_result(
        cls,
        *,
        id: str = "evaluator",
        score: float | None = None,
        reason: str | None = None,
        metadata: Any | None = None,
    ) -> "EvalResult":
        return cls(
            id=id,
            status=EvalRunStatus.PASSED,
            passed=True,
            score=score,
            reason=reason,
            metadata=metadata,
        )

    @classmethod
    def failed(
        cls,
        reason: str,
        *,
        id: str = "evaluator",
        score: float | None = None,
        expected: Any | None = None,
        actual: Any | None = None,
        metadata: Any | None = None,
    ) -> "EvalResult":
        return cls(
            id=id,
            status=EvalRunStatus.FAILED,
            passed=False,
            score=score,
            expected=expected,
            actual=actual,
            reason=reason,
            metadata=metadata,
        )

    @classmethod
    def errored(
        cls,
        reason: str,
        *,
        id: str = "evaluator",
        metadata: Any | None = None,
    ) -> "EvalResult":
        return cls(
            id=id,
            status=EvalRunStatus.ERROR,
            passed=False,
            reason=reason,
            metadata=metadata,
        )


class EvalCaseResult(BaseModel):
    model_config = ConfigDict(extra="forbid")

    case_id: str
    status: EvalRunStatus
    expected: Any | None = None
    actual: Any | None = None
    evaluations: list[EvalResult] = Field(default_factory=list)
    workflow_output: dict[str, Any] | None = None
    error: EvalErrorInfo | None = None


class EvalReport(BaseModel):
    model_config = ConfigDict(extra="forbid")

    suite_id: str
    status: EvalRunStatus
    summary: EvalSummary
    cases: list[EvalCaseResult]


EvalEvaluator = Callable[[EvalCase], EvalResult | dict[str, Any] | bool]


__all__ = [
    "EvalCase",
    "EvalCaseResult",
    "EvalDatasetRecord",
    "EvalErrorInfo",
    "EvalEvaluator",
    "EvalPath",
    "EvalReport",
    "EvalResult",
    "EvalRunStatus",
    "EvalSuiteRequest",
    "EvalSummary",
    "EvalWorkflowPath",
]
