from __future__ import annotations

import argparse
import json
import os
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, TypedDict, cast

from simple_agents_py import Client

from common import resolve_workflow_path
from runner_utils import (
    create_chat_trace_file,
    default_workflow_registry,
    load_provider_config,
    load_workflow_node_names,
    render_json,
    render_terminal_output,
    step_display_name,
)


class WorkflowStepDetails(TypedDict, total=False):
    node_id: str
    node_kind: str
    elapsed_ms: int
    prompt_tokens: int
    completion_tokens: int
    total_tokens: int | None
    reasoning_tokens: int
    model_name: str
    tokens_per_second: float


class NerdstatsFallbackRequest(TypedDict, total=False):
    workflow_id: str
    terminal_node: str
    total_elapsed_ms: int
    ttft_ms: int | None
    step_details: list[WorkflowStepDetails]
    step_timings: list[WorkflowStepDetails]
    total_input_tokens: int
    total_output_tokens: int
    total_tokens: int
    total_reasoning_tokens: int | None
    tokens_per_second: float
    trace_id: str


class NerdstatsFallbackResponse(TypedDict):
    workflow_id: str | None
    terminal_node: str | None
    total_elapsed_ms: int | None
    ttft_ms: int | None
    step_details: list[WorkflowStepDetails]
    total_input_tokens: int | None
    total_output_tokens: int | None
    total_tokens: int | None
    total_reasoning_tokens: int | None
    tokens_per_second: float | None
    trace_id: str | None
    token_metrics_available: bool
    token_metrics_source: str
    llm_nodes_without_usage: list[str]


NerdstatsPayload = dict[str, object] | NerdstatsFallbackResponse


class WorkflowEvent(TypedDict, total=False):
    event_type: str
    node_id: str
    step_id: str
    node_kind: str
    streamable: bool
    message: str
    delta: str
    token_kind: str
    is_terminal_node_token: bool
    elapsed_ms: int
    metadata: dict[str, object]


class WorkflowRunResult(TypedDict, total=False):
    workflow_id: str
    terminal_node: str
    terminal_output: object
    trace: list[str]
    outputs: dict[str, dict[str, object]]
    step_timings: list[WorkflowStepDetails]
    total_elapsed_ms: int
    events: list[WorkflowEvent]


class RunTurnRequest(TypedDict):
    client: Client
    workflow_path: Path
    workflow_input: dict[str, object]
    include_events: bool
    stream: bool
    show_thinking: bool
    nerdstats: bool
    conversation_id: str
    node_names: dict[str, str]
    model: str | None


class RunTurnResponse(TypedDict):
    result: WorkflowRunResult
    streamed_events: list[WorkflowEvent]
    nerdstats: NerdstatsPayload | None


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
    parser.add_argument(
        "--model",
        default=None,
        help="Override llm_call model for all workflow LLM nodes",
    )
    return parser.parse_args()


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


def render_assistant_reply(result: WorkflowRunResult) -> str:
    return render_terminal_output(result.get("terminal_output"))


