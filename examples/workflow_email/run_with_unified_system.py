from __future__ import annotations

import argparse
import json
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import TYPE_CHECKING

from dotenv import load_dotenv
from simple_agents_py import Client

if TYPE_CHECKING:
    from simple_agents_py import WorkflowRunOutput


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Unified email system: capabilities + intake classification + RAG + draft"
    )
    parser.add_argument(
        "--workflow",
        default="workflow_email/email-unified-chat-intake-classification.yaml",
        help="Path to workflow YAML file",
    )
    parser.add_argument(
        "--include-events",
        action="store_true",
        help="Include workflow events in each turn response",
    )
    parser.add_argument(
        "--max-turns",
        type=int,
        default=8,
        help="Maximum chat turns before exiting",
    )
    parser.add_argument(
        "--trace-dir",
        default="examples/workflow_email/traces_unified",
        help="Directory to persist per-turn workflow traces as JSONL",
    )
    return parser.parse_args()


def load_config() -> tuple[str, str, str]:
    load_dotenv(Path(__file__).resolve().parents[1] / ".env")
    load_dotenv()

    provider = os.getenv("WORKFLOW_PROVIDER", "openai")
    api_base = os.getenv("WORKFLOW_API_BASE") or os.getenv("CUSTOM_API_BASE")
    api_key = os.getenv("WORKFLOW_API_KEY") or os.getenv("CUSTOM_API_KEY")

    if not api_base or not api_key:
        raise RuntimeError(
            "Set WORKFLOW_API_BASE and WORKFLOW_API_KEY (or CUSTOM_API_BASE/CUSTOM_API_KEY)."
        )
    return provider, api_base, api_key


def resolve_workflow_path(workflow: str) -> Path:
    workflow_path = Path(workflow)
    if workflow_path.exists():
        return workflow_path

    repo_relative = Path(__file__).resolve().parents[2] / workflow
    if repo_relative.exists():
        return repo_relative

    if workflow.startswith("examples/"):
        trimmed = workflow[len("examples/") :]
        trimmed_path = Path(__file__).resolve().parents[1] / trimmed
        if trimmed_path.exists():
            return trimmed_path

    raise FileNotFoundError(f"workflow file not found: {workflow}")


def initial_messages() -> list[dict[str, str]]:
    return [
        {
            "role": "system",
            "content": (
                "You are a complete email operations assistant. "
                "Explain capabilities for new users, gather missing scenario details, "
                "classify requests, use playbook guidance, and draft professional replies."
            ),
        }
    ]


def render_assistant_reply(result: WorkflowRunOutput) -> str:
    terminal = result.get("terminal_node")
    terminal_output = result.get("terminal_output") or {}

    if terminal in {"explain_capabilities", "ask_for_scenario"}:
        question = terminal_output.get("question")
        if isinstance(question, str) and question.strip():
            return question
        return json.dumps(terminal_output, indent=2)

    if terminal == "generate_email_draft":
        subject = terminal_output.get("subject", "Draft Email")
        body = terminal_output.get("body", "")
        if not isinstance(subject, str):
            subject = "Draft Email"
        if not isinstance(body, str):
            body = ""
        return f"Subject: {subject}\n\n{body}".strip()

    return json.dumps(terminal_output, indent=2)


def main() -> None:
    args = parse_args()
    provider, api_base, api_key = load_config()
    client = Client(provider, api_base=api_base, api_key=api_key)

    workflow_path = resolve_workflow_path(args.workflow)
    messages = initial_messages()
    trace_dir = Path(args.trace_dir)
    trace_dir.mkdir(parents=True, exist_ok=True)
    session_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    trace_file = trace_dir / f"unified-session-{session_id}.jsonl"

    print("Unified Email System")
    print("Type your request. Type 'exit' to quit.\n")
    print(f"Trace log: {trace_file}\n")

    for turn in range(1, args.max_turns + 1):
        user_input = input("You: ").strip()
        if not user_input:
            continue
        if user_input.lower() in {"exit", "quit"}:
            print("Bye!")
            return

        messages.append({"role": "user", "content": user_input})
        workflow_input = {"email_text": user_input, "messages": messages}

        result = client.run_workflow_yaml(
            str(workflow_path),
            workflow_input,
            include_events=args.include_events,
        )

        trace_record = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "turn": turn,
            "workflow_path": str(workflow_path),
            "workflow_id": result.get("workflow_id"),
            "terminal_node": result.get("terminal_node"),
            "trace": result.get("trace", []),
            "step_timings": result.get("step_timings", []),
            "total_elapsed_ms": result.get("total_elapsed_ms"),
            "user_input": user_input,
            "assistant_output": result.get("terminal_output"),
            "events": result.get("events", []) if args.include_events else None,
        }
        with trace_file.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(trace_record, ensure_ascii=True) + "\n")

        reply = render_assistant_reply(result)
        print(f"\nAssistant: {reply}\n")
        messages.append({"role": "assistant", "content": reply})

        if result.get("terminal_node") == "generate_email_draft":
            print("Draft ready. Continue chatting to refine, or type 'exit'.\n")

    print("Reached max turns. Restart to continue.")


if __name__ == "__main__":
    main()
