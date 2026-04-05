from __future__ import annotations

from typing import Any


def _workflow_email_text(context: dict[str, Any]) -> str:
    """Best-effort email or chat body from workflow execution context['input']."""
    raw_input = context.get("input")
    if not isinstance(raw_input, dict):
        return ""
    et = raw_input.get("email_text")
    if isinstance(et, str) and et.strip():
        return et
    messages = raw_input.get("messages")
    if isinstance(messages, list) and len(messages) > 0:
        last = messages[-1]
        if isinstance(last, dict):
            content = last.get("content")
            if isinstance(content, str):
                return content
    return ""


def get_rag_data(*, context: dict[str, Any], payload: dict[str, Any]) -> dict[str, str]:
    """Python custom_worker handler: load topic from payload; email/chat from context input."""
    topic_raw = payload.get("topic") if isinstance(payload, dict) else None
    topic = str(topic_raw if topic_raw is not None else "clarification")
    email_text = _workflow_email_text(context)

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
    *, context: dict[str, Any], payload: dict[str, Any]
) -> dict[str, str]:
    """Deterministic termination handler for interview workflows."""
    topic_raw = payload.get("topic") if isinstance(payload, dict) else None
    topic = str(topic_raw if topic_raw is not None else "terminated")
    email_text = _workflow_email_text(context)

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


def get_employee_record(
    *, context: dict[str, Any], payload: dict[str, Any]
) -> dict[str, str]:
    """Tool-style handler that resolves employee id and location by name."""
    _ = context

    requested_name = "Unknown Employee"
    if isinstance(payload, dict):
        raw_name = payload.get("employee_name")
        if isinstance(raw_name, str) and raw_name.strip():
            requested_name = raw_name.strip()

    directory = {
        "alex johnson": {"employee_id": "EMP-2041", "location": "Austin"},
        "priya sharma": {"employee_id": "EMP-3378", "location": "Bengaluru"},
        "marcus lee": {"employee_id": "EMP-1196", "location": "Singapore"},
        "sarah chen": {"employee_id": "EMP-4450", "location": "Toronto"},
    }

    profile = directory.get(
        requested_name.lower(),
        {"employee_id": "EMP-0000", "location": "Unassigned"},
    )

    return {
        "employee_name": requested_name,
        "employee_id": profile["employee_id"],
        "location": profile["location"],
    }


def get_seller_owner(
    *, context: dict[str, Any], payload: dict[str, Any]
) -> dict[str, str]:
    """Resolve a seller name to its owner name for finance workflows."""
    _ = context

    requested_seller = "unknown"
    if isinstance(payload, dict):
        raw_seller_name = payload.get("seller_name")
        if isinstance(raw_seller_name, str) and raw_seller_name.strip():
            requested_seller = raw_seller_name.strip()

    seller_owner_directory = {
        "google": "sundar pichai",
        "microsoft": "satya nadella",
        "apple": "tim cook",
        "amazon": "andy jassy",
    }

    owner_name = seller_owner_directory.get(requested_seller.lower(), "unknown")

    return {
        "seller_name": requested_seller,
        "owner_name": owner_name,
    }


def get_seller_name(
    *, context: dict[str, Any], payload: dict[str, Any]
) -> dict[str, str]:
    """Resolve an invoice company name to its stakeholder name."""
    _ = context

    requested_company_name = "unknown"
    if isinstance(payload, dict):
        raw_company_name = payload.get("company_name")
        if isinstance(raw_company_name, str) and raw_company_name.strip():
            requested_company_name = raw_company_name.strip()

    stakeholder_directory = {
        "google": "sundar pichai",
        "microsoft": "satya nadella",
        "apple": "tim cook",
        "amazon": "andy jassy",
    }

    stakeholder_name = stakeholder_directory.get(
        requested_company_name.lower(), "unknown"
    )

    return {
        "company_name": requested_company_name,
        "stakeholder_name": stakeholder_name,
    }


# YAML may use PascalCase (e.g. python-intern-fun-interview-system.yaml).
GetRagData = get_rag_data
