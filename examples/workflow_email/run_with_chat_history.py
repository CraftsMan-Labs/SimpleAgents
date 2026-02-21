from __future__ import annotations

import argparse
import json
import os
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from dotenv import load_dotenv
from simple_agents_py import Client


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Interactive chat workflow: ask scenario details or draft email"
    )
    parser.add_argument(
        "--workflow",
        default="workflow_email/email-chat-draft-or-clarify.yaml",
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
        default="examples/workflow_email/traces",
        help="Directory to persist per-turn workflow traces as JSONL",
    )
    parser.add_argument(
        "--conversation-id",
        default=None,
        help="Conversation UUID used for trace correlation (auto-generated if omitted)",
    )
    parser.add_argument(
        "--stream",
        action="store_true",
        help="Stream workflow node deltas live in terminal when YAML nodes have stream=true",
    )
    parser.add_argument(
        "--show-thinking",
        action="store_true",
        help="Show raw model stream deltas (including thinking tokens) for debugging",
    )
    parser.add_argument(
        "--show-step-json",
        action="store_true",
        help="Print per-step JSON summaries after execution",
    )
    parser.add_argument(
        "--nerdstats",
        dest="nerdstats",
        action="store_true",
        default=True,
        help="Show end-of-stream nerdstats payload (enabled by default)",
    )
    parser.add_argument(
        "--no-nerdstats",
        dest="nerdstats",
        action="store_false",
        help="Disable end-of-stream nerdstats payload",
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
                "You are a friendly email drafting assistant for new users. "
                "First, explain capabilities clearly when asked what you can do. "
                "Then gather missing scenario details and draft concise professional emails. "
                "If context is incomplete, ask one specific follow-up question."
            ),
        }
    ]


def load_workflow_node_names(workflow_path: Path) -> dict[str, str]:
    try:
        import yaml  # type: ignore
    except Exception:
        return {}

    try:
        raw = workflow_path.read_text(encoding="utf-8")
        parsed = yaml.safe_load(raw)
    except Exception:
        return {}

    if not isinstance(parsed, dict):
        return {}

    nodes = parsed.get("nodes")
    if not isinstance(nodes, list):
        return {}

    names: dict[str, str] = {}
    for node in nodes:
        if not isinstance(node, dict):
            continue
        node_id = node.get("id")
        node_name = node.get("name")
        if isinstance(node_id, str):
            if isinstance(node_name, str) and node_name.strip():
                names[node_id] = node_name.strip()
            else:
                names[node_id] = node_id.replace("_", " ").title()
    return names


def step_display_name(node_id: str | None, node_names: dict[str, str]) -> str:
    if node_id is None:
        return "Workflow"
    return node_names.get(node_id, node_id.replace("_", " ").title())


def render_assistant_reply(result: dict) -> str:
    terminal_output = result.get("terminal_output")
    if terminal_output is None:
        return ""
    if isinstance(terminal_output, str):
        return terminal_output
    return json.dumps(terminal_output, indent=2, ensure_ascii=True)


def _print_stream_event(
    event: dict[str, object],
    show_thinking: bool,
    stream_state: dict[str, object],
    node_names: dict[str, str],
) -> None:
    event_type = event.get("event_type")
    node_id = event.get("node_id")
    step_id = event.get("step_id")
    delta = event.get("delta")
    token_kind = event.get("token_kind")
    is_terminal_node_token = event.get("is_terminal_node_token")
    is_displayed_stream_event = event_type == "node_stream_delta"
    if show_thinking:
        is_displayed_stream_event = event_type in {
            "node_stream_thinking_delta",
            "node_stream_output_delta",
        }

    if is_displayed_stream_event and isinstance(delta, str):
        display_node_id = (
            node_id
            if isinstance(node_id, str)
            else (step_id if isinstance(step_id, str) else None)
        )
        step_name = step_display_name(display_node_id, node_names)
        current_node = stream_state.get("current_node")
        line_open = bool(stream_state.get("line_open", False))
        if current_node != display_node_id:
            if line_open:
                print()
            print(f"\nStep: {step_name}")
            print("Streaming:", end=" ", flush=True)
            stream_state["current_node"] = display_node_id
            stream_state["line_open"] = True
            stream_state["last_token_label"] = None

        if show_thinking:
            token_label_parts = []
            if isinstance(token_kind, str) and token_kind.strip():
                token_label_parts.append(token_kind.strip())
            if is_terminal_node_token is True:
                token_label_parts.append("terminal")
            token_label = (
                f"[{' '.join(token_label_parts)}] " if token_label_parts else ""
            )
            last_token_label = stream_state.get("last_token_label")
            if token_label and token_label != last_token_label:
                if line_open:
                    print()
                print(f"{token_label}{step_name}: ", end="", flush=True)
                stream_state["last_token_label"] = token_label
                stream_state["line_open"] = True
            print(delta, end="", flush=True)
        else:
            print(delta, end="", flush=True)
        return

    if event_type in {
        "workflow_started",
        "workflow_completed",
    }:
        return

    _ = show_thinking


