/**
 * Stream a YAML workflow with live events and Langfuse OTLP.
 *
 * Same as `test-simple-agents-streaming.ts`, plus mapping `LANGFUSE_*` to OpenTelemetry
 * export settings and `syncOtelEnvFromProcess` so the native layer sees OTLP env
 * (Bun/Node `process.env` updates are not always visible to Rust `std::env`).
 *
 * Parity with `examples/python-test-simpleAgents/test-py-simple-agents-streaming-langfuse.py`.
 * LLM nodes in the YAML should use `stream: true` if you want token deltas.
 *
 * From repo root `examples/`: `bun install` in this directory.
 *
 * Env: `WORKFLOW_API_KEY` (required), `WORKFLOW_API_BASE` (optional).
 *
 * **Langfuse:** set `LANGFUSE_PUBLIC_KEY`, `LANGFUSE_SECRET_KEY`, and `LANGFUSE_BASE_URL`
 * (e.g. `http://localhost:3000`). This script maps them to SimpleAgents OTLP settings
 * (`SIMPLE_AGENTS_TRACING_ENABLED`, `OTEL_EXPORTER_OTLP_*`) per `docs/OTEL_CONFIGURATION.md`.
 *
 * This script loads `.env` with the `dotenv` package (same relaxed parsing as Python’s
 * `python-dotenv`). Bun’s automatic loader often **does not** set variables written as
 * `KEY = "value"` (spaces around `=`), which would skip Langfuse OTLP setup.
 */

import { config as loadEnv } from "dotenv";
import * as readline from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { Client, syncOtelEnvFromProcess } from "simple-agents-node";
import { parseWorkflowEvent } from "simple-agents-node/workflow_event";
import { customWorkerDispatch } from "./handlers.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });
const workflowPath = join(__dirname, "test.yaml");

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

/** Strip optional quotes from `.env` values (some loaders include them literally). */
function stripQuotes(value: string): string {
  const t = value.trim();
  if (
    (t.startsWith('"') && t.endsWith('"') && t.length >= 2) ||
    (t.startsWith("'") && t.endsWith("'") && t.length >= 2)
  ) {
    return t.slice(1, -1);
  }
  return t;
}

/** Map `LANGFUSE_*` into SimpleAgents OpenTelemetry exporter env (OTLP HTTP → Langfuse). */
function configureLangfuseOtelFromEnv(): boolean {
  const publicKey = process.env.LANGFUSE_PUBLIC_KEY
    ? stripQuotes(process.env.LANGFUSE_PUBLIC_KEY)
    : undefined;
  const secretKey = process.env.LANGFUSE_SECRET_KEY
    ? stripQuotes(process.env.LANGFUSE_SECRET_KEY)
    : undefined;
  const baseUrl = stripQuotes(process.env.LANGFUSE_BASE_URL ?? "");
  if (!publicKey || !secretKey || !baseUrl) {
    return false;
  }
  const token = Buffer.from(`${publicKey}:${secretKey}`).toString("base64");
  const endpoint = `${baseUrl.replace(/\/$/, "")}/api/public/otel`;
  process.env.SIMPLE_AGENTS_TRACING_ENABLED = "true";
  process.env.OTEL_EXPORTER_OTLP_PROTOCOL = "http/protobuf";
  process.env.OTEL_EXPORTER_OTLP_ENDPOINT = endpoint;
  process.env.OTEL_EXPORTER_OTLP_HEADERS = `Authorization=Basic ${token},x-langfuse-ingestion-version=4`;
  return true;
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
  if (configureLangfuseOtelFromEnv()) {
    syncOtelEnvFromProcess(
      process.env.SIMPLE_AGENTS_TRACING_ENABLED!,
      process.env.OTEL_EXPORTER_OTLP_PROTOCOL!,
      process.env.OTEL_EXPORTER_OTLP_ENDPOINT!,
      process.env.OTEL_EXPORTER_OTLP_HEADERS!,
      process.env.OTEL_SERVICE_NAME || undefined,
    );
  }

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
  const workflowOptions = {
    telemetry: {
      enabled: true,
      nerdstats: true,
    },
  };
  const result = await client.streamWorkflow(
    workflowPath,
    { messages: [{ role: "user", content: userInput }] },
    onWorkflowEvent,
    workflowOptions,
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
