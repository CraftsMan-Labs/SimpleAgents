"""Eval helpers for output-shaped workflow datasets."""

from __future__ import annotations

from typing import Any

from .eval_request import EvalReport, EvalSuiteRequest
from .simple_agents_py import Client


def run_eval_suite(client: Client, request: EvalSuiteRequest | dict[str, Any]) -> EvalReport:
    payload = (
        request.to_client_payload()
        if isinstance(request, EvalSuiteRequest)
        else EvalSuiteRequest.model_validate(request).to_client_payload()
    )
    return EvalReport.model_validate(client.run_eval_suite(payload))


__all__ = ["run_eval_suite"]
