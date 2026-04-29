import json
from pathlib import Path

from dotenv import load_dotenv
from example_env import require_env
from simple_agents_py import Client
from simple_agents_py.eval_request import EvalReport, EvalSuiteRequest

load_dotenv()

client = Client(
    require_env("WORKFLOW_PROVIDER"),
    api_base=require_env("WORKFLOW_API_BASE"),
    api_key=require_env("WORKFLOW_API_KEY"),
)

request = EvalSuiteRequest(suite_path=Path("friendly-eval.yaml"))
report = EvalReport.model_validate(client.run_eval_suite(request.to_client_payload()))

print(json.dumps(report.model_dump(mode="json"), indent=2))
raise SystemExit(0 if report.status == "passed" else 1)
