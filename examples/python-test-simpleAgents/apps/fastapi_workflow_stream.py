from __future__ import annotations

import asyncio
import json
import queue
import sys
import threading
from collections.abc import AsyncIterator, Mapping
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from example_env import require_env
from example_paths import workflows
from fastapi import FastAPI
from fastapi.responses import StreamingResponse
from pydantic import BaseModel, Field
from simple_agents_py import Client
from simple_agents_py.models import WorkflowRunOutputModel
from simple_agents_py.workflow_request import (
    WorkflowExecutionFlags,
    WorkflowExecutionRequest,
    WorkflowMessage,
    WorkflowRole,
)
from simple_agents_py.workflow_stream import merge_workflow_execution

WORKFLOW_FILE = workflows("email-classification", "test.yaml")
app = FastAPI(title="SimpleAgents workflow stream")


class ChatBody(BaseModel):
    message: str = Field(..., min_length=1)


class StreamResponse(BaseModel):
    data: str = Field(..., description="One SSE payload: JSON envelope or [DONE].")


def _client() -> Client:
    return Client(
        require_env("WORKFLOW_PROVIDER"),
        api_base=require_env("WORKFLOW_API_BASE"),
        api_key=require_env("WORKFLOW_API_KEY"),
    )


def _sse(data: Any) -> bytes:
    return f"data: {json.dumps(data, default=str)}\n\n".encode()


async def _workflow_sse(message: str) -> AsyncIterator[bytes]:
    req = WorkflowExecutionRequest(
        workflow_path=str(WORKFLOW_FILE),
        messages=[WorkflowMessage(role=WorkflowRole.USER, content=message)],
        execution=WorkflowExecutionFlags(
            node_llm_streaming=True,
            split_stream_deltas=True,
        ),
    )
    current_exec = req.execution
    if current_exec is not None:
        merged = merge_workflow_execution(
            current_exec.model_dump(mode="json", exclude_none=True)
        )
        req = req.model_copy(update={"execution": WorkflowExecutionFlags(**merged)})
    q: queue.Queue[tuple[str, Any]] = queue.Queue()

    def on_event(event: Mapping[str, Any]) -> None:
        q.put(("event", event))

    def worker() -> None:
        try:
            q.put(
                (
                    "result",
                    _client().stream_workflow(req, on_event=on_event).to_dict(),
                )
            )
        except Exception as e:
            q.put(("error", e))

    threading.Thread(target=worker, daemon=True).start()
    while True:
        kind, data = await asyncio.to_thread(q.get)
        if kind == "event":
            yield _sse({"workflow_event": data})
            continue
        if kind == "result":
            yield _sse({"workflow_result": data})
        else:
            yield _sse({"error": str(data), "error_type": type(data).__name__})
        yield b"data: [DONE]\n\n"
        return


@app.get("/health")
def health() -> dict[str, str]:
    return {"status": "ok"}


@app.post(
    "/chat/stream",
    response_class=StreamingResponse,
    responses={200: {"model": StreamResponse}},
)
async def chat_stream(body: ChatBody) -> StreamingResponse:
    return StreamingResponse(
        _workflow_sse(body.message),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "X-Accel-Buffering": "no",
        },
    )

# non streaming chat (use :class:`WorkflowRunOutputModel` for OpenAPI — wire shape is ``WorkflowRunOutputWire``)
@app.post("/chat", response_model=WorkflowRunOutputModel)
async def chat(body: ChatBody) -> WorkflowRunOutputModel:
    raw = _client().run_workflow(
        WorkflowExecutionRequest(
            workflow_path=str(WORKFLOW_FILE),
            messages=[WorkflowMessage(role=WorkflowRole.USER, content=body.message)],
        )
    )
    return WorkflowRunOutputModel.model_validate(raw.to_dict())
