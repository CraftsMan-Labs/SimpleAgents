/**
 * Run a YAML workflow with a multimodal user message (text + invoice image).
 *
 * From repo root `examples/`: `bun install` in this directory.
 *
 * Env: `WORKFLOW_API_KEY` (required), `WORKFLOW_API_BASE` (optional).
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import type { MessageInput } from "simple-agents-node";
import { Client } from "simple-agents-node";
import { customWorkerDispatch } from "./handlers.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const workflowPath = join(__dirname, "test.yaml");
const imagePath = join(__dirname, "../python-test-simpleAgents/test-invoice.jpeg");

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

async function main(): Promise<void> {
  const apiKey = requireEnv("WORKFLOW_API_KEY");
  const baseUrl = process.env.WORKFLOW_API_BASE || undefined;
  const b64 = readFileSync(imagePath).toString("base64");

  const messages: MessageInput[] = [
    {
      role: "user",
      content: [
        { type: "text", text: "Invoice image. Classify and route per workflow." },
        { type: "image", mediaType: "image/jpeg", data: b64 },
      ],
    },
  ];

  const client = new Client(apiKey, baseUrl);
  const result = await client.runWorkflow(
    workflowPath,
    { messages },
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
