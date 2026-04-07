# Run the email-classification workflow with streaming.
# Based on examples/python-test-simpleAgents/test-py-simple-agents-streaming.py
#
# pip install simple-agents-py python-dotenv
# Create .env with WORKFLOW_PROVIDER, WORKFLOW_API_BASE, WORKFLOW_API_KEY

import json
import os
from pathlib import Path

from dotenv import load_dotenv
from simple_agents_py import Client
from simple_agents_py.workflow_payload import workflow_execution_request_to_mapping
from simple_agents_py.workflow_request import (
    WorkflowExecutionFlags,
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)

load_dotenv()

client = Client(
    os.environ["WORKFLOW_PROVIDER"],
    api_base=os.environ["WORKFLOW_API_BASE"],
    api_key=os.environ["WORKFLOW_API_KEY"],
)

workflow_file = Path(__file__).resolve().parent / "email-classification.yaml"
user_input = input("Enter your email text: ")

req = WorkflowExecutionRequest(
    workflow_path=str(workflow_file),
    messages=[WorkflowMessage(role=WorkflowRole.USER, content=user_input)],
    execution=WorkflowExecutionFlags(
        node_llm_streaming=True,
        split_stream_deltas=False,
    ),
)

result = client.stream_workflow(
    workflow_execution_request_to_mapping(req),
    on_event=lambda event: print(event),
)
print("\n")
print(json.dumps(result, indent=2))