def _print_step_json_summary(
    result: dict[str, Any], node_names: dict[str, str]
) -> None:
    trace = result.get("trace")
    outputs = result.get("outputs")
    if not isinstance(trace, list) or not isinstance(outputs, dict):
        return

    for node in trace:
        if not isinstance(node, str):
            continue
        node_value = outputs.get(node)
        if not isinstance(node_value, dict):
            continue
        payload = node_value.get("output")
        if payload is None:
            continue
        print(f"\nStep: {step_display_name(node, node_names)}")
        print("JSON")
        print(json.dumps(payload, indent=2, ensure_ascii=True))

    terminal_node = result.get("terminal_node")
    terminal_output = result.get("terminal_output")
    if isinstance(terminal_node, str) and terminal_output is not None:
        print(f"\nTerminal Step: {step_display_name(terminal_node, node_names)}")
        print("JSON")
        print(json.dumps(terminal_output, indent=2, ensure_ascii=True))


def _fallback_nerdstats(result: dict[str, object]) -> dict[str, object]:
    step_timings = result.get("step_timings", [])
    llm_nodes_without_usage: list[str] = []
    if isinstance(step_timings, list):
        for step in step_timings:
            if not isinstance(step, dict):
                continue
            if step.get("node_kind") != "llm_call":
                continue
            if step.get("total_tokens") is None and isinstance(
                step.get("node_id"), str
            ):
                llm_nodes_without_usage.append(step["node_id"])

    token_metrics_available = len(llm_nodes_without_usage) == 0
    total_input_tokens = (
        result.get("total_input_tokens") if token_metrics_available else None
    )
    total_output_tokens = (
        result.get("total_output_tokens") if token_metrics_available else None
    )
    total_tokens = result.get("total_tokens") if token_metrics_available else None
    total_thinking_tokens = (
        result.get("total_thinking_tokens") if token_metrics_available else None
    )
    tokens_per_second = (
        result.get("tokens_per_second") if token_metrics_available else None
    )

    return {
        "workflow_id": result.get("workflow_id"),
        "terminal_node": result.get("terminal_node"),
        "total_elapsed_ms": result.get("total_elapsed_ms"),
        "ttft_ms": result.get("ttft_ms"),
        "step_timings": step_timings,
        "llm_node_metrics": result.get("llm_node_metrics", {}),
        "total_input_tokens": total_input_tokens,
        "total_output_tokens": total_output_tokens,
        "total_tokens": total_tokens,
        "total_thinking_tokens": total_thinking_tokens,
        "tokens_per_second": tokens_per_second,
        "trace_id": result.get("trace_id"),
        "token_metrics_available": token_metrics_available,
        "token_metrics_source": (
            "provider_usage"
            if token_metrics_available
            else "provider_stream_usage_unavailable"
        ),
        "llm_nodes_without_usage": llm_nodes_without_usage,
    }


def _extract_nerdstats_from_events(
    streamed_events: list[dict[str, object]],
) -> dict[str, object] | None:
    for event in reversed(streamed_events):
        if event.get("event_type") != "workflow_completed":
            continue
        metadata = event.get("metadata")
        if not isinstance(metadata, dict):
            continue
        nerdstats = metadata.get("nerdstats")
        if isinstance(nerdstats, dict):
            return nerdstats
    return None


