/**
 * Run a YAML workflow with a multimodal user message (text + invoice image, Jaeger OTLP).
 *
 * Parity with `examples/python-test-simpleAgents/test-py-simple-agents-invoice-image-jaegar.py`.
 * From repo root `examples/`: `bun install` in this directory.
 *
 * Uses standard OTLP env vars (`SIMPLE_AGENTS_TRACING_ENABLED`, `OTEL_EXPORTER_OTLP_*`,
 * `OTEL_SERVICE_NAME`) — see `docs/OTEL_CONFIGURATION.md`. This script turns tracing on and
 * fills defaults (gRPC `http://localhost:4317`) unless `JAEGER_OTEL=false`.
 *
 * `syncOtelEnvFromProcess` copies values into Rust `std::env` (needed for Bun/Node).
 *
 * Env: `WORKFLOW_API_KEY` (required), `WORKFLOW_API_BASE` (optional).
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { config as loadEnv } from "dotenv";
import type { MessageInput } from "simple-agents-node";
import { Client, syncOtelEnvFromProcess } from "simple-agents-node";
import { customWorkerDispatch } from "./handlers.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
loadEnv({ path: join(__dirname, ".env") });

const workflowPath = join(__dirname, "test.yaml");
const imagePath = join(__dirname, "../python-test-simpleAgents/test-invoice.jpeg");

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

/** Enable OTLP export + workflow telemetry unless `JAEGER_OTEL` is explicitly false. */
function configureJaegerOtelFromEnv(): boolean {
  const off = ["0", "false", "no", "off"].includes(
    process.env.JAEGER_OTEL?.trim().toLowerCase() ?? "",
  );
  if (off) return false;

  process.env.SIMPLE_AGENTS_TRACING_ENABLED = "true";
  process.env.OTEL_EXPORTER_OTLP_ENDPOINT ||= "http://localhost:4317";
  process.env.OTEL_EXPORTER_OTLP_PROTOCOL ||= "grpc";
  if (!process.env.OTEL_SERVICE_NAME?.trim()) {
    process.env.OTEL_SERVICE_NAME = "simple-agents-workflow-invoice-image-jaeger";
  }

  console.error(
    `Jaeger OTLP: endpoint=${process.env.OTEL_EXPORTER_OTLP_ENDPOINT} protocol=${process.env.OTEL_EXPORTER_OTLP_PROTOCOL} service=${process.env.OTEL_SERVICE_NAME}`,
  );

  syncOtelEnvFromProcess(
    process.env.SIMPLE_AGENTS_TRACING_ENABLED,
    process.env.OTEL_EXPORTER_OTLP_PROTOCOL,
    process.env.OTEL_EXPORTER_OTLP_ENDPOINT,
    process.env.OTEL_EXPORTER_OTLP_HEADERS ?? "",
    process.env.OTEL_SERVICE_NAME || undefined,
  );
  return true;
}

async function main(): Promise<void> {
  const telemetryOn = configureJaegerOtelFromEnv();
  if (!telemetryOn) {
    console.error("Jaeger OTLP disabled (JAEGER_OTEL=false).");
  }

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
  const workflowOptions = telemetryOn
    ? { telemetry: { enabled: true, nerdstats: true } }
    : undefined;

  const result = await client.runWorkflow(
    workflowPath,
    { messages },
    workflowOptions,
    undefined,
    customWorkerDispatch,
  );

  console.log(JSON.stringify(result, null, 2));
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
