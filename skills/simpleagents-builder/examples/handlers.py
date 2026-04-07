# Custom worker handler for email-classification.yaml
# Place next to the YAML file. The runner loads it automatically.
# Based on examples/python-test-simpleAgents/handlers.py

def get_seller_name(context, payload):
    """Look up the stakeholder for a company name."""
    company_name = str(payload.get("company_name", "")).strip().lower()
    stakeholder_map = {
        "google": "Sundar Pichai",
        "microsoft": "Satya Nadella",
        "apple": "Tim Cook",
        "amazon": "Andy Jassy",
    }
    return stakeholder_map.get(company_name, "unknown")