def _run_turn(
    client: Client,
    workflow_path: Path,
    workflow_input: dict[str, object],
    include_events: bool,
    stream: bool,
    show_thinking: bool,
    nerdstats: bool,
    conversation_id: str,
    node_names: dict[str, str],
) -> tuple[dict[str, object], list[dict[str, object]], dict[str, object] | None]:
    workflow_options = {
        "telemetry": {"nerdstats": nerdstats},
        "trace": {"tenant": {"conversation_id": conversation_id}},
    }
    client_any: Any = client
    if not stream:
        result = client_any.run_workflow_yaml(
            str(workflow_path),
            workflow_input,
            include_events=include_events,
            workflow_options=workflow_options,
        )
        events = result.get("events", []) if include_events else []
        return result, events if isinstance(events, list) else [], None

    streamed_events: list[dict[str, object]] = []
    stream_state: dict[str, object] = {"current_node": None, "line_open": False}

    def on_event(event: dict[str, object]) -> None:
        streamed_events.append(event)
        _print_stream_event(event, show_thinking, stream_state, node_names)

    result = client_any.run_workflow_yaml_stream(
        str(workflow_path),
        workflow_input,
        on_event=on_event,
        workflow_options=workflow_options,
    )

    expected_types = (
        {"node_stream_thinking_delta", "node_stream_output_delta"}
        if show_thinking
        else {"node_stream_delta"}
    )
    if not any(
        isinstance(event, dict) and event.get("event_type") in expected_types
        for event in streamed_events
    ):
        print(
            f"[stream] No {', '.join(sorted(expected_types))} events observed. "
            "Ensure llm_call nodes are configured with stream=true."
        )
    elif stream_state.get("line_open", False):
        print()

    nerdstats_payload: dict[str, object] | None = None
    if nerdstats:
        nerdstats_payload = _extract_nerdstats_from_events(streamed_events)
        if nerdstats_payload is None:
            nerdstats_payload = _fallback_nerdstats(result)
        print(f"Nerdstats: {json.dumps(nerdstats_payload, ensure_ascii=True)}")

    return result, streamed_events, nerdstats_payload


def main() -> None:
    args = parse_args()
    if args.show_thinking:
        os.environ["SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW"] = "1"
    else:
        os.environ.pop("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW", None)

    provider, api_base, api_key = load_config()
    client = Client(provider, api_base=api_base, api_key=api_key)

    workflow_path = resolve_workflow_path(args.workflow)
    node_names = load_workflow_node_names(workflow_path)
    messages = initial_messages()
    trace_dir = Path(args.trace_dir)
    trace_dir.mkdir(parents=True, exist_ok=True)
    conversation_id = args.conversation_id or str(uuid.uuid4())
    session_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    trace_file = trace_dir / f"chat-session-{session_id}-{conversation_id}.jsonl"

    print("Chat Email Assistant")
    print("Type your request. Type 'exit' to quit.\n")
    print(f"Conversation ID: {conversation_id}")
    print(f"Trace log: {trace_file}\n")

    interview_closed = False

    for turn in range(1, args.max_turns + 1):
        user_input = input("You: ").strip()
        if not user_input:
            continue
        if user_input.lower() in {"exit", "quit"}:
            print("Bye!")
            return
        if interview_closed:
            print(
                "\nAssistant: This interview session is already closed after termination. "
                "Please start a new session with a new run.\n"
            )
            continue

        messages.append({"role": "user", "content": user_input})

        workflow_input = {
            "email_text": user_input,
            "messages": messages,
        }

        result, streamed_events, nerdstats_payload = _run_turn(
            client,
            workflow_path,
            workflow_input,
            args.include_events,
            args.stream,
            args.show_thinking,
            args.nerdstats,
            conversation_id,
            node_names,
        )

        if args.show_step_json:
            _print_step_json_summary(result, node_names)

        trace_record = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "turn": turn,
            "conversation_id": conversation_id,
            "workflow_path": str(workflow_path),
            "workflow_id": result.get("workflow_id"),
            "terminal_node": result.get("terminal_node"),
            "trace": result.get("trace", []),
            "step_timings": result.get("step_timings", []),
            "total_elapsed_ms": result.get("total_elapsed_ms"),
            "user_input": user_input,
            "assistant_output": result.get("terminal_output"),
            "nerdstats": nerdstats_payload,
            "events": (
                streamed_events
                if args.stream
                else (result.get("events", []) if args.include_events else None)
            ),
        }
        with trace_file.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(trace_record, ensure_ascii=True) + "\n")

        reply = render_assistant_reply(result)
        print(f"\nAssistant: {reply}\n")
        messages.append({"role": "assistant", "content": reply})

        terminal_output_raw = result.get("terminal_output")
        terminal_output = (
            terminal_output_raw if isinstance(terminal_output_raw, dict) else {}
        )
        if (
            result.get("terminal_node") in {"terminate_candidate", "already_terminated"}
            or terminal_output.get("decision") == "terminated"
        ):
            interview_closed = True
            print(
                "Interview closed for this session. Start a new run for a new candidate.\n"
            )

        if result.get("terminal_node") == "generate_email_draft":
            print("Draft ready. Continue chatting to refine, or type 'exit'.\n")

    print("Reached max turns. Restart to continue.")


if __name__ == "__main__":
    main()
