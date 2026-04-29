import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_paths import eval_suite
from simple_agents_py import Client, EvalSuiteRequest, run_eval_suite

client = Client(
    "openai",
    api_base="https://example.invalid/v1",
    api_key="sk-mocked-rag-eval-000000000000",
)

report = run_eval_suite(
    client,
    EvalSuiteRequest(suite_path=str(eval_suite("rag", "rag-eval.yaml"))),
)

print(json.dumps(report.model_dump(mode="json"), indent=2))
raise SystemExit(0 if report.status == "passed" else 1)
