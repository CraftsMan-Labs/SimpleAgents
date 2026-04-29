/**
 * Run a YAML workflow with a multimodal user message (text + invoice image)
 * via `simple-agents-wasm`.
 */

import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { Client, hasRustBackend } from "../../bindings/wasm/simple-agents-wasm/index.js";
import type {
  MessageInput,
  WorkflowExecutionRequest,
} from "../../bindings/wasm/simple-agents-wasm/index.js";
import { clientConfig } from "./example_config.js";
import { workflowFunctions } from "./handlers.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const defaultWorkflowPath = join(__dirname, "../napi-test-simpleAgents/workflows/email-classification/test.yaml");
const defaultImagePath = join(__dirname, "../python-test-simpleAgents/assets/test-invoice.jpeg");
const fallbackPngB64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7Z6uoAAAAASUVORK5CYII=";

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
  const imagePath = process.env.INVOICE_IMAGE_PATH || defaultImagePath;

  if (!(await hasRustBackend())) {
    throw new Error("Rust WASM backend is unavailable. Build bindings/wasm/simple-agents-wasm first.");
  }
  let mediaType = "image/jpeg";
  let b64 = fallbackPngB64;
  if (existsSync(imagePath)) {
    b64 = readFileSync(imagePath).toString("base64");
  } else {
    mediaType = "image/png";
    console.warn(`Image file not found at ${imagePath}; using embedded placeholder PNG.`);
  }
  const messages: MessageInput[] = [
    {
      role: "user",
      content: [
        { type: "text", text: "Invoice image. Classify and route per workflow." },
        { type: "image", mediaType, data: b64 },
      ],
    },
  ];

  const client = new Client(provider, clientConfig(apiKey, baseUrl));
  const req: WorkflowExecutionRequest = {
    workflow_yaml: readFileSync(workflowPath, "utf8"),
    messages,
    workflow_options: {
      functions: workflowFunctions,
    },
  };

  const result = await client.run(req);
  console.log(JSON.stringify(result, null, 2));
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
