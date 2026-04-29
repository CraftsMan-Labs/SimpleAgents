/**
 * Run a YAML workflow (blocking, no stream events).
 *
 * Parity with `examples/python-test-simpleAgents/runners/test-py-simple-agents.py`.
 *
 * From repo root `examples/`: `bun install` in this directory (uses
 * `simple-agents-node` from `../../crates/simple-agents-napi`).
 *
 * Env: `WORKFLOW_API_KEY` (required), `WORKFLOW_API_BASE` (optional).
 */

import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { Client } from "simple-agents-node";
import { pathToWorkflow } from "../example_paths.js";
import { customWorkerDispatch } from "../workflows/email-classification/handlers.js";

const workflowPath = pathToWorkflow("email-classification", "test.yaml");

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

async function main(): Promise<void> {
  const apiKey = requireEnv("WORKFLOW_API_KEY");
  const baseUrl = process.env.WORKFLOW_API_BASE || undefined;

  const rl = readline.createInterface({ input, output });
  const userInput = await rl.question("Enter your Input: ");
  rl.close();

  const client = new Client(apiKey, baseUrl);
  const result = await client.runWorkflow(
    workflowPath,
    { messages: [{ role: "user", content: userInput }] },
    undefined,
    undefined,
    customWorkerDispatch,
  );

  console.log(JSON.stringify(result, null, 2));
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
