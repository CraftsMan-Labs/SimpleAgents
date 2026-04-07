/**
 * Stream a YAML workflow with live events via `simple-agents-wasm`.
 *
 * Reuses `../napi-test-simpleAgents/test.yaml` for parity with existing examples.
 */

import * as readline from "node:readline/promises";
import { readFileSync } from "node:fs";
import { stdin as input, stdout as output } from "node:process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { Client, hasRustBackend } from "../../bindings/wasm/simple-agents-wasm/index.js";
import type { WorkflowExecutionRequest } from "../../bindings/wasm/simple-agents-wasm/index.js";
import { createWorkflowStreamPrinter } from "../../bindings/wasm/simple-agents-wasm/workflow_stream_printer.mjs";
import { workflowFunctions } from "./handlers.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const defaultWorkflowPath = join(__dirname, "../napi-test-simpleAgents/test.yaml");

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

function getProvider(): "openai" | "openrouter" {
  const provider = process.env.WORKFLOW_PROVIDER ?? "openai";
  if (provider !== "openai" && provider !== "openrouter") {
    throw new Error("Set WORKFLOW_PROVIDER to openai or openrouter.");
  }
  return provider;
}

async function main(): Promise<void> {
  const provider = getProvider();
  const apiKey = requireEnv("WORKFLOW_API_KEY");
  const baseUrl = process.env.WORKFLOW_API_BASE || undefined;
  const workflowPath = process.env.WORKFLOW_YAML_PATH || defaultWorkflowPath;

  if (!(await hasRustBackend())) {
    throw new Error("Rust WASM backend is unavailable. Build bindings/wasm/simple-agents-wasm first.");
  }

  const rl = readline.createInterface({ input, output });
  const userInput = await rl.question("Enter your Input: ");
  rl.close();

  const client = new Client(provider, { apiKey, baseUrl, fetchImpl: globalThis.fetch });
  const req: WorkflowExecutionRequest = {
    workflow_yaml: readFileSync(workflowPath, "utf8"),
    messages: [{ role: "user", content: userInput }],
    execution: {
      node_llm_streaming: true,
      split_stream_deltas: false,
    },
    workflow_options: {
      functions: workflowFunctions,
    },
  };

  const result = await client.streamWorkflow(req, createWorkflowStreamPrinter());
  console.log("\n");
  console.log(JSON.stringify(result, null, 2));
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
