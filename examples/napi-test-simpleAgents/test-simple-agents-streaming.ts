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

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

/** Mirrors the Python demo: log each parsed event on stdout (full object). */
function onWorkflowEvent(err: unknown, eventJson: string): void {
  if (err) {
    console.error(err);
    return;
  }
  console.log(parseWorkflowEvent(eventJson));
}

async function main(): Promise<void> {
  const apiKey = requireEnv("WORKFLOW_API_KEY");
  const baseUrl = process.env.WORKFLOW_API_BASE || undefined;

  const rl = readline.createInterface({ input, output });
  const userInput = await rl.question("Enter your Input: ");
  rl.close();

  const client = new Client(apiKey, baseUrl);
  const result = await client.stream(
    workflowPath,
    [{ role: "user", content: userInput }],
    onWorkflowEvent,
    { customWorker: customWorkerDispatch },
  );

  console.log("\n");
  console.log(JSON.stringify(result, null, 2));
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
