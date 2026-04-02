import argparse
import asyncio
import json
import pathlib
import tempfile
from concurrent import futures

import grpc
from grpc_tools import protoc


PROTO_PATH = (
    pathlib.Path(__file__).resolve().parents[2]
    / "crates"
    / "simple-agents-workflow-workers"
    / "proto"
    / "worker.proto"
)


def load_generated_modules():
    out_dir = tempfile.mkdtemp(prefix="workflow-worker-proto-")
    result = protoc.main(
        [
            "grpc_tools.protoc",
            f"-I{PROTO_PATH.parent}",
            f"--python_out={out_dir}",
            f"--grpc_python_out={out_dir}",
            str(PROTO_PATH),
        ]
    )
    if result != 0:
        raise RuntimeError("failed to generate python grpc contracts")

    import sys

    sys.path.insert(0, out_dir)
    import worker_pb2  # type: ignore
    import worker_pb2_grpc  # type: ignore

    return worker_pb2, worker_pb2_grpc


worker_pb2, worker_pb2_grpc = load_generated_modules()


def _parse_json_text(raw: str):
    if raw == "":
        return {}
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return {"raw": raw}


def _parse_execute_payload(request):
    if request.operation == "llm" and request.HasField("llm_payload"):
        return {
            "prompt": request.llm_payload.prompt,
            "scoped_input": _parse_json_text(request.llm_payload.scoped_input_json),
        }
    if request.operation == "tool" and request.HasField("tool_payload"):
        return {
            "input": _parse_json_text(request.tool_payload.input_json),
            "scoped_input": _parse_json_text(request.tool_payload.scoped_input_json),
        }
    return _parse_json_text(request.payload_json)


class WorkerService(worker_pb2_grpc.WorkerServiceServicer):
    def __init__(self, worker_id: str):
        self.worker_id = worker_id

    async def Health(self, _request, _context):
        return worker_pb2.HealthResponse(
            worker_id=self.worker_id,
            status=worker_pb2.HealthStatus.HEALTH_STATUS_SERVING,
            consecutive_failures=0,
        )

    async def Execute(self, request, _context):
        if request.target == "fail":
            return worker_pb2.ExecuteResponse(
                request_id=request.request_id,
                worker_id=self.worker_id,
                ok=False,
                elapsed_ms=1,
                error=worker_pb2.WorkerError(
                    code="execution_failed",
                    message="forced failure",
                    retryable=False,
                ),
            )

        payload = _parse_execute_payload(request)
        if request.operation == "GetRagData":
            topic = payload.get("topic", "clarification")
            if topic == "terminated":
                output = {
                    "handler": "GetRagData",
                    "topic": topic,
                    "decision": "terminated",
                    "message": "Interview terminated due to process/policy violation.",
                }
            elif topic == "already_terminated":
                output = {
                    "handler": "GetRagData",
                    "topic": topic,
                    "decision": "terminated",
                    "message": "Interview session is already terminated.",
                }
            else:
                output = {
                    "handler": "GetRagData",
                    "topic": topic,
                    "decision": "continue",
                    "message": "Provide the next interview question based on candidate progress.",
                }
        else:
            output = {
                "language": "python",
                "worker_id": self.worker_id,
                "operation": request.operation,
                "target": request.target,
                "payload": payload,
            }
        return worker_pb2.ExecuteResponse(
            request_id=request.request_id,
            worker_id=self.worker_id,
            ok=True,
            elapsed_ms=1,
            output_json=json.dumps(output),
        )


async def serve(listen_addr: str, worker_id: str):
    server = grpc.aio.server(futures.ThreadPoolExecutor(max_workers=4))
    worker_pb2_grpc.add_WorkerServiceServicer_to_server(
        WorkerService(worker_id), server
    )
    server.add_insecure_port(listen_addr)
    await server.start()
    print(f"python worker listening on {listen_addr}")
    await server.wait_for_termination()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--listen", default="127.0.0.1:50061")
    parser.add_argument("--worker-id", default="python-0")
    args = parser.parse_args()
    asyncio.run(serve(args.listen, args.worker_id))


if __name__ == "__main__":
    main()
