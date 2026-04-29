import { Client, type EvalReport } from "simple-agents-node";
import { pathToEvalSuite, pathToWorkflow } from "../example_paths.js";

type CustomWorkerRequest = {
  handler: string;
  payload: unknown;
  context: unknown;
};

function customWorkerDispatch(req: CustomWorkerRequest): unknown {
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

type EvalCase = Parameters<Parameters<Client["runEvalSuite"]>[0]["evaluator"]>[0];

function evaluateRagChunks(case_: EvalCase) {
  const output = case_.actualOutput.outputs as Record<string, { output?: unknown }> | undefined;
  const chunks = (output?.retrieve_chunks?.output ?? []) as Array<{ source_id?: string }>;
  const actualIds = new Set(
    chunks
      .map((chunk) => chunk.source_id)
      .filter((sourceId): sourceId is string => Boolean(sourceId)),
  );
  const custom = case_.record.custom as { expected_sources?: string[] } | undefined;
  const expectedIds = new Set(custom?.expected_sources ?? []);
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

async function main(): Promise<void> {
  // No LLM call happens in this example; the workflow and the eval are both mocked custom workers.
  const client = new Client(
    process.env.WORKFLOW_API_KEY ?? "sk-mocked-rag-eval-000000000000",
  );
  const report: EvalReport = await client.runEvalSuite({
    workflowPath: pathToWorkflow("rag", "rag-eval-workflow.yaml"),
    datasetPath: pathToEvalSuite("rag", "rag-eval.dataset.jsonl"),
    execution: { nodeLlmStreaming: false },
    workflowOptions: { telemetry: { enabled: false } },
    customWorkerDispatch,
    evaluator: evaluateRagChunks,
  });

  console.log(JSON.stringify(report, null, 2));
  process.exit(report.status === "passed" ? 0 : 1);
}

main().catch((error: unknown) => {
  console.error(error);
  process.exit(1);
});
