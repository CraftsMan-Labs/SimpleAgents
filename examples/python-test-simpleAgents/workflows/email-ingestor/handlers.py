"""Custom workers for the email ingestor workflow."""

from __future__ import annotations

from typing import Any

_AUDIT_MESSAGES: dict[str, str] = {
    "reimbursements": (
        "Reimbursement claim logged in financial audit system — "
        "pending policy review."
    ),
    "invoicing": (
        "Vendor invoice captured in financial audit system — "
        "routed to accounts payable queue."
    ),
    "tax": (
        "Tax-related correspondence recorded in financial audit system — "
        "flagged for compliance review."
    ),
}


def financial_audit(
    *, context: dict[str, Any], payload: dict[str, Any]
) -> dict[str, Any]:
    """Route classified finance emails into the (stub) financial audit subsystem."""
    subtype = str(payload.get("finance_subtype") or "unknown").strip().lower()
    print(f"[financial_audit] hit financial audit system (subtype={subtype})")

    audit_message = _AUDIT_MESSAGES.get(
        subtype,
        f"Finance email ({subtype}) recorded in financial audit system.",
    )
    return {
        "audit_status": "recorded",
        "audit_message": audit_message,
        "finance_subtype": subtype,
    }
