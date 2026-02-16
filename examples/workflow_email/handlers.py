from __future__ import annotations

from typing import Any


def get_rag_data(
    topic: str, *, email_text: str, context: dict[str, Any]
) -> dict[str, str]:
    """Real Python handler for YAML custom_worker: GetRagData."""
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
