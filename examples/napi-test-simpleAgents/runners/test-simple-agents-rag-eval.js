// This plain JS example imports the local package directly so it works from a fresh checkout
// after `npm run build:debug` in `crates/simple-agents-napi`, without installing example deps.
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import simpleAgentsNode from "../../../crates/simple-agents-napi/index.js";

const { Client } = simpleAgentsNode;

const __dirname = dirname(fileURLToPath(import.meta.url));
const packageRoot = join(__dirname, "..");
const ragEvalSuitePath = join(packageRoot, "evals", "rag", "rag-eval.yaml");

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

  if (req.handler === "evaluate_rag_chunks") {
    const actualIds = new Set(
      req.payload.actual
        .map((chunk) => chunk.source_id)
        .filter((sourceId) => Boolean(sourceId)),
    );
    const expectedIds = new Set(req.payload.expected);
    const matched = [...expectedIds].filter((sourceId) => actualIds.has(sourceId));
    const score = expectedIds.size === 0 ? 1 : matched.length / expectedIds.size;

    return {
      score,
      passed: score >= (req.payload.threshold ?? 1),
      reason: `${matched.length}/${expectedIds.size} expected sources matched`,
      metadata: {
        matched,
        missing: [...expectedIds].filter((sourceId) => !actualIds.has(sourceId)),
      },
    };
  }

  throw new Error(`unknown custom worker handler: ${req.handler}`);
}

const client = new Client(
  process.env.WORKFLOW_API_KEY ?? "sk-mocked-rag-eval-000000000000",
);
const report = await client.runEvalSuite(
  { suitePath: ragEvalSuitePath },
  customWorkerDispatch,
);

console.log(JSON.stringify(report, null, 2));
process.exit(report.status === "passed" ? 0 : 1);
