"""Three-phase spec HITL pipeline: interview YAML → optional clarify loop → usefulness YAML.

Usage (from ``examples/python-test-simpleAgents``)::

    uv run python runners/run_spec_hitl_pipeline.py assets/sample-spec.md

Requires ``WORKFLOW_PROVIDER``, ``WORKFLOW_API_BASE``, and ``WORKFLOW_API_KEY`` (see ``example_env.py``).
"""

from __future__ import annotations

import argparse
import json
import logging
import sys
import time
import uuid
from collections.abc import Callable, Mapping
from pathlib import Path
from typing import Any, TypeVar

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import workflows
from simple_agents_py import Client as SimpleAgentsClient
from simple_agents_py.models import WorkflowRunOutputWire
from simple_agents_py.workflow_request import (
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)

logger = logging.getLogger(__name__)

T = TypeVar("T")

QUESTION_IDS = tuple(f"q{i:02d}" for i in range(1, 11))

WF_INTERVIEW = workflows("spec-hitl", "spec-interview.yaml")
WF_CLARIFY = workflows("spec-hitl", "spec-clarify.yaml")
WF_USEFULNESS = workflows("spec-hitl", "spec-usefulness.yaml")


class SpecHitlError(RuntimeError):
    """Raised when orchestration cannot proceed with sane workflow outputs."""


class SpecHitlWorkflowError(SpecHitlError):
    """Raised when a workflow finishes in an unexpected state or shape."""


def _retry_call(operation: Callable[[], T], *, attempts: int = 3, base_delay_seconds: float = 0.75) -> T:
    """Invoke *operation* with bounded exponential backoff (transient provider faults)."""
    if attempts < 1:
        raise ValueError("attempts must be >= 1")
    last_exc: BaseException | None = None
    for attempt in range(attempts):
        try:
            return operation()
        except Exception as exc:
            last_exc = exc
            if attempt == attempts - 1:
                break
            delay = base_delay_seconds * (2**attempt)
            logger.warning(
                "workflow_call_failed_retrying",
                extra={
                    "attempt": attempt + 1,
                    "max_attempts": attempts,
                    "delay_seconds": delay,
                    "error_type": type(exc).__name__,
                },
            )
            time.sleep(delay)
    assert last_exc is not None
    raise SpecHitlError(f"Operation failed after {attempts} attempts") from last_exc


def _expect_completed(run: Mapping[str, Any], *, phase: str) -> None:
    status = run.get("status")
    if status != "completed":
        raise SpecHitlWorkflowError(f"{phase}: expected status 'completed', got {status!r}")


def extract_node_output(run: Mapping[str, Any], node_id: str) -> dict[str, Any]:
    """Return structured LLM/human payload under ``outputs[node_id]['output']``."""
    outputs = run.get("outputs")
    if not isinstance(outputs, dict):
        raise SpecHitlWorkflowError(f"missing outputs map while reading node {node_id!r}")
    blob = outputs.get(node_id)
    if not isinstance(blob, dict):
        raise SpecHitlWorkflowError(f"missing node output for {node_id!r}")
    inner = blob.get("output")
    if not isinstance(inner, dict):
        raise SpecHitlWorkflowError(f"node {node_id!r} did not produce object output")
    return inner


def answer_bundle_from_interview(
    draft_questions: Mapping[str, Any],
    form_answers: Mapping[str, Any],
) -> dict[str, dict[str, str]]:
    """Build canonical ``{qid: {question, answer}}`` from WF1 draft + form fields."""
    raw_questions = draft_questions.get("questions")
    if not isinstance(raw_questions, list):
        raise SpecHitlError("draft_questions.output missing 'questions' list")
    by_id: dict[str, str] = {}
    for item in raw_questions:
        if not isinstance(item, dict):
            continue
        qid = item.get("id")
        text = item.get("text")
        if isinstance(qid, str) and isinstance(text, str):
            by_id[qid] = text
    bundle: dict[str, dict[str, str]] = {}
    for idx, qid in enumerate(QUESTION_IDS, start=1):
        field = f"answer_q{idx:02d}"
        ans = form_answers.get(field, "")
        if not isinstance(ans, str):
            ans = str(ans)
        bundle[qid] = {"question": by_id.get(qid, ""), "answer": ans}
    return bundle


