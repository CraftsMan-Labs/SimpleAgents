from __future__ import annotations

from dataclasses import asdict, dataclass
from enum import Enum
from typing import Optional
import json


class TopLevelCategory(str, Enum):
    PROBATION = "probation"
    TERMINATION = "termination"
    LEAVE_REQUEST = "leave_request"
    SUPPLY_CHAIN_REQUEST = "supply_chain_request"
    CLARIFICATION = "clarification"


class SupplyChainSubtype(str, Enum):
    ORDER_ASSESSMENT = "order_assessment"
    ORDER_REPLACEMENT = "order_replacement"
    CLARIFICATION = "clarification"


class TerminationSubtype(str, Enum):
    FIRST_TIME_OFFENSE = "first_time_offense"
    REPEATED_OFFENSE = "repeated_offense"
    CLARIFICATION = "clarification"


@dataclass
class ClassificationResult:
    top_level: TopLevelCategory
    supply_chain_subtype: Optional[SupplyChainSubtype] = None
    termination_subtype: Optional[TerminationSubtype] = None
    confidence: float = 0.0
    reason: str = ""


def normalize(text: str) -> str:
    return " ".join(text.lower().strip().split())


def contains_any(text: str, keywords: list[str]) -> bool:
    return any(keyword in text for keyword in keywords)


def classify_top_level(email_text: str) -> ClassificationResult:
    text = normalize(email_text)

    if contains_any(text, ["probation", "probation period", "extend probation"]):
        return ClassificationResult(
            top_level=TopLevelCategory.PROBATION,
            confidence=0.93,
            reason="Detected probation-related keywords.",
        )

    if contains_any(text, ["termination", "terminate", "dismiss", "fired"]):
        subtype = classify_termination(text)
        return ClassificationResult(
            top_level=TopLevelCategory.TERMINATION,
            termination_subtype=subtype,
            confidence=0.95 if subtype != TerminationSubtype.CLARIFICATION else 0.72,
            reason="Detected termination-related keywords.",
        )

    if contains_any(
        text, ["leave request", "vacation", "sick leave", "pto", "time off"]
    ):
        return ClassificationResult(
            top_level=TopLevelCategory.LEAVE_REQUEST,
            confidence=0.94,
            reason="Detected leave-request keywords.",
        )

    if contains_any(text, ["supply", "shipment", "vendor", "purchase order", "order"]):
        subtype = classify_supply_chain(text)
        return ClassificationResult(
            top_level=TopLevelCategory.SUPPLY_CHAIN_REQUEST,
            supply_chain_subtype=subtype,
            confidence=0.91 if subtype != SupplyChainSubtype.CLARIFICATION else 0.7,
            reason="Detected supply-chain keywords.",
        )

    return ClassificationResult(
        top_level=TopLevelCategory.CLARIFICATION,
        confidence=0.55,
        reason="No strong category match. Needs more information.",
    )


def classify_supply_chain(text: str) -> SupplyChainSubtype:
    replacement_signals = ["replacement", "damaged", "wrong item", "return", "replace"]
    assessment_signals = ["assess", "assessment", "review order", "evaluate", "quote"]

    has_replacement = contains_any(text, replacement_signals)
    has_assessment = contains_any(text, assessment_signals)

    if has_replacement and not has_assessment:
        return SupplyChainSubtype.ORDER_REPLACEMENT
    if has_assessment and not has_replacement:
        return SupplyChainSubtype.ORDER_ASSESSMENT
    return SupplyChainSubtype.CLARIFICATION


def classify_termination(text: str) -> TerminationSubtype:
    repeated_signals = [
        "repeated offense",
        "second warning",
        "third warning",
        "again",
        "pattern",
        "multiple incidents",
    ]
    first_time_signals = [
        "first offense",
        "first incident",
        "initial warning",
        "first time",
    ]

    has_repeated = contains_any(text, repeated_signals)
    has_first_time = contains_any(text, first_time_signals)

    if has_repeated and not has_first_time:
        return TerminationSubtype.REPEATED_OFFENSE
    if has_first_time and not has_repeated:
        return TerminationSubtype.FIRST_TIME_OFFENSE
    return TerminationSubtype.CLARIFICATION


def get_rag_data(classification: ClassificationResult) -> dict:
    """Mock RAG lookup: returns policy snippets/workflow guidance."""
    if classification.top_level == TopLevelCategory.PROBATION:
        return {
            "kb_source": "hr_policy/probation.md",
            "playbook": "Collect manager review, performance evidence, and probation timeline.",
        }

    if classification.top_level == TopLevelCategory.LEAVE_REQUEST:
        return {
            "kb_source": "hr_policy/leave.md",
            "playbook": "Validate leave balance, manager approval, and blackout dates.",
        }

    if classification.top_level == TopLevelCategory.SUPPLY_CHAIN_REQUEST:
        if classification.supply_chain_subtype == SupplyChainSubtype.ORDER_REPLACEMENT:
            return {
                "kb_source": "supply_chain/order_replacement.md",
                "playbook": "Collect order id, damage proof, and replacement SLA policy.",
            }
        if classification.supply_chain_subtype == SupplyChainSubtype.ORDER_ASSESSMENT:
            return {
                "kb_source": "supply_chain/order_assessment.md",
                "playbook": "Review order specs, inventory risk, and vendor lead-time guidance.",
            }
        return {
            "kb_source": "supply_chain/intake_checklist.md",
            "playbook": "Ask whether this is assessment or replacement and gather order metadata.",
        }

    if classification.top_level == TopLevelCategory.TERMINATION:
        if classification.termination_subtype == TerminationSubtype.FIRST_TIME_OFFENSE:
            return {
                "kb_source": "hr_policy/termination_first_offense.md",
                "playbook": "Validate first-incident criteria and route to HRBP review.",
            }
        if classification.termination_subtype == TerminationSubtype.REPEATED_OFFENSE:
            return {
                "kb_source": "hr_policy/termination_repeated_offense.md",
                "playbook": "Collect prior warnings and escalation approvals before final action.",
            }
        return {
            "kb_source": "hr_policy/termination_clarification.md",
            "playbook": "Ask for incident count, prior warnings, and policy section references.",
        }

    return {
        "kb_source": "shared/request_clarification.md",
        "playbook": "Request clarifying details before routing.",
    }


def process_incoming_email(email_text: str) -> dict:
    classification = classify_top_level(email_text)
    rag = get_rag_data(classification)

    return {
        "classification": asdict(classification),
        "rag": rag,
        "next_action": "route_to_queue",
    }


if __name__ == "__main__":
    samples = [
        "Employee termination request: this is a first offense due to policy violation.",
        "Please process supply chain replacement, order 9921 arrived damaged.",
        "I need leave request approval for 3 sick leave days next week.",
        "Termination case with multiple incidents and second warning already issued.",
        "Supply chain team asks to assess order feasibility and vendor timeline.",
    ]

    for idx, email in enumerate(samples, start=1):
        result = process_incoming_email(email)
        print(f"\n--- Email {idx} ---")
        print(email)
        print(json.dumps(result, indent=2))
