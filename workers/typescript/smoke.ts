import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { startWorker } from "./worker.js";

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
});
const proto = grpc.loadPackageDefinition(packageDefinition) as any;

async function unaryCall(client: any, method: string, payload: any): Promise<any> {
  return new Promise((resolve, reject) => {
    client[method](payload, (error: Error | null, response: any) => {
      if (error) {
        reject(error);
        return;
      }
      resolve(response);
    });
  });
}

async function runSmoke() {
  const server = startWorker("127.0.0.1:50083");
  const client = new proto.workflow.worker.v1.WorkerService(
    "127.0.0.1:50083",
    grpc.credentials.createInsecure(),
  );

  const health = await unaryCall(client, "Health", {});
  if (health.worker_id !== "typescript-0") {
    throw new Error("unexpected worker_id");
  }

  const response = await unaryCall(client, "Execute", {
    request_id: "smoke-1",
    workflow_name: "wf",
    node_id: "node-1",
    operation: "tool",
    target: "echo",
    payload_json: JSON.stringify({ input: { x: 1 } }),
  });

  if (!response.ok) {
    throw new Error("expected ok execute response");
  }

  client.close();
  await new Promise<void>((resolve, reject) => {
    server.tryShutdown((error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });
  console.log("typescript smoke ok");
}

runSmoke().catch((error) => {
  console.error(error);
  process.exit(1);
});
