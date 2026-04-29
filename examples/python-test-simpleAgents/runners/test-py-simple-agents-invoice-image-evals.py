"""Run the two bundled invoice multimodal evals (terminal-only vs deep path assertions).

Loads ``assets/test-invoice.jpeg`` and writes vision-style JSONL datasets (text +
``image_url``) beside the suite YAMLs under ``evals/invoice/generated/``, matching the
Jaeger multimodal invoice example."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import asset, eval_suite
from invoice_eval_multimodal import write_invoice_eval_generated_datasets
from simple_agents_py import Client, EvalSuiteRequest, run_eval_suite

SUITS = (
    ("invoice-image-terminal-eval", eval_suite("invoice", "invoice-image-terminal-eval.yaml")),
    ("invoice-image-node-eval", eval_suite("invoice", "invoice-image-node-eval.yaml")),
)

IMAGE_ASSET = asset("test-invoice.jpeg")


def _require_asset(path: Path) -> Path:
    if not path.is_file():
        raise SystemExit(
            f"Required multimodal asset is missing: {path}\n"
            "Add a JPEG (e.g. a sample invoice) at that path—the same asset used "
            "for test-py-simple-agents-invoice-image-jaeger / jaegar)."
        )
    return path


write_invoice_eval_generated_datasets(eval_suite("invoice").resolve(), _require_asset(IMAGE_ASSET))

client = Client(
    require_env("WORKFLOW_PROVIDER"),
    api_base=require_env("WORKFLOW_API_BASE"),
    api_key=require_env("WORKFLOW_API_KEY"),
)

print(f"Invoice image evals (multimodal): starting ({len(SUITS)} suites)…", file=sys.stderr, flush=True)

ok = True
for label, suite_path in SUITS:
    p = suite_path.resolve()
    print("", file=sys.stderr)
    print(f"[{label}] running… ({p.name})", file=sys.stderr, flush=True)
    report = run_eval_suite(client, EvalSuiteRequest(suite_path=str(p)))
    passed = report.status == "passed"
    ok = ok and passed
    verdict = "PASSED" if passed else str(report.status).upper()
    print(f"[{label}] {verdict}", file=sys.stderr, flush=True)
    print(json.dumps(report.model_dump(mode="json"), indent=2))

print("", file=sys.stderr)
print(
    "Invoice image evals (multimodal): OVERALL PASSED." if ok else "Invoice image evals (multimodal): OVERALL FAILED.",
    file=sys.stderr,
    flush=True,
)
raise SystemExit(0 if ok else 1)
