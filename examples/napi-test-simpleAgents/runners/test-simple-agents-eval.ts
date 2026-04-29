import { config } from "dotenv";
import { join } from "node:path";
import { Client, type EvalReport } from "simple-agents-node";
import { PACKAGE_ROOT, pathToEvalSuite, pathToWorkflow } from "../example_paths.js";

config({ path: join(PACKAGE_ROOT, ".env") });

const client = new Client(
  process.env.WORKFLOW_API_KEY ?? "",
  process.env.WORKFLOW_API_BASE,
);

const report: EvalReport = await client.runEvalSuite({
  workflowPath: pathToWorkflow("friendly", "friendly.yaml"),
  datasetPath: pathToEvalSuite("friendly", "friendly-eval.dataset.jsonl"),
  execution: { nodeLlmStreaming: false },
  workflowOptions: { telemetry: { enabled: false } },
  evaluator: ({ expectedOutput, actualOutput }) => {
    const expected = expectedOutput.outputs;
    const actual = actualOutput.outputs;
    return {
      id: "friendly_output",
      status: JSON.stringify(expected) === JSON.stringify(actual) ? "passed" : "failed",
      passed: JSON.stringify(expected) === JSON.stringify(actual),
      expected,
      actual,
      reason: JSON.stringify(expected) === JSON.stringify(actual) ? undefined : "outputs changed",
    };
  },
});

console.log(JSON.stringify(report, null, 2));
process.exit(report.status === "passed" ? 0 : 1);