def _print_stream_event(
    event: WorkflowEvent,
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
    result: WorkflowRunResult, node_names: dict[str, str]
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
        print(render_json(payload))

    terminal_node = result.get("terminal_node")
    terminal_output = result.get("terminal_output")
    if isinstance(terminal_node, str) and terminal_output is not None:
        print(f"\nTerminal Step: {step_display_name(terminal_node, node_names)}")
        print("JSON")
        print(render_json(terminal_output))


def _fallback_nerdstats(request: NerdstatsFallbackRequest) -> NerdstatsFallbackResponse:
    step_details = request.get("step_details", request.get("step_timings", []))
    llm_nodes_without_usage: list[str] = []
    if isinstance(step_details, list):
        for step in step_details:
            if not isinstance(step, dict):
                continue
            if step.get("node_kind") != "llm_call":
                continue
            node_id = step.get("node_id")
            if step.get("total_tokens") is None and isinstance(node_id, str):
                llm_nodes_without_usage.append(node_id)

    token_metrics_available = len(llm_nodes_without_usage) == 0
    total_input_tokens = (
        request.get("total_input_tokens") if token_metrics_available else None
    )
    total_output_tokens = (
        request.get("total_output_tokens") if token_metrics_available else None
    )
    total_tokens = request.get("total_tokens") if token_metrics_available else None
    total_reasoning_tokens = (
        request.get("total_reasoning_tokens") if token_metrics_available else None
    )
    tokens_per_second = (
        request.get("tokens_per_second") if token_metrics_available else None
    )

    return {
        "workflow_id": request.get("workflow_id"),
        "terminal_node": request.get("terminal_node"),
        "total_elapsed_ms": request.get("total_elapsed_ms"),
        "ttft_ms": request.get("ttft_ms"),
        "step_details": step_details,
        "total_input_tokens": total_input_tokens,
        "total_output_tokens": total_output_tokens,
        "total_tokens": total_tokens,
        "total_reasoning_tokens": total_reasoning_tokens,
        "tokens_per_second": tokens_per_second,
        "trace_id": request.get("trace_id"),
        "token_metrics_available": token_metrics_available,
        "token_metrics_source": (
            "provider_usage"
            if token_metrics_available
            else "provider_stream_usage_unavailable"
        ),
        "llm_nodes_without_usage": llm_nodes_without_usage,
    }


def _extract_nerdstats_from_events(
    streamed_events: list[WorkflowEvent],
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


def _run_turn(request: RunTurnRequest) -> RunTurnResponse:
    workflow_options: dict[str, object] = {
        "telemetry": {"nerdstats": request["nerdstats"]},
        "trace": {"tenant": {"conversation_id": request["conversation_id"]}},
    }
    model = request["model"]
    if model is not None and model.strip() != "":
        workflow_options["model"] = model.strip()
    client_any: Any = request["client"]
    if not request["stream"]:
        result_any = client_any.run_workflow_yaml(
            str(request["workflow_path"]),
            request["workflow_input"],
            include_events=request["include_events"],
            workflow_options=workflow_options,
        )
        result = cast(
            WorkflowRunResult,
            result_any if isinstance(result_any, dict) else {},
        )
        events: list[WorkflowEvent] = []
        if request["include_events"]:
            raw_events = result.get("events", [])
            if isinstance(raw_events, list):
                events = [event for event in raw_events if isinstance(event, dict)]
        return {"result": result, "streamed_events": events, "nerdstats": None}

    streamed_events: list[WorkflowEvent] = []
    stream_state: dict[str, object] = {"current_node": None, "line_open": False}

    def on_event(event: WorkflowEvent) -> None:
        streamed_events.append(event)
        _print_stream_event(
            event, request["show_thinking"], stream_state, request["node_names"]
        )

    wf_input = request["workflow_input"]
    raw_messages = wf_input.get("messages")
    if not isinstance(raw_messages, list):
        raise TypeError("workflow_input.messages must be a list of message dicts when streaming")
    messages = [m for m in raw_messages if isinstance(m, dict)]
    extra_input = {k: v for k, v in wf_input.items() if k != "messages"}

    stream_request: dict[str, object] = {
        "workflow_path": str(request["workflow_path"]),
        "messages": messages,
        "input": extra_input,
        "workflow_options": workflow_options,
        "execution": {
            "workflow_streaming": True,
            "node_llm_streaming": True,
            "split_stream_deltas": request["show_thinking"],
        },
    }

    result_any = client_any.stream(stream_request, on_event=on_event)
    result = cast(
        WorkflowRunResult,
        result_any if isinstance(result_any, dict) else {},
    )

    expected_types = (
        {"node_stream_thinking_delta", "node_stream_output_delta"}
        if request["show_thinking"]
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

    nerdstats_payload: NerdstatsPayload | None = None
    if request["nerdstats"]:
        nerdstats_payload = _extract_nerdstats_from_events(streamed_events)
        if nerdstats_payload is None:
            nerdstats_payload = _fallback_nerdstats(
                cast(NerdstatsFallbackRequest, result)
            )
        print(f"Nerdstats: {json.dumps(nerdstats_payload, ensure_ascii=True)}")

    return {
        "result": result,
        "streamed_events": streamed_events,
        "nerdstats": nerdstats_payload,
    }


def main() -> None:
    args = parse_args()
    # Duplicate opt-in: execution.split_stream_deltas is set on the stream request below.
    # Env remains for backward compatibility; remove this block once downstreams migrate.
    if args.show_thinking:
        os.environ["SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW"] = "1"
    else:
        os.environ.pop("SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW", None)

    provider, api_base, api_key = load_provider_config(caller_file=__file__)
    client = Client(provider, api_base=api_base, api_key=api_key)

    workflow_path = resolve_workflow_path(args.workflow, caller_file=__file__)
    workflow_registry = default_workflow_registry(workflow_path)
    node_names = load_workflow_node_names(workflow_path)
    messages = initial_messages()
    conversation_id = args.conversation_id or str(uuid.uuid4())
    trace_file = create_chat_trace_file(args.trace_dir, conversation_id)

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
            "workflow_registry": workflow_registry,
        }

        turn_output = _run_turn(
            {
                "client": client,
                "workflow_path": workflow_path,
                "workflow_input": workflow_input,
                "include_events": args.include_events,
                "stream": args.stream,
                "show_thinking": args.show_thinking,
                "nerdstats": args.nerdstats,
                "conversation_id": conversation_id,
                "node_names": node_names,
                "model": args.model,
            }
        )
        result = turn_output["result"]
        streamed_events = turn_output["streamed_events"]
        nerdstats_payload = turn_output["nerdstats"]

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
