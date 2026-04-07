/**
 * Stream a YAML workflow with live events.
 *
 * Parity with `examples/python-test-simpleAgents/test-py-simple-agents-streaming.py`.
 * LLM nodes in the YAML should use `stream: true` if you want token deltas.
 *
 * From repo root `examples/`: `bun install` in this directory.
 *
 * Env: `WORKFLOW_API_KEY` (required), `WORKFLOW_API_BASE` (optional).
 */

import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { Client } from "simple-agents-node";
import { parseWorkflowEvent } from "simple-agents-node/workflow_event";
import { customWorkerDispatch } from "./handlers.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const workflowPath = join(__dirname, "test.yaml");
const DEBUG_ENDPOINT = "http://127.0.0.1:7242/ingest/21d1cf96-9491-4f21-a7aa-0a8ff58de124";
const DEBUG_RUN_ID = "napi-stream-debug-1";
let debugEventCount = 0;
const DEBUG_EVENT_LIMIT = 120;

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

/** Prints live token deltas; snapshots/lifecycle are still debug-instrumented. */
function onWorkflowEvent(err: unknown, eventJson: string): void {
  if (err) {
    console.error(err);
    // #region agent log
    fetch(DEBUG_ENDPOINT, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        runId: DEBUG_RUN_ID,
        hypothesisId: "H2",
        location: "test-simple-agents-streaming.ts:onWorkflowEvent(err)",
        message: "Event callback error",
        data: { error: String(err) },
        timestamp: Date.now(),
      }),
    }).catch(() => {});
    // #endregion
    return;
  }
  const event = parseWorkflowEvent(eventJson) as unknown as Record<string, unknown>;
  const eventType = typeof event.event_type === "string" ? event.event_type : "unknown";
  const nodeId = typeof event.node_id === "string" ? event.node_id : null;
  const stepId = typeof event.step_id === "string" ? event.step_id : null;
  const delta = typeof event.delta === "string" ? event.delta : null;
  const metadata =
    event.metadata && typeof event.metadata === "object"
      ? (event.metadata as Record<string, unknown>)
      : null;
  const isComplete =
    metadata && typeof metadata.is_complete === "boolean" ? metadata.is_complete : null;
  if (debugEventCount < DEBUG_EVENT_LIMIT) {
    debugEventCount += 1;
    // #region agent log
    fetch(DEBUG_ENDPOINT, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        runId: DEBUG_RUN_ID,
        hypothesisId: "H1,H3,H4",
        location: "test-simple-agents-streaming.ts:onWorkflowEvent",
        message: "Received stream event",
        data: {
          eventCount: debugEventCount,
          eventType,
          nodeId,
          stepId,
          deltaLen: delta ? delta.length : 0,
          deltaPreview: delta ? delta.slice(0, 40) : null,
          isComplete,
        },
        timestamp: Date.now(),
      }),
    }).catch(() => {});
    // #endregion
  }
  if (eventType === "node_stream_delta" && typeof delta === "string") {
    process.stdout.write(delta);
    return;
  }
  if (eventType === "node_stream_snapshot") {
    if (isComplete === true) {
      process.stdout.write("\n");
    }
    return;
  }
}

async function main(): Promise<void> {
  const apiKey = requireEnv("WORKFLOW_API_KEY");
  const baseUrl = process.env.WORKFLOW_API_BASE || undefined;

  const rl = readline.createInterface({ input, output });
  const userInput = await rl.question("Enter your Input: ");
  rl.close();

  const client = new Client(apiKey, baseUrl);
  const executionFlags = {
    nodeLlmStreaming: true,
    splitStreamDeltas: false,
  };
  // #region agent log
  fetch(DEBUG_ENDPOINT, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      runId: DEBUG_RUN_ID,
      hypothesisId: "H1",
      location: "test-simple-agents-streaming.ts:main(beforeStreamWorkflow)",
      message: "Invoking streamWorkflow with execution flags",
      data: {
        workflowPath,
        hasBaseUrl: Boolean(baseUrl),
        executionFlags,
      },
      timestamp: Date.now(),
    }),
  }).catch(() => {});
  // #endregion
  const result = await client.streamWorkflow(
    workflowPath,
    { messages: [{ role: "user", content: userInput }] },
    onWorkflowEvent,
    undefined,
    executionFlags,
    customWorkerDispatch,
  );
  // #region agent log
  fetch(DEBUG_ENDPOINT, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      runId: DEBUG_RUN_ID,
      hypothesisId: "H2,H4",
      location: "test-simple-agents-streaming.ts:main(afterStreamWorkflow)",
      message: "streamWorkflow completed",
      data: {
        debugEventCount,
        resultType: typeof result,
      },
      timestamp: Date.now(),
    }),
  }).catch(() => {});
  // #endregion

  console.log("\n");
  console.log(JSON.stringify(result, null, 2));
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
