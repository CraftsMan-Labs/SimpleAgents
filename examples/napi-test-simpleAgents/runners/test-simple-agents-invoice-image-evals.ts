/**
 * Two invoice multimodal eval suites (terminal-only vs deep path assertions).
 *
 * Regenerates JSONL under `evals/invoice/generated/` from
 * `examples/python-test-simpleAgents/assets/test-invoice.jpeg`, then runs each suite.
 *
 * Env: `WORKFLOW_API_KEY` (required), `WORKFLOW_API_BASE` (optional).
 * Loads `.env` from monorepo root → `examples/` → this package (`loadNapiExampleEnv`).
 */
import { existsSync } from "node:fs";
import { Client, type EvalReport } from "simple-agents-node";
import {
  loadNapiExampleEnv,
  pathToEvalSuite,
  pathToPythonExamplesAsset,
  pathToWorkflow,
} from "../example_paths.js";
import { writeInvoiceEvalGeneratedDatasets } from "../invoice_eval_multimodal.js";
import { customWorkerDispatch } from "../workflows/email-classification/handlers.js";

loadNapiExampleEnv();

type Evaluator = Parameters<Client["runEvalSuite"]>[0]["evaluator"];

const terminalNodeExact: Evaluator = ({ expectedOutput, actualOutput }) => {
  const expected = expectedOutput.terminal_node;
  const actual = actualOutput.terminal_node;
  const passed = expected === actual;
  return {
    id: "terminal_node_exact",
    status: passed ? "passed" : "failed",
    passed,
    expected,
    actual,
    reason: passed ? undefined : "terminal_node changed",
  };
};

function isSubset(expected: unknown, actual: unknown): boolean {
  if (Object.is(expected, actual)) return true;
  if (
    expected &&
    actual &&
    typeof expected === "object" &&
    typeof actual === "object" &&
    !Array.isArray(expected) &&
    !Array.isArray(actual)
  ) {
    return Object.entries(expected as Record<string, unknown>).every(([key, value]) =>
      isSubset(value, (actual as Record<string, unknown>)[key]),
    );
  }
  if (Array.isArray(expected) && Array.isArray(actual)) {
    return expected.every((value, index) => isSubset(value, actual[index]));
  }
  return false;
}

const outputSubset: Evaluator = ({ expectedOutput, actualOutput }) => {
  const passed = isSubset(expectedOutput, actualOutput);
  return {
    id: "output_subset",
    status: passed ? "passed" : "failed",
    passed,
    expected: expectedOutput,
    actual: actualOutput,
    reason: passed ? undefined : "expected_output is not a subset of actual_output",
  };
};

const SUITES: Array<{ label: string; dataset: string; evaluator: Evaluator }> = [
  {
    label: "invoice-image-terminal-eval",
    dataset: "generated/invoice-image-terminal-eval.dataset.jsonl",
    evaluator: terminalNodeExact,
  },
  {
    label: "invoice-image-node-eval",
    dataset: "generated/invoice-image-node-eval.dataset.jsonl",
    evaluator: outputSubset,
  },
];

const IMAGE_ASSET = pathToPythonExamplesAsset("test-invoice.jpeg");

function requireEnv(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`Set ${name}`);
  return v;
}

function requireAsset(path: string): void {
  if (!existsSync(path)) {
    throw new Error(
      `Required multimodal asset is missing: ${path}\n` +
        "Add a JPEG at examples/python-test-simpleAgents/assets/test-invoice.jpeg " +
        "(same as Python invoice evals / Jaeger).",
    );
  }
}

async function main(): Promise<void> {
  requireAsset(IMAGE_ASSET);
  writeInvoiceEvalGeneratedDatasets(pathToEvalSuite("invoice"), IMAGE_ASSET);

  const client = new Client(
    requireEnv("WORKFLOW_API_KEY"),
    process.env.WORKFLOW_API_BASE || undefined,
  );

  console.error(
    `Invoice image evals (multimodal): starting (${SUITES.length} suites)…`,
  );

  let ok = true;
  for (const { label, dataset, evaluator } of SUITES) {
    const datasetPath = pathToEvalSuite("invoice", dataset);
    console.error("");
    console.error(`[${label}] running… (${dataset})`);
    const report: EvalReport = await client.runEvalSuite({
      workflowPath: pathToWorkflow("email-classification", "test.yaml"),
      datasetPath,
      execution: { nodeLlmStreaming: false },
      workflowOptions: { telemetry: { enabled: false } },
      customWorkerDispatch,
      evaluator,
    });
    console.log("=".repeat(100));
    console.log(JSON.stringify(report, null, 2));
    console.log("=".repeat(100));
    const passed = report.status === "passed";
    ok &&= passed;
    const verdict = passed ? "PASSED" : String(report.status).toUpperCase();
    console.error(`[${label}] ${verdict}`);
    console.log(JSON.stringify(report, null, 2));
  }

  console.error("");
  console.error(
    ok
      ? "Invoice image evals (multimodal): OVERALL PASSED."
      : "Invoice image evals (multimodal): OVERALL FAILED.",
  );
  process.exit(ok ? 0 : 1);
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
