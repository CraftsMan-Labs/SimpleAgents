"""Run invoice multimodal evals: golden suites must pass; mismatch suites must fail.

Loads ``assets/test-invoice.jpeg`` and writes JSONL under ``evals/invoice/generated/``.

- **Golden** rows: invoice text + image vs invoice-path ``expected_output`` (should pass).
- **Mismatch** rows: reimbursement text + same image vs **invoice** ``expected_output``. When
  the model correctly reaches ``finalize_finance_classification``, path checks **fail**;
  the script treats ``failed`` status as success for these two suites only."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import asset, eval_suite, workflows
from invoice_eval_multimodal import write_invoice_eval_generated_datasets
from simple_agents_py import Client, EvalRunStatus, output_subset, run_eval_suite, terminal_node_exact

PASSING_SUITES = (
    (
        "invoice-image-terminal-eval",
        eval_suite("invoice", "generated", "invoice-image-terminal-eval.dataset.jsonl"),
        terminal_node_exact,
    ),
    (
        "invoice-image-node-eval",
        eval_suite("invoice", "generated", "invoice-image-node-eval.dataset.jsonl"),
        output_subset,
    ),
)

# Wrong goldens on purpose: expect eval **failure** when routing is correct.
MISMATCH_SUITES = (
    (
        "invoice-image-terminal-eval-mismatch",
        eval_suite("invoice", "generated", "invoice-image-terminal-eval-mismatch.dataset.jsonl"),
        terminal_node_exact,
    ),
    (
        "invoice-image-node-eval-mismatch",
        eval_suite("invoice", "generated", "invoice-image-node-eval-mismatch.dataset.jsonl"),
        output_subset,
    ),
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

print(
    f"Invoice image evals (multimodal): starting ({len(PASSING_SUITES)} golden + "
    f"{len(MISMATCH_SUITES)} deliberate-mismatch suites)…",
    file=sys.stderr,
    flush=True,
)

ok = True

WORKFLOW_PATH = workflows("email-classification", "test.yaml")


for label, dataset_path, evaluator in PASSING_SUITES:
    p = dataset_path.resolve()
    print("", file=sys.stderr)
    print(f"[{label}] running… ({p.name})", file=sys.stderr, flush=True)
    report = run_eval_suite(
        client,
        workflow_path=WORKFLOW_PATH,
        dataset_path=p,
        evaluator=evaluator,
        execution={"node_llm_streaming": False},
        workflow_options={"telemetry": {"enabled": False}},
    )
    print("=" * 100)
    print(json.dumps(report.model_dump(mode="json"), indent=2))
    print("=" * 100)
    passed = report.status == EvalRunStatus.PASSED
    ok = ok and passed
    verdict = "PASSED" if passed else str(report.status).upper()
    print(f"[{label}] {verdict}", file=sys.stderr, flush=True)
    print(json.dumps(report.model_dump(mode="json"), indent=2))

for label, dataset_path, evaluator in MISMATCH_SUITES:
    p = dataset_path.resolve()
    print("", file=sys.stderr)
    print(f"[{label}] running… ({p.name}) [expect suite FAILED]", file=sys.stderr, flush=True)
    report = run_eval_suite(
        client,
        workflow_path=WORKFLOW_PATH,
        dataset_path=p,
        evaluator=evaluator,
        execution={"node_llm_streaming": False},
        workflow_options={"telemetry": {"enabled": False}},
    )
    print("=" * 100)
    print(json.dumps(report.model_dump(mode="json"), indent=2))
    print("=" * 100)

    mismatch_ok = (
        report.status == EvalRunStatus.FAILED
        and report.summary.failed_cases >= 1
        and report.summary.error_cases == 0
    )
    ok = ok and mismatch_ok

    if report.status == EvalRunStatus.ERROR or report.summary.error_cases:
        verdict = "ERROR (unexpected for mismatch suite)"
    elif report.status == EvalRunStatus.PASSED:
        verdict = "UNEXPECTED PASS — model matched invoice golden or harness regression"
    elif mismatch_ok:
        verdict = "FAILED AS EXPECTED"
    else:
        verdict = str(report.status).upper()

    print(f"[{label}] {verdict}", file=sys.stderr, flush=True)
    print(json.dumps(report.model_dump(mode="json"), indent=2))

print("", file=sys.stderr)
print(
    "Invoice image evals (multimodal): OVERALL PASSED." if ok else "Invoice image evals (multimodal): OVERALL FAILED.",
    file=sys.stderr,
    flush=True,
)
raise SystemExit(0 if ok else 1)
