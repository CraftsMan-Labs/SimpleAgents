"""Typed eval request/report models."""

from __future__ import annotations

from enum import Enum
from typing import Annotated, Any

from pydantic import BaseModel, BeforeValidator, ConfigDict, model_validator

from ._path_utils import coerce_path


def _coerce_path(value: Any) -> str:
    return coerce_path(value, field_name="suite_path")


EvalPath = Annotated[str, BeforeValidator(_coerce_path)]


class EvalRunStatus(str, Enum):
    PASSED = "passed"
    FAILED = "failed"
    ERROR = "error"


class EvalSuiteRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    suite_path: EvalPath

    @model_validator(mode="after")
    def _path_not_empty(self) -> "EvalSuiteRequest":
        if not self.suite_path.strip():
            raise ValueError("suite_path cannot be empty")
        return self

    def to_client_payload(self) -> dict[str, Any]:
        return self.model_dump(mode="json", exclude_none=True)


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


class EvalCaseResult(BaseModel):
    model_config = ConfigDict(extra="forbid")

    case_id: str
    status: EvalRunStatus
    first_failed_node: str | None = None
    first_failed_path: str | None = None
    expected: Any | None = None
    actual: Any | None = None
    workflow_output: dict[str, Any] | None = None
    error: EvalErrorInfo | None = None


class EvalReport(BaseModel):
    model_config = ConfigDict(extra="forbid")

    suite_id: str
    status: EvalRunStatus
    summary: EvalSummary
    cases: list[EvalCaseResult]


__all__ = [
    "EvalCaseResult",
    "EvalErrorInfo",
    "EvalPath",
    "EvalReport",
    "EvalRunStatus",
    "EvalSuiteRequest",
    "EvalSummary",
]
