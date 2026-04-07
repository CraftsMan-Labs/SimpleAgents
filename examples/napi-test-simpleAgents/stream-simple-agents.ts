/**
 * Stream a YAML workflow with live events.
 * LLM nodes in the YAML should use stream: true if you want token deltas.
 *
 * From this directory: `bun install` (uses the repo's local `simple-agents-node` via package.json).
 */

import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { Client } from "simple-agents-node";
import { defaultOnEvent } from "simple-agents-node/workflow_event";

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
  const result = await client.stream(
    workflowPath,
    [{ role: "user", content: userInput }],
    defaultOnEvent,
  );

  console.log("\n");
  console.log(JSON.stringify(result, null, 2));
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});