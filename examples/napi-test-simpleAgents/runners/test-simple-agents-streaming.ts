/**
 * Stream a YAML workflow with live events.
 *
 * Parity with `examples/python-test-simpleAgents/runners/test-py-simple-agents-streaming.py`.
 * LLM nodes in the YAML should use `stream: true` if you want token deltas.
 *
 * From repo root `examples/`: `bun install` in this directory.
 *
 * Env: `WORKFLOW_API_KEY` (required), `WORKFLOW_API_BASE` (optional).
 */

import { config as loadEnv } from "dotenv";
import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { join } from "node:path";
import { Client } from "simple-agents-node";
import { parseWorkflowEvent } from "simple-agents-node/workflow_event";
import { PACKAGE_ROOT, pathToWorkflow } from "../example_paths.js";
import { customWorkerDispatch } from "../workflows/email-classification/handlers.js";

loadEnv({ path: join(PACKAGE_ROOT, ".env") });
const workflowPath = pathToWorkflow("email-classification", "test.yaml");

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

/** Prints live token deltas; newline after complete stream snapshots. */
function onWorkflowEvent(err: unknown, eventJson: string): void {
  if (err) {
    console.error(err);
    return;
  }
  const event = parseWorkflowEvent(eventJson) as unknown as Record<string, unknown>;
  const eventType = typeof event.event_type === "string" ? event.event_type : "unknown";
  const delta = typeof event.delta === "string" ? event.delta : null;
  const metadata =
    event.metadata && typeof event.metadata === "object"
      ? (event.metadata as Record<string, unknown>)
      : null;
  const isComplete =
    metadata && typeof metadata.is_complete === "boolean" ? metadata.is_complete : null;
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
  const result = await client.streamWorkflow(
    workflowPath,
    { messages: [{ role: "user", content: userInput }] },
    onWorkflowEvent,
    undefined,
    executionFlags,
    customWorkerDispatch,
  );

  console.log("\n");
  console.log(JSON.stringify(result, null, 2));
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
