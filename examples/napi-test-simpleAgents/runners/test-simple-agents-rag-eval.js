// This plain JS example imports the local package directly so it works from a fresh checkout
// after `npm run build:debug` in `crates/simple-agents-napi`, without installing example deps.
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import simpleAgentsNode from "../../../crates/simple-agents-napi/index.js";

const { Client } = simpleAgentsNode;

const __dirname = dirname(fileURLToPath(import.meta.url));
const packageRoot = join(__dirname, "..");
const ragWorkflowPath = join(packageRoot, "workflows", "rag", "rag-eval-workflow.yaml");
const ragDatasetPath = join(packageRoot, "evals", "rag", "rag-eval.dataset.jsonl");

function customWorkerDispatch(req) {
  if (req.handler === "mock_retrieve_chunks") {
    return [
      {
        source_id: "refund-policy-v3",
        text: "Refunds are available within 30 days for eligible purchases.",
      },
      {
        source_id: "terms-section-8",
        text: "Refund requests require the original order id.",
      },
      {
        source_id: "unrelated-blog-post",
        text: "A noisy chunk that should not hurt recall.",
      },
    ];
  }
  throw new Error(`unknown custom worker handler: ${req.handler}`);
}

function evaluateRagChunks(case_) {
  const chunks = case_.actualOutput.outputs?.retrieve_chunks?.output ?? [];
  const actualIds = new Set(
    chunks
      .map((chunk) => chunk.source_id)
      .filter((sourceId) => Boolean(sourceId)),
  );
  const expectedIds = new Set(case_.record.custom?.expected_sources ?? []);
  const matched = [...expectedIds].filter((sourceId) => actualIds.has(sourceId));
  const score = expectedIds.size === 0 ? 1 : matched.length / expectedIds.size;
  return {
    id: "rag_chunks",
    status: score >= 0.8 ? "passed" : "failed",
    passed: score >= 0.8,
    score,
    expected: [...expectedIds],
    actual: [...actualIds],
    reason: `${matched.length}/${expectedIds.size} expected sources matched`,
    metadata: {
      matched,
      missing: [...expectedIds].filter((sourceId) => !actualIds.has(sourceId)),
    },
  };
}

const client = new Client(
  process.env.WORKFLOW_API_KEY ?? "sk-mocked-rag-eval-000000000000",
);
const report = await client.runEvalSuite({
  workflowPath: ragWorkflowPath,
  datasetPath: ragDatasetPath,
  execution: { nodeLlmStreaming: false },
  workflowOptions: { telemetry: { enabled: false } },
  customWorkerDispatch,
  evaluator: evaluateRagChunks,
});

console.log(JSON.stringify(report, null, 2));
process.exit(report.status === "passed" ? 0 : 1);
