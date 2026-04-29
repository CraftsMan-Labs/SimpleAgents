import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import eval_suite
from simple_agents_py import Client
from simple_agents_py.eval_request import EvalReport, EvalSuiteRequest

client = Client(
    require_env("WORKFLOW_PROVIDER"),
    api_base=require_env("WORKFLOW_API_BASE"),
    api_key=require_env("WORKFLOW_API_KEY"),
)

request = EvalSuiteRequest(
    suite_path=str(eval_suite("friendly", "friendly-eval.yaml")),
)
report = EvalReport.model_validate(client.run_eval_suite(request.to_client_payload()))

print(json.dumps(report.model_dump(mode="json"), indent=2))
raise SystemExit(0 if report.status == "passed" else 1)
