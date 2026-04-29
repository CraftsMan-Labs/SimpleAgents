import { Client, type EvalReport } from "simple-agents-node";
import { pathToEvalSuite } from "../example_paths.js";

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

  if (req.handler === "evaluate_rag_chunks") {
    const payload = req.payload as {
      actual: Array<{ source_id?: string }>;
      expected: string[];
      threshold?: number;
    };
    const actualIds = new Set(
      payload.actual
        .map((chunk) => chunk.source_id)
        .filter((sourceId): sourceId is string => Boolean(sourceId)),
    );
    const expectedIds = new Set(payload.expected);
    const matched = [...expectedIds].filter((sourceId) => actualIds.has(sourceId));
    const score = expectedIds.size === 0 ? 1 : matched.length / expectedIds.size;

    return {
      score,
      passed: score >= (payload.threshold ?? 1),
      reason: `${matched.length}/${expectedIds.size} expected sources matched`,
      metadata: {
        matched,
        missing: [...expectedIds].filter((sourceId) => !actualIds.has(sourceId)),
      },
    };
  }

  throw new Error(`unknown custom worker handler: ${req.handler}`);
}

async function main(): Promise<void> {
  // No LLM call happens in this example; the workflow and the eval are both mocked custom workers.
  const client = new Client(
    process.env.WORKFLOW_API_KEY ?? "sk-mocked-rag-eval-000000000000",
  );
  const report: EvalReport = await client.runEvalSuite(
    { suitePath: pathToEvalSuite("rag", "rag-eval.yaml") },
    customWorkerDispatch,
  );

  console.log(JSON.stringify(report, null, 2));
  process.exit(report.status === "passed" ? 0 : 1);
}

main().catch((error: unknown) => {
  console.error(error);
  process.exit(1);
});
