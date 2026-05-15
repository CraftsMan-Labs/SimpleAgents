"""Three-phase spec HITL pipeline: interview YAML → optional clarify loop → usefulness YAML.
The printed ``result`` includes ``runs``: ``wf1_interview``, ``wf2_clarify`` (null when clarification
is skipped), and ``wf3_usefulness``. Every human pause uses the same ``--- Human input: … ---`` banner.

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

# Human-input node ids (must match spec-hitl YAML). WF3 has no HITL.
NODE_COLLECT_INITIAL_ANSWERS = "collect_initial_answers"
NODE_COLLECT_FOLLOW_UP = "collect_follow_up"

# Expected human_input shape per phase (warn if YAML drifts).
_EXPECTED_HITL_BY_PHASE: dict[str, tuple[str, str]] = {
    "wf1_interview": (NODE_COLLECT_INITIAL_ANSWERS, "form"),
    "wf2_clarify": (NODE_COLLECT_FOLLOW_UP, "text"),
}


def _hitl_section_title(kind: str) -> None:
    """Print a visible banner so every pause uses the same pattern."""
    print(f"\n--- Human input: {kind} ---\n")


def _assistant_section(title: str) -> None:
    """Print a consistent heading for model-generated guidance."""
    print(f"\n=== Assistant: {title} ===")


def _print_bullets(items: list[str]) -> None:
    for item in items:
        print(f"- {item}")


def _safe_list_of_str(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    out: list[str] = []
    for item in value:
        if isinstance(item, str):
            cleaned = item.strip()
            if cleaned:
                out.append(cleaned)
        elif item is not None:
            out.append(str(item))
    return out


def _safe_answer_bundle(value: Any) -> dict[str, dict[str, str]]:
    if not isinstance(value, dict):
        return {}
    out: dict[str, dict[str, str]] = {}
    for qid, payload in value.items():
        if not isinstance(qid, str) or qid not in QUESTION_IDS or not isinstance(payload, dict):
            continue
        out[qid] = {
            "question": str(payload.get("question", "")),
            "answer": str(payload.get("answer", "")),
        }
    return out


def _coerce_form_data(blob: Any) -> dict[str, Any]:
    """Normalize human_request.form_data into a dict."""
    if blob is None:
        return {}
    if isinstance(blob, str):
        try:
            parsed = json.loads(blob)
        except json.JSONDecodeError as exc:
            raise SpecHitlWorkflowError("human_request.form_data string is not valid JSON") from exc
        if not isinstance(parsed, dict):
            raise SpecHitlWorkflowError("human_request.form_data JSON must deserialize to an object")
        return parsed
    if isinstance(blob, dict):
        return blob
    raise SpecHitlWorkflowError(
        f"human_request.form_data must be a dict, null, or JSON string, got {type(blob).__name__}"
    )


def _write_json_if_dir(
    artifact_dir: Path | None,
    filename: str,
    payload: Mapping[str, Any],
) -> None:
    if artifact_dir is None:
        return
    (artifact_dir / filename).write_text(
        json.dumps(payload, indent=2),
        encoding="utf-8",
    )


def _build_stub_messages(spec_text: str, *, bundle_version: str) -> list[WorkflowMessage]:
    return [
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


def _print_interview_review(artifact: Mapping[str, Any]) -> None:
    _assistant_section("Interview review")
    review_summary = str(artifact.get("review_summary") or "").strip()
    if review_summary:
        print(review_summary)
    feedback = str(artifact.get("feedback_markdown") or "").strip()
    if feedback:
        print("\nFeedback:")
        print(feedback)
    unclear_ids = artifact.get("unclear_question_ids")
    if isinstance(unclear_ids, list) and unclear_ids:
        unclear_ids_text = ", ".join(str(qid) for qid in unclear_ids)
        print(f"\nNeeds clarification for ids: {unclear_ids_text}")


def _build_wf2_input(
    *,
    spec_text: str,
    bundle_version: str,
    questions_json: str,
    answers: Mapping[str, Any],
    concerns: list[str],
    unclear_question_ids: list[str],
    iteration_index: int,
    max_clarify_rounds: int,
    follow_up_instruction: str,
) -> dict[str, Any]:
    return {
        "spec_text": spec_text,
        "bundle_version": bundle_version,
        "questions_json": questions_json,
        "answer_bundle_json": json.dumps(answers, ensure_ascii=False),
        "concerns_json": json.dumps(concerns, ensure_ascii=False),
        "unclear_question_ids_json": json.dumps(unclear_question_ids, ensure_ascii=False),
        "iteration_index": str(iteration_index),
        "max_clarify_rounds": str(max_clarify_rounds),
        "follow_up_instruction": follow_up_instruction,
    }


def _build_wf3_input(
    *,
    spec_text: str,
    bundle_version: str,
    questions_json: str,
    answers: Mapping[str, Any],
) -> dict[str, str]:
    return {
        "spec_text": spec_text,
        "bundle_version": bundle_version,
        "final_question_bundle_json": questions_json,
        "final_answer_bundle_json": json.dumps(answers, ensure_ascii=False),
    }


def _read_multiline_until_blank() -> str:
    """Read zero or more non-empty lines; empty line ends input."""
    lines: list[str] = []
    while True:
        line = input()
        if not line:
            break
        lines.append(line)
    return "\n".join(lines).strip()


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
                "workflow_call_failed_retrying attempt=%s/%s delay_s=%.2f: %s: %s",
                attempt + 1,
                attempts,
                delay,
                type(exc).__name__,
                exc,
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


def _try_parse_hitl_questions_block(prompt: str) -> tuple[str, dict[str, str]] | None:
    """If *prompt* has a ``Questions (JSON):`` section with ``[{\"id\",\"text\"}, ...]``, return intro and id->text."""
    marker = "Questions (JSON):"
    idx = prompt.find(marker)
    if idx == -1:
        return None
    intro = prompt[:idx].strip()
    tail = prompt[idx + len(marker) :].strip()
    try:
        data = json.loads(tail)
    except json.JSONDecodeError:
        return None
    if not isinstance(data, list):
        return None
    by_id: dict[str, str] = {}
    for item in data:
        if isinstance(item, dict):
            qid = item.get("id")
            text = item.get("text")
            if isinstance(qid, str) and isinstance(text, str):
                by_id[qid] = text
    if not any(q in by_id for q in QUESTION_IDS):
        return None
    return intro, by_id


def poll_human_form(human_request: Mapping[str, Any]) -> dict[str, Any]:
    """Prompt for structured form answers: per-question prompts when questions are embedded in the prompt."""
    _hitl_section_title("interview (form)")
    form_data = _coerce_form_data(human_request.get("form_data"))
    prompt_raw = human_request.get("prompt")
    prompt_str_full = prompt_raw.strip() if isinstance(prompt_raw, str) else ""

    parsed_block = _try_parse_hitl_questions_block(prompt_str_full) if prompt_str_full else None
    if parsed_block is not None:
        intro, by_qid = parsed_block
        print(
            "Answer each question below. Defaults come from the form preview (often empty).\n"
            "Press Enter on a line to accept the default when shown.\n"
        )
        if intro:
            print(intro)
            print()
        out: dict[str, Any] = {}
        for qid in QUESTION_IDS:
            field = f"answer_{qid}"
            qtext = (by_qid.get(qid) or "").strip() or f"Provide an answer for {qid}."
            default_val = form_data.get(field, "")
            if not isinstance(default_val, str):
                default_val = str(default_val)
            print(f"--- [{qid}] ---\n{qtext}\n")
            hint = f" (default: {default_val})" if default_val else ""
            line = input(f"Your answer{hint}: ").strip()
            out[field] = line if line else default_val
        return out

    print(
        "This form does not use the standard `Questions (JSON):` block.\n"
        "Review the prompt and current JSON below, then paste a full JSON object for the form.\n"
    )
    if prompt_str_full:
        print(prompt_str_full)
        print()
    print("Current form_data:")
    print(json.dumps(form_data, indent=2))
    pasted = input("\nPaste updated JSON for the form (blank keeps defaults): ").strip()
    if not pasted:
        return dict(form_data)
    try:
        parsed = json.loads(pasted)
    except json.JSONDecodeError as exc:
        raise SpecHitlError("Invalid JSON for form response") from exc
    if not isinstance(parsed, dict):
        raise SpecHitlError("Form JSON must be an object")
    return parsed


def poll_human_text(
    human_request: Mapping[str, Any],
    *,
    hitl_context: Mapping[str, Any] | None = None,
) -> str:
    """Collect free-text HITL. This pipeline only uses text for WF2 follow-up (multi-line)."""
    ctx = dict(hitl_context or {})
    clarify_round = int(ctx["clarify_round"]) if ctx.get("clarify_round") is not None else 1
    clarify_max = ctx.get("clarify_max")
    clarify_max_i = int(clarify_max) if clarify_max is not None else None
    unclear_ids = _safe_list_of_str(ctx.get("unclear_question_ids"))
    concerns = _safe_list_of_str(ctx.get("concerns"))
    validator_notes = _safe_list_of_str(ctx.get("validator_notes"))
    answer_bundle = _safe_answer_bundle(ctx.get("answer_bundle"))

    _hitl_section_title("clarification (free text)")
    _assistant_section("Clarification request")
    if unclear_ids:
        print(f"Focus question ids: {', '.join(unclear_ids)}")
    else:
        print("Focus question ids: (none provided)")
    if concerns:
        print("\nOpen concerns from the last review:")
        _print_bullets(concerns)
    if validator_notes:
        print("\nNotes from automated clarity checks:")
        _print_bullets(validator_notes)
    if unclear_ids and answer_bundle:
        print("\nCurrent answers for focus ids:")
        for qid in unclear_ids:
            row = answer_bundle.get(qid)
            if not row:
                continue
            question = row.get("question", "").strip() or "(missing question text)"
            answer = row.get("answer", "").strip() or "(currently blank)"
            print(f"- {qid}: {question}")
            print(f"  current answer: {answer}")

    prompt_raw = human_request.get("prompt")
    prompt = prompt_raw.strip() if isinstance(prompt_raw, str) else ""
    if not concerns and not unclear_ids and prompt:
        print("\nRaw workflow prompt:")
        print(prompt)

    if clarify_round <= 1:
        print(
            "\n--- What to type ---\n"
            "You are adding **follow-up detail** so the model can rewrite weak answers in the bundle.\n"
            "Address the **focus question ids** and **concerns** from the prompt (e.g. q01: …, q02: …).\n"
            "Be concrete and implementation-ready—state what you wish your first answers had said.\n"
            "\n"
            "Enter one or more lines; **press Enter on an empty line to finish**.\n"
        )
    else:
        cap = f"/{clarify_max_i}" if clarify_max_i is not None else ""
        print(
            f"\n--- Follow-up round {clarify_round}{cap} ---\n"
            "The clarity check requested **retry**, so this is another chance to add detail.\n"
            "Use the same format as before (cite question ids, multi-line is OK). **Blank line to finish**.\n"
        )
    return _read_multiline_until_blank()


def run_workflow_until_complete(
    client: SimpleAgentsClient,
    req: WorkflowExecutionRequest,
    *,
    phase: str,
    hitl_context: Mapping[str, Any] | None = None,
) -> WorkflowRunOutputWire:
    """Drain ``awaiting_human_input`` until ``completed``.

    *hitl_context* is passed to text HITL helpers (e.g. ``clarify_round`` / ``clarify_max`` for WF2).
    """
    hitl_ctx = dict(hitl_context or {})
    def launch() -> WorkflowRunOutputWire:
        return client.run_workflow(req).to_dict()

    out = _retry_call(launch)
    while out.get("status") == "awaiting_human_input":
        human_request = out.get("human_request") or {}
        input_type = human_request.get("input_type")
        node_id = human_request.get("node_id")
        expected = _EXPECTED_HITL_BY_PHASE.get(phase)
        if expected is not None:
            exp_node, exp_type = expected
            if node_id != exp_node or input_type != exp_type:
                logger.warning(
                    "hitl_pause phase=%s expected node_id=%s input_type=%s; got node_id=%s input_type=%s "
                    "(if spec-hitl YAML changed, update constants in %s)",
                    phase,
                    exp_node,
                    exp_type,
                    node_id,
                    input_type,
                    Path(__file__).name,
                )
        logger.info(
            "hitl_pause phase=%s node_id=%s input_type=%s",
            phase,
            node_id,
            input_type,
        )
        if input_type == "form":
            human_response = poll_human_form(human_request)
        elif input_type == "text":
            human_response = poll_human_text(human_request, hitl_context=hitl_ctx)
        else:
            raise SpecHitlWorkflowError(f"unsupported human_input type {input_type!r}")
        resume_req = WorkflowExecutionRequest(
            workflow_path=req.workflow_path,
            input=req.input,
            # Same merge as initial run: ``messages_path: input.messages`` requires this on every call.
            messages=list(req.messages),
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

    stub_messages = _build_stub_messages(spec_text, bundle_version=bundle_version)

    wf1_input: dict[str, Any] = {"spec_text": spec_text, "bundle_version": bundle_version}
    wf1_req = WorkflowExecutionRequest(
        workflow_path=str(WF_INTERVIEW),
        input=wf1_input,
        messages=stub_messages,
    )
    logger.info("wf1_start workflow=%s", WF_INTERVIEW.name)
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

    _write_json_if_dir(
        artifact_dir,
        f"{run_id}-wf1-artifact.json",
        {"wf1_run": wf1_run, "artifact": artifact},
    )

    questions_json = json.dumps(draft.get("questions"), ensure_ascii=False)
    validator_notes: list[str] = []
    wf2_run_final: WorkflowRunOutputWire | None = None

    if artifact["needs_clarification"] == "yes" and max_clarify_rounds > 0:
        _print_interview_review(artifact)
        for attempt in range(max_clarify_rounds):
            follow_instruction = build_follow_up_instruction(
                concerns=artifact["concerns"],
                unclear_question_ids=artifact["unclear_question_ids"],
                iteration_index=attempt,
                validator_notes=validator_notes,
            )
            wf2_input = _build_wf2_input(
                spec_text=spec_text,
                bundle_version=bundle_version,
                questions_json=questions_json,
                answers=artifact["answers"],
                concerns=artifact["concerns"],
                unclear_question_ids=artifact["unclear_question_ids"],
                iteration_index=attempt,
                max_clarify_rounds=max_clarify_rounds,
                follow_up_instruction=follow_instruction,
            )
            wf2_req = WorkflowExecutionRequest(
                workflow_path=str(WF_CLARIFY),
                input=wf2_input,
                messages=stub_messages,
            )
            logger.info("wf2_round_start attempt=%s/%s", attempt + 1, max_clarify_rounds)
            wf2_run = run_workflow_until_complete(
                client,
                wf2_req,
                phase="wf2_clarify",
                hitl_context={
                    "clarify_round": attempt + 1,
                    "clarify_max": max_clarify_rounds,
                    "unclear_question_ids": artifact["unclear_question_ids"],
                    "concerns": artifact["concerns"],
                    "validator_notes": validator_notes,
                    "answer_bundle": artifact["answers"],
                },
            )
            wf2_run_final = wf2_run

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

            _write_json_if_dir(
                artifact_dir,
                f"{run_id}-wf2-attempt-{attempt}.json",
                {"wf2_run": wf2_run, "artifact_snapshot": dict(artifact)},
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

    wf3_input = _build_wf3_input(
        spec_text=spec_text,
        bundle_version=bundle_version,
        questions_json=questions_json,
        answers=artifact["answers"],
    )
    wf3_req = WorkflowExecutionRequest(
        workflow_path=str(WF_USEFULNESS),
        input=wf3_input,
        messages=stub_messages,
    )
    logger.info("wf3_start workflow=%s", WF_USEFULNESS.name)
    wf3_run = run_workflow_until_complete(client, wf3_req, phase="wf3_usefulness")
    usefulness = extract_node_output(wf3_run, "assess_fit")

    _write_json_if_dir(
        artifact_dir,
        f"{run_id}-wf3-final.json",
        {"wf3_run": wf3_run, "artifact": artifact, "usefulness": usefulness},
    )

    return {
        "artifact": artifact,
        "usefulness": usefulness,
        "runs": {
            "wf1_interview": wf1_run,
            "wf2_clarify": wf2_run_final,
            "wf3_usefulness": wf3_run,
        },
    }


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
        logger.error("pipeline_failed: %s", exc)
        raise SystemExit(1) from exc

    print(json.dumps(result, indent=2, default=str))


if __name__ == "__main__":
    main()