def merge_answer_bundle(
    previous: Mapping[str, Mapping[str, str]],
    patched: Mapping[str, Any],
) -> dict[str, dict[str, str]]:
    """Merge patched answers; shallow-merge top-level question ids."""
    merged: dict[str, dict[str, str]] = {}
    for qid in QUESTION_IDS:
        base = previous.get(qid)
        if isinstance(base, dict):
            merged[qid] = {"question": str(base.get("question", "")), "answer": str(base.get("answer", ""))}
        else:
            merged[qid] = {"question": "", "answer": ""}
    for qid, payload in patched.items():
        if qid not in QUESTION_IDS:
            continue
        if not isinstance(payload, dict):
            continue
        question = payload.get("question", merged[qid]["question"])
        answer = payload.get("answer", merged[qid]["answer"])
        merged[qid] = {"question": str(question), "answer": str(answer)}
    return merged


def poll_human_form(human_request: Mapping[str, Any]) -> dict[str, Any]:
    """Prompt for structured JSON editing of a HITL form."""
    form_data = human_request.get("form_data")
    if not isinstance(form_data, dict):
        raise SpecHitlWorkflowError("human_request.form_data must be a dict")
    print(json.dumps(form_data, indent=2))
    raw = input("Paste updated JSON for the form (blank keeps defaults): ").strip()
    if not raw:
        return dict(form_data)
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise SpecHitlError("Invalid JSON for form response") from exc
    if not isinstance(parsed, dict):
        raise SpecHitlError("Form JSON must be an object")
    return parsed


def poll_human_text(human_request: Mapping[str, Any]) -> str:
    prompt = human_request.get("prompt") or ""
    print(prompt)
    return input("Your clarification (single-line text): ").strip()


def run_workflow_until_complete(
    client: SimpleAgentsClient,
    req: WorkflowExecutionRequest,
    *,
    phase: str,
) -> WorkflowRunOutputWire:
    """Drain ``awaiting_human_input`` until ``completed``."""
    def launch() -> WorkflowRunOutputWire:
        return client.run_workflow(req).to_dict()

    out = _retry_call(launch)
    while out.get("status") == "awaiting_human_input":
        human_request = out.get("human_request") or {}
        input_type = human_request.get("input_type")
        node_id = human_request.get("node_id")
        logger.info("hitl_pause", extra={"phase": phase, "node_id": node_id, "input_type": input_type})
        if input_type == "form":
            human_response = poll_human_form(human_request)
        elif input_type == "text":
            human_response = poll_human_text(human_request)
        else:
            raise SpecHitlWorkflowError(f"unsupported human_input type {input_type!r}")
        resume_req = WorkflowExecutionRequest(
            workflow_path=req.workflow_path,
            input=req.input,
            messages=[],
            resume=out,
            human_response=human_response,
        )
        out = _retry_call(lambda: client.run_workflow(resume_req).to_dict())
    _expect_completed(out, phase=phase)
    return out


def build_follow_up_instruction(
    *,
    concerns: list[str],
    unclear_question_ids: list[str],
    iteration_index: int,
    validator_notes: list[str],
) -> str:
    lines = [
        f"Clarification round {iteration_index + 1}.",
        "Improve answers **only** for the listed question ids; your text should reference those ids explicitly.",
        f"Focus ids: {', '.join(unclear_question_ids) if unclear_question_ids else '(none — still explain gaps)'}",
    ]
    if concerns:
        lines.append("Outstanding concerns:")
        lines.extend(f"- {c}" for c in concerns)
    if validator_notes:
        lines.append("Notes from the last automated clarity check:")
        lines.extend(f"- {n}" for n in validator_notes)
    lines.append("Provide concise, implementation-ready wording.")
    return "\n".join(lines)


