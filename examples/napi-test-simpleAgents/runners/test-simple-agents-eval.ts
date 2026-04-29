import { config } from "dotenv";
import { join } from "node:path";
import { Client, type EvalReport } from "simple-agents-node";
import { PACKAGE_ROOT, pathToEvalSuite } from "../example_paths.js";

config({ path: join(PACKAGE_ROOT, ".env") });

const client = new Client(
  process.env.WORKFLOW_API_KEY ?? "",
  process.env.WORKFLOW_API_BASE,
);

const report: EvalReport = await client.runEvalSuite({
  suitePath: pathToEvalSuite("friendly", "friendly-eval.yaml"),
});

console.log(JSON.stringify(report, null, 2));
process.exit(report.status === "passed" ? 0 : 1);
