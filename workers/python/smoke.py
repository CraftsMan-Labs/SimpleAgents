import asyncio
import json

import grpc

import worker


async def main():
    listen = "127.0.0.1:50081"
    server = grpc.aio.server()
    worker.worker_pb2_grpc.add_WorkerServiceServicer_to_server(
        worker.WorkerService("python-smoke"),
        server,
    )
    server.add_insecure_port(listen)
    await server.start()

    async with grpc.aio.insecure_channel(listen) as channel:
        stub = worker.worker_pb2_grpc.WorkerServiceStub(channel)
        health = await stub.Health(worker.worker_pb2.HealthRequest())
        assert health.worker_id == "python-smoke"

        response = await stub.Execute(
            worker.worker_pb2.ExecuteRequest(
                request_id="smoke-1",
                workflow_name="wf",
                node_id="tool-1",
                operation="tool",
                target="echo",
                payload_json=json.dumps({"input": {"x": 1}}),
            )
        )
        assert response.ok
        decoded = json.loads(response.output_json)
        assert decoded["language"] == "python"

    await server.stop(0)
    print("python smoke ok")


if __name__ == "__main__":
    asyncio.run(main())
