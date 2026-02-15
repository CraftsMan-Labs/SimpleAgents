import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const protoPath = path.resolve(
  __dirname,
  "../../crates/simple-agents-workflow-workers/proto/worker.proto",
);

const packageDefinition = protoLoader.loadSync(protoPath, {
  keepCase: true,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true,
});
const proto = grpc.loadPackageDefinition(packageDefinition) as any;

type ExecuteRequest = {
  request_id: string;
  operation: string;
  target: string;
  payload_json: string;
};

function execute(call: grpc.ServerUnaryCall<ExecuteRequest, any>, callback: grpc.sendUnaryData<any>) {
  const request = call.request;
  if (request.target === "fail") {
    callback(null, {
      request_id: request.request_id,
      worker_id: "typescript-0",
      elapsed_ms: "1",
      ok: false,
      error: {
        code: "execution_failed",
        message: "forced failure",
        retryable: false,
      },
    });
    return;
  }

  const payload = request.payload_json ? JSON.parse(request.payload_json) : {};
  callback(null, {
    request_id: request.request_id,
    worker_id: "typescript-0",
    elapsed_ms: "1",
    ok: true,
    output_json: JSON.stringify({
      language: "typescript",
      worker_id: "typescript-0",
      operation: request.operation,
      target: request.target,
      payload,
    }),
  });
}

function health(_call: grpc.ServerUnaryCall<any, any>, callback: grpc.sendUnaryData<any>) {
  callback(null, {
    worker_id: "typescript-0",
    status: "HEALTH_STATUS_SERVING",
    consecutive_failures: 0,
  });
}

export function startWorker(address = "127.0.0.1:50063") {
  const server = new grpc.Server();
  server.addService(proto.workflow.worker.v1.WorkerService.service, {
    Execute: execute,
    Health: health,
  });

  server.bindAsync(address, grpc.ServerCredentials.createInsecure(), (error) => {
    if (error) {
      throw error;
    }
    server.start();
    console.log(`typescript worker listening on ${address}`);
  });

  return server;
}

if (process.argv[1] === __filename) {
  startWorker();
}