def run_spec_hitl_pipeline(
    client: SimpleAgentsClient,
    *,
    spec_text: str,
    bundle_version: str = "1",
    max_clarify_rounds: int = 5,
    artifact_dir: Path | None = None,
) -> dict[str, Any]:
    """Run WF1 → optional WF2 loop → WF3; return structured orchestration results."""
    if max_clarify_rounds < 0:
        raise ValueError("max_clarify_rounds must be non-negative")

    run_id = str(uuid.uuid4())
    if artifact_dir is not None:
        artifact_dir.mkdir(parents=True, exist_ok=True)

    stub_messages = [
        WorkflowMessage(
            role=WorkflowRole.USER,
            content=[
                {
                    "type": "text",
                    "text": (
                        "Spec review pipeline user stub.\n\n"
                        f"bundle_version={bundle_version}\n\n"
                        "--- SPEC START ---\n"
                        f"{spec_text}\n"
                        "--- SPEC END ---"
                    ),
                }
            ],
        )
    ]

    wf1_input: dict[str, Any] = {"spec_text": spec_text, "bundle_version": bundle_version}
    wf1_req = WorkflowExecutionRequest(
        workflow_path=str(WF_INTERVIEW),
        input=wf1_input,
        messages=stub_messages,
    )
    logger.info("wf1_start", extra={"workflow_id": WF_INTERVIEW.name})
    wf1_run = run_workflow_until_complete(client, wf1_req, phase="wf1_interview")

    draft = extract_node_output(wf1_run, "draft_questions")
    form_answers = extract_node_output(wf1_run, "collect_initial_answers")
    review = extract_node_output(wf1_run, "review_answers")

    artifact: dict[str, Any] = {
        "run_id": run_id,
        "bundle_version": bundle_version,
        "spec_text": spec_text,
        "review_spec": extract_node_output(wf1_run, "review_spec"),
        "questions": draft,
        "answers": answer_bundle_from_interview(draft, form_answers),
        "needs_clarification": review["needs_clarification"],
        "concerns": list(review.get("concerns") or []),
        "unclear_question_ids": list(review.get("unclear_question_ids") or []),
        "feedback_markdown": review.get("feedback_markdown"),
        "review_summary": review.get("review_summary"),
    }

    if artifact_dir is not None:
        (artifact_dir / f"{run_id}-wf1-artifact.json").write_text(
            json.dumps({"wf1_run": wf1_run, "artifact": artifact}, indent=2),
            encoding="utf-8",
        )

    questions_json = json.dumps(draft.get("questions"), ensure_ascii=False)
    validator_notes: list[str] = []

    if artifact["needs_clarification"] == "yes" and max_clarify_rounds > 0:
        for attempt in range(max_clarify_rounds):
            follow_instruction = build_follow_up_instruction(
                concerns=artifact["concerns"],
                unclear_question_ids=artifact["unclear_question_ids"],
                iteration_index=attempt,
                validator_notes=validator_notes,
            )
            wf2_input: dict[str, Any] = {
                "spec_text": spec_text,
                "bundle_version": bundle_version,
                "questions_json": questions_json,
                "answer_bundle_json": json.dumps(artifact["answers"], ensure_ascii=False),
                "concerns_json": json.dumps(artifact["concerns"], ensure_ascii=False),
                "unclear_question_ids_json": json.dumps(artifact["unclear_question_ids"], ensure_ascii=False),
                "iteration_index": str(attempt),
                "max_clarify_rounds": str(max_clarify_rounds),
                "follow_up_instruction": follow_instruction,
            }
            wf2_req = WorkflowExecutionRequest(
                workflow_path=str(WF_CLARIFY),
                input=wf2_input,
                messages=stub_messages,
            )
            logger.info("wf2_round_start", extra={"attempt": attempt})
            wf2_run = run_workflow_until_complete(client, wf2_req, phase="wf2_clarify")

            patched_obj = extract_node_output(wf2_run, "patch_bundle")
            validation_obj = extract_node_output(wf2_run, "validate_subset")

            patched_bundle = patched_obj.get("patched_answer_bundle")
            if not isinstance(patched_bundle, dict):
                raise SpecHitlWorkflowError("patch_bundle missing patched_answer_bundle object")

            artifact["answers"] = merge_answer_bundle(artifact["answers"], patched_bundle)

            gate = validation_obj.get("clarity_gate")
            validator_notes = list(validation_obj.get("notes_for_human") or [])
            artifact["last_clarity_gate"] = gate
            artifact["last_validator_notes"] = validator_notes

            if artifact_dir is not None:
                (artifact_dir / f"{run_id}-wf2-attempt-{attempt}.json").write_text(
                    json.dumps({"wf2_run": wf2_run, "artifact_snapshot": dict(artifact)}, indent=2),
                    encoding="utf-8",
                )

            if gate == "clear":
                artifact["clarify_outcome"] = "clear"
                break
            if gate == "abort":
                artifact["clarify_outcome"] = "abort"
                break
        else:
            artifact["clarify_outcome"] = artifact.get("clarify_outcome", "incomplete_max_rounds")
    else:
        artifact["clarify_outcome"] = "skipped"

    wf3_input = {
        "spec_text": spec_text,
        "bundle_version": bundle_version,
        "final_question_bundle_json": questions_json,
        "final_answer_bundle_json": json.dumps(artifact["answers"], ensure_ascii=False),
    }
    wf3_req = WorkflowExecutionRequest(
        workflow_path=str(WF_USEFULNESS),
        input=wf3_input,
        messages=stub_messages,
    )
    logger.info("wf3_start")
    wf3_run = run_workflow_until_complete(client, wf3_req, phase="wf3_usefulness")
    usefulness = extract_node_output(wf3_run, "assess_fit")

    if artifact_dir is not None:
        (artifact_dir / f"{run_id}-wf3-final.json").write_text(
            json.dumps({"wf3_run": wf3_run, "artifact": artifact, "usefulness": usefulness}, indent=2),
            encoding="utf-8",
        )

    return {"artifact": artifact, "usefulness": usefulness, "runs": {"wf1": wf1_run, "wf3": wf3_run}}


