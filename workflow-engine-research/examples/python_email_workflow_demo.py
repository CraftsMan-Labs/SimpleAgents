from __future__ import annotations

from dataclasses import asdict, dataclass
from enum import Enum
from typing import Optional
import json
import os

try:
    from dotenv import load_dotenv  # type: ignore[reportMissingImports]
except ImportError:

    def load_dotenv() -> None:
        return None


try:
    from simple_agents_py import Client, ResponseWithMetadata  # type: ignore[reportMissingImports]
except ImportError:
    Client = None  # type: ignore[assignment]
    ResponseWithMetadata = None  # type: ignore[assignment]


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
    mode: str = "heuristic"


def normalize(text: str) -> str:
    return " ".join(text.lower().strip().split())


def contains_any(text: str, keywords: list[str]) -> bool:
    return any(keyword in text for keyword in keywords)


def load_llm_settings() -> Optional[tuple[str, str, str]]:
    load_dotenv()
    api_base = os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("CUSTOM_API_KEY")
    model = os.getenv("CUSTOM_API_MODEL")
    if not api_base or not api_key or not model:
        return None
    return api_base, api_key, model


def coerce_top_level(value: str) -> TopLevelCategory:
    try:
        return TopLevelCategory(value)
    except ValueError:
        return TopLevelCategory.CLARIFICATION


def coerce_supply_chain(value: Optional[str]) -> Optional[SupplyChainSubtype]:
    if not value:
        return None
    try:
        return SupplyChainSubtype(value)
    except ValueError:
        return SupplyChainSubtype.CLARIFICATION


def coerce_termination(value: Optional[str]) -> Optional[TerminationSubtype]:
    if not value:
        return None
    try:
        return TerminationSubtype(value)
    except ValueError:
        return TerminationSubtype.CLARIFICATION


def classify_with_llm(email_text: str) -> Optional[ClassificationResult]:
    settings = load_llm_settings()
    if not settings or Client is None or ResponseWithMetadata is None:
        return None

    api_base, api_key, model = settings
    client = Client("openai", api_base=api_base, api_key=api_key)

    schema = {
        "type": "object",
        "properties": {
            "top_level": {
                "type": "string",
                "enum": [category.value for category in TopLevelCategory],
            },
            "supply_chain_subtype": {
                "type": ["string", "null"],
                "enum": [subtype.value for subtype in SupplyChainSubtype] + [None],
            },
            "termination_subtype": {
                "type": ["string", "null"],
                "enum": [subtype.value for subtype in TerminationSubtype] + [None],
            },
            "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "reason": {"type": "string"},
        },
        "required": ["top_level", "confidence", "reason"],
        "additionalProperties": False,
    }

    prompt = (
        "You are an email intake classifier.\n"
        "Classify the incoming email into one top-level category:\n"
        "- probation\n"
        "- termination\n"
        "- leave_request\n"
        "- supply_chain_request\n"
        "- clarification\n\n"
        "If top_level is supply_chain_request, set supply_chain_subtype to one of:\n"
        "- order_assessment\n"
        "- order_replacement\n"
        "- clarification\n\n"
        "If top_level is termination, set termination_subtype to one of:\n"
        "- first_time_offense\n"
        "- repeated_offense\n"
        "- clarification\n\n"
        "For non-applicable subtype fields, set null.\n"
        "Output valid JSON only.\n\n"
        "Email:\n"
        f"{email_text}"
    )

    messages: list[dict[str, object]] = [
        {"role": "system", "content": "You classify HR and supply-chain emails."},
        {"role": "user", "content": prompt},
    ]

    result = client.complete(
        model, messages, schema=schema, schema_name="email_classification"
    )
    if not isinstance(result, ResponseWithMetadata):
        return None

    try:
        payload = json.loads(result.content)
    except json.JSONDecodeError:
        return None

    return ClassificationResult(
        top_level=coerce_top_level(str(payload.get("top_level", "clarification"))),
        supply_chain_subtype=coerce_supply_chain(payload.get("supply_chain_subtype")),
        termination_subtype=coerce_termination(payload.get("termination_subtype")),
        confidence=float(payload.get("confidence", 0.0)),
        reason=str(payload.get("reason", "LLM classification")),
        mode="llm",
    )


def classify_top_level_heuristic(email_text: str) -> ClassificationResult:
    text = normalize(email_text)

    if contains_any(text, ["probation", "probation period", "extend probation"]):
        return ClassificationResult(
            top_level=TopLevelCategory.PROBATION,
            confidence=0.93,
            reason="Detected probation-related keywords.",
            mode="heuristic",
        )

    if contains_any(text, ["termination", "terminate", "dismiss", "fired"]):
        subtype = classify_termination_heuristic(text)
        return ClassificationResult(
            top_level=TopLevelCategory.TERMINATION,
            termination_subtype=subtype,
            confidence=0.95 if subtype != TerminationSubtype.CLARIFICATION else 0.72,
            reason="Detected termination-related keywords.",
            mode="heuristic",
        )

    if contains_any(
        text, ["leave request", "vacation", "sick leave", "pto", "time off"]
    ):
        return ClassificationResult(
            top_level=TopLevelCategory.LEAVE_REQUEST,
            confidence=0.94,
            reason="Detected leave-request keywords.",
            mode="heuristic",
        )

    if contains_any(text, ["supply", "shipment", "vendor", "purchase order", "order"]):
        subtype = classify_supply_chain_heuristic(text)
        return ClassificationResult(
            top_level=TopLevelCategory.SUPPLY_CHAIN_REQUEST,
            supply_chain_subtype=subtype,
            confidence=0.91 if subtype != SupplyChainSubtype.CLARIFICATION else 0.7,
            reason="Detected supply-chain keywords.",
            mode="heuristic",
        )

    return ClassificationResult(
        top_level=TopLevelCategory.CLARIFICATION,
        confidence=0.55,
        reason="No strong category match. Needs more information.",
        mode="heuristic",
    )


def classify_supply_chain_heuristic(text: str) -> SupplyChainSubtype:
    replacement_signals = ["replacement", "damaged", "wrong item", "return", "replace"]
    assessment_signals = ["assess", "assessment", "review order", "evaluate", "quote"]

    has_replacement = contains_any(text, replacement_signals)
    has_assessment = contains_any(text, assessment_signals)

    if has_replacement and not has_assessment:
        return SupplyChainSubtype.ORDER_REPLACEMENT
    if has_assessment and not has_replacement:
        return SupplyChainSubtype.ORDER_ASSESSMENT
    return SupplyChainSubtype.CLARIFICATION


def classify_termination_heuristic(text: str) -> TerminationSubtype:
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


def classify_email(email_text: str) -> ClassificationResult:
    llm_result = classify_with_llm(email_text)
    if llm_result is not None:
        return llm_result
    return classify_top_level_heuristic(email_text)


def get_rag_data(classification: ClassificationResult) -> dict:
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
    classification = classify_email(email_text)
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

    llm_enabled = load_llm_settings() is not None and Client is not None
    print(f"Mode: {'llm' if llm_enabled else 'heuristic_fallback'}")

    for idx, email in enumerate(samples, start=1):
        result = process_incoming_email(email)
        print(f"\n--- Email {idx} ---")
        print(email)
        print(json.dumps(result, indent=2))
