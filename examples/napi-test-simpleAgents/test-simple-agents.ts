/**
 * Run a YAML workflow (blocking, no stream events).
 *
 * Parity with `examples/python-test-simpleAgents/test-py-simple-agents.py`.
 *
 * From repo root `examples/`: `bun install` in this directory (uses
 * `simple-agents-node` from `../../crates/simple-agents-napi`).
 *
 * Env: `WORKFLOW_API_KEY` (required), `WORKFLOW_API_BASE` (optional).
 */

import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { Client } from "simple-agents-node";
import { customWorkerDispatch } from "./handlers.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const workflowPath = join(__dirname, "test.yaml");

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
  const result = await client.run(
    workflowPath,
    [{ role: "user", content: userInput }],
    { customWorker: customWorkerDispatch },
  );

  console.log(JSON.stringify(result, null, 2));
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