def load_spec_file(path: Path) -> str:
    if not path.is_file():
        raise SpecHitlError(f"Spec path is not a file: {path}")
    return path.read_text(encoding="utf-8")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run spec HITL workflow pipeline (Python orchestrator).")
    parser.add_argument(
        "spec_path",
        type=Path,
        help="Path to a markdown/text specification file.",
    )
    parser.add_argument(
        "--max-clarify-rounds",
        type=int,
        default=5,
        help="Deterministic cap for WF2 host-driven clarification loops.",
    )
    parser.add_argument(
        "--artifact-dir",
        type=Path,
        default=None,
        help="Optional directory to persist JSON snapshots after each phase.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
    args = parse_args(sys.argv[1:] if argv is None else argv)

    client = SimpleAgentsClient(
        require_env("WORKFLOW_PROVIDER"),
        api_base=require_env("WORKFLOW_API_BASE"),
        api_key=require_env("WORKFLOW_API_KEY"),
    )

    spec_text = load_spec_file(args.spec_path.resolve())
    try:
        result = run_spec_hitl_pipeline(
            client,
            spec_text=spec_text,
            max_clarify_rounds=args.max_clarify_rounds,
            artifact_dir=args.artifact_dir.resolve() if args.artifact_dir else None,
        )
    except SpecHitlError as exc:
        logger.error("pipeline_failed", extra={"error": str(exc)})
        raise SystemExit(1) from exc

    print(json.dumps(result, indent=2, default=str))


if __name__ == "__main__":
    main()
