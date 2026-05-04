from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _ensure_parent(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)


def _append_jsonl(path: Path, record: dict[str, Any]) -> None:
    _ensure_parent(path)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record, ensure_ascii=False) + "\n")


def _default_store_file(filename: str) -> Path:
    return Path(__file__).resolve().parent.joinpath(filename)


def save_invoice_review(*, context: dict[str, Any], payload: dict[str, Any]) -> dict[str, Any]:
    store_path = _default_store_file("invoice-review-log.jsonl")
    record = {
        "event": "invoice_review_decision",
        "created_at": _utc_now(),
        "payload": payload,
        "workflow_trace": context.get("trace"),
    }
    _append_jsonl(store_path, record)
    return {
        "saved": True,
        "store_path": str(store_path),
        "review_status": payload.get("review_status"),
        "reviewer_decision": payload.get("reviewer_decision"),
    }


def save_feedback(*, context: dict[str, Any], payload: dict[str, Any]) -> dict[str, Any]:
    configured_path = payload.get("feedback_store_path")
    if isinstance(configured_path, str):
        store_path = Path(configured_path)
    else:
        store_path = _default_store_file("reviewer-feedback-log.jsonl")
    record = {
        "event": "invoice_feedback",
        "created_at": _utc_now(),
        "payload": payload,
        "workflow_trace": context.get("trace"),
    }
    _append_jsonl(store_path, record)
    return {
        "saved": True,
        "store_path": str(store_path),
        "feedback_preview": payload.get("reviewer_feedback"),
    }


def save_reviewed_form(*, context: dict[str, Any], payload: dict[str, Any]) -> dict[str, Any]:
    configured_path = payload.get("form_store_path")
    if isinstance(configured_path, str):
        store_path = Path(configured_path)
    else:
        store_path = _default_store_file("reviewed-form-log.jsonl")
    record = {
        "event": "invoice_form_review",
        "created_at": _utc_now(),
        "payload": payload,
        "workflow_trace": context.get("trace"),
    }
    _append_jsonl(store_path, record)
    return {
        "saved": True,
        "store_path": str(store_path),
        "reviewed_extraction": payload.get("reviewed_extraction"),
    }
