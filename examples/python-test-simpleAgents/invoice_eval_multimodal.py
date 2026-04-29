"""Shared multimodal invoice content for Jaeger demos and invoice eval suites.

Uses Chat Completions vision style: ``text`` + ``image_url`` parts.
``write_invoice_eval_generated_datasets`` embeds the same structure as goldens.
"""


from __future__ import annotations

import base64
import json
from pathlib import Path
from typing import Any


# Same prose as legacy text datasets; with ``test-invoice.jpeg`` this is a true multimodal run.

INVOICE_USER_TEXT_FOR_EVAL = """Invoice image. Classify and route this per workflow.

Invoice issuer/vendor: Google
Invoice type: cloud services invoice
Amount due: $50,000
Please classify this as the invoice workflow would classify the attached image."""

_CASE_ID_TERMINAL = "google-invoice-terminal-node"
_CASE_ID_NODE = "google-invoice-node-paths"

_EXPECTED_TERMINAL: dict[str, Any] = {
    "terminal_node": "finalize_invoice_classification",
}

_EXPECTED_NODE: dict[str, Any] = {
    "terminal_node": "finalize_invoice_classification",
    "trace": [
        "detect_email_domain",
        "route_email_domain",
        "detect_finance_subtype",
        "route_finance_subtype",
        "extract_invoice_company_name",
        "lookup_invoice_stakeholder",
        "finalize_invoice_classification",
    ],
    "outputs": {
        "detect_email_domain": {"output": {"domain": "finance"}},
        "detect_finance_subtype": {"output": {"finance_subtype": "invoice"}},
        "extract_invoice_company_name": {"output": {"company_name": "Google"}},
        "lookup_invoice_stakeholder": {"output": "Sundar Pichai"},
        "finalize_invoice_classification": {
            "output": {
                "top_level_category": "finance",
                "subtype": "invoice",
                "label": "finance/invoice",
                "company_name": "Google",
                "stakeholder_name": "Sundar Pichai",
            }
        },
    },
}


# Reimbursement prose + same JPEG: model should route to ``finalize_finance_classification``.
# Paired with **invoice** ``expected_output`` in mismatch datasets so path comparison **fails**
# when the route is correct (harness / deliberate-wrong-golden check).

REIMBURSEMENT_USER_TEXT_FOR_EVAL = """Employee expense reimbursement request — not a vendor invoice.

I need reimbursement for $1,240.80 in airfare and meals from a client workshop last week, per company travel policy. I am submitting employee expense claims with receipts attached; this is not a supplier bill, payable notice, or vendor cloud-services invoice.

Classify and route this per the workflow, using both the text and the attached image."""

_CASE_ID_TERMINAL_MISMATCH = "finance-reimbursement-input-invoice-expected-terminal"
_CASE_ID_NODE_MISMATCH = "finance-reimbursement-input-invoice-expected-node-paths"


def multimodal_content_parts(user_text: str, image_b64: str) -> list[dict[str, Any]]:
    data_url = f"data:image/jpeg;base64,{image_b64}"
    return [
        {"type": "text", "text": user_text},
        {"type": "image_url", "image_url": {"url": data_url}},
    ]


def multimodal_invoice_content_parts(image_b64: str) -> list[dict[str, Any]]:
    return multimodal_content_parts(INVOICE_USER_TEXT_FOR_EVAL, image_b64)


def eval_input_json(image_b64: str) -> dict[str, Any]:
    """Eval ``input`` blob (forwarded to the workflow as JSON)."""

    return eval_input_json_with_text(image_b64, INVOICE_USER_TEXT_FOR_EVAL)


def eval_input_json_with_text(image_b64: str, user_text: str) -> dict[str, Any]:
    return {
        "messages": [
            {
                "role": "user",
                "content": multimodal_content_parts(user_text, image_b64),
            },
        ]
    }


def write_invoice_eval_generated_datasets(invoice_dir: Path, image_path: Path) -> None:
    """Write four JSONL files: two invoice goldens + two mismatch (wrong goldens) rows."""

    invoice_dir.mkdir(parents=True, exist_ok=True)
    generated = invoice_dir / "generated"
    generated.mkdir(parents=True, exist_ok=True)

    image_b64 = base64.b64encode(image_path.read_bytes()).decode("ascii")
    inp = eval_input_json(image_b64)

    terminal_path = generated / "invoice-image-terminal-eval.dataset.jsonl"
    terminal_path.write_text(
        json.dumps(
            {
                "id": _CASE_ID_TERMINAL,
                "input": inp,
                "expected_output": _EXPECTED_TERMINAL,
            },
        )
        + "\n",
        encoding="utf-8",
    )

    node_path = generated / "invoice-image-node-eval.dataset.jsonl"
    node_path.write_text(
        json.dumps(
            {
                "id": _CASE_ID_NODE,
                "input": inp,
                "expected_output": _EXPECTED_NODE,
            },
        )
        + "\n",
        encoding="utf-8",
    )

    inp_mismatch = eval_input_json_with_text(image_b64, REIMBURSEMENT_USER_TEXT_FOR_EVAL)

    mismatch_terminal_path = generated / "invoice-image-terminal-eval-mismatch.dataset.jsonl"
    mismatch_terminal_path.write_text(
        json.dumps(
            {
                "id": _CASE_ID_TERMINAL_MISMATCH,
                "input": inp_mismatch,
                "expected_output": _EXPECTED_TERMINAL,
            },
        )
        + "\n",
        encoding="utf-8",
    )

    mismatch_node_path = generated / "invoice-image-node-eval-mismatch.dataset.jsonl"
    mismatch_node_path.write_text(
        json.dumps(
            {
                "id": _CASE_ID_NODE_MISMATCH,
                "input": inp_mismatch,
                "expected_output": _EXPECTED_NODE,
            },
        )
        + "\n",
        encoding="utf-8",
    )


__all__ = [
    "INVOICE_USER_TEXT_FOR_EVAL",
    "REIMBURSEMENT_USER_TEXT_FOR_EVAL",
    "eval_input_json",
    "eval_input_json_with_text",
    "multimodal_content_parts",
    "multimodal_invoice_content_parts",
    "write_invoice_eval_generated_datasets",
]
