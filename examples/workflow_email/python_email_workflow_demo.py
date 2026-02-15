from __future__ import annotations

from dataclasses import asdict, dataclass
from enum import Enum
from pathlib import Path
from typing import Optional
import json
import os

try:
    from dotenv import load_dotenv  # type: ignore[reportMissingImports]
except ImportError:

    def load_dotenv(*_args: object, **_kwargs: object) -> None:
        return None


from simple_agents_py import Client, ResponseWithMetadata


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
    mode: str = "llm"


def load_llm_settings() -> tuple[str, str, str]:
    env_path = Path(__file__).resolve().parents[1] / ".env"
    load_dotenv(env_path)
    load_dotenv()

    api_base = os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("CUSTOM_API_KEY")
    model = os.getenv("CUSTOM_API_MODEL")
    if not api_base or not api_key or not model:
        raise RuntimeError(
            "Missing LLM settings. Set CUSTOM_API_BASE, CUSTOM_API_KEY, CUSTOM_API_MODEL in examples/.env or environment."
        )
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


def classify_with_llm(email_text: str) -> ClassificationResult:
    api_base, api_key, model = load_llm_settings()
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
        raise RuntimeError(
            "Unexpected response type from simple_agents_py client.complete"
        )

    try:
        payload = json.loads(result.content)
    except json.JSONDecodeError as error:
        raise RuntimeError(
            "LLM returned non-JSON output for structured classification"
        ) from error

    return ClassificationResult(
        top_level=coerce_top_level(str(payload.get("top_level", "clarification"))),
        supply_chain_subtype=coerce_supply_chain(payload.get("supply_chain_subtype")),
        termination_subtype=coerce_termination(payload.get("termination_subtype")),
        confidence=float(payload.get("confidence", 0.0)),
        reason=str(payload.get("reason", "LLM classification")),
        mode="llm",
    )


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
    classification = classify_with_llm(email_text)
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

    print("Mode: llm")

    for idx, email in enumerate(samples, start=1):
        result = process_incoming_email(email)
        print(f"\n--- Email {idx} ---")
        print(email)
        print(json.dumps(result, indent=2))
