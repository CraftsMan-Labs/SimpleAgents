# Canonical copy lives at `../../workflows/email-classification/handlers.py`.
# Duplicated because `run_eval_suite` resolves `handlers.py` relative to the eval suite YAML directory.


def get_seller_name(context, payload):
    """`context`: workflow context from the runner; `payload`: this node's `config.payload` (see test.yaml)."""
    # print(f"context: {context}")
    # print(f"payload: {payload}")
    company_name = None
    if isinstance(payload, dict):
        company_name = payload.get("company_name")
    company_name = str(company_name or "").strip().lower()

    stakeholder_map = {
        "google": "Sundar Pichai",
        "microsoft": "Satya Nadella",
        "apple": "Tim Cook",
        "amazon": "Andy Jassy",
    }

    return stakeholder_map.get(company_name, "unknown")
