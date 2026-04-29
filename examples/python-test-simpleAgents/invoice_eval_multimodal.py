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


def multimodal_invoice_content_parts(image_b64: str) -> list[dict[str, Any]]:
    data_url = f"data:image/jpeg;base64,{image_b64}"
    return [
        {"type": "text", "text": INVOICE_USER_TEXT_FOR_EVAL},
        {"type": "image_url", "image_url": {"url": data_url}},
    ]


def eval_input_json(image_b64: str) -> dict[str, Any]:
    """Eval ``input`` blob (forwarded to the workflow as JSON)."""

    return {
        "messages": [
            {
                "role": "user",
                "content": multimodal_invoice_content_parts(image_b64),
            },
        ]
    }


def write_invoice_eval_generated_datasets(invoice_dir: Path, image_path: Path) -> None:
    """Write terminal + node JSONL under invoice_dir/generated/ from JPEG bytes."""


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


__all__ = [
    "INVOICE_USER_TEXT_FOR_EVAL",
    "eval_input_json",
    "multimodal_invoice_content_parts",
    "write_invoice_eval_generated_datasets",
]
