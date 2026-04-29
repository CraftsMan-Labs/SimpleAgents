import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import eval_suite, workflows
from simple_agents_py import Client, output_subset, run_eval_suite

client = Client(
    require_env("WORKFLOW_PROVIDER"),
    api_base=require_env("WORKFLOW_API_BASE"),
    api_key=require_env("WORKFLOW_API_KEY"),
)

report = run_eval_suite(
    client,
    workflow_path=workflows("friendly", "friendly.yaml"),
    dataset_path=eval_suite("friendly", "friendly-eval.dataset.jsonl"),
    evaluator=output_subset,
    execution={"node_llm_streaming": False},
    workflow_options={"telemetry": {"enabled": False}},
)

print(json.dumps(report.model_dump(mode="json"), indent=2))
raise SystemExit(0 if report.status == "passed" else 1)
