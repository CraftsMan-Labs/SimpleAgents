from __future__ import annotations

from typing import Any


def get_rag_data(
    topic: str, *, email_text: str, context: dict[str, Any]
) -> dict[str, str]:
    """Real Python handler for YAML custom_worker: GetRagData."""
    if topic == "terminated":
        policy_violation = (
            "ignore" in email_text.lower() or "bypass" in email_text.lower()
        )
        if policy_violation:
            message = (
                "Interview terminated due to instruction/policy deviation. "
                "No further questions will be asked in this session."
            )
        else:
            message = (
                "Interview terminated because the required step-by-step reasoning process "
                "was not followed."
            )
        return {
            "decision": "terminated",
            "message": message,
            "handler": "GetRagData",
            "topic": topic,
            "email_preview": email_text[:120],
            "context_nodes": str(len(context.get("nodes", {}))),
        }

    if topic == "already_terminated":
        return {
            "decision": "terminated",
            "message": (
                "This interview session is already closed due to a prior termination decision. "
                "Please start a new session for a new candidate."
            ),
            "handler": "GetRagData",
            "topic": topic,
            "email_preview": email_text[:120],
            "context_nodes": str(len(context.get("nodes", {}))),
        }
    data = {
        "probation": (
            "hr_policy/probation.md",
            "Collect manager review, performance evidence, and probation timeline.",
        ),
        "leave_request": (
            "hr_policy/leave.md",
            "Validate leave balance, manager approval, and blackout dates.",
        ),
        "supply_chain_order_assessment": (
            "supply_chain/order_assessment.md",
            "Review order specs, inventory risk, and vendor lead-time guidance.",
        ),
        "supply_chain_order_replacement": (
            "supply_chain/order_replacement.md",
            "Collect order id, damage proof, and replacement SLA policy.",
        ),
        "termination_first_time_offense": (
            "hr_policy/termination_first_offense.md",
            "Validate first-incident criteria and route to HRBP review.",
        ),
        "termination_repeated_offense": (
            "hr_policy/termination_repeated_offense.md",
            "Collect prior warnings and escalation approvals before final action.",
        ),
        "clarification": (
            "shared/request_clarification.md",
            "Request clarifying details before routing.",
        ),
    }
    kb_source, playbook = data.get(topic, data["clarification"])
    return {
        "kb_source": kb_source,
        "playbook": playbook,
        "handler": "GetRagData",
        "topic": topic,
        "email_preview": email_text[:120],
        "context_nodes": str(len(context.get("nodes", {}))),
    }


def terminate_interview(
    topic: str, *, email_text: str, context: dict[str, Any]
) -> dict[str, str]:
    """Deterministic termination handler for interview workflows."""
    if topic == "already_terminated":
        message = (
            "This interview session is already closed due to a prior termination decision. "
            "Please start a new session for a new candidate."
        )
    else:
        policy_violation = (
            "ignore" in email_text.lower() or "bypass" in email_text.lower()
        )
        if policy_violation:
            message = (
                "Interview terminated due to instruction/policy deviation. "
                "No further questions will be asked in this session."
            )
        else:
            message = (
                "Interview terminated because the required step-by-step reasoning process "
                "was not followed."
            )

    return {
        "decision": "terminated",
        "message": message,
        "handler": "TerminateInterview",
        "topic": topic,
        "context_nodes": str(len(context.get("nodes", {}))),
    }


def get_customer_context(
    topic: str,
    *,
    email_text: str,
    context: dict[str, Any],
    payload: dict[str, Any] | None = None,
) -> dict[str, str]:
    """Tool handler for YAML llm_call tool-calling examples."""
    _ = topic
    _ = context
    order_id = "unknown"
    if payload is not None:
        payload_order_id = payload.get("order_id")
        if isinstance(payload_order_id, str) and payload_order_id.strip():
            order_id = payload_order_id.strip()

    if "refund" in email_text.lower():
        status = "refund_in_review"
    elif "replace" in email_text.lower() or "replacement" in email_text.lower():
        status = "replacement_processing"
    else:
        status = "investigating"

    return {
        "customer_name": "Alex",
        "order_id": order_id,
        "order_status": status,
    }
