import { config } from "dotenv";
import { Client, type EvalReport } from "simple-agents-node";

config();

const client = new Client(
  process.env.WORKFLOW_API_KEY ?? "",
  process.env.WORKFLOW_API_BASE,
);

const report: EvalReport = await client.runEvalSuite({
  suitePath: "friendly-eval.yaml",
});

console.log(JSON.stringify(report, null, 2));
process.exit(report.status === "passed" ? 0 : 1);
