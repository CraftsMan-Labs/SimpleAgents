/**
 * Run the email-classification workflow (TypeScript).
 * Based on examples/napi-test-simpleAgents/test-simple-agents.ts
 *
 * npm install simple-agents-node dotenv
 * Create .env with WORKFLOW_API_KEY, WORKFLOW_API_BASE
 */

import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { Client } from "simple-agents-node";
import { config as loadEnv } from "dotenv";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

const workflowPath = join(__dirname, "email-classification.yaml");

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

// Custom worker dispatch (matches handlers.py logic)
function customWorkerDispatch(req: {
  handler: string;
  payload: unknown;
  context: unknown;
}): string {
  if (req.handler === "get_seller_name") {
    const p = req.payload as Record<string, unknown>;
    const name = String(p.company_name ?? "").trim().toLowerCase();
    const map: Record<string, string> = {
      google: "Sundar Pichai",
      microsoft: "Satya Nadella",
      apple: "Tim Cook",
      amazon: "Andy Jassy",
    };
    return map[name] ?? "unknown";
  }
  throw new Error(`unknown handler: ${req.handler}`);
}

async function main(): Promise<void> {
  const apiKey = requireEnv("WORKFLOW_API_KEY");
  const baseUrl = process.env.WORKFLOW_API_BASE || undefined;

  const rl = readline.createInterface({ input, output });
  const userInput = await rl.question("Enter your email text: ");
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
